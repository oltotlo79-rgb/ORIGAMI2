import {
  isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext,
  type FoldPreviewTreeSingleHingeStaticCandidatePathCertificate,
} from './foldPreviewTreeSingleHingeStaticCandidatePath.ts'
import type {
  FoldPreviewTreeMotionContext,
} from './foldPreviewTreeMotionContext.ts'

export const FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION =
  'tree_single_hinge_static_candidate_path_presentation_v1'

type ContinuousStats =
  FoldPreviewTreeSingleHingeStaticCandidatePathCertificate['path']['stats']

export type FoldPreviewTreeSingleHingeStaticCandidatePathPresentation =
  Readonly<{
    version:
      typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION
    kind: 'certified_static_candidate_path_presentation'
    identity: Readonly<{
      projectId: string
      revision: number
      selectedHingeEdgeId: string
    }>
    candidate: Readonly<{
      rank: number
    }>
    angles: Readonly<{
      sourceDegrees: number
      targetDegrees: number
      deltaDegrees: number
      absoluteDeltaDegrees: number
      direction: 'increasing' | 'decreasing'
    }>
    continuous: Readonly<{
      stats: ContinuousStats
      aggregateStats: ContinuousStats
      precedingAttemptCount: number
    }>
    staticInteractionSummary: Readonly<{
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
    workBounds: Readonly<{
      candidateCount: number
      maximumCumulativeIntervalTests: number
      maximumCumulativeIntervalPairVisits: number
      maximumCumulativePointTriangleTests: number
      terminalEvidenceFullScanEnabled: false
    }>
    badgeText: string
    accessibleText: string
    limitation: string
    safety: Readonly<{
      analysisOnly: true
      staticCandidateRevalidated: true
      continuousCandidatePathCertified: true
      runtimeRequestBound: false
      activeRequestLeaseBound: false
      startScenePoseMatched: false
      sceneApplied: false
      autoApplicable: false
    }>
  }>

/**
 * Produces a display-only summary from the exact path certificate issued for
 * the exact motion context. This boundary intentionally omits face identities,
 * angle vectors, pose keys, and every scene/runtime application capability.
 */
export function createFoldPreviewTreeSingleHingeStaticCandidatePathPresentation(
  context: FoldPreviewTreeMotionContext,
  value: unknown,
): FoldPreviewTreeSingleHingeStaticCandidatePathPresentation | null {
  try {
    if (
      !isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
        context,
        value,
      )
      || !hasPresentableCertificateInvariants(value)
    ) return null

    const sourceDegrees = value.path.sourceSelectedAngleDegrees
    const targetDegrees = value.path.targetSelectedAngleDegrees
    const deltaDegrees = targetDegrees - sourceDegrees
    if (!Number.isFinite(deltaDegrees) || deltaDegrees === 0) return null

    const direction = deltaDegrees > 0 ? 'increasing' : 'decreasing'
    const directionText = direction === 'increasing' ? '増加' : '減少'
    const sourceText = formatAngle(sourceDegrees)
    const targetText = formatAngle(targetDegrees)
    const deltaText = formatAngle(Math.abs(deltaDegrees))
    const limitation =
      '解析時点の結果で、現在も有効であることは保証されません。現在姿勢から安全に移動できることを示さず、この表示から3D表示や設計データへ適用できません。層順と材料変形も未確認です。'

    return deepFreeze({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_PRESENTATION_VERSION,
      kind: 'certified_static_candidate_path_presentation',
      identity: {
        projectId: value.sourceIdentity.projectId,
        revision: value.sourceIdentity.revision,
        selectedHingeEdgeId: value.sourceIdentity.selectedHingeEdgeId,
      },
      candidate: {
        rank: value.selectedCandidate.rank,
      },
      angles: {
        sourceDegrees,
        targetDegrees,
        deltaDegrees,
        absoluteDeltaDegrees: Math.abs(deltaDegrees),
        direction,
      },
      continuous: {
        stats: copyStats(value.path.stats),
        aggregateStats: copyStats(value.aggregateStats),
        precedingAttemptCount: value.precedingAttempts.length,
      },
      staticInteractionSummary: {
        broadPhaseCandidateCount:
          value.staticAnalysis.broadPhaseCandidateCount,
        broadPhaseNonAdjacentCandidateCount:
          value.staticAnalysis.broadPhaseNonAdjacentCandidateCount,
        broadPhaseHingeAdjacentCandidateCount:
          value.staticAnalysis.broadPhaseHingeAdjacentCandidateCount,
        interactionCount: value.staticAnalysis.interactionCount,
        allowedHingeInteractionCount:
          value.staticAnalysis.allowedHingeInteractionCount,
        trianglePairTests: value.staticAnalysis.trianglePairTests,
        satTests: value.staticAnalysis.satTests,
        numericalMargin: value.staticAnalysis.numericalMargin,
        fullScanBroadPhaseCandidateCount:
          value.staticAnalysis.fullScanBroadPhaseCandidateCount,
        fullScanExpectedTrianglePairCount:
          value.staticAnalysis.fullScanExpectedTrianglePairCount,
        fullScanTrianglePairTests:
          value.staticAnalysis.fullScanTrianglePairTests,
        fullScanAabbRejectedPairCount:
          value.staticAnalysis.fullScanAabbRejectedPairCount,
        fullScanSatTests: value.staticAnalysis.fullScanSatTests,
        fullScanSatSeparatedPairCount:
          value.staticAnalysis.fullScanSatSeparatedPairCount,
      },
      workBounds: {
        candidateCount: value.workBounds.candidateCount,
        maximumCumulativeIntervalTests:
          value.workBounds.maximumCumulativeIntervalTests,
        maximumCumulativeIntervalPairVisits:
          value.workBounds.maximumCumulativeIntervalPairVisits,
        maximumCumulativePointTriangleTests:
          value.workBounds.maximumCumulativePointTriangleTests,
        terminalEvidenceFullScanEnabled: false,
      },
      badgeText:
        `解析上の補正候補${value.selectedCandidate.rank}・`
        + `静的／連続経路確認済み（現在姿勢未照合）・`
        + `${sourceText}° → ${targetText}°`,
      accessibleText:
        `補正候補${value.selectedCandidate.rank}。選択した折り目を`
        + `${sourceText}度から${targetText}度へ${deltaText}度${directionText}`
        + `する単一ヒンジ経路は、静的衝突検査と連続経路検査を通過しました。`
        + limitation,
      limitation,
      safety: {
        analysisOnly: true,
        staticCandidateRevalidated: true,
        continuousCandidatePathCertified: true,
        runtimeRequestBound: false,
        activeRequestLeaseBound: false,
        startScenePoseMatched: false,
        sceneApplied: false,
        autoApplicable: false,
      },
    })
  } catch {
    return null
  }
}

function hasPresentableCertificateInvariants(
  certificate: FoldPreviewTreeSingleHingeStaticCandidatePathCertificate,
) {
  const { safety } = certificate
  return certificate.kind === 'continuously_certified_static_candidate'
    && certificate.path.certifiedSafeThrough === 1
    && certificate.path.stopTime === 1
    && validIdentity(certificate)
    && validCandidate(certificate)
    && validFinite(certificate.path.sourceSelectedAngleDegrees)
    && validFinite(certificate.path.targetSelectedAngleDegrees)
    && validStats(certificate.path.stats)
    && validStats(certificate.aggregateStats)
    && validStaticSummary(certificate)
    && validWorkBounds(certificate)
    && safety.modelIdentityBound === true
    && safety.sourcePoseIdentityVerified === true
    && safety.candidatePoseIdentityVerified === true
    && safety.partitionRevalidated === true
    && safety.completeLegalAngleVectorGenerated === true
    && safety.legalCorrectionPoseGenerated === true
    && safety.collisionConstraintsRevalidated === true
    && safety.hingeContactPolicySatisfied === true
    && safety.wholeSceneStaticClear === true
    && safety.staticCandidateRevalidated === true
    && safety.continuousCandidatePathCertified === true
    && safety.runtimeRequestBound === false
    && safety.startScenePoseMatched === false
    && safety.sceneApplied === false
    && safety.autoApplicable === false
}

function validIdentity(
  certificate: FoldPreviewTreeSingleHingeStaticCandidatePathCertificate,
) {
  const identity = certificate.sourceIdentity
  return typeof identity.projectId === 'string'
    && identity.projectId.length > 0
    && Number.isSafeInteger(identity.revision)
    && identity.revision >= 0
    && typeof identity.selectedHingeEdgeId === 'string'
    && identity.selectedHingeEdgeId.length > 0
}

function validCandidate(
  certificate: FoldPreviewTreeSingleHingeStaticCandidatePathCertificate,
) {
  const candidate = certificate.selectedCandidate
  return validPositiveInteger(candidate.rank)
    && validPositiveInteger(candidate.sourceSeedRank)
    && typeof candidate.source === 'string'
}

function validStats(stats: ContinuousStats) {
  return validCount(stats.intervalTests)
    && validCount(stats.pointTests)
    && validCount(stats.pointCacheHits)
    && validCount(stats.maximumDepthReached)
}

function validStaticSummary(
  certificate: FoldPreviewTreeSingleHingeStaticCandidatePathCertificate,
) {
  const summary = certificate.staticAnalysis
  return validCount(summary.broadPhaseCandidateCount)
    && validCount(summary.broadPhaseNonAdjacentCandidateCount)
    && validCount(summary.broadPhaseHingeAdjacentCandidateCount)
    && validCount(summary.interactionCount)
    && validCount(summary.allowedHingeInteractionCount)
    && validCount(summary.trianglePairTests)
    && validCount(summary.satTests)
    && validFinite(summary.numericalMargin)
    && summary.numericalMargin >= 0
    && validCount(summary.fullScanBroadPhaseCandidateCount)
    && validCount(summary.fullScanExpectedTrianglePairCount)
    && validCount(summary.fullScanTrianglePairTests)
    && validCount(summary.fullScanAabbRejectedPairCount)
    && validCount(summary.fullScanSatTests)
    && validCount(summary.fullScanSatSeparatedPairCount)
}

function validWorkBounds(
  certificate: FoldPreviewTreeSingleHingeStaticCandidatePathCertificate,
) {
  const bounds = certificate.workBounds
  return validPositiveInteger(bounds.candidateCount)
    && validPositiveInteger(bounds.maximumCumulativeIntervalTests)
    && validPositiveInteger(bounds.maximumCumulativeIntervalPairVisits)
    && validPositiveInteger(bounds.maximumCumulativePointTriangleTests)
    && bounds.terminalEvidenceFullScanEnabled === false
}

function copyStats(stats: ContinuousStats): ContinuousStats {
  return {
    intervalTests: stats.intervalTests,
    pointTests: stats.pointTests,
    pointCacheHits: stats.pointCacheHits,
    maximumDepthReached: stats.maximumDepthReached,
  }
}

function validFinite(value: number) {
  return Number.isFinite(value)
}

function validCount(value: number) {
  return Number.isSafeInteger(value) && value >= 0
}

function validPositiveInteger(value: number) {
  return Number.isSafeInteger(value) && value > 0
}

function formatAngle(value: number) {
  const rounded = Math.round(value * 1_000) / 1_000
  return Object.is(rounded, -0) ? '0' : String(rounded)
}

function deepFreeze<T>(value: T): T {
  if (typeof value !== 'object' || value === null || Object.isFrozen(value)) {
    return value
  }
  for (const key of Reflect.ownKeys(value)) {
    deepFreeze((value as Record<PropertyKey, unknown>)[key])
  }
  return Object.freeze(value)
}
