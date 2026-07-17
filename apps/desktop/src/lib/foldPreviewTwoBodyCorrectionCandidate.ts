import {
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_TERMINAL_EVIDENCE_TRIANGLE_PAIRS,
  type FoldPreviewTreeTerminalFullScanBinding,
} from './foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from './foldPreviewTreeScenePose.ts'

export const FOLD_PREVIEW_TWO_BODY_CORRECTION_CANDIDATE_VERSION =
  'two_body_translation_candidate_v1'
export const MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS = 16
export const MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS = 696

const MAX_ID_LENGTH = 8 * 1_024 * 1_024
const MAX_ANGLE_COUNT = 10_000
const MAX_WITNESS_SUPPORT_POINTS = 4
const MAX_WITNESS_POSITION_CANDIDATES = 16
const UNIT_VECTOR_TOLERANCE = 1e-9
const LINEAR_SOLVE_TOLERANCE = Number.EPSILON * 4_096
const MAX_STRICT_PROJECTION_INFLATION_STEPS = 8

type Point = Readonly<{ x: number; y: number; z: number }>
type Body = 'stationary' | 'moving'

export type FoldPreviewTwoBodyCorrectionConstraint = Readonly<{
  witnessIndex: number
  firstFaceId: string
  secondFaceId: string
  firstTriangleIndex: number
  secondTriangleIndex: number
  geometryClass: 'touching' | 'penetrating'
  movingSide: 'first' | 'second'
  direction: Point
  requiredProjection: number
  solverTargetProjection: number
  achievedProjection: number
  certifiedProjectionLowerBound: number
}>

export type FoldPreviewTwoBodyCorrectionSourcePartition = Readonly<{
  version: 'rerooted_selected_hinge_partition_v1'
  stationaryFaceIds: readonly string[]
  movingFaceIds: readonly string[]
}>

export type FoldPreviewTwoBodyCorrectionCandidate = Readonly<{
  version: typeof FOLD_PREVIEW_TWO_BODY_CORRECTION_CANDIDATE_VERSION
  kind: 'unverified_two_body_translation_candidate'
  sourceIdentity: Readonly<{
    bindingVersion: 'tree_single_hinge_terminal_full_scan_binding_v1'
    projectId: string
    revision: number
    fixedFaceId: string
    selectedHingeEdgeId: string
    contextKey: string
    sourcePoseRequestKey: string
    blockingPoseRequestKey: string
    generation: number
    requestSequence: number
    blockingSampleTime: number
    selectedAngleDegrees: number
    collisionThickness: number
  }>
  sourcePartition: FoldPreviewTwoBodyCorrectionSourcePartition
  translation: Point
  magnitude: number
  certifiedMagnitudeUpperBound: number
  clearance: number
  maximumTranslation: number
  constraints: readonly FoldPreviewTwoBodyCorrectionConstraint[]
  solver: Readonly<{
    method: 'certified_outward_active_set_3d_v1'
    seedMethod: 'minimum_norm_kkt_active_set_3d_v1'
    activeConstraintIndices: readonly number[]
    activeSetSize: 1 | 2 | 3
    evaluatedActiveSetCount: number
    maximumActiveSetCount: typeof MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS
  }>
  safety: Readonly<{
    sourcePairConstraintsSatisfied: true
    nonAdjacentScopeOnly: true
    hingeAdjacentPairsIncluded: false
    wholeSceneConstraintsRepresented: false
    legalCorrectionPoseGenerated: false
    staticCandidateRevalidated: false
    continuousCandidatePathCertified: false
    autoApplicable: false
  }>
}>

type SnapshotIdentity = Readonly<{
  bindingVersion: 'tree_single_hinge_terminal_full_scan_binding_v1'
  projectId: string
  revision: number
  fixedFaceId: string
  selectedHingeEdgeId: string
  contextKey: string
  sourcePoseRequestKey: string
  blockingPoseRequestKey: string
  generation: number
  requestSequence: number
  blockingSampleTime: number
  selectedAngleDegrees: number
  collisionThickness: number
}>

type SnapshotConstraint = Readonly<{
  witnessIndex: number
  firstFaceId: string
  secondFaceId: string
  firstTriangleIndex: number
  secondTriangleIndex: number
  geometryClass: 'touching' | 'penetrating'
  movingSide: 'first' | 'second'
  direction: Point
  requiredBaseWithoutClearance: number
}>

type BindingSnapshot = Readonly<{
  identity: SnapshotIdentity
  sourcePartition: FoldPreviewTwoBodyCorrectionSourcePartition
  numericalMargin: number
  constraints: readonly SnapshotConstraint[]
}>

type SolverConstraint = SnapshotConstraint & Readonly<{
  requiredProjection: number
  solverTargetProjection: number
}>

type SolverCandidate = Readonly<{
  translation: Point
  magnitude: number
  certifiedMagnitudeUpperBound: number
  activeConstraintIndices: readonly number[]
}>

/**
 * Derives one analysis-only common translation for the moving tree partition.
 *
 * Returning null means that this bounded active-set calculation could not
 * certify a candidate; it is not a proof that no correction exists. The
 * returned translation is not a legal fold pose and is never auto-applicable.
 */
export function deriveFoldPreviewTwoBodyCorrectionCandidate(
  binding: FoldPreviewTreeTerminalFullScanBinding,
  clearance: number,
  maximumTranslation: number,
): FoldPreviewTwoBodyCorrectionCandidate | null {
  try {
    if (
      typeof clearance !== 'number'
      || !Number.isFinite(clearance)
      || clearance <= 0
      || typeof maximumTranslation !== 'number'
      || !Number.isFinite(maximumTranslation)
      || maximumTranslation <= 0
      || clearance > maximumTranslation
    ) return null
    const snapshot = snapshotBinding(binding)
    if (!snapshot) return null

    const constraints: SolverConstraint[] = []
    for (let index = 0; index < snapshot.constraints.length; index += 1) {
      const source = snapshot.constraints[index]
      const requiredProjection = addNonNegativeUp(
        source.requiredBaseWithoutClearance,
        clearance,
      )
      if (requiredProjection === null) return null
      const solverTargetProjection = nextUpPositive(requiredProjection)
      if (
        requiredProjection <= 0
        || solverTargetProjection === null
        || solverTargetProjection > maximumTranslation
      ) return null
      constraints.push(Object.freeze({
        ...source,
        requiredProjection,
        solverTargetProjection,
      }))
    }
    if (
      constraints.length === 0
      || constraints.length
        > MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS
    ) return null

    let evaluatedActiveSetCount = 0
    let best: SolverCandidate | null = null
    const consider = (activeConstraintIndices: readonly number[]) => {
      evaluatedActiveSetCount += 1
      return solveActiveSet(
        constraints,
        activeConstraintIndices,
      )
    }

    for (let first = 0; first < constraints.length; first += 1) {
      best = preferSolverCandidate(best, consider([first]))
    }
    for (let first = 0; first < constraints.length; first += 1) {
      for (let second = first + 1; second < constraints.length; second += 1) {
        best = preferSolverCandidate(best, consider([first, second]))
      }
    }
    for (let first = 0; first < constraints.length; first += 1) {
      for (let second = first + 1; second < constraints.length; second += 1) {
        for (
          let third = second + 1;
          third < constraints.length;
          third += 1
        ) {
          best = preferSolverCandidate(
            best,
            consider([first, second, third]),
          )
        }
      }
    }
    const expectedActiveSetCount = combinationCount(
      constraints.length,
      1,
    ) + combinationCount(
      constraints.length,
      2,
    ) + combinationCount(
      constraints.length,
      3,
    )
    if (
      !best
      || evaluatedActiveSetCount !== expectedActiveSetCount
      || evaluatedActiveSetCount
        > MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS
    ) return null

    if (
      best.magnitude <= 0
      || best.certifiedMagnitudeUpperBound > maximumTranslation
    ) return null
    const outputConstraints: FoldPreviewTwoBodyCorrectionConstraint[] = []
    for (let index = 0; index < constraints.length; index += 1) {
      const constraint = constraints[index]
      const achievedProjection = dot(
        constraint.direction,
        best.translation,
      )
      const certifiedProjectionLowerBound = dotLowerBound(
        constraint.direction,
        best.translation,
      )
      if (
        !Number.isFinite(achievedProjection)
        || certifiedProjectionLowerBound === null
        || certifiedProjectionLowerBound <= constraint.requiredProjection
      ) return null
      outputConstraints.push(Object.freeze({
        witnessIndex: constraint.witnessIndex,
        firstFaceId: constraint.firstFaceId,
        secondFaceId: constraint.secondFaceId,
        firstTriangleIndex: constraint.firstTriangleIndex,
        secondTriangleIndex: constraint.secondTriangleIndex,
        geometryClass: constraint.geometryClass,
        movingSide: constraint.movingSide,
        direction: constraint.direction,
        requiredProjection: constraint.requiredProjection,
        solverTargetProjection: constraint.solverTargetProjection,
        achievedProjection,
        certifiedProjectionLowerBound,
      }))
    }

    return Object.freeze({
      version: FOLD_PREVIEW_TWO_BODY_CORRECTION_CANDIDATE_VERSION,
      kind: 'unverified_two_body_translation_candidate',
      sourceIdentity: snapshot.identity,
      sourcePartition: snapshot.sourcePartition,
      translation: best.translation,
      magnitude: best.magnitude,
      certifiedMagnitudeUpperBound: best.certifiedMagnitudeUpperBound,
      clearance,
      maximumTranslation,
      constraints: Object.freeze(outputConstraints),
      solver: Object.freeze({
        method: 'certified_outward_active_set_3d_v1',
        seedMethod: 'minimum_norm_kkt_active_set_3d_v1',
        activeConstraintIndices: best.activeConstraintIndices,
        activeSetSize:
          best.activeConstraintIndices.length as 1 | 2 | 3,
        evaluatedActiveSetCount,
        maximumActiveSetCount:
          MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS,
      }),
      safety: Object.freeze({
        sourcePairConstraintsSatisfied: true,
        nonAdjacentScopeOnly: true,
        hingeAdjacentPairsIncluded: false,
        wholeSceneConstraintsRepresented: false,
        legalCorrectionPoseGenerated: false,
        staticCandidateRevalidated: false,
        continuousCandidatePathCertified: false,
        autoApplicable: false,
      }),
    })
  } catch {
    return null
  }
}

function snapshotBinding(value: unknown): BindingSnapshot | null {
  if (!isRecord(value)) return null
  const version = value.version
  const sourcePose = value.sourcePose
  const requestIdentityBound = value.requestIdentityBound
  const rawIdentity = value.identity
  const blockingSampleTime = value.blockingSampleTime
  const selectedAngleDegrees = value.selectedAngleDegrees
  const collisionThickness = value.collisionThickness
  const rawAngleVectors = value.angleVectors
  const rawPartition = value.partition
  const rawEvidence = value.evidence
  const rawSafety = value.safety
  if (
    version !== 'tree_single_hinge_terminal_full_scan_binding_v1'
    || sourcePose !== 'blocking_evaluate_point_pose'
    || requestIdentityBound !== true
    || !validUnitTime(blockingSampleTime)
    || !validAngle(selectedAngleDegrees)
    || !validPositive(collisionThickness)
  ) return null

  const identity = snapshotIdentity(
    rawIdentity,
    blockingSampleTime,
    selectedAngleDegrees,
    collisionThickness,
  )
  const angleVectors = snapshotAngleVectors(
    rawAngleVectors,
    identity?.selectedHingeEdgeId ?? '',
    blockingSampleTime,
    selectedAngleDegrees,
  )
  const partition = snapshotPartition(rawPartition)
  const evidence = snapshotEvidence(rawEvidence, collisionThickness)
  if (
    !identity
    || !angleVectors
    || !partition
    || !evidence
    || !validBindingSafety(rawSafety)
    || partition.relations.length !== evidence.samples.length
  ) return null
  const poseIdentity = Object.freeze({
    projectId: identity.projectId,
    revision: identity.revision,
    kind: 'fold_graph' as const,
  })
  const expectedSourcePoseRequestKey =
    createFoldPreviewTreeSceneCollisionPoseKey(
      poseIdentity,
      identity.fixedFaceId,
      collisionThickness,
      angleVectors.start,
    )
  const expectedBlockingPoseRequestKey =
    createFoldPreviewTreeSceneCollisionPoseKey(
      poseIdentity,
      identity.fixedFaceId,
      collisionThickness,
      angleVectors.sample,
    )
  if (
    expectedSourcePoseRequestKey === null
    || expectedBlockingPoseRequestKey === null
    || identity.sourcePoseRequestKey !== expectedSourcePoseRequestKey
    || identity.blockingPoseRequestKey !== expectedBlockingPoseRequestKey
  ) return null

  const stationary = new Set(partition.stationaryFaceIds)
  const moving = new Set(partition.movingFaceIds)
  if (
    !stationary.has(identity.fixedFaceId)
    || [...stationary].some((faceId) => moving.has(faceId))
  ) return null

  const constraints: SnapshotConstraint[] = []
  for (let index = 0; index < evidence.samples.length; index += 1) {
    const sample = evidence.samples[index]
    const relation = partition.relations[index]
    if (
      relation.witnessIndex !== index
      || relation.relation !== 'cross_partition'
      || relation.firstBody === relation.secondBody
    ) return null
    const firstExpectedBody: Body | null =
      stationary.has(sample.firstFaceId)
        ? 'stationary'
        : moving.has(sample.firstFaceId)
          ? 'moving'
          : null
    const secondExpectedBody: Body | null =
      stationary.has(sample.secondFaceId)
        ? 'stationary'
        : moving.has(sample.secondFaceId)
          ? 'moving'
          : null
    if (
      !firstExpectedBody
      || !secondExpectedBody
      || relation.firstBody !== firstExpectedBody
      || relation.secondBody !== secondExpectedBody
      || firstExpectedBody === secondExpectedBody
    ) return null
    const movingSide = firstExpectedBody === 'moving' ? 'first' : 'second'
    const direction = movingSide === 'second'
      ? sample.normal
      : freezePoint({
          x: -sample.normal.x,
          y: -sample.normal.y,
          z: -sample.normal.z,
        })
    if (!direction) return null
    const remainingMargin = subtractNonNegativeUp(
      evidence.numericalMargin,
      sample.toleratedGap,
    )
    const requiredBaseWithoutClearance = remainingMargin === null
      ? null
      : addNonNegativeUp(sample.escapeDistance, remainingMargin)
    if (
      requiredBaseWithoutClearance === null
      || requiredBaseWithoutClearance < 0
    ) return null
    constraints.push(Object.freeze({
      witnessIndex: index,
      firstFaceId: sample.firstFaceId,
      secondFaceId: sample.secondFaceId,
      firstTriangleIndex: sample.firstTriangleIndex,
      secondTriangleIndex: sample.secondTriangleIndex,
      geometryClass: sample.geometryClass,
      movingSide,
      direction,
      requiredBaseWithoutClearance,
    }))
  }
  const sourcePartition = Object.freeze({
    version: partition.version,
    stationaryFaceIds: partition.stationaryFaceIds,
    movingFaceIds: partition.movingFaceIds,
  })
  return Object.freeze({
    identity,
    sourcePartition,
    numericalMargin: evidence.numericalMargin,
    constraints: Object.freeze(constraints),
  })
}

function snapshotIdentity(
  value: unknown,
  blockingSampleTime: number,
  selectedAngleDegrees: number,
  collisionThickness: number,
): SnapshotIdentity | null {
  if (!isRecord(value)) return null
  const projectId = value.projectId
  const revision = value.revision
  const revisionBinding = value.revisionBinding
  const fixedFaceId = value.fixedFaceId
  const selectedHingeEdgeId = value.selectedHingeEdgeId
  const rawRequest = value.request
  const blockingPoseRequestKey = value.blockingPoseRequestKey
  if (
    !validId(projectId)
    || !Number.isSafeInteger(revision)
    || (revision as number) < 0
    || revisionBinding !== 'project_response_source_equal_v1'
    || !validId(fixedFaceId)
    || !validId(selectedHingeEdgeId)
    || !validId(blockingPoseRequestKey)
    || !isRecord(rawRequest)
  ) return null
  const contextKey = rawRequest.contextKey
  const sourcePoseRequestKey = rawRequest.sourcePoseRequestKey
  const generation = rawRequest.generation
  const requestSequence = rawRequest.requestSequence
  if (
    !validId(contextKey)
    || !validId(sourcePoseRequestKey)
    || !Number.isSafeInteger(generation)
    || (generation as number) < 0
    || !Number.isSafeInteger(requestSequence)
    || (requestSequence as number) <= 0
  ) return null
  return Object.freeze({
    bindingVersion: 'tree_single_hinge_terminal_full_scan_binding_v1',
    projectId,
    revision: revision as number,
    fixedFaceId,
    selectedHingeEdgeId,
    contextKey,
    sourcePoseRequestKey,
    blockingPoseRequestKey,
    generation: generation as number,
    requestSequence: requestSequence as number,
    blockingSampleTime,
    selectedAngleDegrees,
    collisionThickness,
  })
}

type AngleSnapshot = Readonly<{ edgeId: string; angleDegrees: number }>

type AngleVectorSnapshot = Readonly<{
  start: readonly AngleSnapshot[]
  target: readonly AngleSnapshot[]
  sample: readonly AngleSnapshot[]
}>

function snapshotAngleVectors(
  value: unknown,
  selectedHingeEdgeId: string,
  blockingSampleTime: number,
  selectedAngleDegrees: number,
): AngleVectorSnapshot | null {
  if (!isRecord(value)) return null
  const rawStart = value.start
  const rawTarget = value.target
  const rawSample = value.sample
  const start = snapshotAngles(rawStart)
  const target = snapshotAngles(rawTarget)
  const sample = snapshotAngles(rawSample)
  if (
    !start
    || !target
    || !sample
    || start.length !== target.length
    || start.length !== sample.length
  ) return null
  let selectedCount = 0
  for (let index = 0; index < start.length; index += 1) {
    if (
      start[index].edgeId !== target[index].edgeId
      || start[index].edgeId !== sample[index].edgeId
    ) return null
    const selected = start[index].edgeId === selectedHingeEdgeId
    if (selected) {
      selectedCount += 1
      const expected = start[index].angleDegrees
        + (target[index].angleDegrees - start[index].angleDegrees)
          * blockingSampleTime
      if (
        sample[index].angleDegrees !== expected
        || sample[index].angleDegrees !== selectedAngleDegrees
      ) return null
    } else if (
      start[index].angleDegrees !== target[index].angleDegrees
      || start[index].angleDegrees !== sample[index].angleDegrees
    ) return null
  }
  return selectedCount === 1
    ? Object.freeze({ start, target, sample })
    : null
}

function snapshotAngles(value: unknown): readonly AngleSnapshot[] | null {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (
    !Number.isSafeInteger(length)
    || length <= 0
    || length > MAX_ANGLE_COUNT
  ) return null
  const seen = new Set<string>()
  const result: AngleSnapshot[] = []
  for (let index = 0; index < length; index += 1) {
    const item = value[index]
    if (!isRecord(item)) return null
    const edgeId = item.edgeId
    const angleDegrees = item.angleDegrees
    if (
      !validId(edgeId)
      || seen.has(edgeId)
      || !validAngle(angleDegrees)
    ) return null
    seen.add(edgeId)
    result.push(Object.freeze({ edgeId, angleDegrees }))
  }
  return Object.freeze(result)
}

type PartitionSnapshot = Readonly<{
  version: 'rerooted_selected_hinge_partition_v1'
  stationaryFaceIds: readonly string[]
  movingFaceIds: readonly string[]
  relations: readonly Readonly<{
    witnessIndex: number
    firstBody: Body
    secondBody: Body
    relation: 'cross_partition'
  }>[]
}>

function snapshotPartition(value: unknown): PartitionSnapshot | null {
  if (!isRecord(value)) return null
  const version = value.version
  const rawStationary = value.stationaryFaceIds
  const rawMoving = value.movingFaceIds
  const rawRelations = value.witnessRelations
  if (version !== 'rerooted_selected_hinge_partition_v1') return null
  const stationaryFaceIds = snapshotIds(rawStationary)
  const movingFaceIds = snapshotIds(rawMoving)
  if (
    !stationaryFaceIds
    || !movingFaceIds
    || !Array.isArray(rawRelations)
  ) return null
  const length = rawRelations.length
  if (
    !Number.isSafeInteger(length)
    || length <= 0
    || length > MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS
  ) return null
  const relations: PartitionSnapshot['relations'][number][] = []
  for (let index = 0; index < length; index += 1) {
    const item = rawRelations[index]
    if (!isRecord(item)) return null
    const witnessIndex = item.witnessIndex
    const firstBody = item.firstBody
    const secondBody = item.secondBody
    const relation = item.relation
    if (
      witnessIndex !== index
      || (firstBody !== 'stationary' && firstBody !== 'moving')
      || (secondBody !== 'stationary' && secondBody !== 'moving')
      || firstBody === secondBody
      || relation !== 'cross_partition'
    ) return null
    relations.push(Object.freeze({
      witnessIndex,
      firstBody,
      secondBody,
      relation,
    }))
  }
  return Object.freeze({
    version,
    stationaryFaceIds,
    movingFaceIds,
    relations: Object.freeze(relations),
  })
}

function snapshotIds(value: unknown): readonly string[] | null {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (
    !Number.isSafeInteger(length)
    || length <= 0
    || length > MAX_ANGLE_COUNT
  ) return null
  const seen = new Set<string>()
  const result: string[] = []
  for (let index = 0; index < length; index += 1) {
    const id = value[index]
    if (!validId(id) || seen.has(id)) return null
    seen.add(id)
    result.push(id)
  }
  return Object.freeze(result)
}

type EvidenceSample = Readonly<{
  firstFaceId: string
  secondFaceId: string
  firstTriangleIndex: number
  secondTriangleIndex: number
  geometryClass: 'touching' | 'penetrating'
  normal: Point
  escapeDistance: number
  toleratedGap: number
}>

type EvidenceSnapshot = Readonly<{
  numericalMargin: number
  samples: readonly EvidenceSample[]
}>

function snapshotEvidence(
  value: unknown,
  collisionThickness: number,
): EvidenceSnapshot | null {
  if (!isRecord(value)) return null
  const kind = value.kind
  const algorithm = value.algorithm
  const sourcePose = value.sourcePose
  const requestIdentityBound = value.requestIdentityBound
  const rawThickness = value.collisionThickness
  const numericalMargin = value.numericalMargin
  const rawCoverage = value.coverage
  const rawSamples = value.witnessSamples
  const autoApplicable = value.autoApplicable
  if (
    kind !== 'complete'
    || algorithm !== 'full_non_adjacent_prism_witness_scan_v2'
    || sourcePose !== 'analyzed_input_pose'
    || requestIdentityBound !== false
    || rawThickness !== collisionThickness
    || !validNonNegative(numericalMargin)
    || autoApplicable !== false
    || !Array.isArray(rawSamples)
  ) return null
  const length = rawSamples.length
  if (
    !Number.isSafeInteger(length)
    || length <= 0
    || length > MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS
  ) return null
  const coverage = snapshotCompleteCoverage(rawCoverage, length)
  if (!coverage) return null
  const samples: EvidenceSample[] = []
  const identities = new Set<string>()
  let touchingSampleCount = 0
  let penetratingSampleCount = 0
  for (let index = 0; index < length; index += 1) {
    const sample = snapshotEvidenceSample(
      rawSamples[index],
      numericalMargin,
    )
    if (!sample) return null
    const identity = JSON.stringify([
      sample.firstFaceId,
      sample.secondFaceId,
      sample.firstTriangleIndex,
      sample.secondTriangleIndex,
    ])
    if (identities.has(identity)) return null
    identities.add(identity)
    if (sample.geometryClass === 'touching') touchingSampleCount += 1
    else penetratingSampleCount += 1
    samples.push(sample)
  }
  if (
    touchingSampleCount !== coverage.touchingPairCount
    || penetratingSampleCount !== coverage.penetratingPairCount
  ) return null
  return Object.freeze({
    numericalMargin,
    samples: Object.freeze(samples),
  })
}

type CompleteCoverageSnapshot = Readonly<{
  touchingPairCount: number
  penetratingPairCount: number
}>

function snapshotCompleteCoverage(
  value: unknown,
  sampleCount: number,
): CompleteCoverageSnapshot | null {
  if (!isRecord(value)) return null
  const scope = value.scope
  const broadPhaseCandidateCount = value.broadPhaseCandidateCount
  const expectedTrianglePairCount = value.expectedTrianglePairCount
  const trianglePairTests = value.trianglePairTests
  const aabbRejectedPairCount = value.aabbRejectedPairCount
  const satTests = value.satTests
  const satSeparatedPairCount = value.satSeparatedPairCount
  const touchingPairCount = value.touchingPairCount
  const penetratingPairCount = value.penetratingPairCount
  const indeterminatePairCount = value.indeterminatePairCount
  const eligiblePairCount = value.eligiblePairCount
  const attemptedPairCount = value.attemptedPairCount
  const availablePairCount = value.availablePairCount
  const unavailablePairCount = value.unavailablePairCount
  const omittedByLimitCount = value.omittedByLimitCount
  const authoritativePairScanComplete = value.authoritativePairScanComplete
  const allCollisionConstraintsRepresented =
    value.allCollisionConstraintsRepresented
  const counts = [
    broadPhaseCandidateCount,
    expectedTrianglePairCount,
    trianglePairTests,
    aabbRejectedPairCount,
    satTests,
    satSeparatedPairCount,
    touchingPairCount,
    penetratingPairCount,
    eligiblePairCount,
    attemptedPairCount,
    availablePairCount,
  ]
  if (!counts.every(validCount)) return null
  const broadPhaseCandidateCountNumber = broadPhaseCandidateCount as number
  const expectedTrianglePairCountNumber =
    expectedTrianglePairCount as number
  const trianglePairTestsNumber = trianglePairTests as number
  const aabbRejectedPairCountNumber = aabbRejectedPairCount as number
  const satTestsNumber = satTests as number
  const satSeparatedPairCountNumber = satSeparatedPairCount as number
  const touchingPairCountNumber = touchingPairCount as number
  const penetratingPairCountNumber = penetratingPairCount as number
  const eligiblePairCountNumber = eligiblePairCount as number
  const attemptedPairCountNumber = attemptedPairCount as number
  const availablePairCountNumber = availablePairCount as number
  const valid =
    scope === 'all_broad_phase_non_adjacent_triangle_pairs_full_scan_v2'
    && indeterminatePairCount === 0
    && unavailablePairCount === 0
    && omittedByLimitCount === 0
    && authoritativePairScanComplete === true
    && allCollisionConstraintsRepresented === true
    && broadPhaseCandidateCountNumber > 0
    && broadPhaseCandidateCountNumber <= expectedTrianglePairCountNumber
    && expectedTrianglePairCountNumber
      <= MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_TERMINAL_EVIDENCE_TRIANGLE_PAIRS
    && expectedTrianglePairCountNumber === trianglePairTestsNumber
    && trianglePairTestsNumber
      === aabbRejectedPairCountNumber + satTestsNumber
    && satTestsNumber === satSeparatedPairCountNumber
      + touchingPairCountNumber + penetratingPairCountNumber
    && eligiblePairCountNumber
      === touchingPairCountNumber + penetratingPairCountNumber
    && eligiblePairCountNumber === attemptedPairCountNumber
    && attemptedPairCountNumber === availablePairCountNumber
    && availablePairCountNumber === sampleCount
  return valid
    ? Object.freeze({
        touchingPairCount: touchingPairCountNumber,
        penetratingPairCount: penetratingPairCountNumber,
      })
    : null
}

function snapshotEvidenceSample(
  value: unknown,
  numericalMargin: number,
): EvidenceSample | null {
  if (!isRecord(value)) return null
  const firstFaceId = value.firstFaceId
  const secondFaceId = value.secondFaceId
  const relation = value.relation
  const firstTriangleIndex = value.firstTriangleIndex
  const secondTriangleIndex = value.secondTriangleIndex
  const geometryClass = value.geometryClass
  const rawWitness = value.witness
  if (
    !validId(firstFaceId)
    || !validId(secondFaceId)
    || firstFaceId === secondFaceId
    || relation !== 'non_adjacent'
    || !validTriangleIndex(firstTriangleIndex)
    || !validTriangleIndex(secondTriangleIndex)
    || (
      geometryClass !== 'touching'
      && geometryClass !== 'penetrating'
    )
    || !isRecord(rawWitness)
  ) return null
  const algorithm = rawWitness.algorithm
  const witnessGeometryClass = rawWitness.geometryClass
  const witnessMargin = rawWitness.numericalMargin
  const rawNormal = rawWitness.normal
  const escapeDistance = rawWitness.escapeDistance
  const toleratedGap = rawWitness.toleratedGap
  const rawFirstSupport = rawWitness.firstSupport
  const rawSecondSupport = rawWitness.secondSupport
  const rawPositionRegion = rawWitness.positionRegion
  const rawHint = rawWitness.localSeparationHint
  if (
    algorithm !== 'triangle_prism_sat_witness_v1'
    || witnessGeometryClass !== geometryClass
    || witnessMargin !== numericalMargin
    || !validNonNegative(escapeDistance)
    || !validNonNegative(toleratedGap)
    || toleratedGap > numericalMargin
    || (
      geometryClass === 'penetrating'
      && (escapeDistance <= 0 || toleratedGap !== 0)
    )
    || !isRecord(rawNormal)
    || !isRecord(rawPositionRegion)
    || !isRecord(rawHint)
  ) return null
  const rawVector = rawNormal.vector
  const convention = rawNormal.convention
  const uniqueness = rawNormal.uniqueness
  const positionKind = rawPositionRegion.kind
  const positionSourcePose = rawPositionRegion.sourcePose
  const rawGenerators = rawPositionRegion.generators
  const hintDistance = rawHint.distance
  const hintScope = rawHint.scope
  const hintAutoApplicable = rawHint.autoApplicable
  const normal = snapshotUnitPoint(rawVector)
  const firstSupport = snapshotPoints(
    rawFirstSupport,
    MAX_WITNESS_SUPPORT_POINTS,
  )
  const secondSupport = snapshotPoints(
    rawSecondSupport,
    MAX_WITNESS_SUPPORT_POINTS,
  )
  const generators = snapshotPoints(
    rawGenerators,
    MAX_WITNESS_POSITION_CANDIDATES,
  )
  const hintTranslation = snapshotPoint(rawHint.translation)
  if (
    !normal
    || !firstSupport
    || !secondSupport
    || !generators
    || convention !== 'moves_second_away_from_first'
    || (uniqueness !== 'unique' && uniqueness !== 'one_of_multiple')
    || positionKind !== 'support_midpoint_hull_v1'
    || positionSourcePose !== 'analyzed_input_pose'
    || generators.length !== firstSupport.length * secondSupport.length
    || !positionGeneratorsMatch(firstSupport, secondSupport, generators)
    || hintDistance !== escapeDistance
    || hintScope !== 'selected_triangle_prism_pair_only'
    || hintAutoApplicable !== false
    || !hintTranslation
    || hintTranslation.x !== normal.x * escapeDistance
    || hintTranslation.y !== normal.y * escapeDistance
    || hintTranslation.z !== normal.z * escapeDistance
  ) return null
  return Object.freeze({
    firstFaceId,
    secondFaceId,
    firstTriangleIndex,
    secondTriangleIndex,
    geometryClass,
    normal,
    escapeDistance,
    toleratedGap,
  })
}

function validBindingSafety(value: unknown) {
  if (!isRecord(value)) return false
  const nonAdjacentScopeOnly = value.nonAdjacentScopeOnly
  const hingeAdjacentPairsIncluded = value.hingeAdjacentPairsIncluded
  const allWitnessesCrossPartition = value.allWitnessesCrossPartition
  const sameBodyWitnessCount = value.sameBodyWitnessCount
  const twoBodyTranslationInputEligible =
    value.twoBodyTranslationInputEligible
  const wholeSceneConstraintsRepresented =
    value.wholeSceneConstraintsRepresented
  const legalCorrectionPoseGenerated = value.legalCorrectionPoseGenerated
  const staticCandidateRevalidated = value.staticCandidateRevalidated
  const continuousCandidatePathCertified =
    value.continuousCandidatePathCertified
  const autoApplicable = value.autoApplicable
  return nonAdjacentScopeOnly === true
    && hingeAdjacentPairsIncluded === false
    && allWitnessesCrossPartition === true
    && sameBodyWitnessCount === 0
    && twoBodyTranslationInputEligible === true
    && wholeSceneConstraintsRepresented === false
    && legalCorrectionPoseGenerated === false
    && staticCandidateRevalidated === false
    && continuousCandidatePathCertified === false
    && autoApplicable === false
}

function preferSolverCandidate(
  current: SolverCandidate | null,
  candidate: SolverCandidate | null,
): SolverCandidate | null {
  if (!candidate) return current
  if (
    !current
    || candidate.magnitude < current.magnitude
  ) return candidate
  return current
}

function solveActiveSet(
  constraints: readonly SolverConstraint[],
  activeConstraintIndices: readonly number[],
): SolverCandidate | null {
  const count = activeConstraintIndices.length
  if (count < 1 || count > 3) return null
  const gram: number[][] = []
  const targets: number[] = []
  for (let row = 0; row < count; row += 1) {
    const rowConstraint = constraints[activeConstraintIndices[row]]
    targets.push(rowConstraint.solverTargetProjection)
    const values: number[] = []
    for (let column = 0; column < count; column += 1) {
      values.push(dot(
        rowConstraint.direction,
        constraints[activeConstraintIndices[column]].direction,
      ))
    }
    gram.push(values)
  }
  const multipliers = solveLinearSystem(gram, targets)
  if (!multipliers) return null
  if (
    multipliers.some((value) =>
      !Number.isFinite(value) || value < 0
    )
  ) return null

  let mutable = { x: 0, y: 0, z: 0 }
  for (let index = 0; index < count; index += 1) {
    const multiplier = multipliers[index]
    const direction =
      constraints[activeConstraintIndices[index]].direction
    mutable = {
      x: mutable.x + direction.x * multiplier,
      y: mutable.y + direction.y * multiplier,
      z: mutable.z + direction.z * multiplier,
    }
  }
  let translation = freezePoint(mutable)
  if (!translation) return null

  let scale = 1
  for (const activeIndex of activeConstraintIndices) {
    const constraint = constraints[activeIndex]
    const achieved = dot(constraint.direction, translation)
    if (!Number.isFinite(achieved) || achieved <= 0) return null
    if (achieved < constraint.solverTargetProjection) {
      scale = Math.max(
        scale,
        constraint.solverTargetProjection / achieved,
      )
    }
  }
  if (scale > 1) {
    const outwardScale = nextUpPositive(scale)
    if (outwardScale === null) return null
    translation = freezePoint({
      x: translation.x * outwardScale,
      y: translation.y * outwardScale,
      z: translation.z * outwardScale,
    })
    if (!translation) return null
  }
  for (const constraint of constraints) {
    const achieved = dot(constraint.direction, translation)
    if (
      !Number.isFinite(achieved)
      || achieved < constraint.solverTargetProjection
    ) return null
  }
  translation = inflateForStrictProjectionProof(translation, constraints)
  if (!translation) return null
  const magnitude = Math.hypot(
    translation.x,
    translation.y,
    translation.z,
  )
  const certifiedMagnitudeUpperBound = normUpperBound(translation)
  if (
    !Number.isFinite(magnitude)
    || magnitude <= 0
    || certifiedMagnitudeUpperBound === null
  ) return null
  return Object.freeze({
    translation,
    magnitude,
    certifiedMagnitudeUpperBound,
    activeConstraintIndices: Object.freeze([...activeConstraintIndices]),
  })
}

function solveLinearSystem(
  matrix: readonly (readonly number[])[],
  rightHandSide: readonly number[],
): readonly number[] | null {
  const size = rightHandSide.length
  if (
    size < 1
    || size > 3
    || matrix.length !== size
    || matrix.some((row) => row.length !== size)
  ) return null
  if (!rightHandSide.every(Number.isFinite)) return null
  const augmented = matrix.map((row, index) => [
    ...row,
    rightHandSide[index],
  ])
  let scale = 1
  for (const row of matrix) {
    for (const value of row) {
      if (!Number.isFinite(value)) return null
      scale = Math.max(scale, Math.abs(value))
    }
  }
  const pivotTolerance = scale * LINEAR_SOLVE_TOLERANCE
  for (let pivot = 0; pivot < size; pivot += 1) {
    let pivotRow = pivot
    for (let row = pivot + 1; row < size; row += 1) {
      if (
        Math.abs(augmented[row][pivot])
        > Math.abs(augmented[pivotRow][pivot])
      ) pivotRow = row
    }
    if (Math.abs(augmented[pivotRow][pivot]) <= pivotTolerance) return null
    if (pivotRow !== pivot) {
      const temporary = augmented[pivot]
      augmented[pivot] = augmented[pivotRow]
      augmented[pivotRow] = temporary
    }
    const divisor = augmented[pivot][pivot]
    for (let column = pivot; column <= size; column += 1) {
      augmented[pivot][column] /= divisor
    }
    for (let row = 0; row < size; row += 1) {
      if (row === pivot) continue
      const factor = augmented[row][pivot]
      for (let column = pivot; column <= size; column += 1) {
        augmented[row][column] -= factor * augmented[pivot][column]
      }
    }
  }
  const result = augmented.map((row) => row[size])
  return result.every(Number.isFinite) ? Object.freeze(result) : null
}

function inflateForStrictProjectionProof(
  initial: Point,
  constraints: readonly SolverConstraint[],
): Point | null {
  let translation = initial
  for (
    let step = 0;
    step <= MAX_STRICT_PROJECTION_INFLATION_STEPS;
    step += 1
  ) {
    let requiredScale = 1
    let allSatisfied = true
    for (const constraint of constraints) {
      const lowerBound = dotLowerBound(
        constraint.direction,
        translation,
      )
      if (lowerBound === null) return null
      if (lowerBound > constraint.requiredProjection) continue
      allSatisfied = false
      if (lowerBound <= 0) return null
      const ratio = constraint.solverTargetProjection / lowerBound
      if (!Number.isFinite(ratio) || ratio <= 0) return null
      requiredScale = Math.max(requiredScale, ratio)
    }
    if (allSatisfied) return translation
    if (step === MAX_STRICT_PROJECTION_INFLATION_STEPS) return null
    const scale = nextUpPositive(requiredScale)
    if (scale === null) return null
    const scaled = freezePoint({
      x: translation.x * scale,
      y: translation.y * scale,
      z: translation.z * scale,
    })
    if (!scaled) return null
    translation = scaled
  }
  return null
}

function dotLowerBound(first: Point, second: Point): number | null {
  const x = multiplyDown(first.x, second.x)
  const y = multiplyDown(first.y, second.y)
  const z = multiplyDown(first.z, second.z)
  if (x === null || y === null || z === null) return null
  const xy = addDown(x, y)
  if (xy === null) return null
  return addDown(xy, z)
}

function normUpperBound(value: Point): number | null {
  // The outward-rounded L1 norm is a deliberately conservative upper bound
  // for the exact Euclidean norm and avoids square overflow/underflow.
  let upperBound = 0
  for (const component of [
    Math.abs(value.x),
    Math.abs(value.y),
    Math.abs(value.z),
  ]) {
    const nextSum = addNonNegativeUp(upperBound, component)
    if (nextSum === null) return null
    upperBound = nextSum
  }
  return upperBound
}

function subtractNonNegativeUp(
  minuend: number,
  subtrahend: number,
): number | null {
  if (
    !Number.isFinite(minuend)
    || minuend < 0
    || !Number.isFinite(subtrahend)
    || subtrahend < 0
    || subtrahend > minuend
  ) return null
  if (minuend === subtrahend) return 0
  const rounded = minuend - subtrahend
  return rounded > 0 ? nextUpPositive(rounded) : null
}

function addNonNegativeUp(
  first: number,
  second: number,
): number | null {
  if (
    !Number.isFinite(first)
    || first < 0
    || !Number.isFinite(second)
    || second < 0
  ) return null
  const rounded = first + second
  if (!Number.isFinite(rounded)) return null
  return rounded === 0 ? 0 : nextUpPositive(rounded)
}

function multiplyDown(first: number, second: number): number | null {
  const rounded = first * second
  return Number.isFinite(rounded) ? nextDownFinite(rounded) : null
}

function addDown(first: number, second: number): number | null {
  const rounded = first + second
  return Number.isFinite(rounded) ? nextDownFinite(rounded) : null
}

function nextDownFinite(value: number): number | null {
  if (!Number.isFinite(value)) return null
  if (value === 0) return -Number.MIN_VALUE
  if (value > 0) return nextDownPositive(value)
  const upwardMagnitude = nextUpPositive(-value)
  return upwardMagnitude === null ? null : -upwardMagnitude
}

function nextDownPositive(value: number): number | null {
  if (!Number.isFinite(value) || value <= 0) return null
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  view.setFloat64(0, value, false)
  let high = view.getUint32(0, false)
  let low = view.getUint32(4, false)
  if (low === 0) {
    low = 0xffff_ffff
    high -= 1
  } else {
    low -= 1
  }
  view.setUint32(0, high, false)
  view.setUint32(4, low, false)
  const result = view.getFloat64(0, false)
  return Number.isFinite(result) && result < value ? result : null
}

function nextUpPositive(value: number): number | null {
  if (!Number.isFinite(value) || value <= 0) return null
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  view.setFloat64(0, value, false)
  let high = view.getUint32(0, false)
  let low = view.getUint32(4, false)
  if (low === 0xffff_ffff) {
    low = 0
    high += 1
  } else {
    low += 1
  }
  if (high > 0x7ff0_0000) return null
  view.setUint32(0, high, false)
  view.setUint32(4, low, false)
  const result = view.getFloat64(0, false)
  return Number.isFinite(result) && result > value ? result : null
}

function combinationCount(total: number, selection: number) {
  if (selection > total) return 0
  if (selection === 1) return total
  if (selection === 2) return total * (total - 1) / 2
  if (selection === 3) return total * (total - 1) * (total - 2) / 6
  return 0
}

function snapshotPoints(
  value: unknown,
  maximum: number,
): readonly Point[] | null {
  if (!Array.isArray(value)) return null
  const length = value.length
  if (
    !Number.isSafeInteger(length)
    || length <= 0
    || length > maximum
  ) return null
  const points: Point[] = []
  for (let index = 0; index < length; index += 1) {
    const point = snapshotPoint(value[index])
    if (!point) return null
    points.push(point)
  }
  return Object.freeze(points)
}

function positionGeneratorsMatch(
  firstSupport: readonly Point[],
  secondSupport: readonly Point[],
  generators: readonly Point[],
) {
  let index = 0
  for (const first of firstSupport) {
    for (const second of secondSupport) {
      const generator = generators[index]
      if (
        !generator
        || generator.x !== first.x / 2 + second.x / 2
        || generator.y !== first.y / 2 + second.y / 2
        || generator.z !== first.z / 2 + second.z / 2
      ) return false
      index += 1
    }
  }
  return index === generators.length
}

function snapshotUnitPoint(value: unknown): Point | null {
  const point = snapshotPoint(value)
  if (!point) return null
  const length = Math.hypot(point.x, point.y, point.z)
  return Number.isFinite(length)
    && Math.abs(length - 1) <= UNIT_VECTOR_TOLERANCE
    ? point
    : null
}

function snapshotPoint(value: unknown): Point | null {
  if (!isRecord(value)) return null
  const x = value.x
  const y = value.y
  const z = value.z
  return freezePoint({ x, y, z })
}

function freezePoint(value: { x: unknown; y: unknown; z: unknown }): Point | null {
  const x = value.x
  const y = value.y
  const z = value.z
  return typeof x === 'number'
    && Number.isFinite(x)
    && typeof y === 'number'
    && Number.isFinite(y)
    && typeof z === 'number'
    && Number.isFinite(z)
    ? Object.freeze({
        x: canonicalZero(x),
        y: canonicalZero(y),
        z: canonicalZero(z),
      })
    : null
}

function dot(first: Point, second: Point) {
  return first.x * second.x + first.y * second.y + first.z * second.z
}

function canonicalZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_ID_LENGTH
}

function validCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validTriangleIndex(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && (value as number) >= 0
    && (value as number) <= 1_000_000
}

function validNonNegative(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
}

function validPositive(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validUnitTime(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 1
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}
