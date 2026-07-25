import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  FOLD_IMPORT_DIALOG_TEXT as TEXT,
  formatFoldImportAssignmentLabel,
  formatFoldImportBoundaryAssigned,
  formatFoldImportBoundaryEdgeCount,
  formatFoldImportConvertedScale,
  formatFoldImportGeometry,
  formatFoldImportLineCount,
  formatFoldImportUnresolvedAssignments,
} from '../src/lib/foldImportDialogText.ts'

const EXPECTED_KEYS = [
  'eyebrow',
  'title',
  'close',
  'closeGlyph',
  'description',
  'preview',
  'previewUnavailable',
  'previewTruncated',
  'metadata',
  'unspecified',
  'geometryValue',
  'boundaryEdgeCount',
  'boundaryEdgeCountOne',
  'lineCount',
  'lineCountOne',
  'assignmentCountSeparator',
  'listSeparator',
  'millimetresUnit',
  'name',
  'invalidName',
  'scale',
  'missingScale',
  'sourceUnit',
  'convertedScaleValue',
  'mappingTitle',
  'mappingDescription',
  'boundaryTitle',
  'boundaryDescription',
  'boundaryAssignedValue',
  'boundarySelect',
  'boundaryUnavailable',
  'boundaryFixed',
  'assignmentLabel',
  'select',
  'unresolvedValue',
  'warningTitle',
  'acknowledge',
  'cancel',
  'importing',
  'import',
]

const EXPECTED_TEXT = [
  ['eyebrow', 'FOLD 1.0–1.2 取込', 'Import FOLD 1.0–1.2'],
  ['title', '線種と縮尺を確認', 'Review line types and scale'],
  ['close', '閉じる', 'Close'],
  ['closeGlyph', '×', '×'],
  [
    'description',
    '元のFOLDファイルは変更しません。確認後、編集可能な未保存プロジェクトとして取り込みます。',
    'The source FOLD file is not modified. After review, it is imported as an editable unsaved project.',
  ],
  ['preview', '取り込む展開図のプレビュー', 'Preview of the crease pattern to import'],
  ['previewUnavailable', 'プレビューを表示できません。', 'The preview is unavailable.'],
  [
    'previewTruncated',
    '表示用に一部の線だけを描画しています。',
    'Only a subset of lines is drawn in this preview.',
  ],
  ['unspecified', '記載なし', 'Not specified'],
  ['geometryValue', '{vertices}頂点・{edges}辺', '{vertices} vertices · {edges} edges'],
  ['boundaryEdgeCount', '{count}辺', '{count} edges'],
  ['boundaryEdgeCountOne', '{count}辺', '{count} edge'],
  ['lineCount', '{count}本', '{count} lines'],
  ['lineCountOne', '{count}本', '{count} line'],
  ['assignmentCountSeparator', ' ', ' '],
  ['listSeparator', '、', ', '],
  ['millimetresUnit', 'mm', 'mm'],
  ['name', '作品名', 'Work name'],
  [
    'invalidName',
    '制御文字を含まない120文字以内の名前が必要です。',
    'Enter a name of at most 120 characters without control characters.',
  ],
  ['scale', '1 FOLD単位の長さ', 'Length of 1 FOLD unit'],
  [
    'missingScale',
    '単位情報がないため、実寸への換算値を指定してください。',
    'No unit metadata is available. Enter a conversion to real-world size.',
  ],
  ['sourceUnit', '元の単位', 'source unit'],
  [
    'convertedScaleValue',
    '{sourceUnit}から換算した値です。必要なら変更できます。',
    '{sourceUnit} conversion. Change it if needed.',
  ],
  ['mappingTitle', '線種の割当', 'Line type mapping'],
  [
    'mappingDescription',
    'F・U・JはORIGAMI2に同じ意味の線種がないため、用途を明示的に選んでください。',
    'F, U, and J have no directly equivalent ORIGAMI2 line type. Explicitly choose how to use them.',
  ],
  ['boundaryTitle', '用紙外周', 'Paper boundary'],
  [
    'boundaryDescription',
    '検証済み候補から、この作品で使う一枚紙の外周を明示してください。候補外のB線は取り込みません。',
    'Explicitly select the validated outline of the single sheet. Source B lines outside the selected candidate are not imported.',
  ],
  [
    'boundaryAssignedValue',
    '元のB線が単一の有効な外周を構成しています。 {candidate}',
    'The source B lines form one valid paper boundary. {candidate}',
  ],
  ['boundarySelect', '外周候補を選択してください', 'Select a boundary candidate'],
  [
    'boundaryUnavailable',
    '安全に使える外周候補がありません。このファイルは取り込めません。',
    'No boundary candidate can be used safely. This file cannot be imported.',
  ],
  ['boundaryFixed', '用紙境界（固定）', 'Paper boundary (fixed)'],
  ['assignmentLabel', '{assignment}の割当', '{assignment} mapping'],
  ['select', '選択してください', 'Select a mapping'],
  ['unresolvedValue', '未選択: {assignments}', 'Not selected: {assignments}'],
  ['warningTitle', '取り込まれない情報', 'Information that will not be imported'],
  [
    'acknowledge',
    '上記を確認し、展開図として取り込む',
    'I have reviewed the above and want to import the crease pattern',
  ],
  ['cancel', 'キャンセル', 'Cancel'],
  ['importing', '取込中…', 'Importing…'],
  ['import', '取り込む', 'Import'],
] as const

const EXPECTED_METADATA = [
  ['file', 'ファイル', 'File'],
  ['specification', '仕様', 'Specification'],
  ['unit', '単位', 'Unit'],
  ['geometry', '形状', 'Geometry'],
  ['boundary', '境界', 'Boundary'],
] as const

test('FOLD import dialog catalog preserves locale shape and exact copy', () => {
  assert.deepEqual(Object.keys(TEXT), ['ja', 'en'])
  assert.deepEqual(Object.keys(TEXT.ja), EXPECTED_KEYS)
  assert.deepEqual(Object.keys(TEXT.en), EXPECTED_KEYS)
  assert.deepEqual(
    EXPECTED_TEXT.map(([key]) => [key, TEXT.ja[key], TEXT.en[key]]),
    EXPECTED_TEXT,
  )
  assert.deepEqual(
    Object.keys(TEXT.ja.metadata),
    EXPECTED_METADATA.map(([key]) => key),
  )
  assert.deepEqual(Object.keys(TEXT.en.metadata), Object.keys(TEXT.ja.metadata))
  assert.deepEqual(
    EXPECTED_METADATA.map(([key]) => [
      key,
      TEXT.ja.metadata[key],
      TEXT.en.metadata[key],
    ]),
    EXPECTED_METADATA,
  )
})

test('FOLD import dialog catalog is deeply frozen with matching placeholders', () => {
  assertDeeplyFrozen(TEXT)
  for (const key of EXPECTED_KEYS) {
    const ja = TEXT.ja[key]
    const en = TEXT.en[key]
    if (typeof ja === 'string' && typeof en === 'string') {
      assert.deepEqual(placeholders(ja), placeholders(en), key)
      continue
    }
    assert.deepEqual(Object.keys(ja), Object.keys(en), key)
    for (const nestedKey of Object.keys(ja)) {
      assert.deepEqual(
        placeholders(ja[nestedKey as keyof typeof ja]),
        placeholders(en[nestedKey as keyof typeof en]),
        `${key}.${nestedKey}`,
      )
    }
  }

  assert.deepEqual(
    EXPECTED_KEYS.flatMap((key) => {
      const value = TEXT.ja[key]
      return typeof value === 'string' && placeholders(value).length > 0
        ? [[key, placeholders(value)]]
        : []
    }),
    [
      ['geometryValue', ['vertices', 'edges']],
      ['boundaryEdgeCount', ['count']],
      ['boundaryEdgeCountOne', ['count']],
      ['lineCount', ['count']],
      ['lineCountOne', ['count']],
      ['convertedScaleValue', ['sourceUnit']],
      ['boundaryAssignedValue', ['candidate']],
      ['assignmentLabel', ['assignment']],
      ['unresolvedValue', ['assignments']],
    ],
  )
})

test('FOLD import dialog formatters preserve locale, singulars, and fallback copy', () => {
  assert.equal(formatFoldImportGeometry(12_345, 67_890, 'ja'), '12,345頂点・67,890辺')
  assert.equal(
    formatFoldImportGeometry(12_345, 67_890, 'en'),
    '12,345 vertices · 67,890 edges',
  )
  assert.equal(formatFoldImportBoundaryEdgeCount(1, 'en'), '1 edge')
  assert.equal(formatFoldImportBoundaryEdgeCount(2, 'en'), '2 edges')
  assert.equal(formatFoldImportBoundaryEdgeCount(1_234, 'ja'), '1,234辺')
  assert.equal(formatFoldImportLineCount(1, 'en'), '1 line')
  assert.equal(formatFoldImportLineCount(2, 'en'), '2 lines')
  assert.equal(formatFoldImportLineCount(1_234, 'ja'), '1,234本')
  assert.equal(
    formatFoldImportConvertedScale('cm', 'en'),
    'cm conversion. Change it if needed.',
  )
  assert.equal(
    formatFoldImportConvertedScale(null, 'ja'),
    '元の単位から換算した値です。必要なら変更できます。',
  )
  assert.equal(
    formatFoldImportBoundaryAssigned('元のB線による外周（2辺）', 'ja'),
    '元のB線が単一の有効な外周を構成しています。 元のB線による外周（2辺）',
  )
  assert.equal(
    formatFoldImportAssignmentLabel('F · Flat crease', 'en'),
    'F · Flat crease mapping',
  )
  assert.equal(
    formatFoldImportUnresolvedAssignments(
      ['F · Flat crease', 'J · Face join'],
      'en',
    ),
    'Not selected: F · Flat crease, J · Face join',
  )
})

test('FOLD import dialog delegates display formatting and native copy boundaries', () => {
  const source = readFileSync(
    new URL('../src/components/FoldImportDialog.tsx', import.meta.url),
    'utf8',
  )

  for (const formatter of [
    'formatFoldImportGeometry',
    'formatFoldImportBoundaryEdgeCount',
    'formatFoldImportLineCount',
    'formatFoldImportConvertedScale',
    'formatFoldImportBoundaryAssigned',
    'formatFoldImportAssignmentLabel',
    'formatFoldImportUnresolvedAssignments',
  ]) {
    assert.match(source, new RegExp(`${formatter}\\(`, 'u'), formatter)
  }
  assert.match(source, /\{copy\.closeGlyph\}/u)
  assert.match(source, /foldImportPreviewFileName\(preview\.file_name, locale\)/u)
  assert.match(source, /foldImportSuggestedName\(preview\.suggested_name, locale\)/u)
  assert.match(source, /foldImportWarningMessage\(warning, locale\)/u)
  assert.match(source, /foldImportTargetLabel\(option\.value, locale\)/u)
  assert.doesNotMatch(source, /\blocale\s*[!=]==?/u)
  assert.doesNotMatch(source, /\.toLocaleString\(/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, />\s*×\s*</u)
})

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) {
    assertDeeplyFrozen(child)
  }
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
