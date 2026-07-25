import assert from 'node:assert/strict'
import test from 'node:test'

import { FOLD_IMPORT_DIALOG_TEXT as TEXT } from '../src/lib/foldImportDialogText.ts'

const EXPECTED_KEYS = [
  'eyebrow',
  'title',
  'close',
  'description',
  'preview',
  'previewUnavailable',
  'previewTruncated',
  'metadata',
  'unspecified',
  'vertexUnit',
  'edgeUnit',
  'edgeUnitOne',
  'unitPrefix',
  'geometrySeparator',
  'listSeparator',
  'millimetresUnit',
  'name',
  'invalidName',
  'scale',
  'missingScale',
  'sourceUnit',
  'convertedScale',
  'mappingTitle',
  'mappingDescription',
  'boundaryTitle',
  'boundaryDescription',
  'boundaryAssigned',
  'boundarySelect',
  'boundaryUnavailable',
  'lineUnit',
  'lineUnitOne',
  'boundaryFixed',
  'assignmentSuffix',
  'select',
  'unresolved',
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
  ['vertexUnit', '頂点', 'vertices'],
  ['edgeUnit', '辺', 'edges'],
  ['edgeUnitOne', '辺', 'edge'],
  ['unitPrefix', '', ' '],
  ['geometrySeparator', '・', ' · '],
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
    'convertedScale',
    'から換算した値です。必要なら変更できます。',
    ' conversion. Change it if needed.',
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
    'boundaryAssigned',
    '元のB線が単一の有効な外周を構成しています。',
    'The source B lines form one valid paper boundary.',
  ],
  ['boundarySelect', '外周候補を選択してください', 'Select a boundary candidate'],
  [
    'boundaryUnavailable',
    '安全に使える外周候補がありません。このファイルは取り込めません。',
    'No boundary candidate can be used safely. This file cannot be imported.',
  ],
  ['lineUnit', '本', 'lines'],
  ['lineUnitOne', '本', 'line'],
  ['boundaryFixed', '用紙境界（固定）', 'Paper boundary (fixed)'],
  ['assignmentSuffix', 'の割当', ' mapping'],
  ['select', '選択してください', 'Select a mapping'],
  ['unresolved', '未選択', 'Not selected'],
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

test('FOLD import dialog catalog is deeply frozen with no placeholders', () => {
  assert.equal(Object.isFrozen(TEXT), true)
  assert.equal(Object.isFrozen(TEXT.ja), true)
  assert.equal(Object.isFrozen(TEXT.en), true)
  assert.equal(Object.isFrozen(TEXT.ja.metadata), true)
  assert.equal(Object.isFrozen(TEXT.en.metadata), true)

  const placeholders: Array<readonly [string, string, string[]]> = []
  for (const locale of ['ja', 'en'] as const) {
    for (const [key, value] of Object.entries(TEXT[locale])) {
      if (typeof value === 'string') {
        placeholders.push([
          locale,
          key,
          [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
            .map((match) => match[1]),
        ])
        continue
      }
      for (const [metadataKey, metadataValue] of Object.entries(value)) {
        placeholders.push([
          locale,
          `metadata.${metadataKey}`,
          [...metadataValue.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
            .map((match) => match[1]),
        ])
      }
    }
  }
  assert.deepEqual(
    placeholders.filter(([, , keys]) => keys.length > 0),
    [],
  )
})
