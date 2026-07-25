import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  FOLD_PREVIEW_COMPONENT_TEXT as TEXT,
} from '../src/lib/foldPreviewComponentText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../src/lib/i18n.ts'

const EXPECTED_KEYS = [
  'preparingPose',
  'fixedFace',
  'fixedFaceNote',
  'vertexDistance',
  'faceNormalAngle',
  'measurementUnavailable',
  'selectTwoSameKind',
  'faceLabel',
  'fixedFaceSuffix',
  'paperDrag',
  'verticalDrag',
  'motionTargetBadge',
  'motionReadyBadge',
  'cutComponentsPlanarOnly',
  'cycleConstraintsPlanarOnly',
  'treePoseNote',
  'staticGraphPoseNote',
  'noteSeparator',
  'keyboardHingeAndFaceHint',
  'keyboardHingeHint',
  'keyboardFaceHint',
  'singleFoldOperationNoteWithDrag',
  'singleFoldOperationNote',
  'treeFoldOperationNote',
  'sentenceDetail',
  'motionViewDescription',
  'unverifiedTargetDescription',
  'singleFoldPreviewDescription',
  'treeFoldPreviewDescription',
  'staticGraphPreviewDescription',
  'planarPreviewDescription',
  'unavailablePreviewDescription',
  'selectionHingeAndFaceDescription',
  'selectionHingeDescription',
  'selectionFaceDescription',
  'keyboardSelectionDescription',
  'keyboardHingeSelectionDescription',
  'keyboardHingeSelected',
  'keyboardNoHingeSelected',
  'keyboardSelectionBetween',
  'keyboardFaceSelectionDescription',
  'keyboardFixedFaceSelected',
  'keyboardNoFixedFaceSelected',
  'singleFoldAngleDragDescription',
  'treeAngleDragDescription',
  'cameraDescription',
  'cameraMouseFoldExclusion',
  'cameraTouchFoldExclusion',
  'previewGroup',
  'measurementGroup',
  'measurementMode',
  'resetMeasurement',
  'view',
  'motionPathBadge',
  'correctionAnalysisBadge',
  'resetCamera',
  'resetView',
] as const

const EXPECTED_PLACEHOLDERS = new Map<string, readonly string[]>([
  ['fixedFace', ['index']],
  ['fixedFaceNote', ['label']],
  ['selectTwoSameKind', ['count']],
  ['faceLabel', ['index', 'fixedSuffix']],
  ['motionTargetBadge', ['action', 'target', 'displayed']],
  ['motionReadyBadge', ['action', 'displayed']],
  ['treePoseNote', ['faces', 'hinges', 'treeAngleNote', 'fixedFaceNote']],
  ['staticGraphPoseNote', ['faces', 'hinges', 'reason']],
  ['singleFoldOperationNoteWithDrag', ['basePreviewNote']],
  ['singleFoldOperationNote', ['basePreviewNote']],
  ['treeFoldOperationNote', ['basePreviewNote']],
  ['sentenceDetail', ['text']],
  ['motionViewDescription', ['text']],
  ['unverifiedTargetDescription', ['action', 'target']],
  [
    'singleFoldPreviewDescription',
    [
      'displayed',
      'requested',
      'unverifiedTarget',
      'fixedFaceNote',
      'motionView',
      'motionDetail',
      'collision',
      'thickness',
    ],
  ],
  [
    'treeFoldPreviewDescription',
    [
      'faces',
      'hinges',
      'treeAngleNote',
      'fixedFaceNote',
      'motionView',
      'motionDetail',
      'correctionAnalysis',
      'collision',
      'thickness',
    ],
  ],
  [
    'staticGraphPreviewDescription',
    ['faces', 'hinges', 'reason', 'collision', 'thickness'],
  ],
  ['planarPreviewDescription', ['collision', 'thickness']],
  ['unavailablePreviewDescription', ['message']],
  [
    'keyboardSelectionDescription',
    ['hingeDescription', 'between', 'faceDescription'],
  ],
  ['keyboardHingeSelectionDescription', ['selection']],
  ['keyboardHingeSelected', ['index', 'total']],
  ['keyboardFaceSelectionDescription', ['selection']],
  ['keyboardFixedFaceSelected', ['index', 'total']],
  ['cameraDescription', ['mouseExclusion', 'touchExclusion']],
  ['motionPathBadge', ['text']],
  ['correctionAnalysisBadge', ['text']],
])

const PLACEHOLDER = /\{([A-Za-z][A-Za-z0-9_]*)\}/gu

function placeholders(value: string): string[] {
  return [...value.matchAll(PLACEHOLDER)].map((match) => match[1])
}

function assertDeeplyFrozen(value: unknown): void {
  if (value === null || typeof value !== 'object') return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) assertDeeplyFrozen(child)
}

function keyboardDescription(locale: Locale): string {
  const hingeDescription = formatLocalizedText(
    locale,
    TEXT.keyboardHingeSelectionDescription,
    {
      selection: formatLocalizedText(
        locale,
        TEXT.keyboardHingeSelected,
        { index: 2, total: 4 },
      ),
    },
  )
  const faceDescription = formatLocalizedText(
    locale,
    TEXT.keyboardFaceSelectionDescription,
    {
      selection: formatLocalizedText(
        locale,
        TEXT.keyboardFixedFaceSelected,
        { index: 3, total: 5 },
      ),
    },
  )
  return formatLocalizedText(
    locale,
    TEXT.keyboardSelectionDescription,
    {
      hingeDescription,
      between: selectLocalizedText(locale, TEXT.keyboardSelectionBetween),
      faceDescription,
    },
  )
}

test('fold preview component catalog is exact, closed, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), EXPECTED_KEYS)
  for (const value of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(value), ['ja', 'en'])
    assert.equal(typeof value.ja, 'string')
    assert.equal(typeof value.en, 'string')
  }
  assertDeeplyFrozen(TEXT)
  assert.equal(
    createHash('sha256').update(JSON.stringify(TEXT), 'utf8').digest('hex'),
    'b1454da5d61eaa560fcdf76075d13f9b1490f39a4e355b98bcb99c429d6cc824',
  )

  assert.deepEqual(TEXT.noteSeparator, { ja: '・', en: ' · ' })
  assert.deepEqual(TEXT.fixedFaceSuffix, { ja: '（固定）', en: ' (fixed)' })
  assert.equal(
    TEXT.singleFoldAngleDragDescription.en.includes('paper’s rotation path'),
    true,
  )
})

test('fold preview component placeholders preserve their exact set and order', () => {
  const actualPlaceholderKeys: string[] = []
  for (const [key, value] of Object.entries(TEXT)) {
    const ja = placeholders(value.ja)
    const en = placeholders(value.en)
    assert.deepEqual(en, ja, `${key} must use the same ordered placeholders`)
    if (ja.length === 0) continue
    actualPlaceholderKeys.push(key)
    assert.deepEqual(ja, EXPECTED_PLACEHOLDERS.get(key), key)
  }
  assert.deepEqual(actualPlaceholderKeys, [...EXPECTED_PLACEHOLDERS.keys()])
})

test('fold preview component templates preserve formatted output', () => {
  assert.equal(
    formatLocalizedText('ja', TEXT.motionTargetBadge, {
      action: '紙面ドラッグ',
      target: '45',
      displayed: '30',
    }),
    '紙面ドラッグ目標 45°・表示 30° / 離すと検証',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.motionTargetBadge, {
      action: 'Paper drag',
      target: '45',
      displayed: '30',
    }),
    'Paper drag target 45° · displayed 30° / release to verify',
  )
  assert.equal(
    keyboardDescription('ja'),
    '。3Dビューにフォーカス中、Hで次、Shift+Hで前のヒンジを選択し、Escapeで解除できます。現在はヒンジ 2/4。Fで次、Shift+Fで前の面を固定面にできます。現在は固定面 3/5',
  )
  assert.equal(
    keyboardDescription('en'),
    ' With focus in the 3D view, press H for the next hinge, Shift+H for the previous hinge, or Escape to clear the selection. Current selection: hinge 2 of 4. press F for the next fixed face or Shift+F for the previous one. Current selection: fixed face 3 of 5.',
  )
})

test('FoldPreview consumes only the dedicated catalog for its fixed bilingual copy', async () => {
  const source = await readFile(
    new URL('../src/components/FoldPreview.tsx', import.meta.url),
    'utf8',
  )
  assert.match(source, /FOLD_PREVIEW_COMPONENT_TEXT as TEXT/u)
  assert.doesNotMatch(source, /\bfoldPreviewText\s*\(/u)
  assert.doesNotMatch(source, /[\u3040-\u30ff\u3400-\u9fff]/u)
  assert.doesNotMatch(
    source,
    /formatLocalizedText\s*\(\s*locale\s*,\s*\{/u,
  )
})
