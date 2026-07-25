import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  CREASE_EXPORT_COPY,
  formatCreaseExportInteger,
  resolveCreaseExportFormatSummary,
} from '../src/lib/creaseExportDialogText.ts'
import { formatMessage } from '../src/lib/i18n.ts'

const COPY_KEYS = [
  'eyebrow',
  'title',
  'close',
  'description',
  'format',
  'formatOption',
  'optionDetails',
  'generating',
  'rebuild',
  'retry',
  'metadata',
  'geometryValue',
  'revisionValue',
  'includes',
  'excludes',
  'lines',
  'lineCount',
  'lineCountOne',
  'assignmentLabels',
  'lossTitle',
  'acknowledge',
  'lossless',
  'cancel',
  'processing',
  'save',
  'formatSummaries',
] as const
const FORMAT_KEYS = ['fold', 'svg', 'pdf', 'dxf'] as const
const METADATA_KEYS = [
  'format',
  'specification',
  'suggestedName',
  'size',
  'geometry',
  'revision',
  'cuts',
] as const
const ASSIGNMENT_KEYS = [
  'boundary',
  'mountain',
  'valley',
  'auxiliary',
  'cut',
] as const

test('crease export dialog catalog is locale-complete and deeply frozen', () => {
  assert.deepEqual(Object.keys(CREASE_EXPORT_COPY), ['ja', 'en'])
  assertLocaleShape(CREASE_EXPORT_COPY.ja, CREASE_EXPORT_COPY.en)
  assertDeeplyFrozen(CREASE_EXPORT_COPY)
  for (const locale of ['ja', 'en'] as const) {
    const copy = CREASE_EXPORT_COPY[locale]
    assert.deepEqual(Object.keys(copy), COPY_KEYS, locale)
    assert.deepEqual(Object.keys(copy.optionDetails), FORMAT_KEYS, locale)
    assert.deepEqual(Object.keys(copy.metadata), METADATA_KEYS, locale)
    assert.deepEqual(Object.keys(copy.assignmentLabels), ASSIGNMENT_KEYS, locale)
    assert.deepEqual(Object.keys(copy.formatSummaries), FORMAT_KEYS, locale)
  }
})

test('crease export dialog catalog preserves reviewed byte-sensitive copy', () => {
  assert.equal(CREASE_EXPORT_COPY.ja.eyebrow, '展開図の書き出し')
  assert.equal(CREASE_EXPORT_COPY.en.title, 'Review format and information loss')
  assert.equal(
    CREASE_EXPORT_COPY.ja.description,
    '現在の編集リビジョンから展開図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。',
  )
  assert.equal(
    CREASE_EXPORT_COPY.en.generating,
    '{format} data is being validated and generated…',
  )
  assert.deepEqual(CREASE_EXPORT_COPY.ja.assignmentLabels, {
    boundary: '外周',
    mountain: '山折り',
    valley: '谷折り',
    auxiliary: '補助線',
    cut: '切断線',
  })
  assert.deepEqual(CREASE_EXPORT_COPY.en.formatSummaries, {
    fold: 'FOLD 1.2 · 2D creasePattern · coordinates in mm',
    svg: 'Static line SVG · 1 SVG unit = 1 mm',
    pdf: 'Full-size 1:1 vector · drawing bounds + 10 mm margins',
    dxf: 'AC1021 text form · UTF-8 · mm · 5 semantic layers',
  })
})

test('crease export templates preserve locale-specific formatting without component branches', () => {
  for (const key of [
    'formatOption',
    'generating',
    'geometryValue',
    'revisionValue',
    'lineCount',
    'lineCountOne',
  ] as const) {
    assert.deepEqual(
      placeholders(CREASE_EXPORT_COPY.ja[key]),
      placeholders(CREASE_EXPORT_COPY.en[key]),
      key,
    )
  }
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.ja.formatOption, {
      label: 'SVG',
      detail: CREASE_EXPORT_COPY.ja.optionDetails.svg,
    }),
    'SVG — 印刷・作図ソフトで扱いやすい静的な線図',
  )
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.ja.generating, {
      format: 'FOLD 1.2',
    }),
    'FOLD 1.2データを検証・生成しています…',
  )
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.en.generating, {
      format: 'FOLD 1.2',
    }),
    'FOLD 1.2 data is being validated and generated…',
  )
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.ja.geometryValue, {
      vertices: formatCreaseExportInteger(12_345, 'ja'),
      edges: formatCreaseExportInteger(67_890, 'ja'),
    }),
    '12,345頂点・67,890辺',
  )
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.en.geometryValue, {
      vertices: formatCreaseExportInteger(12_345, 'en'),
      edges: formatCreaseExportInteger(67_890, 'en'),
    }),
    '12,345 vertices · 67,890 edges',
  )
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.ja.lineCountOne, { count: '1' }),
    '1本',
  )
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.en.lineCountOne, { count: '1' }),
    '1 line',
  )
  assert.equal(
    formatMessage(CREASE_EXPORT_COPY.en.lineCount, { count: '2' }),
    '2 lines',
  )
  assert.equal(
    resolveCreaseExportFormatSummary(
      'ja',
      'fold',
      'native summary is authoritative',
    ),
    'native summary is authoritative',
  )
  assert.equal(
    resolveCreaseExportFormatSummary(
      'en',
      'fold',
      'native summary must not leak',
    ),
    'FOLD 1.2 · 2D creasePattern · coordinates in mm',
  )
})

test('crease export dialog source delegates every locale-sensitive rendering path', () => {
  const source = readFileSync(
    new URL('../src/components/CreaseExportDialog.tsx', import.meta.url),
    'utf8',
  )

  assert.match(source, /const copy = CREASE_EXPORT_COPY\[locale\]/u)
  assert.match(source, /formatMessage\(copy\.formatOption/u)
  assert.match(source, /formatMessage\(copy\.generating/u)
  assert.match(source, /resolveCreaseExportFormatSummary\(/u)
  assert.match(source, /formatMessage\(copy\.geometryValue/u)
  assert.match(source, /formatMessage\(copy\.revisionValue/u)
  assert.match(source, /copy\.lineCountOne/u)
  assert.match(source, /formatCreaseExportInteger\(/u)
  assert.doesNotMatch(source, /\blocale\s*[!=]==?/u)
  assert.doesNotMatch(source, /\.toLocaleString\(/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /['"`](?:revision| line| vertices| edges)/u)
})

function assertLocaleShape(
  left: Readonly<Record<string, unknown>>,
  right: Readonly<Record<string, unknown>>,
) {
  assert.deepEqual(Object.keys(left), Object.keys(right))
  for (const key of Object.keys(left)) {
    const leftValue = left[key]
    const rightValue = right[key]
    assert.equal(typeof leftValue, typeof rightValue, key)
    if (
      typeof leftValue === 'object'
      && leftValue !== null
      && typeof rightValue === 'object'
      && rightValue !== null
    ) {
      assertLocaleShape(
        leftValue as Readonly<Record<string, unknown>>,
        rightValue as Readonly<Record<string, unknown>>,
      )
    }
  }
}

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
