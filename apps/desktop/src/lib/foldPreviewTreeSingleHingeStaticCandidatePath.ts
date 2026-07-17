import type {
  FoldPreviewContinuousMotionJob,
  FoldPreviewContinuousMotionStats,
} from './foldPreviewContinuousMotion.ts'
import type {
  FoldPreviewHingeAngle,
} from './foldPreviewKinematics.ts'
import {
  classifyFoldPreviewTreeMotionTarget,
  replaceFoldPreviewTreeMotionSelectedAngle,
  type FoldPreviewTreeMotionContext,
} from './foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from './foldPreviewTreeScenePose.ts'
import {
  prepareFoldPreviewTreeSingleHingeContinuousCollision,
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS,
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS,
  type FoldPreviewTreeSingleHingeContinuousBlocker,
  type FoldPreviewTreeSingleHingeContinuousOptions,
} from './foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext,
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES,
  type FoldPreviewTreeSingleHingeStaticCorrectionCandidate,
  type FoldPreviewTreeSingleHingeStaticCorrectionCandidates,
} from './foldPreviewTreeSingleHingeStaticCorrectionCandidates.ts'

export const FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION =
  'tree_single_hinge_static_candidate_path_v1'

const DEFAULT_MAX_INTERVAL_TESTS = 2_048
const DEFAULT_MAX_DEPTH = 24
const MAX_INTERVAL_TESTS = 1_000_000
const MAX_REASON_LENGTH = 512

export type FoldPreviewTreeSingleHingeStaticCandidatePathOptions =
  Readonly<{
    maxDepth?: number
    maxIntervalTests?: number
    minTimeSpan?: number
    maxIntervalPairVisits?: number
    maxPointTriangleTests?: number
  }>

type CandidateSnapshot = Readonly<{
  rank: number
  sourceSeedRank: number
  source: FoldPreviewTreeSingleHingeStaticCorrectionCandidate['source']
  poseRequestKey: string
  selectedAngleDegrees: number
  appliedAngles: readonly FoldPreviewHingeAngle[]
}>

type SourceIdentity = Readonly<{
  staticCandidateSetVersion:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidates['version']
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
  sourcePartition:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidates['sourcePartition']
}>

export type FoldPreviewTreeSingleHingeStaticCandidatePathWorkBounds =
  Readonly<{
    candidateCount: number
    maximumCumulativeIntervalTests: number
    maximumCumulativeIntervalPairVisits: number
    maximumCumulativePointTriangleTests: number
    terminalEvidenceFullScanEnabled: false
  }>

export type FoldPreviewTreeSingleHingeStaticCandidatePathAttempt =
  | Readonly<{
      kind: 'blocked'
      candidate: CandidateSnapshot
      certifiedSafeThrough: number
      stopTime: number
      unsafeBracket: readonly [number, number]
      blockingSampleTime: number
      blocker: Readonly<{
        firstFaceId: string
        secondFaceId: string
        relation: 'hinge_adjacent' | 'non_adjacent'
        geometryClass: 'touching' | 'penetrating' | 'indeterminate'
        hingeDecisionKind?: string
      }> | null
      stats: FoldPreviewContinuousMotionStats
      safety: UncertifiedSafety
    }>
  | Readonly<{
      kind: 'indeterminate'
      candidate: CandidateSnapshot
      certifiedSafeThrough: number
      stopTime: number
      unresolvedBracket: readonly [number, number]
      reason: string
      stats: FoldPreviewContinuousMotionStats
      safety: UncertifiedSafety
    }>

type UncertifiedSafety = Readonly<{
  continuousCandidatePathCertified: false
  sceneApplied: false
  autoApplicable: false
}>

type CertifiedSafety = Readonly<{
  modelIdentityBound: true
  sourcePoseIdentityVerified: true
  candidatePoseIdentityVerified: true
  partitionRevalidated: true
  completeLegalAngleVectorGenerated: true
  legalCorrectionPoseGenerated: true
  collisionConstraintsRevalidated: true
  hingeContactPolicySatisfied: true
  wholeSceneStaticClear: true
  staticCandidateRevalidated: true
  continuousCandidatePathCertified: true
  runtimeRequestBound: false
  startScenePoseMatched: false
  sceneApplied: false
  autoApplicable: false
}>

export type FoldPreviewTreeSingleHingeStaticCandidatePathCertificate =
  Readonly<{
    version:
      typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION
    kind: 'continuously_certified_static_candidate'
    sourceIdentity: SourceIdentity
    selectedCandidate: CandidateSnapshot
    path: Readonly<{
      interpolation: 'selected_hinge_linear_angle_v1'
      sourceSelectedAngleDegrees: number
      targetSelectedAngleDegrees: number
      sourceAngles: readonly FoldPreviewHingeAngle[]
      targetAngles: readonly FoldPreviewHingeAngle[]
      sourcePoseRequestKey: string
      targetPoseRequestKey: string
      certifiedSafeThrough: 1
      stopTime: 1
      stats: FoldPreviewContinuousMotionStats
    }>
    staticAnalysis:
      FoldPreviewTreeSingleHingeStaticCorrectionCandidate['staticAnalysis']
    precedingAttempts:
      readonly FoldPreviewTreeSingleHingeStaticCandidatePathAttempt[]
    aggregateStats: FoldPreviewContinuousMotionStats
    workBounds: FoldPreviewTreeSingleHingeStaticCandidatePathWorkBounds
    safety: CertifiedSafety
  }>

type CertificateProvenance = Readonly<{
  context: FoldPreviewTreeMotionContext
  sourcePoseRequestKey: string
  targetPoseRequestKey: string
  sourceAngles: readonly FoldPreviewHingeAngle[]
  targetAngles: readonly FoldPreviewHingeAngle[]
  candidateRank: number
}>

const staticCandidatePathCertificateProvenance = new WeakMap<
  object,
  CertificateProvenance
>()

/**
 * Confirms that this exact certificate was issued for this exact authentic
 * context by a terminal clear path job. Structural clones and equivalent
 * replacement contexts deliberately do not cross this provenance boundary.
 */
export function isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
  context: FoldPreviewTreeMotionContext,
  value: unknown,
): value is FoldPreviewTreeSingleHingeStaticCandidatePathCertificate {
  try {
    return typeof context === 'object'
      && context !== null
      && typeof value === 'object'
      && value !== null
      && staticCandidatePathCertificateProvenance.get(value)?.context
        === context
  } catch {
    return false
  }
}

export type FoldPreviewTreeSingleHingeStaticCandidatePathStep =
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION
      kind: 'pending'
      sourceIdentity: SourceIdentity
      completedAttempts:
        readonly FoldPreviewTreeSingleHingeStaticCandidatePathAttempt[]
      activeCandidate: CandidateSnapshot
      certifiedSafeThrough: number
      aggregateStats: FoldPreviewContinuousMotionStats
      workBounds: FoldPreviewTreeSingleHingeStaticCandidatePathWorkBounds
      safety: UncertifiedSafety
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION
      kind: 'certified'
      certificate: FoldPreviewTreeSingleHingeStaticCandidatePathCertificate
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION
      kind: 'exhausted'
      sourceIdentity: SourceIdentity
      attempts:
        readonly FoldPreviewTreeSingleHingeStaticCandidatePathAttempt[]
      aggregateStats: FoldPreviewContinuousMotionStats
      workBounds: FoldPreviewTreeSingleHingeStaticCandidatePathWorkBounds
      safety: UncertifiedSafety
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION
      kind: 'indeterminate'
      sourceIdentity: SourceIdentity
      reason:
        | 'invalid_work_budget'
        | 'candidate_job_error'
        | 'malformed_candidate_step'
        | 'work_accounting_error'
      completedAttempts:
        readonly FoldPreviewTreeSingleHingeStaticCandidatePathAttempt[]
      aggregateStats: FoldPreviewContinuousMotionStats
      workBounds: FoldPreviewTreeSingleHingeStaticCandidatePathWorkBounds
      safety: UncertifiedSafety
    }>
  | Readonly<{
      version:
        typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION
      kind: 'cancelled'
      sourceIdentity: SourceIdentity
      completedAttempts:
        readonly FoldPreviewTreeSingleHingeStaticCandidatePathAttempt[]
      aggregateStats: FoldPreviewContinuousMotionStats
      workBounds: FoldPreviewTreeSingleHingeStaticCandidatePathWorkBounds
      safety: UncertifiedSafety
    }>

export type FoldPreviewTreeSingleHingeStaticCandidatePathJob =
  Readonly<{
    step(workBudget: number): FoldPreviewTreeSingleHingeStaticCandidatePathStep
    cancel(): void
  }>

type PreparedCandidate = Readonly<{
  snapshot: CandidateSnapshot
  staticAnalysis:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidate['staticAnalysis']
}>

type ResolvedOptions = Readonly<{
  inner: FoldPreviewTreeSingleHingeContinuousOptions
  maxDepth: number
  maxIntervalTests: number
  maxIntervalPairVisits: number
  maxPointTriangleTests: number
}>

const UNCERTIFIED_SAFETY: UncertifiedSafety = Object.freeze({
  continuousCandidatePathCertified: false,
  sceneApplied: false,
  autoApplicable: false,
})

/**
 * Tries statically safe candidates in fit order and certifies the first
 * complete source-to-candidate selected-hinge path.
 *
 * The returned job is analysis-only. It never passes a runtime request
 * identity to the inner analyzer, applies no scene pose, and emits no project
 * command.
 */
export function createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
  context: FoldPreviewTreeMotionContext,
  staticCandidates: FoldPreviewTreeSingleHingeStaticCorrectionCandidates,
  options: FoldPreviewTreeSingleHingeStaticCandidatePathOptions = {},
): FoldPreviewTreeSingleHingeStaticCandidatePathJob | null {
  try {
    if (
      !isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
        context,
        staticCandidates,
      )
    ) return null
    const resolvedOptions = resolveOptions(options)
    if (!resolvedOptions) return null
    const sourceAngles = replaceFoldPreviewTreeMotionSelectedAngle(
      context,
      context.selectedAngleDegrees,
    )
    if (!sourceAngles || !sameAngles(sourceAngles, context.appliedAngles)) {
      return null
    }
    const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
      context.model,
      context.fixedFaceId,
      context.collisionThickness,
      sourceAngles,
    )
    if (!sourcePoseRequestKey) return null
    const sourceIdentity = verifySourceIdentity(
      context,
      staticCandidates,
      sourcePoseRequestKey,
    )
    if (!sourceIdentity) return null

    const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
      context.model,
      context.fixedFaceId,
      context.selectedHingeEdgeId,
    )
    if (
      !analyzer
      || analyzer.fixedFaceId !== context.fixedFaceId
      || analyzer.selectedHingeEdgeId !== context.selectedHingeEdgeId
      || !sameIds(
        analyzer.stationaryFaceIds,
        staticCandidates.sourcePartition.stationaryFaceIds,
      )
      || !sameIds(
        analyzer.movingFaceIds,
        staticCandidates.sourcePartition.movingFaceIds,
      )
    ) return null

    const preparedCandidates = verifyCandidates(
      context,
      staticCandidates,
    )
    if (!preparedCandidates) return null
    const workBounds = createWorkBounds(
      preparedCandidates.length,
      resolvedOptions,
    )
    if (!workBounds) return null

    const innerJobs: FoldPreviewContinuousMotionJob<
      FoldPreviewTreeSingleHingeContinuousBlocker
    >[] = []
    for (const candidate of preparedCandidates) {
      const innerJob = analyzer.createJob(
        sourceAngles,
        candidate.snapshot.selectedAngleDegrees,
        context.collisionThickness,
        resolvedOptions.inner,
      )
      if (!innerJob) {
        for (const existing of innerJobs) existing.cancel()
        return null
      }
      innerJobs.push(innerJob)
    }

    const sourceAngleSnapshot = copyAngles(sourceAngles)
    const attempts:
      FoldPreviewTreeSingleHingeStaticCandidatePathAttempt[] = []
    let activeIndex = 0
    let activeStats = zeroStats()
    let activeCertifiedSafeThrough = 0
    let cancelled = false
    let stepping = false
    let terminal: FoldPreviewTreeSingleHingeStaticCandidatePathStep | null =
      null

    const cancelAll = () => {
      for (const innerJob of innerJobs) {
        try {
          innerJob.cancel()
        } catch {
          // Cancellation remains best effort inside this analysis boundary.
        }
      }
    }
    const finish = (
      value: FoldPreviewTreeSingleHingeStaticCandidatePathStep,
    ) => {
      terminal = deepFreeze(value)
      return terminal
    }
    const terminalFailure = (
      reason: Extract<
        FoldPreviewTreeSingleHingeStaticCandidatePathStep,
        { kind: 'indeterminate' }
      >['reason'],
    ) => {
      cancelAll()
      return finish({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
        kind: 'indeterminate',
        sourceIdentity,
        reason,
        completedAttempts: [...attempts],
        aggregateStats: aggregateStats(attempts, activeStats),
        workBounds,
        safety: UNCERTIFIED_SAFETY,
      })
    }
    const terminalCancelled = () => finish({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
      kind: 'cancelled',
      sourceIdentity,
      completedAttempts: [...attempts],
      aggregateStats: aggregateStats(attempts, activeStats),
      workBounds,
      safety: UNCERTIFIED_SAFETY,
    })
    const pending = (
      certifiedSafeThrough: number,
    ): FoldPreviewTreeSingleHingeStaticCandidatePathStep => {
      const activeCandidate = preparedCandidates[activeIndex]
      if (!activeCandidate) {
        return terminalFailure('malformed_candidate_step')
      }
      return deepFreeze({
        version:
          FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
        kind: 'pending',
        sourceIdentity,
        completedAttempts: [...attempts],
        activeCandidate: activeCandidate.snapshot,
        certifiedSafeThrough,
        aggregateStats: aggregateStats(attempts, activeStats),
        workBounds,
        safety: UNCERTIFIED_SAFETY,
      })
    }

    return Object.freeze({
      step(
        workBudget: number,
      ): FoldPreviewTreeSingleHingeStaticCandidatePathStep {
        if (terminal) return terminal
        if (cancelled) return terminalCancelled()
        if (stepping) {
          cancelled = true
          cancelAll()
          return terminalCancelled()
        }
        if (!Number.isSafeInteger(workBudget) || workBudget <= 0) {
          return terminalFailure('invalid_work_budget')
        }
        const activeCandidate = preparedCandidates[activeIndex]
        const innerJob = innerJobs[activeIndex]
        if (!activeCandidate || !innerJob) {
          return terminalFailure('malformed_candidate_step')
        }

        stepping = true
        try {
          let rawStep: unknown
          try {
            rawStep = innerJob.step(workBudget)
          } catch {
            return terminalFailure('candidate_job_error')
          }
          if (terminal) return terminal
          if (cancelled) return terminalCancelled()
          const step = snapshotInnerStep(rawStep)
          if (!step) return terminalFailure('malformed_candidate_step')
          if (
            !statsMonotonic(activeStats, step.stats)
            || step.certifiedSafeThrough < activeCertifiedSafeThrough
            || step.stats.intervalTests - activeStats.intervalTests
              > workBudget
            || step.stats.intervalTests > resolvedOptions.maxIntervalTests
            || step.stats.maximumDepthReached > resolvedOptions.maxDepth
          ) {
            return terminalFailure('work_accounting_error')
          }
          activeStats = step.stats
          activeCertifiedSafeThrough = step.certifiedSafeThrough

          if (step.kind === 'pending') {
            return pending(step.certifiedSafeThrough)
          }
          if (step.kind === 'clear') {
            const aggregate = aggregateStats(attempts, step.stats)
            const certificate = deepFreeze({
              version:
                FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
              kind: 'continuously_certified_static_candidate',
              sourceIdentity,
              selectedCandidate: activeCandidate.snapshot,
              path: {
                interpolation: 'selected_hinge_linear_angle_v1',
                sourceSelectedAngleDegrees:
                  sourceIdentity.sourceSelectedAngleDegrees,
                targetSelectedAngleDegrees:
                  activeCandidate.snapshot.selectedAngleDegrees,
                sourceAngles: copyAngles(sourceAngleSnapshot),
                targetAngles: copyAngles(
                  activeCandidate.snapshot.appliedAngles,
                ),
                sourcePoseRequestKey:
                  sourceIdentity.sourcePoseRequestKey,
                targetPoseRequestKey:
                  activeCandidate.snapshot.poseRequestKey,
                certifiedSafeThrough: 1,
                stopTime: 1,
                stats: step.stats,
              },
              staticAnalysis: copyStaticAnalysis(
                activeCandidate.staticAnalysis,
              ),
              precedingAttempts: [...attempts],
              aggregateStats: aggregate,
              workBounds,
              safety: {
                modelIdentityBound: true,
                sourcePoseIdentityVerified: true,
                candidatePoseIdentityVerified: true,
                partitionRevalidated: true,
                completeLegalAngleVectorGenerated: true,
                legalCorrectionPoseGenerated: true,
                collisionConstraintsRevalidated: true,
                hingeContactPolicySatisfied: true,
                wholeSceneStaticClear: true,
                staticCandidateRevalidated: true,
                continuousCandidatePathCertified: true,
                runtimeRequestBound: false,
                startScenePoseMatched: false,
                sceneApplied: false,
                autoApplicable: false,
              },
            }) satisfies
              FoldPreviewTreeSingleHingeStaticCandidatePathCertificate
            staticCandidatePathCertificateProvenance.set(
              certificate,
              Object.freeze({
                context,
                sourcePoseRequestKey:
                  sourceIdentity.sourcePoseRequestKey,
                targetPoseRequestKey:
                  activeCandidate.snapshot.poseRequestKey,
                sourceAngles: copyAngles(sourceAngleSnapshot),
                targetAngles: copyAngles(
                  activeCandidate.snapshot.appliedAngles,
                ),
                candidateRank: activeCandidate.snapshot.rank,
              }),
            )
            cancelRemaining(innerJobs, activeIndex + 1)
            return finish({
              version:
                FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
              kind: 'certified',
              certificate,
            })
          }
          if (step.kind === 'cancelled') {
            cancelled = true
            cancelAll()
            return terminalCancelled()
          }
          const attempt = step.kind === 'blocked'
            ? createBlockedAttempt(activeCandidate.snapshot, step)
            : createIndeterminateAttempt(activeCandidate.snapshot, step)
          if (!attempt) {
            return terminalFailure('malformed_candidate_step')
          }
          attempts.push(attempt)
          activeIndex += 1
          activeStats = zeroStats()
          activeCertifiedSafeThrough = 0
          if (activeIndex >= preparedCandidates.length) {
            return finish({
              version:
                FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
              kind: 'exhausted',
              sourceIdentity,
              attempts: [...attempts],
              aggregateStats: aggregateStats(attempts, activeStats),
              workBounds,
              safety: UNCERTIFIED_SAFETY,
            })
          }
          return pending(0)
        } finally {
          stepping = false
        }
      },
      cancel() {
        if (terminal || cancelled) return
        cancelled = true
        cancelAll()
      },
    })
  } catch {
    return null
  }
}

function verifySourceIdentity(
  context: FoldPreviewTreeMotionContext,
  candidates: FoldPreviewTreeSingleHingeStaticCorrectionCandidates,
  sourcePoseRequestKey: string,
): SourceIdentity | null {
  const identity = candidates.sourceIdentity
  if (
    candidates.version !==
      'tree_single_hinge_static_correction_candidates_v1'
    || candidates.kind !==
      'statically_revalidated_single_hinge_correction_candidates'
    || identity.projectId !== context.model.projectId
    || identity.revision !== context.model.revision
    || identity.fixedFaceId !== context.fixedFaceId
    || identity.selectedHingeEdgeId !== context.selectedHingeEdgeId
    || identity.contextKey !== context.contextKey
    || identity.sourceSelectedAngleDegrees !== context.selectedAngleDegrees
    || identity.collisionThickness !== context.collisionThickness
    || identity.sourcePoseRequestKey !== sourcePoseRequestKey
    || !validStaticGroupSafety(candidates)
  ) return null
  const blockingAngles = replaceFoldPreviewTreeMotionSelectedAngle(
    context,
    identity.blockingSelectedAngleDegrees,
  )
  if (!blockingAngles) return null
  const blockingPoseRequestKey =
    createFoldPreviewTreeSceneCollisionPoseKey(
      context.model,
      context.fixedFaceId,
      context.collisionThickness,
      blockingAngles,
    )
  if (
    !blockingPoseRequestKey
    || blockingPoseRequestKey !== identity.blockingPoseRequestKey
  ) return null
  return deepFreeze({
    staticCandidateSetVersion: candidates.version,
    projectId: identity.projectId,
    revision: identity.revision,
    fixedFaceId: identity.fixedFaceId,
    selectedHingeEdgeId: identity.selectedHingeEdgeId,
    contextKey: identity.contextKey,
    sourcePoseRequestKey,
    blockingPoseRequestKey,
    generation: identity.generation,
    requestSequence: identity.requestSequence,
    blockingSampleTime: identity.blockingSampleTime,
    sourceSelectedAngleDegrees: identity.sourceSelectedAngleDegrees,
    blockingSelectedAngleDegrees: identity.blockingSelectedAngleDegrees,
    collisionThickness: identity.collisionThickness,
    sourcePartition: {
      version: candidates.sourcePartition.version,
      stationaryFaceIds: [...candidates.sourcePartition.stationaryFaceIds],
      movingFaceIds: [...candidates.sourcePartition.movingFaceIds],
    },
  })
}

function verifyCandidates(
  context: FoldPreviewTreeMotionContext,
  value: FoldPreviewTreeSingleHingeStaticCorrectionCandidates,
): readonly PreparedCandidate[] | null {
  if (
    value.candidates.length <= 0
    || value.candidates.length
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES
  ) return null
  const result: PreparedCandidate[] = []
  let previousSourceSeedRank = 0
  for (let index = 0; index < value.candidates.length; index += 1) {
    const candidate = value.candidates[index]
    if (
      candidate.rank !== index + 1
      || !Number.isSafeInteger(candidate.sourceSeedRank)
      || candidate.sourceSeedRank <= previousSourceSeedRank
      || !validStaticCandidateSafety(candidate)
    ) return null
    previousSourceSeedRank = candidate.sourceSeedRank
    const target = classifyFoldPreviewTreeMotionTarget(
      context,
      candidate.pose.appliedAngles,
    )
    if (
      target.kind !== 'selected_only'
      || target.targetSelectedAngleDegrees
        !== candidate.pose.selectedAngleDegrees
    ) return null
    const expectedAngles = replaceFoldPreviewTreeMotionSelectedAngle(
      context,
      candidate.pose.selectedAngleDegrees,
    )
    if (
      !expectedAngles
      || !sameAngles(expectedAngles, candidate.pose.appliedAngles)
    ) return null
    const poseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
      context.model,
      context.fixedFaceId,
      context.collisionThickness,
      expectedAngles,
    )
    if (
      !poseRequestKey
      || poseRequestKey !== candidate.pose.poseRequestKey
    ) return null
    result.push(deepFreeze({
      snapshot: {
        rank: candidate.rank,
        sourceSeedRank: candidate.sourceSeedRank,
        source: candidate.source,
        poseRequestKey,
        selectedAngleDegrees: candidate.pose.selectedAngleDegrees,
        appliedAngles: copyAngles(expectedAngles),
      },
      staticAnalysis: copyStaticAnalysis(candidate.staticAnalysis),
    }))
  }
  return Object.freeze(result)
}

function validStaticGroupSafety(
  value: FoldPreviewTreeSingleHingeStaticCorrectionCandidates,
) {
  const safety = value.safety
  return safety.modelIdentityBound === true
    && safety.sourcePoseIdentityVerified === true
    && safety.blockingPoseIdentityVerified === true
    && safety.partitionRevalidated === true
    && safety.completeLegalAngleVectorsGenerated === true
    && safety.legalCorrectionPoseGenerated === true
    && safety.collisionConstraintsRevalidated === true
    && safety.hingeContactPolicySatisfied === true
    && safety.wholeSceneStaticClear === true
    && safety.staticCandidateRevalidated === true
    && safety.continuousCandidatePathCertified === false
    && safety.sceneApplied === false
    && safety.autoApplicable === false
}

function validStaticCandidateSafety(
  value: FoldPreviewTreeSingleHingeStaticCorrectionCandidate,
) {
  const safety = value.safety
  return safety.modelIdentityBound === true
    && safety.completeLegalAngleVectorGenerated === true
    && safety.legalCorrectionPoseGenerated === true
    && safety.collisionConstraintsRevalidated === true
    && safety.hingeContactPolicySatisfied === true
    && safety.wholeSceneStaticClear === true
    && safety.staticCandidateRevalidated === true
    && safety.continuousCandidatePathCertified === false
    && safety.sceneApplied === false
    && safety.autoApplicable === false
}

function resolveOptions(
  value: FoldPreviewTreeSingleHingeStaticCandidatePathOptions,
): ResolvedOptions | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const source = value as Record<string, unknown>
  const maxDepth = source.maxDepth ?? DEFAULT_MAX_DEPTH
  const maxIntervalTests = source.maxIntervalTests
    ?? DEFAULT_MAX_INTERVAL_TESTS
  const minTimeSpan = source.minTimeSpan
  const maxIntervalPairVisits = source.maxIntervalPairVisits
    ?? MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS
  const maxPointTriangleTests = source.maxPointTriangleTests
    ?? MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS
  if (
    !Number.isSafeInteger(maxDepth)
    || (maxDepth as number) < 0
    || (maxDepth as number) > 52
    || !Number.isSafeInteger(maxIntervalTests)
    || (maxIntervalTests as number) <= 0
    || (maxIntervalTests as number) > MAX_INTERVAL_TESTS
    || (minTimeSpan !== undefined
      && (!Number.isFinite(minTimeSpan)
        || (minTimeSpan as number) <= 0
        || (minTimeSpan as number) > 1))
    || !Number.isSafeInteger(maxIntervalPairVisits)
    || (maxIntervalPairVisits as number) <= 0
    || (maxIntervalPairVisits as number)
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS
    || !Number.isSafeInteger(maxPointTriangleTests)
    || (maxPointTriangleTests as number) <= 0
    || (maxPointTriangleTests as number)
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS
  ) return null
  const inner: FoldPreviewTreeSingleHingeContinuousOptions = Object.freeze({
    maxDepth: maxDepth as number,
    maxIntervalTests: maxIntervalTests as number,
    ...(minTimeSpan === undefined
      ? {}
      : { minTimeSpan: minTimeSpan as number }),
    maxIntervalPairVisits: maxIntervalPairVisits as number,
    maxPointTriangleTests: maxPointTriangleTests as number,
  })
  return Object.freeze({
    inner,
    maxDepth: maxDepth as number,
    maxIntervalTests: maxIntervalTests as number,
    maxIntervalPairVisits: maxIntervalPairVisits as number,
    maxPointTriangleTests: maxPointTriangleTests as number,
  })
}

function createWorkBounds(
  candidateCount: number,
  options: ResolvedOptions,
): FoldPreviewTreeSingleHingeStaticCandidatePathWorkBounds | null {
  const maximumCumulativeIntervalTests = boundedProduct(
    candidateCount,
    options.maxIntervalTests,
  )
  const maximumCumulativeIntervalPairVisits = boundedProduct(
    candidateCount,
    options.maxIntervalPairVisits,
  )
  const maximumCumulativePointTriangleTests = boundedProduct(
    candidateCount,
    options.maxPointTriangleTests,
  )
  if (
    maximumCumulativeIntervalTests === null
    || maximumCumulativeIntervalPairVisits === null
    || maximumCumulativePointTriangleTests === null
  ) return null
  return Object.freeze({
    candidateCount,
    maximumCumulativeIntervalTests,
    maximumCumulativeIntervalPairVisits,
    maximumCumulativePointTriangleTests,
    terminalEvidenceFullScanEnabled: false,
  })
}

function snapshotInnerStep(value: unknown):
  | Readonly<{
      kind: 'pending'
      certifiedSafeThrough: number
      stats: FoldPreviewContinuousMotionStats
    }>
  | Readonly<{
      kind: 'clear'
      certifiedSafeThrough: 1
      stopTime: 1
      stats: FoldPreviewContinuousMotionStats
    }>
  | Readonly<{
      kind: 'blocked'
      certifiedSafeThrough: number
      stopTime: number
      unsafeBracket: readonly [number, number]
      blockingSampleTime: number
      blocker: FoldPreviewTreeSingleHingeContinuousBlocker | null
      stats: FoldPreviewContinuousMotionStats
    }>
  | Readonly<{
      kind: 'indeterminate'
      certifiedSafeThrough: number
      stopTime: number
      unresolvedBracket: readonly [number, number]
      reason: string
      stats: FoldPreviewContinuousMotionStats
    }>
  | Readonly<{
      kind: 'cancelled'
      certifiedSafeThrough: number
      stats: FoldPreviewContinuousMotionStats
    }>
  | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const source = value as Record<string, unknown>
  const kind = source.kind
  const stats = snapshotStats(source.stats)
  const certifiedSafeThrough = validUnitInterval(
    source.certifiedSafeThrough,
  )
  if (!stats || certifiedSafeThrough === null) return null
  if (kind === 'pending') {
    return Object.freeze({ kind, certifiedSafeThrough, stats })
  }
  if (
    kind === 'clear'
    && certifiedSafeThrough === 1
    && source.stopTime === 1
  ) {
    return Object.freeze({
      kind,
      certifiedSafeThrough: 1,
      stopTime: 1,
      stats,
    })
  }
  if (
    kind === 'blocked'
    && validUnitInterval(source.stopTime) !== null
    && source.stopTime === certifiedSafeThrough
  ) {
    const unsafeBracket = snapshotBracket(source.unsafeBracket)
    const blockingSampleTime = validUnitInterval(source.blockingSampleTime)
    const blocker = source.blocker === undefined
      ? null
      : snapshotBlocker(source.blocker)
    if (
      !unsafeBracket
      || blockingSampleTime === null
      || unsafeBracket[0] !== certifiedSafeThrough
      || unsafeBracket[1] !== blockingSampleTime
      || (source.blocker !== undefined && !blocker)
    ) return null
    return Object.freeze({
      kind,
      certifiedSafeThrough,
      stopTime: certifiedSafeThrough,
      unsafeBracket,
      blockingSampleTime,
      blocker,
      stats,
    })
  }
  if (
    kind === 'indeterminate'
    && validUnitInterval(source.stopTime) !== null
    && source.stopTime === certifiedSafeThrough
    && validReason(source.reason)
  ) {
    const unresolvedBracket = snapshotBracket(source.unresolvedBracket)
    if (
      !unresolvedBracket
      || unresolvedBracket[0] !== certifiedSafeThrough
    ) return null
    return Object.freeze({
      kind,
      certifiedSafeThrough,
      stopTime: certifiedSafeThrough,
      unresolvedBracket,
      reason: source.reason,
      stats,
    })
  }
  if (kind === 'cancelled') {
    return Object.freeze({ kind, certifiedSafeThrough, stats })
  }
  return null
}

function createBlockedAttempt(
  candidate: CandidateSnapshot,
  step: Extract<
    NonNullable<ReturnType<typeof snapshotInnerStep>>,
    { kind: 'blocked' }
  >,
): FoldPreviewTreeSingleHingeStaticCandidatePathAttempt {
  return deepFreeze({
    kind: 'blocked',
    candidate,
    certifiedSafeThrough: step.certifiedSafeThrough,
    stopTime: step.stopTime,
    unsafeBracket: [...step.unsafeBracket] as [number, number],
    blockingSampleTime: step.blockingSampleTime,
    blocker: step.blocker
      ? {
          firstFaceId: step.blocker.firstFaceId,
          secondFaceId: step.blocker.secondFaceId,
          relation: step.blocker.relation,
          geometryClass: step.blocker.geometryClass,
          ...(
            step.blocker.hingeDecisionKind === undefined
              ? {}
              : { hingeDecisionKind: step.blocker.hingeDecisionKind }
          ),
        }
      : null,
    stats: step.stats,
    safety: UNCERTIFIED_SAFETY,
  })
}

function createIndeterminateAttempt(
  candidate: CandidateSnapshot,
  step: Extract<
    NonNullable<ReturnType<typeof snapshotInnerStep>>,
    { kind: 'indeterminate' }
  >,
): FoldPreviewTreeSingleHingeStaticCandidatePathAttempt {
  return deepFreeze({
    kind: 'indeterminate',
    candidate,
    certifiedSafeThrough: step.certifiedSafeThrough,
    stopTime: step.stopTime,
    unresolvedBracket: [...step.unresolvedBracket] as [number, number],
    reason: step.reason,
    stats: step.stats,
    safety: UNCERTIFIED_SAFETY,
  })
}

function snapshotBlocker(
  value: unknown,
): FoldPreviewTreeSingleHingeContinuousBlocker | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const source = value as Record<string, unknown>
  const firstFaceId = source.firstFaceId
  const secondFaceId = source.secondFaceId
  const relation = source.relation
  const geometryClass = source.geometryClass
  const hingeDecisionKind = source.hingeDecisionKind
  if (
    !validText(firstFaceId)
    || !validText(secondFaceId)
    || (relation !== 'hinge_adjacent' && relation !== 'non_adjacent')
    || (
      geometryClass !== 'touching'
      && geometryClass !== 'penetrating'
      && geometryClass !== 'indeterminate'
    )
    || (
      hingeDecisionKind !== undefined
      && !validText(hingeDecisionKind)
    )
  ) return null
  return Object.freeze({
    firstFaceId,
    secondFaceId,
    relation,
    geometryClass,
    ...(hingeDecisionKind === undefined ? {} : { hingeDecisionKind }),
    blockingSample: null,
  })
}

function snapshotStats(value: unknown): FoldPreviewContinuousMotionStats | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const source = value as Record<string, unknown>
  const intervalTests = source.intervalTests
  const pointTests = source.pointTests
  const pointCacheHits = source.pointCacheHits
  const maximumDepthReached = source.maximumDepthReached
  if (
    !validCount(intervalTests)
    || !validCount(pointTests)
    || !validCount(pointCacheHits)
    || !validCount(maximumDepthReached)
  ) return null
  return Object.freeze({
    intervalTests,
    pointTests,
    pointCacheHits,
    maximumDepthReached,
  })
}

function aggregateStats(
  attempts: readonly FoldPreviewTreeSingleHingeStaticCandidatePathAttempt[],
  active: FoldPreviewContinuousMotionStats,
): FoldPreviewContinuousMotionStats {
  let intervalTests = active.intervalTests
  let pointTests = active.pointTests
  let pointCacheHits = active.pointCacheHits
  let maximumDepthReached = active.maximumDepthReached
  for (const attempt of attempts) {
    intervalTests = boundedSum(intervalTests, attempt.stats.intervalTests)
    pointTests = boundedSum(pointTests, attempt.stats.pointTests)
    pointCacheHits = boundedSum(
      pointCacheHits,
      attempt.stats.pointCacheHits,
    )
    maximumDepthReached = Math.max(
      maximumDepthReached,
      attempt.stats.maximumDepthReached,
    )
  }
  return Object.freeze({
    intervalTests,
    pointTests,
    pointCacheHits,
    maximumDepthReached,
  })
}

function statsMonotonic(
  previous: FoldPreviewContinuousMotionStats,
  next: FoldPreviewContinuousMotionStats,
) {
  return next.intervalTests >= previous.intervalTests
    && next.pointTests >= previous.pointTests
    && next.pointCacheHits >= previous.pointCacheHits
    && next.maximumDepthReached >= previous.maximumDepthReached
}

function zeroStats(): FoldPreviewContinuousMotionStats {
  return Object.freeze({
    intervalTests: 0,
    pointTests: 0,
    pointCacheHits: 0,
    maximumDepthReached: 0,
  })
}

function copyStaticAnalysis(
  value: FoldPreviewTreeSingleHingeStaticCorrectionCandidate['staticAnalysis'],
) {
  return Object.freeze({
    broadPhaseCandidateCount: value.broadPhaseCandidateCount,
    broadPhaseNonAdjacentCandidateCount:
      value.broadPhaseNonAdjacentCandidateCount,
    broadPhaseHingeAdjacentCandidateCount:
      value.broadPhaseHingeAdjacentCandidateCount,
    interactionCount: value.interactionCount,
    allowedHingeInteractionCount: value.allowedHingeInteractionCount,
    trianglePairTests: value.trianglePairTests,
    satTests: value.satTests,
    numericalMargin: value.numericalMargin,
    fullScanBroadPhaseCandidateCount:
      value.fullScanBroadPhaseCandidateCount,
    fullScanExpectedTrianglePairCount:
      value.fullScanExpectedTrianglePairCount,
    fullScanTrianglePairTests: value.fullScanTrianglePairTests,
    fullScanAabbRejectedPairCount:
      value.fullScanAabbRejectedPairCount,
    fullScanSatTests: value.fullScanSatTests,
    fullScanSatSeparatedPairCount:
      value.fullScanSatSeparatedPairCount,
  })
}

function copyAngles(angles: readonly FoldPreviewHingeAngle[]) {
  return Object.freeze(angles.map((angle) => Object.freeze({
    edgeId: angle.edgeId,
    angleDegrees: angle.angleDegrees,
  })))
}

function sameAngles(
  first: readonly FoldPreviewHingeAngle[],
  second: readonly FoldPreviewHingeAngle[],
) {
  return first.length === second.length
    && first.every((angle, index) =>
      angle.edgeId === second[index]?.edgeId
      && angle.angleDegrees === second[index]?.angleDegrees)
}

function sameIds(first: readonly string[], second: readonly string[]) {
  return first.length === second.length
    && first.every((value, index) => value === second[index])
}

function snapshotBracket(value: unknown): readonly [number, number] | null {
  if (!Array.isArray(value) || value.length !== 2) return null
  const first = validUnitInterval(value[0])
  const second = validUnitInterval(value[1])
  return first !== null && second !== null && first <= second
    ? Object.freeze([first, second]) as readonly [number, number]
    : null
}

function validUnitInterval(value: unknown): number | null {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 1
    ? value
    : null
}

function validReason(value: unknown): value is string {
  return validText(value)
}

function validText(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_REASON_LENGTH
}

function validCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
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

function boundedSum(first: number, second: number) {
  return Number.isSafeInteger(first)
    && first >= 0
    && Number.isSafeInteger(second)
    && second >= 0
    && second <= Number.MAX_SAFE_INTEGER - first
    ? first + second
    : Number.MAX_SAFE_INTEGER
}

function cancelRemaining(
  jobs: readonly Readonly<{ cancel(): void }>[],
  startIndex: number,
) {
  for (let index = startIndex; index < jobs.length; index += 1) {
    try {
      jobs[index]?.cancel()
    } catch {
      // A certified earlier candidate remains authoritative.
    }
  }
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
