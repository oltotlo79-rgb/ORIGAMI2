import {
  normalizeStackedFoldReadRequest,
  normalizeStackedFoldReadResponse,
  type StackedFoldReadRequest,
  type StackedFoldReadResponse,
} from './stackedFoldRead.ts'

export type StackedFoldReadAuthority = Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
}>

export type StackedFoldReadCoordinatorState =
  | Readonly<{ status: 'idle'; generation: number }>
  | Readonly<{
      status: 'reading'
      generation: number
      request: StackedFoldReadRequest
    }>
  | Readonly<{
      status: 'ready'
      generation: number
      request: StackedFoldReadRequest
      response: StackedFoldReadResponse
    }>
  | Readonly<{
      status: 'failed'
      generation: number
      request: StackedFoldReadRequest
      reason: 'native_failure' | 'invalid_response' | 'cycle_nonclosing' | 'cycle_path_uncertified' | 'cycle_path_unsupported' | 'cycle_path_resource_limit' | 'cycle_path_no_certified_path' | 'cycle_path_cancelled' | 'cycle_path_collision'
    }>

export type StackedFoldReadCoordinatorResult =
  | Readonly<{ status: 'ready'; response: StackedFoldReadResponse }>
  | Readonly<{
      status: 'cancelled'
      reason: 'superseded' | 'invalidated' | 'disposed' | 'stale_authority'
    }>
  | Readonly<{ status: 'failed'; reason: 'native_failure' | 'invalid_response' | 'cycle_nonclosing' | 'cycle_path_uncertified' | 'cycle_path_unsupported' | 'cycle_path_resource_limit' | 'cycle_path_no_certified_path' | 'cycle_path_cancelled' | 'cycle_path_collision' }>

export type StackedFoldReadCoordinator = Readonly<{
  read(request: StackedFoldReadRequest): Promise<StackedFoldReadCoordinatorResult>
  invalidate(): void
  dispose(): void
  getState(): StackedFoldReadCoordinatorState
}>

export type StackedFoldReadCoordinatorOptions = Readonly<{
  transport(request: StackedFoldReadRequest): Promise<unknown>
  getAuthority(): StackedFoldReadAuthority | null
  onState?(state: StackedFoldReadCoordinatorState): void
  onOrphanedReadyResponse?(
    response: StackedFoldReadResponse,
  ): void | Promise<void>
}>

type StackedFoldReadFailureReason = Extract<
  StackedFoldReadCoordinatorResult,
  { status: 'failed' }
>['reason']

type ActiveRead = {
  generation: number
  settled: boolean
  orphanedReadyReported: boolean
  resolve(result: StackedFoldReadCoordinatorResult): void
}

const snapshotRequest = (request: StackedFoldReadRequest): StackedFoldReadRequest | null => {
  try {
    return normalizeStackedFoldReadRequest(request)
  } catch {
    return null
  }
}

const readAuthority = (
  getAuthority: () => StackedFoldReadAuthority | null,
): StackedFoldReadAuthority | null => {
  try {
    const value: unknown = getAuthority()
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
    ) return null
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const keys = Reflect.ownKeys(descriptors)
    const expectedKeys = [
      'projectInstanceId',
      'projectId',
      'revision',
    ] as const
    if (
      keys.length !== expectedKeys.length
      || keys.some((key) =>
        typeof key !== 'string'
        || !expectedKeys.includes(key as typeof expectedKeys[number]))
    ) return null
    const fields = Object.create(null) as Record<string, unknown>
    for (const key of expectedKeys) {
      const descriptor = descriptors[key]
      if (
        !descriptor
        || !descriptor.enumerable
        || !('value' in descriptor)
      ) return null
      fields[key] = descriptor.value
    }
    if (
      typeof fields.projectInstanceId !== 'string'
      || typeof fields.projectId !== 'string'
      || !Number.isSafeInteger(fields.revision)
      || Number(fields.revision) < 0
      || Object.is(fields.revision, -0)
    ) return null
    return Object.freeze({
      projectInstanceId: fields.projectInstanceId,
      projectId: fields.projectId,
      revision: Number(fields.revision),
    })
  } catch {
    return null
  }
}

function authorityMatches(
  authority: StackedFoldReadAuthority | null,
  request: StackedFoldReadRequest,
): boolean {
  return (
    authority !== null &&
    authority.projectInstanceId === request.expectedProjectInstanceId &&
    authority.projectId === request.expectedProjectId &&
    authority.revision === request.expectedRevision
  )
}

function nativeFailureReason(error: unknown): StackedFoldReadFailureReason {
  if (typeof error !== 'object' || error === null) return 'native_failure'
  let descriptor: PropertyDescriptor | undefined
  try {
    descriptor = Object.getOwnPropertyDescriptor(error, 'reason')
  } catch {
    return 'native_failure'
  }
  if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) {
    return 'native_failure'
  }
  switch (descriptor.value) {
    case 'cycle_nonclosing':
    case 'cycle_path_uncertified':
    case 'cycle_path_unsupported':
    case 'cycle_path_resource_limit':
    case 'cycle_path_no_certified_path':
    case 'cycle_path_cancelled':
    case 'cycle_path_collision':
      return descriptor.value
    default:
      return 'native_failure'
  }
}

export function createStackedFoldReadCoordinator(
  options: StackedFoldReadCoordinatorOptions,
): StackedFoldReadCoordinator {
  let generation = 0
  let disposed = false
  let active: ActiveRead | null = null
  let state: StackedFoldReadCoordinatorState = Object.freeze({
    status: 'idle',
    generation,
  })

  const publish = (
    next: StackedFoldReadCoordinatorState,
    owner: ActiveRead,
  ) => {
    if (
      disposed
      || active !== owner
      || generation !== owner.generation
    ) return false
    state = Object.freeze(next)
    try {
      options.onState?.(state)
    } catch {
      // Presentation callbacks are not part of native-read authority.
    }
    return (
      !disposed
      && active === owner
      && generation === owner.generation
    )
  }

  const settle = (
    owner: ActiveRead,
    result: StackedFoldReadCoordinatorResult,
  ) => {
    if (owner.settled) return
    owner.settled = true
    if (active === owner) active = null
    owner.resolve(result)
  }

  const normalizeResponse = (
    value: unknown,
    request: StackedFoldReadRequest,
  ): StackedFoldReadResponse | null => {
    try {
      return normalizeStackedFoldReadResponse(value, request)
    } catch {
      return null
    }
  }

  const reportOrphanedReadyResponse = (
    owner: ActiveRead,
    response: StackedFoldReadResponse | null,
  ) => {
    if (!response || owner.orphanedReadyReported) return
    owner.orphanedReadyReported = true
    try {
      void Promise.resolve(
        options.onOrphanedReadyResponse?.(response),
      ).catch(() => {
        // Cleanup callbacks cannot affect read authority or settlement.
      })
    } catch {
      // Cleanup callbacks cannot affect read authority or settlement.
    }
  }

  const revoke = (
    reason: Extract<StackedFoldReadCoordinatorResult, { status: 'cancelled' }>['reason'],
    publishIdle = true,
  ) => {
    if (generation < Number.MAX_SAFE_INTEGER) generation += 1
    const previous = active
    active = null
    if (previous) settle(previous, { status: 'cancelled', reason })
    if (!disposed && publishIdle) {
      state = Object.freeze({ status: 'idle', generation })
      try {
        options.onState?.(state)
      } catch {
        // A hostile observer cannot restore a revoked generation.
      }
    }
  }

  return Object.freeze({
    read(request) {
      if (disposed) {
        return Promise.resolve({ status: 'cancelled', reason: 'disposed' })
      }
      const entryGeneration = generation
      const entryActive = active
      const snapshot = snapshotRequest(request)
      if (!snapshot) {
        return Promise.resolve({ status: 'failed', reason: 'invalid_response' })
      }
      if (disposed) {
        return Promise.resolve({ status: 'cancelled', reason: 'disposed' })
      }
      if (generation !== entryGeneration || active !== entryActive) {
        return Promise.resolve({ status: 'cancelled', reason: 'superseded' })
      }
      const authority = readAuthority(options.getAuthority)
      if (disposed) {
        return Promise.resolve({ status: 'cancelled', reason: 'disposed' })
      }
      if (generation !== entryGeneration || active !== entryActive) {
        return Promise.resolve({ status: 'cancelled', reason: 'superseded' })
      }
      if (!authorityMatches(authority, snapshot)) {
        return Promise.resolve({ status: 'cancelled', reason: 'stale_authority' })
      }

      // Replacement never publishes a transient idle state: an observer could
      // otherwise start a nested read that this outer call would overwrite.
      revoke('superseded', false)
      const ownerGeneration = generation
      return new Promise<StackedFoldReadCoordinatorResult>((resolve) => {
        const owner: ActiveRead = {
          generation: ownerGeneration,
          settled: false,
          orphanedReadyReported: false,
          resolve,
        }
        active = owner
        if (
          !publish(
            { status: 'reading', generation: ownerGeneration, request: snapshot },
            owner,
          )
        ) {
          if (!owner.settled) {
            settle(owner, {
              status: 'cancelled',
              reason: disposed ? 'disposed' : 'superseded',
            })
          }
          return
        }

        let pending: Promise<unknown>
        try {
          pending = Promise.resolve(options.transport(snapshot))
        } catch {
          pending = Promise.reject(new Error('transport failed'))
        }
        void pending.then(
          (value) => {
            if (
              owner.settled ||
              active !== owner ||
              disposed ||
              generation !== owner.generation
            ) {
              if (!owner.settled) {
                settle(owner, {
                  status: 'cancelled',
                  reason: disposed ? 'disposed' : 'superseded',
                })
              }
              reportOrphanedReadyResponse(
                owner,
                normalizeResponse(value, snapshot),
              )
              return
            }
            const response = normalizeResponse(value, snapshot)
            if (
              owner.settled ||
              active !== owner ||
              disposed ||
              generation !== owner.generation
            ) {
              if (!owner.settled) {
                settle(owner, {
                  status: 'cancelled',
                  reason: disposed ? 'disposed' : 'superseded',
                })
              }
              reportOrphanedReadyResponse(owner, response)
              return
            }
            const authority = readAuthority(options.getAuthority)
            if (
              owner.settled ||
              active !== owner ||
              disposed ||
              generation !== owner.generation
            ) {
              if (!owner.settled) {
                settle(owner, {
                  status: 'cancelled',
                  reason: disposed ? 'disposed' : 'superseded',
                })
              }
              reportOrphanedReadyResponse(owner, response)
              return
            }
            if (!authorityMatches(authority, snapshot)) {
              revoke('stale_authority')
              reportOrphanedReadyResponse(owner, response)
              return
            }
            if (!response) {
              if (!publish(
                {
                  status: 'failed',
                  generation: ownerGeneration,
                  request: snapshot,
                  reason: 'invalid_response',
                },
                owner,
              )) {
                if (!owner.settled) {
                  settle(owner, {
                    status: 'cancelled',
                    reason: disposed ? 'disposed' : 'superseded',
                  })
                }
                return
              }
              settle(owner, { status: 'failed', reason: 'invalid_response' })
              return
            }
            if (
              !publish(
                {
                  status: 'ready',
                  generation: ownerGeneration,
                  request: snapshot,
                  response,
                },
                owner,
              )
            ) {
              settle(owner, {
                status: 'cancelled',
                reason: disposed ? 'disposed' : 'superseded',
              })
              reportOrphanedReadyResponse(owner, response)
              return
            }
            settle(owner, { status: 'ready', response })
          },
          (error: unknown) => {
            if (owner.settled) return
            if (active !== owner || disposed || generation !== owner.generation) {
              settle(owner, {
                status: 'cancelled',
                reason: disposed ? 'disposed' : 'superseded',
              })
              return
            }
            const authority = readAuthority(options.getAuthority)
            if (owner.settled) return
            if (active !== owner || disposed || generation !== owner.generation) {
              settle(owner, {
                status: 'cancelled',
                reason: disposed ? 'disposed' : 'superseded',
              })
              return
            }
            if (!authorityMatches(authority, snapshot)) {
              revoke('stale_authority')
              return
            }
            const reason = nativeFailureReason(error)
            if (!publish(
              {
                status: 'failed',
                generation: ownerGeneration,
                request: snapshot,
                reason,
              },
              owner,
            )) {
              if (!owner.settled) {
                settle(owner, {
                  status: 'cancelled',
                  reason: disposed ? 'disposed' : 'superseded',
                })
              }
              return
            }
            settle(owner, { status: 'failed', reason })
          },
        )
      })
    },
    invalidate() {
      if (!disposed) revoke('invalidated')
    },
    dispose() {
      if (disposed) return
      disposed = true
      revoke('disposed')
      state = Object.freeze({ status: 'idle', generation })
    },
    getState() {
      return state
    },
  })
}
