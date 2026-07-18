import { Vector3 } from 'three'
import { collectFoldTreeDependentFaces } from './foldPreviewAnchoring.ts'
import type {
  FoldPreviewCollisionAdjacency,
} from './foldPreviewCollision.ts'
import type {
  FoldPreviewHingeContactConstraint,
} from './foldPreviewHingeCollision.ts'
import { triangulateFoldPreviewPolygon } from './foldPreviewGeometry.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
  type FoldPreviewTreePose,
} from './foldPreviewKinematics.ts'
import {
  FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
  FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
  MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  prepareFoldPreviewNarrowPhase,
  type FoldPreviewFullScanNonAdjacentWitnessJob,
  type FoldPreviewFullScanNonAdjacentWitnessJobStep,
  type FoldPreviewFullScanNonAdjacentWitnessSet,
  type FoldPreviewNarrowPhaseAnalysisJob,
  type FoldPreviewNarrowPhaseAnalysisJobStep,
  type FoldPreviewNarrowPhaseResult,
} from './foldPreviewNarrowCollision.ts'
import {
  deriveFoldPreviewSingleHingeRotationFitSeeds,
  MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS,
  type FoldPreviewSingleHingeRotationFitSeed,
} from './foldPreviewSingleHingeRotationFitSeeds.ts'
import {
  replaceFoldPreviewTreeMotionSelectedAngle,
  type FoldPreviewTreeMotionContext,
} from './foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from './foldPreviewTreeScenePose.ts'
import {
  isFoldPreviewTreeTerminalFullScanBindingAuthenticForModel,
  type FoldPreviewTreeTerminalFullScanBinding,
} from './foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  deriveFoldPreviewTwoBodyCorrectionCandidate,
  type FoldPreviewTwoBodyCorrectionCandidate,
} from './foldPreviewTwoBodyCorrectionCandidate.ts'

export const FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION =
  'tree_single_hinge_static_correction_candidates_v1'
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES =
  MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS =
  1_000_000
export const FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION =
  'tree_single_hinge_static_correction_candidates_job_v1'

const MATERIAL_POINT_EQUIVALENCE_FACTOR = 4_096
const isSafeIntegerIntrinsic = Number.isSafeInteger

type Point = Readonly<{ x: number; y: number; z: number }>

export type FoldPreviewTreeSingleHingeStaticCorrectionAnalysis = Readonly<{
  broadPhaseCandidateCount: number
  broadPhaseNonAdjacentCandidateCount: number
  broadPhaseHingeAdjacentCandidateCount: number
  interactionCount: number
  allowedHingeInteractionCount: number
  trianglePairTests: number
  satTests: number
  numericalMargin: number
  fullScanBroadPhaseCandidateCount: number
  fullScanExpectedTrianglePairCount: number
  fullScanTrianglePairTests: number
  fullScanAabbRejectedPairCount: number
  fullScanSatTests: number
  fullScanSatSeparatedPairCount: number
}>

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidate = Readonly<{
  rank: number
  sourceSeedRank: number
  source: FoldPreviewSingleHingeRotationFitSeed['source']
  pose: Readonly<{
    poseRequestKey: string
    selectedAngleDegrees: number
    appliedAngles: readonly FoldPreviewHingeAngle[]
  }>
  fit: Readonly<{
    signedDeltaDegrees: number
    signedRotationRadians: number
    residualSquared: number
    residualRms: number
    improvementSquared: number
    improvementRatio: number
  }>
  staticAnalysis: FoldPreviewTreeSingleHingeStaticCorrectionAnalysis
  safety: Readonly<{
    modelIdentityBound: true
    completeLegalAngleVectorGenerated: true
    legalCorrectionPoseGenerated: true
    collisionConstraintsRevalidated: true
    hingeContactPolicySatisfied: true
    wholeSceneStaticClear: true
    staticCandidateRevalidated: true
    continuousCandidatePathCertified: false
    sceneApplied: false
    autoApplicable: false
  }>
}>

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidates = Readonly<{
  version:
    typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION
  kind: 'statically_revalidated_single_hinge_correction_candidates'
  sourceIdentity: Readonly<{
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
    sourceSelectedAngleDegrees: number
    blockingSelectedAngleDegrees: number
    collisionThickness: number
  }>
  sourcePartition: Readonly<{
    version: 'rerooted_selected_hinge_partition_v1'
    stationaryFaceIds: readonly string[]
    movingFaceIds: readonly string[]
  }>
  commonTranslation: Readonly<{
    version: FoldPreviewTwoBodyCorrectionCandidate['version']
    translation: Point
    magnitude: number
    certifiedMagnitudeUpperBound: number
    clearance: number
    maximumTranslation: number
    constraintCount: number
    solver: Readonly<{
      method: FoldPreviewTwoBodyCorrectionCandidate['solver']['method']
      seedMethod: FoldPreviewTwoBodyCorrectionCandidate['solver']['seedMethod']
      activeConstraintIndices: readonly number[]
      activeSetSize: 1 | 2 | 3
      evaluatedActiveSetCount: number
      maximumActiveSetCount: number
    }>
  }>
  rotationFit: Readonly<{
    version: 'single_hinge_rotation_fit_seeds_v1'
    method: 'bounded_finite_rotation_least_squares_v1'
    objective: 'moving_material_points_match_common_translation'
    maximumAngleDeltaDegrees: number
    angleDomain: Readonly<{
      minimumDegrees: number
      maximumDegrees: number
    }>
    worldAxis: Readonly<{
      point: Point
      direction: Point
    }>
    childRotationSign: 1 | -1
    movingPointCount: number
    baselineResidualSquared: number
    baselineResidualRms: number
    evaluatedCandidateCount: number
    seedCount: number
  }>
  staticValidationWork: Readonly<{
    strategy: 'full_non_adjacent_then_hinge_policy_v1'
    maximumTrianglePairVisits:
      typeof MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
    plannedTrianglePairVisitUpperBound: number
    actualTrianglePairVisits: number
    fullScanCount: number
    narrowScanCount: number
  }>
  candidates: readonly FoldPreviewTreeSingleHingeStaticCorrectionCandidate[]
  safety: Readonly<{
    modelIdentityBound: true
    sourcePoseIdentityVerified: true
    blockingPoseIdentityVerified: true
    partitionRevalidated: true
    completeLegalAngleVectorsGenerated: true
    legalCorrectionPoseGenerated: true
    collisionConstraintsRevalidated: true
    hingeContactPolicySatisfied: true
    wholeSceneStaticClear: true
    staticCandidateRevalidated: true
    continuousCandidatePathCertified: false
    sceneApplied: false
    autoApplicable: false
  }>
}>

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWork =
  Readonly<{
    totalWorkUnits: number
    /** Full and narrow scans share the global one-million pair-visit cap. */
    trianglePairTests: number
    /** Witness attempts are metered separately from pair visits. */
    witnessDerivations: number
  }>

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds =
  Readonly<{
    /** Synchronous stages below prevent a whole-step wall-clock claim. */
    entireStepTimeBounded: false
    /**
     * Authenticity checks, the two-body solve, context/partition checks,
     * rotation fitting, analyzer construction, and triangle counting are
     * synchronous factory work.
     */
    synchronousFactoryPreparation: true
    /** Angle vectors, pose keys, and candidate poses are factory work. */
    synchronousCandidatePosePreparation: true
    /**
     * Child factories synchronously snapshot transforms, run broad phase, and
     * construct prisms in a dedicated outer preparation step.
     */
    synchronousChildJobPreparation: true
    /** Child hinge-contact policy finalization remains synchronous. */
    synchronousHingePolicyFinalization: true
    /** Successful result construction and deep freezing are synchronous. */
    synchronousResultFinalization: true
    candidateSeedCount: number
    maximumTrianglePairTests:
      typeof MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
    plannedTrianglePairVisitUpperBound: number
    maximumWitnessDerivations: number
    maximumTotalWorkUnits: number
  }>

type FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobPhase =
  | 'full_scan_preparation'
  | 'full_scan'
  | 'narrow_scan_preparation'
  | 'narrow_scan'

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep =
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION
      kind: 'pending'
      phase: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobPhase
      work: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWork
      workBounds:
        FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION
      kind: 'complete'
      result: FoldPreviewTreeSingleHingeStaticCorrectionCandidates
      work: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWork
      workBounds:
        FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION
      kind: 'exhausted'
      work: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWork
      workBounds:
        FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION
      kind: 'indeterminate'
      reason:
        | 'invalid_work_budget'
        | 'child_job_creation_error'
        | 'child_job_error'
        | 'malformed_child_step'
        | 'work_accounting_error'
        | 'result_finalization_error'
      work: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWork
      workBounds:
        FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION
      kind: 'cancelled'
      work: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWork
      workBounds:
        FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds
    }>

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob =
  Readonly<{
    /**
     * Delegates at most `workBudget` metered pair/witness units to the active
     * child. A phase transition returns immediately, leaving a cancellation
     * window before the next child factory or candidate starts.
     */
    step(
      workBudget: number,
    ): FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep
    workBounds:
      FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds
    cancel(): void
  }>

const staticCorrectionCandidateContexts = new WeakMap<
  object,
  FoldPreviewTreeMotionContext
>()

/**
 * Confirms that an unchanged successful result came from this exact authentic
 * motion-context snapshot. Structural clones and equivalent replacement
 * contexts deliberately do not cross this provenance boundary.
 */
export function isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
  context: FoldPreviewTreeMotionContext,
  value: unknown,
): value is FoldPreviewTreeSingleHingeStaticCorrectionCandidates {
  try {
    return typeof context === 'object'
      && context !== null
      && typeof value === 'object'
      && value !== null
      && staticCorrectionCandidateContexts.get(value) === context
  } catch {
    return false
  }
}

type VerifiedContext = Readonly<{
  sourceAngles: readonly FoldPreviewHingeAngle[]
  blockingAngles: readonly FoldPreviewHingeAngle[]
  sourcePoseRequestKey: string
  blockingPoseRequestKey: string
  blockingPose: FoldPreviewTreePose
  selectedJoint: FoldPreviewTreeMotionContext['tree']['joints'][number]
  sourceSelectedAngleDegrees: number
}>

type VerifiedPartition = Readonly<{
  stationaryFaceIds: readonly string[]
  movingFaceIds: readonly string[]
}>

/**
 * Rebinds terminal two-body evidence to one authentic tree-motion context and
 * retains only complete one-hinge angle vectors that are collision-free in a
 * fresh whole-scene static analysis.
 *
 * This compatibility wrapper synchronously drains the resumable job. The
 * returned candidates remain analysis-only: no scene pose is applied and no
 * path to a returned candidate is certified.
 */
export function deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
  context: FoldPreviewTreeMotionContext,
  binding: FoldPreviewTreeTerminalFullScanBinding,
  clearance: number,
  maximumTranslation: number,
  maximumAngleDeltaDegrees: number,
): FoldPreviewTreeSingleHingeStaticCorrectionCandidates | null {
  const job = createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
    context,
    binding,
    clearance,
    maximumTranslation,
    maximumAngleDeltaDegrees,
  )
  if (!job) return null
  try {
    const maximumSteps = job.workBounds.candidateSeedCount * 4 + 2
    for (let index = 0; index < maximumSteps; index += 1) {
      const step = job.step(job.workBounds.maximumTotalWorkUnits)
      if (step.kind === 'complete') return step.result
      if (step.kind !== 'pending') return null
    }
    job.cancel()
    return null
  } catch {
    try {
      job.cancel()
    } catch {
      // Legacy callers retain the original fail-closed null boundary.
    }
    return null
  }
}

type PreparedStaticCorrectionSeed = Readonly<{
  seed: FoldPreviewSingleHingeRotationFitSeed
  appliedAngles: readonly FoldPreviewHingeAngle[]
  poseRequestKey: string
  pose: FoldPreviewTreePose
}>

type PreparedStaticCorrectionFactory = Readonly<{
  context: FoldPreviewTreeMotionContext
  translationCandidate: FoldPreviewTwoBodyCorrectionCandidate
  verifiedContext: VerifiedContext
  partition: VerifiedPartition
  worldAxis: Readonly<{ point: Point; direction: Point }>
  movingPointCount: number
  fit: NonNullable<
    ReturnType<typeof deriveFoldPreviewSingleHingeRotationFitSeeds>
  >
  analyzer: NonNullable<ReturnType<typeof prepareStaticAnalyzer>>
  preparedSeeds: readonly PreparedStaticCorrectionSeed[]
  plannedTrianglePairVisitUpperBound: number
  workBounds:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds
}>

type ChildWorkSnapshot = Readonly<{
  totalWorkUnits: number
  trianglePairTests: number
  witnessDerivations: number
}>

type ActiveStaticCorrectionChild =
  | {
      kind: 'full_scan'
      job: FoldPreviewFullScanNonAdjacentWitnessJob
      workBounds: FoldPreviewFullScanNonAdjacentWitnessJob['workBounds']
      observedWork: ChildWorkSnapshot
      inFlightBudget: number | null
    }
  | {
      kind: 'narrow_scan'
      job: FoldPreviewNarrowPhaseAnalysisJob
      workBounds: FoldPreviewNarrowPhaseAnalysisJob['workBounds']
      observedWork: ChildWorkSnapshot
      inFlightBudget: number | null
    }

type AccountedStaticCorrectionChildStep =
  | Readonly<{
      kind: 'accepted'
      step:
        | FoldPreviewFullScanNonAdjacentWitnessJobStep
        | FoldPreviewNarrowPhaseAnalysisJobStep
    }>
  | Readonly<{ kind: 'malformed' }>
  | Readonly<{ kind: 'work_accounting_error' }>

/**
 * Creates a resumable full-scan-then-hinge-policy job for every fitted seed.
 * Only complete success publishes candidates or provenance.
 */
export function createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
  context: FoldPreviewTreeMotionContext,
  binding: FoldPreviewTreeTerminalFullScanBinding,
  clearance: number,
  maximumTranslation: number,
  maximumAngleDeltaDegrees: number,
): FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob | null {
  let prepared: PreparedStaticCorrectionFactory | null
  try {
    if (
      replaceFoldPreviewTreeMotionSelectedAngle(context, 0) === null
      || !isFoldPreviewTreeTerminalFullScanBindingAuthenticForModel(
        context.model,
        binding,
      )
    ) return null
    prepared = prepareStaticCorrectionFactory(
      context,
      binding,
      clearance,
      maximumTranslation,
      maximumAngleDeltaDegrees,
    )
  } catch {
    return null
  }
  if (!prepared) return null

  const {
    analyzer,
    preparedSeeds,
    workBounds,
  } = prepared
  const candidates: FoldPreviewTreeSingleHingeStaticCorrectionCandidate[] = []
  let seedIndex = 0
  let phase:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobPhase =
      'full_scan_preparation'
  let fullScanForSeed: Extract<
    FoldPreviewFullScanNonAdjacentWitnessSet,
    { kind: 'complete' }
  > | null = null
  let activeChild: ActiveStaticCorrectionChild | null = null
  let trianglePairTests = 0
  let witnessDerivations = 0
  let fullScanCount = 0
  let narrowScanCount = 0
  let cancelled = false
  let stepping = false
  let terminal:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep | null = null

  const work = ():
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWork =>
    Object.freeze({
      totalWorkUnits: trianglePairTests + witnessDerivations,
      trianglePairTests,
      witnessDerivations,
    })

  const publish = (
    value: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep,
  ): FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep => {
    if (terminal) return terminal
    if (cancelled && value.kind !== 'cancelled') {
      return cancelledStep()
    }
    const snapshot = value.kind === 'pending'
      ? Object.freeze(value)
      : deepFreeze(value)
    if (terminal) return terminal
    if (cancelled && snapshot.kind !== 'cancelled') {
      return cancelledStep()
    }
    if (snapshot.kind === 'pending') return snapshot
    terminal = snapshot
    return terminal
  }

  const pending = () => publish({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION,
    kind: 'pending',
    phase,
    work: work(),
    workBounds,
  })

  const indeterminate = (
    reason: Extract<
      FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep,
      { kind: 'indeterminate' }
    >['reason'],
  ) => publish({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION,
    kind: 'indeterminate',
    reason,
    work: work(),
    workBounds,
  })

  const cancelledStep = ():
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep =>
    publish({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION,
    kind: 'cancelled',
    work: work(),
    workBounds,
  })

  const exhausted = () => publish({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION,
    kind: 'exhausted',
    work: work(),
    workBounds,
  })

  const cancelActiveChild = () => {
    try {
      activeChild?.job.cancel()
    } catch {
      // Parent cancellation still takes precedence at this boundary.
    }
  }

  const accountChildStep = (
    rawStep: unknown,
    delegatedBudget: number,
  ): AccountedStaticCorrectionChildStep => {
    const child = activeChild
    if (!child || !isRecord(rawStep)) return { kind: 'malformed' }
    const expectedVersion = child.kind === 'full_scan'
      ? FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
      : FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
    if (
      rawStep.version !== expectedVersion
      || rawStep.workBounds !== child.workBounds
      || !isRecord(rawStep.work)
      || (
        rawStep.kind !== 'pending'
        && rawStep.kind !== 'complete'
        && rawStep.kind !== 'indeterminate'
        && rawStep.kind !== 'cancelled'
      )
      || (
        rawStep.kind === 'pending'
        && rawStep.phase !== 'triangle_pair_scan'
        && rawStep.phase !== 'witness_derivation'
      )
      || (rawStep.kind === 'complete' && !isRecord(rawStep.result))
    ) return { kind: 'malformed' }
    const current = snapshotChildWork(rawStep.work)
    if (!current) return { kind: 'work_accounting_error' }
    const previous = child.observedWork
    const totalDelta =
      current.totalWorkUnits - previous.totalWorkUnits
    const pairDelta =
      current.trianglePairTests - previous.trianglePairTests
    const witnessDelta =
      current.witnessDerivations - previous.witnessDerivations
    const maximumTrianglePairTests = child.kind === 'full_scan'
      ? child.workBounds.expectedTrianglePairCount
      : child.workBounds.maximumTrianglePairTests
    if (
      totalDelta < 0
      || pairDelta < 0
      || witnessDelta < 0
      || totalDelta !== pairDelta + witnessDelta
      || totalDelta > delegatedBudget
      || current.trianglePairTests > maximumTrianglePairTests
      || current.witnessDerivations
        > child.workBounds.maximumWitnessDerivations
      || current.totalWorkUnits > child.workBounds.maximumTotalWorkUnits
    ) return { kind: 'work_accounting_error' }
    const nextPairTests = trianglePairTests + pairDelta
    const nextWitnessDerivations =
      witnessDerivations + witnessDelta
    const nextTotal = nextPairTests + nextWitnessDerivations
    if (
      !isSafeIntegerIntrinsic(nextPairTests)
      || nextPairTests < 0
      || nextPairTests
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
      || nextPairTests
        > workBounds.plannedTrianglePairVisitUpperBound
      || !isSafeIntegerIntrinsic(nextWitnessDerivations)
      || !isSafeIntegerIntrinsic(nextTotal)
      || nextWitnessDerivations > workBounds.maximumWitnessDerivations
      || nextTotal > workBounds.maximumTotalWorkUnits
    ) return { kind: 'work_accounting_error' }
    trianglePairTests = nextPairTests
    witnessDerivations = nextWitnessDerivations
    child.observedWork = current
    return {
      kind: 'accepted',
      step: rawStep as
        | FoldPreviewFullScanNonAdjacentWitnessJobStep
        | FoldPreviewNarrowPhaseAnalysisJobStep,
    }
  }

  const observeCancelledChild = () => {
    const child = activeChild
    if (!child) return
    const delegatedBudget = child.inFlightBudget ?? 0
    try {
      child.job.cancel()
      const rawStep = child.job.step(1)
      accountChildStep(rawStep, delegatedBudget)
    } catch {
      // Cancellation remains terminal even if observation itself fails.
    }
  }

  const finishCancellation = () => {
    cancelled = true
    observeCancelledChild()
    return cancelledStep()
  }

  const finishCandidates = () => {
    if (candidates.length === 0) return exhausted()
    if (
      candidates.length
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES
    ) return indeterminate('result_finalization_error')
    try {
      const result = createStaticCorrectionCandidatesResult(
        prepared,
        candidates,
        trianglePairTests,
        fullScanCount,
        narrowScanCount,
      )
      const published = publish({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION,
        kind: 'complete',
        result,
        work: work(),
        workBounds,
      })
      if (
        published.kind === 'complete'
        && published.result === result
      ) {
        staticCorrectionCandidateContexts.set(result, context)
      }
      return published
    } catch {
      return indeterminate('result_finalization_error')
    }
  }

  const finishSeed = () => {
    activeChild = null
    fullScanForSeed = null
    seedIndex += 1
    if (seedIndex >= preparedSeeds.length) return finishCandidates()
    phase = 'full_scan_preparation'
    return pending()
  }

  const prepareFullScanChild = () => {
    const seed = preparedSeeds[seedIndex]
    if (!seed) return indeterminate('child_job_creation_error')
    let job: FoldPreviewFullScanNonAdjacentWitnessJob | null
    try {
      job = analyzer.createFullScanNonAdjacentWitnessSetJob(
        seed.pose.faceTransforms,
        context.collisionThickness,
      )
    } catch {
      job = null
    }
    if (terminal) {
      try {
        job?.cancel()
      } catch {
        // The already-published terminal remains authoritative.
      }
      return terminal
    }
    if (cancelled) {
      try {
        job?.cancel()
      } catch {
        // Parent cancellation remains authoritative.
      }
      return finishCancellation()
    }
    if (!job) return indeterminate('child_job_creation_error')
    activeChild = {
      kind: 'full_scan',
      job,
      workBounds: job.workBounds,
      observedWork: zeroChildWork(),
      inFlightBudget: null,
    }
    phase = 'full_scan'
    return pending()
  }

  const prepareNarrowScanChild = () => {
    const seed = preparedSeeds[seedIndex]
    if (!seed || !fullScanForSeed) {
      return indeterminate('child_job_creation_error')
    }
    let job: FoldPreviewNarrowPhaseAnalysisJob | null
    try {
      job = analyzer.createAnalysisJob(
        seed.pose.faceTransforms,
        context.collisionThickness,
      )
    } catch {
      job = null
    }
    if (terminal) {
      try {
        job?.cancel()
      } catch {
        // The already-published terminal remains authoritative.
      }
      return terminal
    }
    if (cancelled) {
      try {
        job?.cancel()
      } catch {
        // Parent cancellation remains authoritative.
      }
      return finishCancellation()
    }
    if (!job) return indeterminate('child_job_creation_error')
    activeChild = {
      kind: 'narrow_scan',
      job,
      workBounds: job.workBounds,
      observedWork: zeroChildWork(),
      inFlightBudget: null,
    }
    phase = 'narrow_scan'
    return pending()
  }

  const advanceFullScan = (workBudget: number) => {
    const child = activeChild
    if (!child || child.kind !== 'full_scan') {
      return indeterminate('child_job_error')
    }
    child.inFlightBudget = workBudget
    let rawStep: unknown
    try {
      rawStep = child.job.step(workBudget)
    } catch {
      cancelActiveChild()
      observeCancelledChild()
      child.inFlightBudget = null
      return cancelled
        ? cancelledStep()
        : indeterminate('child_job_error')
    }
    const accounted = accountChildStep(rawStep, workBudget)
    child.inFlightBudget = null
    if (terminal) return terminal
    if (cancelled) return finishCancellation()
    if (accounted.kind === 'malformed') {
      return indeterminate('malformed_child_step')
    }
    if (accounted.kind === 'work_accounting_error') {
      return indeterminate('work_accounting_error')
    }
    const step = accounted.step
    if (step.kind === 'pending') return pending()
    if (step.kind === 'cancelled') return finishCancellation()
    if (step.kind === 'indeterminate') {
      return indeterminate('child_job_error')
    }
    if (
      step.version
        !== FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION
    ) return indeterminate('malformed_child_step')
    fullScanCount += 1
    activeChild = null
    if (!fullNonAdjacentScanIsClear(step.result)) return finishSeed()
    fullScanForSeed = step.result
    phase = 'narrow_scan_preparation'
    return pending()
  }

  const advanceNarrowScan = (workBudget: number) => {
    const child = activeChild
    const seed = preparedSeeds[seedIndex]
    if (
      !child
      || child.kind !== 'narrow_scan'
      || !seed
      || !fullScanForSeed
    ) return indeterminate('child_job_error')
    child.inFlightBudget = workBudget
    let rawStep: unknown
    try {
      rawStep = child.job.step(workBudget)
    } catch {
      cancelActiveChild()
      observeCancelledChild()
      child.inFlightBudget = null
      return cancelled
        ? cancelledStep()
        : indeterminate('child_job_error')
    }
    const accounted = accountChildStep(rawStep, workBudget)
    child.inFlightBudget = null
    if (terminal) return terminal
    if (cancelled) return finishCancellation()
    if (accounted.kind === 'malformed') {
      return indeterminate('malformed_child_step')
    }
    if (accounted.kind === 'work_accounting_error') {
      return indeterminate('work_accounting_error')
    }
    const step = accounted.step
    if (step.kind === 'pending') return pending()
    if (step.kind === 'cancelled') return finishCancellation()
    if (step.kind === 'indeterminate') {
      return indeterminate('child_job_error')
    }
    if (
      step.version !== FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION
    ) return indeterminate('malformed_child_step')
    narrowScanCount += 1
    const staticAnalysis = staticClearAnalysis(
      step.result,
      fullScanForSeed,
    )
    if (staticAnalysis) {
      candidates.push(createStaticCorrectionCandidate(
        candidates.length + 1,
        seed,
        staticAnalysis,
      ))
    }
    return finishSeed()
  }

  return Object.freeze({
    workBounds,
    step(
      workBudget: number,
    ): FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep {
      if (terminal) return terminal
      if (cancelled) return finishCancellation()
      if (stepping) return finishCancellation()
      stepping = true
      try {
        const validWorkBudget =
          Number.isSafeInteger(workBudget) && workBudget > 0
        if (terminal) return terminal
        if (cancelled) return finishCancellation()
        if (!validWorkBudget) {
          return indeterminate('invalid_work_budget')
        }
        if (phase === 'full_scan_preparation') {
          return prepareFullScanChild()
        }
        if (phase === 'narrow_scan_preparation') {
          return prepareNarrowScanChild()
        }
        if (phase === 'full_scan') return advanceFullScan(workBudget)
        return advanceNarrowScan(workBudget)
      } catch {
        if (terminal) return terminal
        if (cancelled) return finishCancellation()
        cancelActiveChild()
        observeCancelledChild()
        return indeterminate('child_job_error')
      } finally {
        stepping = false
      }
    },
    cancel() {
      if (terminal || cancelled) return
      cancelled = true
      cancelActiveChild()
    },
  })
}

function prepareStaticCorrectionFactory(
  context: FoldPreviewTreeMotionContext,
  binding: FoldPreviewTreeTerminalFullScanBinding,
  clearance: number,
  maximumTranslation: number,
  maximumAngleDeltaDegrees: number,
): PreparedStaticCorrectionFactory | null {
  if (!validPositiveAngleDelta(maximumAngleDeltaDegrees)) return null

  const translationCandidate =
    deriveFoldPreviewTwoBodyCorrectionCandidate(
      binding,
      clearance,
      maximumTranslation,
    )
  if (!translationCandidate) return null

  const verifiedContext = verifyContextAndPoses(
    context,
    translationCandidate,
  )
  if (!verifiedContext) return null
  const partition = verifyPartition(
    context,
    translationCandidate,
    verifiedContext.selectedJoint,
  )
  if (!partition) return null

  const worldAxis = worldSelectedHingeAxis(
    verifiedContext.blockingPose,
    verifiedContext.selectedJoint,
  )
  if (!worldAxis) return null
  const movingPoints = collectMovingWorldMaterialPoints(
    context,
    verifiedContext.blockingPose,
    partition.movingFaceIds,
  )
  if (!movingPoints) return null

  const fit = deriveFoldPreviewSingleHingeRotationFitSeeds({
    axis: worldAxis,
    childRotationSign: verifiedContext.selectedJoint.childRotationSign,
    blockingAngleDegrees:
      translationCandidate.sourceIdentity.selectedAngleDegrees,
    maximumAngleDeltaDegrees,
    translation: translationCandidate.translation,
    movingPoints,
  })
  if (!fit || fit.seeds.length === 0) return null

  const trianglePairUpperBound =
    allFaceTrianglePairUpperBound(context)
  const plannedTrianglePairVisitUpperBound =
    trianglePairUpperBound === null
      ? null
      : boundedProduct(
          trianglePairUpperBound,
          fit.seeds.length * 2,
        )
  if (
    plannedTrianglePairVisitUpperBound === null
    || plannedTrianglePairVisitUpperBound <= 0
    || plannedTrianglePairVisitUpperBound
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
  ) return null
  const maximumWitnessDerivations = boundedProduct(
    fit.seeds.length * 2,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  if (maximumWitnessDerivations === null) return null
  const maximumTotalWorkUnits =
    plannedTrianglePairVisitUpperBound + maximumWitnessDerivations
  if (!Number.isSafeInteger(maximumTotalWorkUnits)) return null
  const workBounds:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobWorkBounds =
    Object.freeze({
      entireStepTimeBounded: false,
      synchronousFactoryPreparation: true,
      synchronousCandidatePosePreparation: true,
      synchronousChildJobPreparation: true,
      synchronousHingePolicyFinalization: true,
      synchronousResultFinalization: true,
      candidateSeedCount: fit.seeds.length,
      maximumTrianglePairTests:
        MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS,
      plannedTrianglePairVisitUpperBound,
      maximumWitnessDerivations,
      maximumTotalWorkUnits,
    })

  const analyzer = prepareStaticAnalyzer(context)
  if (!analyzer) return null
  const preparedSeeds: PreparedStaticCorrectionSeed[] = []
  for (const seed of fit.seeds) {
    const appliedAngles = replaceFoldPreviewTreeMotionSelectedAngle(
      context,
      seed.angleDegrees,
    )
    if (!appliedAngles) return null
    const poseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
      context.model,
      context.fixedFaceId,
      context.collisionThickness,
      appliedAngles,
    )
    if (!poseRequestKey) return null
    const pose = calculateFoldTreePoseWithAngles(context.tree, {
      kind: 'per_hinge',
      angles: appliedAngles,
    })
    if (!pose) return null
    preparedSeeds.push(Object.freeze({
      seed,
      appliedAngles: Object.freeze(copyAngles(appliedAngles)),
      poseRequestKey,
      pose,
    }))
  }
  if (preparedSeeds.length !== fit.seeds.length) return null
  return Object.freeze({
    context,
    translationCandidate,
    verifiedContext,
    partition,
    worldAxis,
    movingPointCount: movingPoints.length,
    fit,
    analyzer,
    preparedSeeds: Object.freeze(preparedSeeds),
    plannedTrianglePairVisitUpperBound,
    workBounds,
  })
}

function createStaticCorrectionCandidate(
  rank: number,
  prepared: PreparedStaticCorrectionSeed,
  staticAnalysis: FoldPreviewTreeSingleHingeStaticCorrectionAnalysis,
) {
  const seed = prepared.seed
  return deepFreeze<FoldPreviewTreeSingleHingeStaticCorrectionCandidate>({
    rank,
    sourceSeedRank: seed.rank,
    source: seed.source,
    pose: {
      poseRequestKey: prepared.poseRequestKey,
      selectedAngleDegrees: seed.angleDegrees,
      appliedAngles: copyAngles(prepared.appliedAngles),
    },
    fit: {
      signedDeltaDegrees: seed.signedDeltaDegrees,
      signedRotationRadians: seed.signedRotationRadians,
      residualSquared: seed.residualSquared,
      residualRms: seed.residualRms,
      improvementSquared: seed.improvementSquared,
      improvementRatio: seed.improvementRatio,
    },
    staticAnalysis,
    safety: {
      modelIdentityBound: true,
      completeLegalAngleVectorGenerated: true,
      legalCorrectionPoseGenerated: true,
      collisionConstraintsRevalidated: true,
      hingeContactPolicySatisfied: true,
      wholeSceneStaticClear: true,
      staticCandidateRevalidated: true,
      continuousCandidatePathCertified: false,
      sceneApplied: false,
      autoApplicable: false,
    },
  })
}

function createStaticCorrectionCandidatesResult(
  prepared: PreparedStaticCorrectionFactory,
  candidates: readonly FoldPreviewTreeSingleHingeStaticCorrectionCandidate[],
  actualTrianglePairVisits: number,
  fullScanCount: number,
  narrowScanCount: number,
) {
  const {
    translationCandidate,
    verifiedContext,
    partition,
    worldAxis,
    movingPointCount,
    fit,
    plannedTrianglePairVisitUpperBound,
  } = prepared
  return deepFreeze<FoldPreviewTreeSingleHingeStaticCorrectionCandidates>({
    version:
      FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION,
    kind: 'statically_revalidated_single_hinge_correction_candidates',
    sourceIdentity: {
      projectId: translationCandidate.sourceIdentity.projectId,
      revision: translationCandidate.sourceIdentity.revision,
      fixedFaceId: translationCandidate.sourceIdentity.fixedFaceId,
      selectedHingeEdgeId:
        translationCandidate.sourceIdentity.selectedHingeEdgeId,
      contextKey: translationCandidate.sourceIdentity.contextKey,
      sourcePoseRequestKey: verifiedContext.sourcePoseRequestKey,
      blockingPoseRequestKey: verifiedContext.blockingPoseRequestKey,
      generation: translationCandidate.sourceIdentity.generation,
      requestSequence:
        translationCandidate.sourceIdentity.requestSequence,
      blockingSampleTime:
        translationCandidate.sourceIdentity.blockingSampleTime,
      sourceSelectedAngleDegrees:
        verifiedContext.sourceSelectedAngleDegrees,
      blockingSelectedAngleDegrees:
        translationCandidate.sourceIdentity.selectedAngleDegrees,
      collisionThickness:
        translationCandidate.sourceIdentity.collisionThickness,
    },
    sourcePartition: {
      version: translationCandidate.sourcePartition.version,
      stationaryFaceIds: [...partition.stationaryFaceIds],
      movingFaceIds: [...partition.movingFaceIds],
    },
    commonTranslation: {
      version: translationCandidate.version,
      translation: copyPoint(translationCandidate.translation),
      magnitude: translationCandidate.magnitude,
      certifiedMagnitudeUpperBound:
        translationCandidate.certifiedMagnitudeUpperBound,
      clearance: translationCandidate.clearance,
      maximumTranslation: translationCandidate.maximumTranslation,
      constraintCount: translationCandidate.constraints.length,
      solver: {
        method: translationCandidate.solver.method,
        seedMethod: translationCandidate.solver.seedMethod,
        activeConstraintIndices: [
          ...translationCandidate.solver.activeConstraintIndices,
        ],
        activeSetSize: translationCandidate.solver.activeSetSize,
        evaluatedActiveSetCount:
          translationCandidate.solver.evaluatedActiveSetCount,
        maximumActiveSetCount:
          translationCandidate.solver.maximumActiveSetCount,
      },
    },
    rotationFit: {
      version: fit.version,
      method: fit.analysis.method,
      objective: fit.analysis.objective,
      maximumAngleDeltaDegrees: fit.maximumAngleDeltaDegrees,
      angleDomain: {
        minimumDegrees: fit.angleDomain.minimumDegrees,
        maximumDegrees: fit.angleDomain.maximumDegrees,
      },
      worldAxis: {
        point: copyPoint(worldAxis.point),
        direction: copyPoint(worldAxis.direction),
      },
      childRotationSign:
        verifiedContext.selectedJoint.childRotationSign,
      movingPointCount,
      baselineResidualSquared: fit.baselineResidualSquared,
      baselineResidualRms: fit.baselineResidualRms,
      evaluatedCandidateCount: fit.evaluatedCandidateCount,
      seedCount: fit.seeds.length,
    },
    staticValidationWork: {
      strategy: 'full_non_adjacent_then_hinge_policy_v1',
      maximumTrianglePairVisits:
        MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS,
      plannedTrianglePairVisitUpperBound,
      actualTrianglePairVisits,
      fullScanCount,
      narrowScanCount,
    },
    candidates: [...candidates],
    safety: {
      modelIdentityBound: true,
      sourcePoseIdentityVerified: true,
      blockingPoseIdentityVerified: true,
      partitionRevalidated: true,
      completeLegalAngleVectorsGenerated: true,
      legalCorrectionPoseGenerated: true,
      collisionConstraintsRevalidated: true,
      hingeContactPolicySatisfied: true,
      wholeSceneStaticClear: true,
      staticCandidateRevalidated: true,
      continuousCandidatePathCertified: false,
      sceneApplied: false,
      autoApplicable: false,
    },
  })
}

function snapshotChildWork(value: Record<PropertyKey, unknown>) {
  const totalWorkUnits = value.totalWorkUnits
  const trianglePairTests = value.trianglePairTests
  const witnessDerivations = value.witnessDerivations
  if (
    !isSafeIntegerIntrinsic(totalWorkUnits)
    || !isSafeIntegerIntrinsic(trianglePairTests)
    || !isSafeIntegerIntrinsic(witnessDerivations)
    || (totalWorkUnits as number) < 0
    || (trianglePairTests as number) < 0
    || (witnessDerivations as number) < 0
    || totalWorkUnits !==
      (trianglePairTests as number) + (witnessDerivations as number)
  ) return null
  return Object.freeze({
    totalWorkUnits,
    trianglePairTests,
    witnessDerivations,
  }) as ChildWorkSnapshot
}

function zeroChildWork(): ChildWorkSnapshot {
  return Object.freeze({
    totalWorkUnits: 0,
    trianglePairTests: 0,
    witnessDerivations: 0,
  })
}

function isRecord(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === 'object' && value !== null
}

function verifyContextAndPoses(
  context: FoldPreviewTreeMotionContext,
  candidate: FoldPreviewTwoBodyCorrectionCandidate,
): VerifiedContext | null {
  const sourceSelectedAngleDegrees = context.selectedAngleDegrees
  const sourceAngles = replaceFoldPreviewTreeMotionSelectedAngle(
    context,
    sourceSelectedAngleDegrees,
  )
  if (
    !sourceAngles
    || context.version !== 'tree_single_hinge_motion_v1'
    || context.model.kind !== 'fold_graph'
    || context.model.projectId !== candidate.sourceIdentity.projectId
    || context.model.revision !== candidate.sourceIdentity.revision
    || context.fixedFaceId !== candidate.sourceIdentity.fixedFaceId
    || context.selectedHingeEdgeId
      !== candidate.sourceIdentity.selectedHingeEdgeId
    || context.contextKey !== candidate.sourceIdentity.contextKey
    || context.collisionThickness
      !== candidate.sourceIdentity.collisionThickness
    || !sameAngles(sourceAngles, context.appliedAngles)
  ) return null
  const selectedJoint = context.tree.joints.find(
    (joint) => joint.hinge.edgeId === context.selectedHingeEdgeId,
  )
  if (!selectedJoint) return null

  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    context.model,
    context.fixedFaceId,
    context.collisionThickness,
    sourceAngles,
  )
  if (
    !sourcePoseRequestKey
    || sourcePoseRequestKey !== candidate.sourceIdentity.sourcePoseRequestKey
  ) return null
  const blockingAngles = replaceFoldPreviewTreeMotionSelectedAngle(
    context,
    candidate.sourceIdentity.selectedAngleDegrees,
  )
  if (!blockingAngles) return null
  const blockingPoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    context.model,
    context.fixedFaceId,
    context.collisionThickness,
    blockingAngles,
  )
  if (
    !blockingPoseRequestKey
    || blockingPoseRequestKey
      !== candidate.sourceIdentity.blockingPoseRequestKey
  ) return null
  const blockingPose = calculateFoldTreePoseWithAngles(context.tree, {
    kind: 'per_hinge',
    angles: blockingAngles,
  })
  if (!blockingPose) return null
  return Object.freeze({
    sourceAngles: copyAngles(sourceAngles),
    blockingAngles: copyAngles(blockingAngles),
    sourcePoseRequestKey,
    blockingPoseRequestKey,
    blockingPose,
    selectedJoint,
    sourceSelectedAngleDegrees,
  })
}

function verifyPartition(
  context: FoldPreviewTreeMotionContext,
  candidate: FoldPreviewTwoBodyCorrectionCandidate,
  selectedJoint: FoldPreviewTreeMotionContext['tree']['joints'][number],
): VerifiedPartition | null {
  const movingFaceIds = collectFoldTreeDependentFaces(
    context.tree,
    context.selectedHingeEdgeId,
  )
  if (!movingFaceIds || movingFaceIds.length === 0) return null
  const moving = new Set(movingFaceIds)
  const stationaryFaceIds = context.model.faces
    .map((face) => face.id)
    .filter((faceId) => !moving.has(faceId))
  if (
    moving.size !== movingFaceIds.length
    || stationaryFaceIds.length === 0
    || !stationaryFaceIds.includes(context.fixedFaceId)
    || !stationaryFaceIds.includes(selectedJoint.parentFaceId)
    || !moving.has(selectedJoint.childFaceId)
    || !sameIds(
      stationaryFaceIds,
      candidate.sourcePartition.stationaryFaceIds,
    )
    || !sameIds(
      movingFaceIds,
      candidate.sourcePartition.movingFaceIds,
    )
  ) return null
  const allFaceIds = context.model.faces.map((face) => face.id)
  if (
    new Set(allFaceIds).size !== allFaceIds.length
    || stationaryFaceIds.length + movingFaceIds.length !== allFaceIds.length
    || allFaceIds.some((faceId) =>
      !moving.has(faceId) && !stationaryFaceIds.includes(faceId))
  ) return null
  for (const joint of context.tree.joints) {
    const parentMoving = moving.has(joint.parentFaceId)
    const childMoving = moving.has(joint.childFaceId)
    if (
      parentMoving !== childMoving
      && joint.hinge.edgeId !== context.selectedHingeEdgeId
    ) return null
  }
  return Object.freeze({
    stationaryFaceIds: Object.freeze([...stationaryFaceIds]),
    movingFaceIds: Object.freeze([...movingFaceIds]),
  })
}

function worldSelectedHingeAxis(
  pose: FoldPreviewTreePose,
  joint: FoldPreviewTreeMotionContext['tree']['joints'][number],
): Readonly<{ point: Point; direction: Point }> | null {
  const parentTransform = pose.faceTransforms.get(joint.parentFaceId)
  const hingeTransform = pose.hingeTransforms.get(joint.hinge.edgeId)
  if (
    !parentTransform
    || !hingeTransform
    || !sameMatrix(parentTransform.elements, hingeTransform.elements)
  ) return null
  const start = new Vector3(
    joint.hinge.start.x,
    0,
    joint.hinge.start.z,
  ).applyMatrix4(parentTransform)
  const end = new Vector3(
    joint.hinge.end.x,
    0,
    joint.hinge.end.z,
  ).applyMatrix4(parentTransform)
  const direction = end.clone().sub(start)
  const length = direction.length()
  if (
    !finiteVector(start)
    || !finiteVector(end)
    || !Number.isFinite(length)
    || length <= 0
  ) return null
  direction.multiplyScalar(1 / length)
  if (!finiteVector(direction)) return null
  return Object.freeze({
    point: freezePoint(start.x, start.y, start.z),
    direction: freezePoint(direction.x, direction.y, direction.z),
  })
}

function collectMovingWorldMaterialPoints(
  context: FoldPreviewTreeMotionContext,
  pose: FoldPreviewTreePose,
  movingFaceIds: readonly string[],
) {
  const moving = new Set(movingFaceIds)
  const points = new Map<string, Point>()
  const jointFactor = Math.max(1, context.tree.joints.length)
  for (const face of context.model.faces) {
    if (!moving.has(face.id)) continue
    const transform = pose.faceTransforms.get(face.id)
    if (!transform) return null
    for (const vertex of face.polygon) {
      const world = new Vector3(vertex.x, 0, vertex.z)
        .applyMatrix4(transform)
      if (!finiteVector(world)) return null
      const position = freezePoint(world.x, world.y, world.z)
      const existing = points.get(vertex.vertexId)
      if (
        existing
        && !equivalentPoint(existing, position, jointFactor)
      ) return null
      if (!existing) points.set(vertex.vertexId, position)
    }
  }
  if (points.size === 0) return null
  return Object.freeze(
    [...points.entries()]
      .sort(([first], [second]) => compareText(first, second))
      .map(([id, position]) => Object.freeze({ id, position })),
  )
}

function prepareStaticAnalyzer(context: FoldPreviewTreeMotionContext) {
  const adjacencies: FoldPreviewCollisionAdjacency[] =
    context.model.hinges.map((hinge) => ({
      edgeId: hinge.edgeId,
      firstFaceId: hinge.leftFaceId,
      secondFaceId: hinge.rightFaceId,
    }))
  const constraints: FoldPreviewHingeContactConstraint[] =
    context.model.hinges.map((hinge) => ({
      edgeId: hinge.edgeId,
      leftFaceId: hinge.leftFaceId,
      rightFaceId: hinge.rightFaceId,
      start: {
        vertexId: hinge.start.vertexId,
        x: hinge.start.x,
        z: hinge.start.z,
      },
      end: {
        vertexId: hinge.end.vertexId,
        x: hinge.end.x,
        z: hinge.end.z,
      },
      thicknessRule: 'centered_mid_surface_v1',
    }))
  return prepareFoldPreviewNarrowPhase(
    context.model.faces,
    adjacencies,
    constraints,
  )
}

function allFaceTrianglePairUpperBound(
  context: FoldPreviewTreeMotionContext,
) {
  let previousTriangleCount = 0
  let pairCount = 0
  for (const face of context.model.faces) {
    const triangleCount =
      triangulateFoldPreviewPolygon(face.polygon).length
    if (!Number.isSafeInteger(triangleCount) || triangleCount <= 0) {
      return null
    }
    const contribution = boundedProduct(
      previousTriangleCount,
      triangleCount,
    )
    if (contribution === null) return null
    pairCount += contribution
    previousTriangleCount += triangleCount
    if (
      !Number.isSafeInteger(pairCount)
      || !Number.isSafeInteger(previousTriangleCount)
      || pairCount
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
    ) return null
  }
  return pairCount
}

function boundedProduct(first: number, second: number) {
  if (
    !Number.isSafeInteger(first)
    || first < 0
    || !Number.isSafeInteger(second)
    || second < 0
    || (first !== 0 && second > Number.MAX_SAFE_INTEGER / first)
  ) return null
  const result = first * second
  return Number.isSafeInteger(result) ? result : null
}

function fullNonAdjacentScanIsClear(
  value: FoldPreviewFullScanNonAdjacentWitnessSet | null,
): value is Extract<
  FoldPreviewFullScanNonAdjacentWitnessSet,
  { kind: 'complete' }
> {
  if (!value || value.kind !== 'complete') return false
  const coverage = value.coverage
  return value.sourcePose === 'analyzed_input_pose'
    && value.requestIdentityBound === false
    && value.autoApplicable === false
    && value.witnessSamples.length === 0
    && coverage.authoritativePairScanComplete === true
    && coverage.allCollisionConstraintsRepresented === true
    && coverage.indeterminatePairCount === 0
    && coverage.touchingPairCount === 0
    && coverage.penetratingPairCount === 0
    && coverage.eligiblePairCount === 0
    && coverage.attemptedPairCount === 0
    && coverage.availablePairCount === 0
    && coverage.unavailablePairCount === 0
    && coverage.omittedByLimitCount === 0
    && coverage.expectedTrianglePairCount === coverage.trianglePairTests
    && coverage.trianglePairTests
      === coverage.aabbRejectedPairCount + coverage.satTests
    && coverage.satTests === coverage.satSeparatedPairCount
}

function staticClearAnalysis(
  result: FoldPreviewNarrowPhaseResult,
  fullScan: Extract<
    FoldPreviewFullScanNonAdjacentWitnessSet,
    { kind: 'complete' }
  >,
): FoldPreviewTreeSingleHingeStaticCorrectionAnalysis | null {
  let allowedHingeInteractionCount = 0
  for (const interaction of result.interactions) {
    if (
      interaction.relation !== 'hinge_adjacent'
      || interaction.geometryClass === 'indeterminate'
      || interaction.hingeDecision?.kind !== 'allowed_by_hinge_model'
    ) return null
    allowedHingeInteractionCount += 1
  }
  if (
    result.witnessSamples.length !== 0
    || result.witnessCoverage.eligiblePairCount !== 0
    || result.witnessCoverage.attemptedPairCount !== 0
    || result.witnessCoverage.unavailablePairCount !== 0
    || result.witnessCoverage.omittedByLimitCount !== 0
    || allowedHingeInteractionCount !== result.interactions.length
  ) return null
  const coverage = fullScan.coverage
  return deepFreeze({
    broadPhaseCandidateCount: result.broadPhaseCandidates,
    broadPhaseNonAdjacentCandidateCount:
      result.broadPhaseNonAdjacentCandidates,
    broadPhaseHingeAdjacentCandidateCount:
      result.broadPhaseHingeAdjacentCandidates,
    interactionCount: result.interactions.length,
    allowedHingeInteractionCount,
    trianglePairTests: result.trianglePairTests,
    satTests: result.satTests,
    numericalMargin: result.numericalMargin,
    fullScanBroadPhaseCandidateCount:
      coverage.broadPhaseCandidateCount,
    fullScanExpectedTrianglePairCount:
      coverage.expectedTrianglePairCount,
    fullScanTrianglePairTests: coverage.trianglePairTests,
    fullScanAabbRejectedPairCount:
      coverage.aabbRejectedPairCount,
    fullScanSatTests: coverage.satTests,
    fullScanSatSeparatedPairCount:
      coverage.satSeparatedPairCount,
  })
}

function sameAngles(
  first: readonly FoldPreviewHingeAngle[],
  second: readonly FoldPreviewHingeAngle[],
) {
  if (first.length !== second.length) return false
  for (let index = 0; index < first.length; index += 1) {
    if (
      first[index].edgeId !== second[index].edgeId
      || first[index].angleDegrees !== second[index].angleDegrees
    ) return false
  }
  return true
}

function sameIds(first: readonly string[], second: readonly string[]) {
  return first.length === second.length
    && first.every((value, index) => value === second[index])
}

function sameMatrix(first: readonly number[], second: readonly number[]) {
  return first.length === 16
    && second.length === 16
    && first.every((value, index) =>
      Number.isFinite(value) && value === second[index])
}

function equivalentPoint(first: Point, second: Point, jointFactor: number) {
  const scale = Math.max(
    1,
    Math.abs(first.x),
    Math.abs(first.y),
    Math.abs(first.z),
    Math.abs(second.x),
    Math.abs(second.y),
    Math.abs(second.z),
  )
  const tolerance = scale
    * Number.EPSILON
    * MATERIAL_POINT_EQUIVALENCE_FACTOR
    * jointFactor
  return Number.isFinite(tolerance)
    && Math.abs(first.x - second.x) <= tolerance
    && Math.abs(first.y - second.y) <= tolerance
    && Math.abs(first.z - second.z) <= tolerance
}

function copyAngles(angles: readonly FoldPreviewHingeAngle[]) {
  return angles.map((angle) => Object.freeze({
    edgeId: angle.edgeId,
    angleDegrees: angle.angleDegrees,
  }))
}

function copyPoint(value: Point): Point {
  return Object.freeze({
    x: canonicalZero(value.x),
    y: canonicalZero(value.y),
    z: canonicalZero(value.z),
  })
}

function freezePoint(x: number, y: number, z: number): Point {
  return Object.freeze({
    x: canonicalZero(x),
    y: canonicalZero(y),
    z: canonicalZero(z),
  })
}

function finiteVector(value: Vector3) {
  return Number.isFinite(value.x)
    && Number.isFinite(value.y)
    && Number.isFinite(value.z)
}

function validPositiveAngleDelta(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
    && value <= 180
}

function canonicalZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function compareText(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}

function deepFreeze<T>(value: T, seen = new WeakSet<object>()): T {
  if (typeof value !== 'object' || value === null) return value
  const object = value as object
  if (seen.has(object)) return value
  seen.add(object)
  for (const key of Reflect.ownKeys(object)) {
    deepFreeze(
      (object as Record<PropertyKey, unknown>)[key],
      seen,
    )
  }
  return Object.freeze(value)
}
