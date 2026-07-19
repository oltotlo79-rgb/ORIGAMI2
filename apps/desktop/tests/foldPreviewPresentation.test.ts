import assert from 'node:assert/strict'
import test from 'node:test'

import {
  describeFoldPreviewCorrectionAnalysis,
  describeFoldPreviewKeyboardAnnouncement,
  describeFoldPreviewRenderError,
  describeFoldPreviewStatus,
  describeFoldPreviewThickness,
  describeFoldPreviewTreeAngles,
  normalizeFoldPreviewKeyboardAnnouncement,
  type FoldPreviewRenderErrorCode,
} from '../src/lib/foldPreviewPresentation.ts'
import { MILLIMETRE_LENGTH_DISPLAY_UNIT } from '../src/lib/lengthUnit.ts'

test('render failures use a complete trusted bilingual code table', () => {
  const codes: readonly FoldPreviewRenderErrorCode[] = [
    'fixed_face_unavailable',
    'geometry_unavailable',
    'camera_unavailable',
    'render_unavailable',
    'tree_motion_unavailable',
    'tree_pose_application_failed',
    'tree_pose_render_failed',
    'scene_initialization_failed',
    'selection_render_failed',
  ]
  for (const code of codes) {
    assert.match(describeFoldPreviewRenderError(code, 'ja'), /[ぁ-んァ-ヶ一-龠]/u)
    assert.doesNotMatch(
      describeFoldPreviewRenderError(code, 'en'),
      /[ぁ-んァ-ヶ一-龠]/u,
    )
  }
})

test('topology statuses are retranslated from either locale and raw failures are hidden', () => {
  assert.equal(
    describeFoldPreviewStatus('4面・3ヒンジ', 'en'),
    '4 faces · 3 hinges',
  )
  assert.equal(
    describeFoldPreviewStatus('3D analysis blocked (7 issues)', 'ja'),
    '3D解析で遮断（7件）',
  )
  const failure = describeFoldPreviewStatus(
    '3D analysis error: native-secret-payload',
    'en',
  )
  assert.equal(failure, '3D analysis failed.')
  assert.doesNotMatch(failure, /native-secret-payload/u)

  const unknown = describeFoldPreviewStatus(
    'hostile unknown external message',
    'en',
  )
  assert.equal(unknown, 'Waiting for face and hinge analysis.')
  assert.doesNotMatch(unknown, /hostile/u)
})

test('paper-thickness and tree-angle summaries have English presentations', () => {
  assert.equal(
    describeFoldPreviewThickness({
      hasAuthoritativeThickness: true,
      thicknessIsEmphasised: false,
      thicknessIsLimited: false,
      formattedLength: '0.1 mm',
      lengthDisplayUnit: MILLIMETRE_LENGTH_DISPLAY_UNIT,
    }, 'en'),
    'Paper thickness 0.1 mm',
  )
  assert.equal(
    describeFoldPreviewTreeAngles([
      { edgeId: 'a', angleDegrees: 45 },
      { edgeId: 'b', angleDegrees: 90 },
    ], 0, 'en'),
    'Per hinge 45–90°',
  )
})

test('keyboard announcements retain only structured counts and retranslate live', () => {
  const announcement = normalizeFoldPreviewKeyboardAnnouncement(
    'ヒンジ 2/4 を選択しました',
  )
  assert.equal(
    describeFoldPreviewKeyboardAnnouncement(announcement, 'en'),
    'Selected hinge 2 of 4.',
  )
  assert.equal(
    describeFoldPreviewKeyboardAnnouncement(announcement, 'ja'),
    'ヒンジ 2/4 を選択しました',
  )

  const hostile = normalizeFoldPreviewKeyboardAnnouncement(
    'native-secret selection text',
  )
  const text = describeFoldPreviewKeyboardAnnouncement(hostile, 'en')
  assert.equal(text, 'The 3D selection changed.')
  assert.doesNotMatch(text, /native-secret/u)
})

test('correction-analysis state is derived again for the active locale', () => {
  const working = {
    version: 'tree_single_hinge_correction_analysis_coordinator_v1',
    generation: 2,
    status: 'working',
    phase: 'candidate_path_analysis',
  } as const
  const japanese = describeFoldPreviewCorrectionAnalysis(working, 'ja')
  const english = describeFoldPreviewCorrectionAnalysis(working, 'en')
  assert.equal(japanese.badgeText, '作業中・連続経路を確認中')
  assert.equal(english.badgeText, 'Working · Checking continuous paths')
  assert.match(english.accessibleText, /not applied automatically/u)
  assert.doesNotMatch(english.accessibleText, /[ぁ-んァ-ヶ一-龠]/u)
})

test('certified correction copy is rebuilt from numbers instead of stored text', () => {
  const stats = {
    intervalTests: 1,
    pointTests: 2,
    pointCacheHits: 0,
    maximumDepthReached: 1,
  }
  const state = {
    version: 'tree_single_hinge_correction_analysis_coordinator_v1',
    generation: 3,
    status: 'certified',
    presentation: {
      version: 'tree_single_hinge_static_candidate_path_presentation_v1',
      kind: 'certified_static_candidate_path_presentation',
      identity: {
        projectId: 'project',
        revision: 1,
        selectedHingeEdgeId: 'hinge',
      },
      candidate: { rank: 2 },
      angles: {
        sourceDegrees: 45,
        targetDegrees: 60,
        deltaDegrees: 15,
        absoluteDeltaDegrees: 15,
        direction: 'increasing',
      },
      continuous: {
        stats,
        aggregateStats: stats,
        precedingAttemptCount: 1,
      },
      staticInteractionSummary: {
        broadPhaseCandidateCount: 0,
        broadPhaseNonAdjacentCandidateCount: 0,
        broadPhaseHingeAdjacentCandidateCount: 0,
        interactionCount: 0,
        allowedHingeInteractionCount: 0,
        trianglePairTests: 0,
        satTests: 0,
        numericalMargin: 0,
        fullScanBroadPhaseCandidateCount: 0,
        fullScanExpectedTrianglePairCount: 0,
        fullScanTrianglePairTests: 0,
        fullScanAabbRejectedPairCount: 0,
        fullScanSatTests: 0,
        fullScanSatSeparatedPairCount: 0,
      },
      workBounds: {
        entireStepTimeBounded: false,
        synchronousFactoryPreparation: true,
        synchronousChildJobPreparation: true,
        synchronousResultFinalization: true,
        candidateCount: 2,
        maximumCumulativeIntervalTests: 1,
        maximumCumulativeIntervalPairVisits: 1,
        maximumCumulativePointTriangleTests: 1,
        terminalEvidenceFullScanEnabled: false,
      },
      badgeText: 'native-secret-badge',
      accessibleText: 'native-secret-accessibility',
      limitation: 'native-secret-limitation',
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
    },
  } as const

  const english = describeFoldPreviewCorrectionAnalysis(state, 'en')
  assert.match(
    english.badgeText,
    /Analysis-only correction candidate 2/u,
  )
  assert.match(english.accessibleText, /from 45 to 60 degrees/u)
  assert.doesNotMatch(
    `${english.badgeText} ${english.accessibleText}`,
    /native-secret/u,
  )
})
