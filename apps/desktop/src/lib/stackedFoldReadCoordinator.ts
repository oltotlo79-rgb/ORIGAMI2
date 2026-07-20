import {
  isStackedFoldReadRequest,
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
      reason: 'native_failure' | 'invalid_response' | 'cycle_nonclosing' | 'cycle_path_uncertified' | 'cycle_path_unsupported' | 'cycle_path_resource_limit' | 'cycle_path_no_certified_path' | 'cycle_path_collision'
    }>

export type StackedFoldReadCoordinatorResult =
  | Readonly<{ status: 'ready'; response: StackedFoldReadResponse }>
  | Readonly<{
      status: 'cancelled'
      reason: 'superseded' | 'invalidated' | 'disposed' | 'stale_authority'
    }>
  | Readonly<{ status: 'failed'; reason: 'native_failure' | 'invalid_response' | 'cycle_nonclosing' | 'cycle_path_uncertified' | 'cycle_path_unsupported' | 'cycle_path_resource_limit' | 'cycle_path_no_certified_path' | 'cycle_path_collision' }>

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
}>

type ActiveRead = {
  generation: number
  settled: boolean
  resolve(result: StackedFoldReadCoordinatorResult): void
}

const detachRequest = (request: StackedFoldReadRequest): StackedFoldReadRequest =>
  Object.freeze({
    ...request,
    first: Object.freeze([...request.first]) as unknown as readonly [number, number, number],
    second: Object.freeze([...request.second]) as unknown as readonly [number, number, number],
  })

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

  const publish = (next: StackedFoldReadCoordinatorState, owner: number) => {
    if (disposed || generation !== owner) return false
    state = Object.freeze(next)
    try {
      options.onState?.(state)
    } catch {
      // Presentation callbacks are not part of native-read authority.
    }
    return !disposed && generation === owner
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

  const revoke = (
    reason: Extract<StackedFoldReadCoordinatorResult, { status: 'cancelled' }>['reason'],
  ) => {
    generation += 1
    const previous = active
    active = null
    if (previous) settle(previous, { status: 'cancelled', reason })
    if (!disposed) {
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
      if (!isStackedFoldReadRequest(request)) {
        return Promise.resolve({ status: 'failed', reason: 'invalid_response' })
      }
      const snapshot = detachRequest(request)
      if (!authorityMatches(options.getAuthority(), snapshot)) {
        return Promise.resolve({ status: 'cancelled', reason: 'stale_authority' })
      }

      revoke('superseded')
      const ownerGeneration = generation
      return new Promise<StackedFoldReadCoordinatorResult>((resolve) => {
        const owner: ActiveRead = {
          generation: ownerGeneration,
          settled: false,
          resolve,
        }
        active = owner
        if (
          !publish(
            { status: 'reading', generation: ownerGeneration, request: snapshot },
            ownerGeneration,
          )
        ) {
          settle(owner, {
            status: 'cancelled',
            reason: disposed ? 'disposed' : 'superseded',
          })
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
            if (owner.settled) return
            if (
              active !== owner ||
              disposed ||
              generation !== owner.generation
            ) {
              settle(owner, {
                status: 'cancelled',
                reason: disposed ? 'disposed' : 'superseded',
              })
              return
            }
            if (!authorityMatches(options.getAuthority(), snapshot)) {
              revoke('stale_authority')
              return
            }
            const response = normalizeStackedFoldReadResponse(value, snapshot)
            if (!response) {
              publish(
                {
                  status: 'failed',
                  generation: ownerGeneration,
                  request: snapshot,
                  reason: 'invalid_response',
                },
                ownerGeneration,
              )
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
                ownerGeneration,
              )
            ) {
              settle(owner, {
                status: 'cancelled',
                reason: disposed ? 'disposed' : 'superseded',
              })
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
            const reason =
              typeof error === 'object' && error !== null && 'reason' in error &&
              (error.reason === 'cycle_nonclosing' ||
                error.reason === 'cycle_path_uncertified' ||
                error.reason === 'cycle_path_unsupported' ||
                error.reason === 'cycle_path_resource_limit' ||
                error.reason === 'cycle_path_no_certified_path' ||
                error.reason === 'cycle_path_collision')
                ? error.reason
                : 'native_failure'
            publish(
              {
                status: 'failed',
                generation: ownerGeneration,
                request: snapshot,
                reason,
              },
              ownerGeneration,
            )
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
      revoke('disposed')
      disposed = true
      state = Object.freeze({ status: 'idle', generation })
    },
    getState() {
      return state
    },
  })
}
