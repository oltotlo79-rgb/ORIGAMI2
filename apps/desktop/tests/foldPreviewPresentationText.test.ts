import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  describeFoldPreviewStatus,
  formatFoldPreviewAngle,
} from '../src/lib/foldPreviewPresentation.ts'
import {
  FOLD_PREVIEW_PRESENTATION_INPUT,
  FOLD_PREVIEW_PRESENTATION_TEXT,
  localizeFoldPreviewPaperEdgeRatioLength,
  selectFoldPreviewNumberLocale,
} from '../src/lib/foldPreviewPresentationText.ts'
import type { Locale } from '../src/lib/i18n.ts'

function assertDeepFrozen(value: unknown, seen = new Set<object>()): void {
  if (!value || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.equal(Object.isFrozen(value), true)
  for (const nested of Object.values(value)) {
    assertDeepFrozen(nested, seen)
  }
}

const TEXT_KEYS = [
  'trustedStatuses',
  'renderErrors',
  'statusFaceCount',
  'statusFaceSingular',
  'statusFacePlural',
  'statusHingeSingular',
  'statusHingePlural',
  'statusBlockedCount',
  'statusIssueSingular',
  'statusIssuePlural',
  'statusAnalysisErrorPrefix',
  'statusAnalysisFailed',
  'statusWaiting',
  'thicknessInvalid',
  'thicknessEmphasised',
  'thicknessLimited',
  'thicknessNormal',
  'correctionIdleBadge',
  'correctionIdleAccessible',
  'correctionWorkingBadge',
  'correctionWorkingAccessible',
  'correctionWorkingLive',
  'correctionStaleBadge',
  'correctionStaleAccessible',
  'correctionNoCandidateBadge',
  'correctionNoCandidateAccessible',
  'correctionNoCandidateLive',
  'correctionIndeterminateBadge',
  'correctionIndeterminateAccessible',
  'correctionIndeterminateLive',
  'correctionPhases',
  'correctionInvalidCertifiedAccessible',
  'correctionInvalidCertifiedLive',
  'correctionDirections',
  'correctionCertifiedLimitation',
  'correctionCertifiedBadge',
  'correctionCertifiedAccessible',
  'treeAnglesUniform',
  'treeAnglesPerHinge',
  'treeAnglesAllHinges',
  'treeAnglesRange',
  'keyboardHingeSelected',
  'keyboardFixedFaceSelected',
  'keyboardHingeCleared',
  'keyboardSelectionChanged',
  'numberLocale',
  'paperEdgeRatioLabel',
] as const

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{([a-z]+)\}/gu)]
    .map((match) => match[1]!)
    .sort()
}

function assertLocalizedLeaves(value: unknown): number {
  if (Array.isArray(value)) {
    return value.reduce(
      (count, item) => count + assertLocalizedLeaves(item),
      0,
    )
  }
  assert.ok(value && typeof value === 'object')
  const record = value as Record<string, unknown>
  const keys = Object.keys(record)
  if (keys.length === 2 && keys[0] === 'ja' && keys[1] === 'en') {
    assert.equal(typeof record.ja, 'string')
    assert.equal(typeof record.en, 'string')
    assert.notEqual(record.ja, '')
    assert.notEqual(record.en, '')
    assert.deepEqual(
      placeholders(record.ja as string),
      placeholders(record.en as string),
    )
    return 1
  }
  return Object.values(record).reduce(
    (count, item) => count + assertLocalizedLeaves(item),
    0,
  )
}

test('fold-preview presentation catalogs are exact, complete, and deeply frozen', () => {
  assertDeepFrozen(FOLD_PREVIEW_PRESENTATION_TEXT)
  assertDeepFrozen(FOLD_PREVIEW_PRESENTATION_INPUT)
  assert.deepEqual(
    Object.keys(FOLD_PREVIEW_PRESENTATION_TEXT),
    TEXT_KEYS,
  )
  assert.equal(FOLD_PREVIEW_PRESENTATION_TEXT.trustedStatuses.length, 4)
  assert.deepEqual(
    Object.keys(FOLD_PREVIEW_PRESENTATION_TEXT.renderErrors),
    [
      'fixed_face_unavailable',
      'geometry_unavailable',
      'camera_unavailable',
      'render_unavailable',
      'tree_motion_unavailable',
      'tree_pose_application_failed',
      'tree_pose_render_failed',
      'scene_initialization_failed',
      'selection_render_failed',
    ],
  )
  assert.deepEqual(
    Object.keys(FOLD_PREVIEW_PRESENTATION_TEXT.correctionPhases),
    [
      'preparing',
      'static_candidate_preparation',
      'static_candidate_analysis',
      'candidate_path_preparation',
      'candidate_path_analysis',
    ],
  )
  assert.deepEqual(
    Object.keys(FOLD_PREVIEW_PRESENTATION_TEXT.correctionDirections),
    ['increasing', 'decreasing'],
  )
  assert.equal(assertLocalizedLeaves(FOLD_PREVIEW_PRESENTATION_TEXT), 63)
  assert.equal(
    createHash('sha256')
      .update(JSON.stringify(FOLD_PREVIEW_PRESENTATION_TEXT), 'utf8')
      .digest('hex'),
    '94465d7391d74b5bde2b2fab25bf6d4e506b856b12ac907e5b44def07a99e420',
  )
  assert.deepEqual(
    Object.keys(FOLD_PREVIEW_PRESENTATION_INPUT),
    [
      'statusFaceCountPatterns',
      'statusBlockedCountPatterns',
      'keyboardHingeSelectedPattern',
      'keyboardFixedFaceSelectedPattern',
      'keyboardHingeCleared',
    ],
  )
  for (const patterns of [
    FOLD_PREVIEW_PRESENTATION_INPUT.statusFaceCountPatterns,
    FOLD_PREVIEW_PRESENTATION_INPUT.statusBlockedCountPatterns,
  ]) {
    assert.deepEqual(Object.keys(patterns), ['ja', 'en'])
    assert.equal(patterns.ja.flags, 'u')
    assert.equal(patterns.en.flags, 'u')
  }
})

test('unknown locales consistently fall back to Japanese presentation data', () => {
  const unsupported = 'fr' as Locale
  assert.equal(
    describeFoldPreviewStatus('4 faces · 3 hinges', unsupported),
    '4面・3ヒンジ',
  )
  assert.equal(selectFoldPreviewNumberLocale(unsupported), 'ja-JP')
  assert.equal(
    localizeFoldPreviewPaperEdgeRatioLength(
      '0.5 紙辺比',
      '紙辺比',
      unsupported,
    ),
    '0.5 紙辺比',
  )
})

test('number and paper-edge-ratio presentation preserve locale and precision', () => {
  assert.equal(formatFoldPreviewAngle(1234.56, 'ja'), '1,234.6')
  assert.equal(formatFoldPreviewAngle(1234.56, 'en'), '1,234.6')
  assert.equal(
    localizeFoldPreviewPaperEdgeRatioLength(
      '0.5 紙辺比',
      '紙辺比',
      'en',
    ),
    '0.5 paper-edge ratio',
  )
  assert.equal(
    localizeFoldPreviewPaperEdgeRatioLength(
      '紙辺比 0.5',
      '紙辺比',
      'en',
    ),
    '紙辺比 0.5',
  )
  assert.equal(
    localizeFoldPreviewPaperEdgeRatioLength(
      '0.5 紙辺比',
      'mm',
      'en',
    ),
    '0.5 紙辺比',
  )
})

test('presentation logic consumes fixed copy and locale data only from its catalog', async () => {
  const source = await readFile(
    new URL('../src/lib/foldPreviewPresentation.ts', import.meta.url),
    'utf8',
  )
  assert.match(source, /FOLD_PREVIEW_PRESENTATION_TEXT as TEXT/u)
  assert.match(source, /FOLD_PREVIEW_PRESENTATION_INPUT as INPUT/u)
  assert.doesNotMatch(source, /\bfoldPreviewText\s*\(/u)
  assert.doesNotMatch(source, /\blocale\s*===\s*['"](?:ja|en)['"]/u)
  assert.doesNotMatch(source, /\.toLocaleString\s*\(/u)
  assert.doesNotMatch(source, /[\u3040-\u30ff\u3400-\u9fff]/u)
})
