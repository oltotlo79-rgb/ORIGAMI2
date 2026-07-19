import { invoke } from '@tauri-apps/api/core'

import type {
  CurrentStaticCollisionDiagnostic,
  CurrentStaticCollisionDiagnosticReason,
  CurrentStaticCollisionFacePair,
} from './nativeStaticCollisionView.ts'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
const MAX_HINGE_COUNT = 100_000

export type NativeStaticCollisionBinding = Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
  poseGeneration: string
}>

export type NativeStaticCollisionPose = Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
  fixedFaceId: string | null
  completeHingeAngles: readonly Readonly<{
    edgeId: string
    angleDegrees: number
  }>[]
}>

type NormalizedNativePoseRequest = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  fixedFaceId: string | null
  completeHingeAngles: readonly Readonly<{
    edgeId: string
    angleDegrees: number
  }>[]
}>

export type NativeStaticCollisionInspection = Readonly<{
  binding: NativeStaticCollisionBinding | null
  diagnostic: CurrentStaticCollisionDiagnostic
}>

export type NativeStaticCollisionNativeTransport = Readonly<{
  applyPose(pose: NativeStaticCollisionPose): Promise<NativeStaticCollisionBinding>
  inspect(): Promise<NativeStaticCollisionInspection>
}>

export type NativeStaticCollisionInspectionCoordinator = Readonly<{
  inspectLatest(
    pose: NativeStaticCollisionPose,
  ): Promise<CurrentStaticCollisionDiagnostic>
  retry(): Promise<CurrentStaticCollisionDiagnostic>
  dispose(): void
}>

export type NativeStaticCollisionNativeInvoke = (
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) => unknown

export class NativeStaticCollisionNativeError extends Error {
  readonly category: 'invalid_request' | 'native_unavailable'

  constructor(category: 'invalid_request' | 'native_unavailable') {
    super(category)
    this.name = 'NativeStaticCollisionNativeError'
    this.category = category
  }
}

export class NativeStaticCollisionCoordinatorError extends Error {
  readonly category:
    | 'invalid_request'
    | 'native_unavailable'
    | 'superseded'
    | 'disposed'

  constructor(
    category:
      | 'invalid_request'
      | 'native_unavailable'
      | 'superseded'
      | 'disposed',
  ) {
    super(category)
    this.name = 'NativeStaticCollisionCoordinatorError'
    this.category = category
  }
}

export function createNativeStaticCollisionNativeTransport(
  nativeInvoke: NativeStaticCollisionNativeInvoke = defaultNativeInvoke,
): NativeStaticCollisionNativeTransport {
  return Object.freeze({
    async applyPose(pose) {
      const request = normalizePoseRequest(pose)
      if (!request) throw new NativeStaticCollisionNativeError('invalid_request')
      let raw: unknown
      try {
        raw = await Promise.resolve(nativeInvoke(
          'apply_current_native_pose',
          { request },
        ))
      } catch {
        throw new NativeStaticCollisionNativeError('native_unavailable')
      }
      const response = exactDataRecord(raw, ['binding'])
      const binding = response ? parseBinding(response.binding) : null
      if (!binding || !bindingMatchesPose(binding, pose)) {
        throw new NativeStaticCollisionNativeError('native_unavailable')
      }
      return binding
    },

    async inspect() {
      let raw: unknown
      try {
        raw = await Promise.resolve(nativeInvoke('inspect_current_static_collision'))
      } catch {
        throw new NativeStaticCollisionNativeError('native_unavailable')
      }
      const inspection = parseInspection(raw)
      if (!inspection) {
        throw new NativeStaticCollisionNativeError('native_unavailable')
      }
      return inspection
    },
  })
}

/**
 * Serializes the complete native apply-and-inspect transaction. One running
 * transaction cannot be cancelled after native exact work starts, so waiting
 * requests are reduced to the latest distinct pose instead of starting more
 * workers. Superseded callers are rejected with a fixed, data-free category.
 */
export function createNativeStaticCollisionInspectionCoordinator(
  transport: NativeStaticCollisionNativeTransport,
): NativeStaticCollisionInspectionCoordinator {
  type Waiter = Readonly<{
    resolve(value: CurrentStaticCollisionDiagnostic): void
    reject(error: NativeStaticCollisionCoordinatorError): void
  }>
  type Work = {
    readonly key: string
    readonly pose: NativeStaticCollisionPose
    waiters: Waiter[]
  }

  let active: Work | null = null
  let pending: Work | null = null
  let lastPose: Readonly<{ key: string; pose: NativeStaticCollisionPose }> | null =
    null
  let disposed = false

  const rejectWaiters = (
    work: Work,
    category: NativeStaticCollisionCoordinatorError['category'],
  ) => {
    const waiters = work.waiters
    work.waiters = []
    for (const waiter of waiters) {
      waiter.reject(new NativeStaticCollisionCoordinatorError(category))
    }
  }

  const start = (work: Work) => {
    active = work
    void inspectAppliedPoseStaticCollision(transport, work.pose).then(
      (diagnostic) => {
        if (active !== work) return
        const waiters = work.waiters
        work.waiters = []
        active = null
        for (const waiter of waiters) waiter.resolve(diagnostic)
        startPending()
      },
      () => {
        if (active !== work) return
        const waiters = work.waiters
        work.waiters = []
        active = null
        for (const waiter of waiters) {
          waiter.reject(
            new NativeStaticCollisionCoordinatorError('native_unavailable'),
          )
        }
        startPending()
      },
    )
  }

  const startPending = () => {
    if (disposed || active !== null || pending === null) return
    const next = pending
    pending = null
    start(next)
  }

  const enqueue = (
    key: string,
    pose: NativeStaticCollisionPose,
    waiter: Waiter,
  ) => {
    if (disposed) {
      waiter.reject(new NativeStaticCollisionCoordinatorError('disposed'))
      return
    }
    if (active === null) {
      start({ key, pose, waiters: [waiter] })
      return
    }
    if (active.key === key) {
      if (pending !== null) {
        rejectWaiters(pending, 'superseded')
        pending = null
      }
      active.waiters.push(waiter)
      return
    }

    rejectWaiters(active, 'superseded')
    if (pending?.key === key) {
      pending.waiters.push(waiter)
      return
    }
    if (pending !== null) rejectWaiters(pending, 'superseded')
    pending = { key, pose, waiters: [waiter] }
  }

  const inspectLatest = (
    pose: NativeStaticCollisionPose,
  ): Promise<CurrentStaticCollisionDiagnostic> => {
    const prepared = detachCoordinatorPose(pose)
    if (!prepared) {
      return Promise.reject(
        new NativeStaticCollisionCoordinatorError('invalid_request'),
      )
    }
    lastPose = prepared
    return new Promise((resolve, reject) => {
      enqueue(prepared.key, prepared.pose, { resolve, reject })
    })
  }

  return Object.freeze({
    inspectLatest,
    retry() {
      if (disposed) {
        return Promise.reject(
          new NativeStaticCollisionCoordinatorError('disposed'),
        )
      }
      if (!lastPose) {
        return Promise.reject(
          new NativeStaticCollisionCoordinatorError('invalid_request'),
        )
      }
      const retryPose = lastPose
      return new Promise<CurrentStaticCollisionDiagnostic>((resolve, reject) => {
        enqueue(retryPose.key, retryPose.pose, { resolve, reject })
      })
    },
    dispose() {
      if (disposed) return
      disposed = true
      if (active !== null) rejectWaiters(active, 'disposed')
      if (pending !== null) rejectWaiters(pending, 'disposed')
      pending = null
    },
  })
}

export async function inspectAppliedPoseStaticCollision(
  transport: NativeStaticCollisionNativeTransport,
  pose: NativeStaticCollisionPose,
): Promise<CurrentStaticCollisionDiagnostic> {
  const appliedBinding = await transport.applyPose(pose)
  const inspection = await transport.inspect()
  if (
    inspection.binding === null
    || !nativeStaticCollisionBindingsEqual(appliedBinding, inspection.binding)
  ) {
    throw new NativeStaticCollisionNativeError('native_unavailable')
  }
  return inspection.diagnostic
}

export function nativeStaticCollisionPoseKey(
  pose: NativeStaticCollisionPose,
): string | null {
  const request = normalizePoseRequest(pose)
  if (!request) return null
  try {
    return JSON.stringify(request)
  } catch {
    return null
  }
}

export function nativeStaticCollisionBindingsEqual(
  first: NativeStaticCollisionBinding,
  second: NativeStaticCollisionBinding,
): boolean {
  return first.projectInstanceId === second.projectInstanceId
    && first.projectId === second.projectId
    && first.revision === second.revision
    && first.poseGeneration === second.poseGeneration
}

function normalizePoseRequest(
  pose: NativeStaticCollisionPose,
): NormalizedNativePoseRequest | null {
  try {
    if (
      !isUuid(pose.projectInstanceId)
      || !isUuid(pose.projectId)
      || !isSafeNonNegativeInteger(pose.revision)
      || !(pose.fixedFaceId === null || isUuid(pose.fixedFaceId))
      || !Array.isArray(pose.completeHingeAngles)
      || pose.completeHingeAngles.length > MAX_HINGE_COUNT
    ) return null
    const edgeIds = new Set<string>()
    const completeHingeAngles: Array<Readonly<{
      edgeId: string
      angleDegrees: number
    }>> = []
    for (const value of pose.completeHingeAngles) {
      if (
        !isExactPlainRecord(value, ['edgeId', 'angleDegrees'])
        || !isUuid(value.edgeId)
        || !isFoldAngle(value.angleDegrees)
        || edgeIds.has(value.edgeId)
      ) return null
      edgeIds.add(value.edgeId)
      completeHingeAngles.push(Object.freeze({
        edgeId: value.edgeId,
        angleDegrees: normalizeZero(value.angleDegrees),
      }))
    }
    completeHingeAngles.sort((left, right) =>
      compareCodeUnits(left.edgeId, right.edgeId))
    return Object.freeze({
      expectedProjectInstanceId: pose.projectInstanceId,
      expectedProjectId: pose.projectId,
      expectedRevision: pose.revision,
      fixedFaceId: pose.fixedFaceId,
      completeHingeAngles: Object.freeze(completeHingeAngles),
    })
  } catch {
    return null
  }
}

function detachCoordinatorPose(
  pose: NativeStaticCollisionPose,
): Readonly<{ key: string; pose: NativeStaticCollisionPose }> | null {
  const request = normalizePoseRequest(pose)
  if (!request) return null
  try {
    const detachedPose: NativeStaticCollisionPose = Object.freeze({
      projectInstanceId: request.expectedProjectInstanceId,
      projectId: request.expectedProjectId,
      revision: request.expectedRevision,
      fixedFaceId: request.fixedFaceId,
      completeHingeAngles: request.completeHingeAngles,
    })
    return Object.freeze({
      key: JSON.stringify(request),
      pose: detachedPose,
    })
  } catch {
    return null
  }
}

function parseInspection(value: unknown): NativeStaticCollisionInspection | null {
  const record = exactDataRecord(value, [
    'binding',
    'status',
    'reason',
    'expectedUnorderedFacePairs',
    'provenPenetratingPairs',
    'firstProvenPenetratingPair',
  ])
  if (!record) return null
  const binding = record.binding === null ? null : parseBinding(record.binding)
  const expected = nullableCount(record.expectedUnorderedFacePairs)
  const proven = nullableCount(record.provenPenetratingPairs)
  const pair = record.firstProvenPenetratingPair === null
    ? null
    : parseFacePair(record.firstProvenPenetratingPair)
  if (
    (record.binding !== null && binding === null)
    || expected === undefined
    || proven === undefined
    || (record.firstProvenPenetratingPair !== null && pair === null)
    || !isDiagnosticReason(record.reason)
  ) return null

  const diagnostic: CurrentStaticCollisionDiagnostic = Object.freeze({
    status: record.status as CurrentStaticCollisionDiagnostic['status'],
    reason: record.reason,
    expectedUnorderedFacePairs: expected,
    provenPenetratingPairs: proven,
    firstProvenPenetratingPair: pair,
  })
  if (!diagnosticContractIsValid(diagnostic, binding)) return null
  return Object.freeze({ binding, diagnostic })
}

function diagnosticContractIsValid(
  diagnostic: CurrentStaticCollisionDiagnostic,
  binding: NativeStaticCollisionBinding | null,
): boolean {
  const {
    status,
    reason,
    expectedUnorderedFacePairs: expected,
    provenPenetratingPairs: proven,
    firstProvenPenetratingPair: pair,
  } = diagnostic
  if (status === 'certified_nonblocking') {
    return binding !== null
      && reason === null
      && expected !== null
      && proven === 0
      && pair === null
  }
  if (status === 'unavailable') {
    return binding === null
      && reason === 'pose_authority_unavailable'
      && expected === null
      && proven === null
      && pair === null
  }
  if (status !== 'blocking' || binding === null || reason === null) return false
  if (reason === 'proven_zero_thickness_penetration') {
    return expected !== null
      && expected > 0
      && proven !== null
      && proven > 0
      && proven <= expected
      && pair !== null
  }
  if (reason === 'evidence_unavailable') {
    return expected !== null
      && expected > 0
      && proven === null
      && pair === null
  }
  return (
    reason === 'resource_limit_exceeded'
    || reason === 'inconsistent_state'
  )
    && expected === null
    && proven === null
    && pair === null
}

function parseBinding(value: unknown): NativeStaticCollisionBinding | null {
  const record = exactDataRecord(value, [
    'projectInstanceId',
    'projectId',
    'revision',
    'poseGeneration',
  ])
  if (
    !record
    || !isUuid(record.projectInstanceId)
    || !isUuid(record.projectId)
    || !isSafeNonNegativeInteger(record.revision)
    || !isCanonicalU64(record.poseGeneration)
  ) return null
  return Object.freeze({
    projectInstanceId: record.projectInstanceId,
    projectId: record.projectId,
    revision: record.revision,
    poseGeneration: record.poseGeneration,
  })
}

function parseFacePair(value: unknown): CurrentStaticCollisionFacePair | null {
  const record = exactDataRecord(value, ['firstFaceId', 'secondFaceId'])
  if (
    !record
    || !isUuid(record.firstFaceId)
    || !isUuid(record.secondFaceId)
    || compareCodeUnits(record.firstFaceId, record.secondFaceId) >= 0
  ) return null
  return Object.freeze({
    firstFaceId: record.firstFaceId,
    secondFaceId: record.secondFaceId,
  })
}

function nullableCount(value: unknown): number | null | undefined {
  return value === null
    ? null
    : isSafeNonNegativeInteger(value)
      ? value
      : undefined
}

function isDiagnosticReason(
  value: unknown,
): value is CurrentStaticCollisionDiagnosticReason | null {
  return value === null
    || value === 'proven_zero_thickness_penetration'
    || value === 'evidence_unavailable'
    || value === 'resource_limit_exceeded'
    || value === 'inconsistent_state'
    || value === 'pose_authority_unavailable'
}

function bindingMatchesPose(
  binding: NativeStaticCollisionBinding,
  pose: NativeStaticCollisionPose,
): boolean {
  return binding.projectInstanceId === pose.projectInstanceId
    && binding.projectId === pose.projectId
    && binding.revision === pose.revision
}

function defaultNativeInvoke(
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) {
  return invoke(command, arguments_)
}

function isUuid(value: unknown): value is string {
  return isCanonicalNonNilUuid(value)
}

function isFoldAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function isSafeNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
}

function isCanonicalU64(value: unknown): value is string {
  if (
    typeof value !== 'string'
    || !/^(?:0|[1-9][0-9]{0,19})$/u.test(value)
  ) return false
  return value.length < 20
    || value <= '18446744073709551615'
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function compareCodeUnits(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}

function isExactPlainRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): value is Readonly<Record<Keys[number], unknown>> {
  return exactDataRecord(value, keys) !== null
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  try {
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
      || Object.getPrototypeOf(value) !== Object.prototype
      || Object.getOwnPropertySymbols(value).length !== 0
    ) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const keys = Object.keys(descriptors)
    if (
      keys.length !== expectedKeys.length
      || expectedKeys.some((key) => !Object.hasOwn(descriptors, key))
    ) return null
    const result = Object.create(null) as Record<string, unknown>
    for (const key of expectedKeys) {
      const descriptor = descriptors[key]
      if (
        descriptor === undefined
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      result[key] = descriptor.value
    }
    return result as Readonly<Record<Keys[number], unknown>>
  } catch {
    return null
  }
}
