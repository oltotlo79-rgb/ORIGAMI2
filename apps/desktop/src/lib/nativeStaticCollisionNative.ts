import { invoke } from '@tauri-apps/api/core'

import type {
  CurrentStaticCollisionDiagnostic,
  CurrentStaticCollisionDiagnosticReason,
  CurrentStaticCollisionEvidence,
  CurrentStaticCollisionFacePair,
  CurrentStaticCollisionPairClassificationCounts,
  CurrentStaticCollisionPairDiagnostic,
  CurrentStaticCollisionPairDisposition,
  CurrentStaticCollisionPolicyDecision,
  CurrentStaticCollisionTopology,
} from './nativeStaticCollisionView.ts'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
const MAX_HINGE_COUNT = 100_000
const MAX_PAIR_DIAGNOSTICS = 50_000

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
    'pairClassificationCounts',
    'pairDiagnostics',
  ])
  if (!record) return null
  const binding = record.binding === null ? null : parseBinding(record.binding)
  const expected = nullableCount(record.expectedUnorderedFacePairs)
  const proven = nullableCount(record.provenPenetratingPairs)
  const pair = record.firstProvenPenetratingPair === null
    ? null
    : parseFacePair(record.firstProvenPenetratingPair)
  const counts = record.pairClassificationCounts === null
    ? null
    : parsePairClassificationCounts(record.pairClassificationCounts)
  const pairs = record.pairDiagnostics === null
    ? null
    : parsePairDiagnostics(record.pairDiagnostics, expected)
  if (
    (record.binding !== null && binding === null)
    || !isDiagnosticStatus(record.status)
    || expected === undefined
    || proven === undefined
    || (record.firstProvenPenetratingPair !== null && pair === null)
    || (record.pairClassificationCounts !== null && counts === null)
    || (record.pairDiagnostics !== null && pairs === null)
    || !isDiagnosticReason(record.reason)
  ) return null

  const diagnostic: CurrentStaticCollisionDiagnostic = Object.freeze({
    status: record.status,
    reason: record.reason,
    expectedUnorderedFacePairs: expected,
    provenPenetratingPairs: proven,
    firstProvenPenetratingPair: pair,
    pairClassificationCounts: counts,
    pairDiagnostics: pairs,
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
    pairClassificationCounts: counts,
    pairDiagnostics: pairs,
  } = diagnostic
  const hasSnapshot = counts !== null && pairs !== null
  if (
    (counts === null) !== (pairs === null)
    || (expected === null) !== !hasSnapshot
    || (
      hasSnapshot
      && !pairSnapshotContractIsValid(expected, counts, pairs)
    )
  ) return false

  const firstPenetratingPair = pairs?.find(
    (candidate) => candidate.disposition === 'penetrating',
  ) ?? null
  if (
    proven !== null
    && (
      counts === null
      || proven !== counts.penetrating
    )
  ) return false
  if (
    pair !== null
    && (
      firstPenetratingPair === null
      || !facePairsEqual(pair, firstPenetratingPair)
    )
  ) return false

  if (status === 'certified_nonblocking') {
    return binding !== null
      && reason === null
      && expected !== null
      && proven === 0
      && pair === null
      && counts !== null
      && counts.penetrating === 0
      && counts.indeterminate === 0
  }
  if (status === 'unavailable') {
    return binding === null
      && reason === 'pose_authority_unavailable'
      && expected === null
      && proven === null
      && pair === null
      && !hasSnapshot
  }
  if (status !== 'blocking' || binding === null || reason === null) return false
  if (reason === 'proven_zero_thickness_penetration') {
    return expected !== null
      && expected > 0
      && proven !== null
      && proven > 0
      && proven <= expected
      && pair !== null
      && counts !== null
      && counts.penetrating === proven
  }
  if (reason === 'proven_positive_thickness_penetration') {
    return expected !== null
      && expected > 0
      && proven !== null
      && proven > 0
      && proven <= expected
      && pair !== null
      && counts !== null
      && counts.penetrating === proven
  }
  if (reason === 'evidence_unavailable') {
    return expected !== null
      && expected > 0
      && proven === null
      && pair === null
      && hasSnapshot
      && counts.penetrating === 0
  }
  return (
    reason === 'resource_limit_exceeded'
    || reason === 'inconsistent_state'
  )
    && expected === null
    && proven === null
    && pair === null
    && !hasSnapshot
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

function parsePairClassificationCounts(
  value: unknown,
): CurrentStaticCollisionPairClassificationCounts | null {
  const record = exactDataRecord(value, [
    'separated',
    'touching',
    'allowed',
    'penetrating',
    'indeterminate',
    'candidateExcluded',
  ])
  if (
    !record
    || !isSafeNonNegativeInteger(record.separated)
    || !isSafeNonNegativeInteger(record.touching)
    || !isSafeNonNegativeInteger(record.allowed)
    || !isSafeNonNegativeInteger(record.penetrating)
    || !isSafeNonNegativeInteger(record.indeterminate)
    || !isSafeNonNegativeInteger(record.candidateExcluded)
  ) return null
  return Object.freeze({
    separated: record.separated,
    touching: record.touching,
    allowed: record.allowed,
    penetrating: record.penetrating,
    indeterminate: record.indeterminate,
    candidateExcluded: record.candidateExcluded,
  })
}

function parsePairDiagnostics(
  value: unknown,
  expectedUnorderedFacePairs: number | null | undefined,
): readonly CurrentStaticCollisionPairDiagnostic[] | null {
  if (
    !Array.isArray(value)
    || expectedUnorderedFacePairs === null
    || expectedUnorderedFacePairs === undefined
    || expectedUnorderedFacePairs > MAX_PAIR_DIAGNOSTICS
    || value.length !== expectedUnorderedFacePairs
  ) return null
  const result: CurrentStaticCollisionPairDiagnostic[] = []
  let previous: CurrentStaticCollisionPairDiagnostic | null = null
  for (const candidate of value) {
    const pair = parsePairDiagnostic(candidate)
    if (
      pair === null
      || (
        previous !== null
        && compareFacePairs(previous, pair) >= 0
      )
    ) return null
    result.push(pair)
    previous = pair
  }
  return Object.freeze(result)
}

function parsePairDiagnostic(
  value: unknown,
): CurrentStaticCollisionPairDiagnostic | null {
  const record = exactDataRecord(value, [
    'firstFaceId',
    'secondFaceId',
    'topology',
    'evidence',
    'policyDecision',
    'disposition',
    'strictTransversalDualGateProven',
    'wholeFaceOverlapProven',
    'sharedHingeBoundaryContactProven',
    'sharedHingeSolidClassified',
  ])
  if (
    !record
    || !isUuid(record.firstFaceId)
    || !isUuid(record.secondFaceId)
    || compareCodeUnits(record.firstFaceId, record.secondFaceId) >= 0
    || !isPairTopology(record.topology)
    || !isPairEvidence(record.evidence)
    || !isPairPolicyDecision(record.policyDecision)
    || !isPairDisposition(record.disposition)
    || typeof record.strictTransversalDualGateProven !== 'boolean'
    || typeof record.wholeFaceOverlapProven !== 'boolean'
    || typeof record.sharedHingeBoundaryContactProven !== 'boolean'
    || typeof record.sharedHingeSolidClassified !== 'boolean'
  ) return null
  const pair: CurrentStaticCollisionPairDiagnostic = Object.freeze({
    firstFaceId: record.firstFaceId,
    secondFaceId: record.secondFaceId,
    topology: record.topology,
    evidence: record.evidence,
    policyDecision: record.policyDecision,
    disposition: record.disposition,
    strictTransversalDualGateProven:
      record.strictTransversalDualGateProven,
    wholeFaceOverlapProven: record.wholeFaceOverlapProven,
    sharedHingeBoundaryContactProven:
      record.sharedHingeBoundaryContactProven,
    sharedHingeSolidClassified: record.sharedHingeSolidClassified,
  })
  return pairProvenanceIsValid(pair) ? pair : null
}

function pairSnapshotContractIsValid(
  expectedUnorderedFacePairs: number | null,
  counts: CurrentStaticCollisionPairClassificationCounts,
  pairs: readonly CurrentStaticCollisionPairDiagnostic[],
): boolean {
  if (
    expectedUnorderedFacePairs === null
    || expectedUnorderedFacePairs !== pairs.length
    || counts.candidateExcluded !== 0
  ) return false
  const values = [
    counts.separated,
    counts.touching,
    counts.allowed,
    counts.penetrating,
    counts.indeterminate,
    counts.candidateExcluded,
  ]
  let sum = 0
  for (const value of values) {
    if (!isSafeNonNegativeInteger(value) || sum > Number.MAX_SAFE_INTEGER - value) {
      return false
    }
    sum += value
  }
  if (sum !== expectedUnorderedFacePairs) return false

  const actual = {
    separated: 0,
    touching: 0,
    allowed: 0,
    penetrating: 0,
    indeterminate: 0,
  }
  for (const pair of pairs) actual[pair.disposition] += 1
  return actual.separated === counts.separated
    && actual.touching === counts.touching
    && actual.allowed === counts.allowed
    && actual.penetrating === counts.penetrating
    && actual.indeterminate === counts.indeterminate
}

function pairProvenanceIsValid(
  pair: CurrentStaticCollisionPairDiagnostic,
): boolean {
  if (
    pair.policyDecision
      !== topologyPolicyDecision(pair.topology, pair.evidence)
  ) return false
  if (pair.sharedHingeBoundaryContactProven) {
    return pair.topology === 'shared_hinge_edge'
      && pair.evidence === 'shared_feature_contact'
      && pair.policyDecision === 'requires_hinge_model'
      && pair.disposition === 'allowed'
      && !pair.strictTransversalDualGateProven
      && !pair.wholeFaceOverlapProven
      && !pair.sharedHingeSolidClassified
  }
  if (pair.sharedHingeSolidClassified) {
    if (
      pair.topology !== 'shared_hinge_edge'
      || pair.strictTransversalDualGateProven
      || pair.wholeFaceOverlapProven
      || pair.sharedHingeBoundaryContactProven
    ) return false
    return (
      pair.policyDecision === 'requires_hinge_model'
      && pair.disposition === 'allowed'
      && (
        pair.evidence === 'shared_feature_contact'
        || pair.evidence === 'shared_feature_thickness_overlap'
        || pair.evidence === 'boundary_area_contact'
      )
    ) || (
      pair.policyDecision === 'penetrating'
      && pair.disposition === 'penetrating'
      && pair.evidence === 'positive_volume_overlap'
    ) || (
      pair.policyDecision === 'indeterminate'
      && pair.disposition === 'indeterminate'
      && pair.evidence === 'indeterminate'
    )
  }
  if (
    pair.strictTransversalDualGateProven
    || pair.wholeFaceOverlapProven
  ) {
    return !(pair.strictTransversalDualGateProven
        && pair.wholeFaceOverlapProven)
      && !pair.sharedHingeBoundaryContactProven
      && pair.disposition === 'penetrating'
      && pair.policyDecision === 'penetrating'
      && (
        (
          pair.strictTransversalDualGateProven
          && pair.evidence === 'transversal_crossing'
        )
        || (
          pair.wholeFaceOverlapProven
          && pair.evidence === 'coplanar_area_overlap'
        )
      )
  }
  const expectedDisposition:
    CurrentStaticCollisionPairDisposition = pair.policyDecision === 'separated'
      ? 'separated'
      : pair.policyDecision === 'touching'
        ? 'touching'
        : pair.policyDecision === 'allowed_shared_vertex_contact'
          ? 'allowed'
          : 'indeterminate'
  return pair.disposition === expectedDisposition
}

function topologyPolicyDecision(
  topology: CurrentStaticCollisionTopology,
  evidence: CurrentStaticCollisionEvidence,
): CurrentStaticCollisionPolicyDecision {
  if (
    evidence === 'coplanar_area_overlap'
    || evidence === 'transversal_crossing'
    || evidence === 'positive_volume_overlap'
  ) return 'penetrating'
  if (topology === 'no_shared_feature') {
    if (evidence === 'separated') return 'separated'
    if (
      evidence === 'point_contact'
      || evidence === 'boundary_line_contact'
      || evidence === 'boundary_area_contact'
    ) return 'touching'
    return 'indeterminate'
  }
  if (topology === 'shared_vertex') {
    if (
      evidence === 'point_contact'
      || evidence === 'boundary_line_contact'
      || evidence === 'boundary_area_contact'
    ) return 'touching'
    if (
      evidence === 'shared_feature_contact'
      || evidence === 'shared_feature_thickness_overlap'
    ) return 'allowed_shared_vertex_contact'
    return 'indeterminate'
  }
  if (
    evidence === 'boundary_area_contact'
    || evidence === 'shared_feature_contact'
    || evidence === 'shared_feature_thickness_overlap'
    || evidence === 'shared_feature_flat_stack'
  ) return 'requires_hinge_model'
  return 'indeterminate'
}

function compareFacePairs(
  first: CurrentStaticCollisionFacePair,
  second: CurrentStaticCollisionFacePair,
): number {
  const firstFace = compareCodeUnits(first.firstFaceId, second.firstFaceId)
  return firstFace !== 0
    ? firstFace
    : compareCodeUnits(first.secondFaceId, second.secondFaceId)
}

function facePairsEqual(
  first: CurrentStaticCollisionFacePair,
  second: CurrentStaticCollisionFacePair,
): boolean {
  return first.firstFaceId === second.firstFaceId
    && first.secondFaceId === second.secondFaceId
}

function nullableCount(value: unknown): number | null | undefined {
  return value === null
    ? null
    : isSafeNonNegativeInteger(value)
      ? value
      : undefined
}

function isDiagnosticStatus(
  value: unknown,
): value is CurrentStaticCollisionDiagnostic['status'] {
  return value === 'certified_nonblocking'
    || value === 'blocking'
    || value === 'unavailable'
}

function isDiagnosticReason(
  value: unknown,
): value is CurrentStaticCollisionDiagnosticReason | null {
  return value === null
    || value === 'proven_zero_thickness_penetration'
    || value === 'proven_positive_thickness_penetration'
    || value === 'evidence_unavailable'
    || value === 'resource_limit_exceeded'
    || value === 'inconsistent_state'
    || value === 'pose_authority_unavailable'
}

function isPairTopology(
  value: unknown,
): value is CurrentStaticCollisionTopology {
  return value === 'no_shared_feature'
    || value === 'shared_vertex'
    || value === 'shared_hinge_edge'
}

function isPairEvidence(
  value: unknown,
): value is CurrentStaticCollisionEvidence {
  return value === 'separated'
    || value === 'point_contact'
    || value === 'boundary_line_contact'
    || value === 'boundary_area_contact'
    || value === 'shared_feature_contact'
    || value === 'shared_feature_thickness_overlap'
    || value === 'shared_feature_flat_stack'
    || value === 'coplanar_area_overlap'
    || value === 'transversal_crossing'
    || value === 'positive_volume_overlap'
    || value === 'indeterminate'
}

function isPairPolicyDecision(
  value: unknown,
): value is CurrentStaticCollisionPolicyDecision {
  return value === 'separated'
    || value === 'touching'
    || value === 'allowed_shared_vertex_contact'
    || value === 'requires_hinge_model'
    || value === 'penetrating'
    || value === 'indeterminate'
}

function isPairDisposition(
  value: unknown,
): value is CurrentStaticCollisionPairDisposition {
  return value === 'separated'
    || value === 'touching'
    || value === 'allowed'
    || value === 'penetrating'
    || value === 'indeterminate'
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
