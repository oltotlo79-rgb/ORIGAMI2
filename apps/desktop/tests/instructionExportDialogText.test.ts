import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  formatInstructionExportDialogCount,
  formatInstructionExportDialogOption,
  formatInstructionExportDialogProgress,
  formatInstructionExportDialogRevision,
  INSTRUCTION_EXPORT_COPY as TEXT,
  instructionExportDialogSummary,
} from '../src/lib/instructionExportDialogText.ts'
import { selectLocalizedText } from '../src/lib/i18n.ts'

const TOP_LEVEL_KEYS = [
  'eyebrow',
  'title',
  'close',
  'closeGlyph',
  'description',
  'format',
  'formatOption',
  'optionDetails',
  'progress',
  'rebuild',
  'retry',
  'metadata',
  'counts',
  'revisionValue',
  'warningTitle',
  'acknowledge',
  'warningFree',
  'stop',
  'cancel',
  'processing',
  'save',
  'summaries',
  'emptyNotice',
  'numberLocale',
] as const

test('instruction export dialog catalog is closed, locale-complete, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), TOP_LEVEL_KEYS)
  assert.deepEqual(Object.keys(TEXT.optionDetails), ['pdf', 'svg_zip'])
  assert.deepEqual(Object.keys(TEXT.metadata), [
    'format',
    'specification',
    'profile',
    'projection',
    'suggestedName',
    'size',
    'steps',
    'pages',
    'cautions',
    'revision',
  ])
  assert.deepEqual(Object.keys(TEXT.counts), ['steps', 'pages', 'cautions'])
  assert.deepEqual(Object.keys(TEXT.summaries), ['pdf', 'svg_zip'])
  assertLocalizedLeaves(TEXT)
  assertDeeplyFrozen(TEXT)
})

test('instruction export dialog placeholders are locale-equivalent', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    formatOption: {
      ja: ['label', 'detail'],
      en: ['label', 'detail'],
    },
    progress: {
      ja: ['format', 'phase'],
      en: ['format', 'phase'],
    },
    'counts.steps.one': { ja: ['count'], en: ['count'] },
    'counts.steps.other': { ja: ['count'], en: ['count'] },
    'counts.pages.one': { ja: ['count'], en: ['count'] },
    'counts.pages.other': { ja: ['count'], en: ['count'] },
    'counts.cautions.one': { ja: ['count'], en: ['count'] },
    'counts.cautions.other': { ja: ['count'], en: ['count'] },
    revisionValue: { ja: ['revision'], en: ['revision'] },
  })
})

test('instruction export dialog formatters preserve reviewed copy and grammar', () => {
  assert.equal(selectLocalizedText('ja', TEXT.eyebrow), '折り図の書き出し')
  assert.equal(selectLocalizedText('en', TEXT.title), 'Review format and output')
  assert.equal(
    formatInstructionExportDialogOption('pdf', 'PDF 1.7', 'en'),
    'PDF 1.7 — Combine fixed-isometric diagrams with authored camera and hand/regrip guide details into a multi-page PDF',
  )
  assert.equal(
    formatInstructionExportDialogProgress(
      'SVG画像 ZIP',
      '面構造を解析しています',
      'ja',
    ),
    'SVG画像 ZIP: 面構造を解析しています…',
  )
  assert.equal(formatInstructionExportDialogCount(1, 'steps', 'en'), '1 step')
  assert.equal(formatInstructionExportDialogCount(2, 'steps', 'en'), '2 steps')
  assert.equal(formatInstructionExportDialogCount(1, 'pages', 'en'), '1 page')
  assert.equal(formatInstructionExportDialogCount(2, 'pages', 'en'), '2 pages')
  assert.equal(formatInstructionExportDialogCount(1, 'cautions', 'en'), '1 notice')
  assert.equal(formatInstructionExportDialogCount(2, 'cautions', 'en'), '2 notices')
  assert.equal(
    formatInstructionExportDialogCount(1_234, 'steps', 'ja'),
    '1,234手順',
  )
  assert.equal(
    formatInstructionExportDialogRevision(1_234, 'en'),
    'revision 1,234',
  )
  assert.equal(
    instructionExportDialogSummary(
      'pdf',
      'PDF 1.7・固定アイソメトリック投影・A4縦',
      'ja',
    ),
    'PDF 1.7・固定アイソメトリック投影・A4縦',
  )
  assert.equal(
    instructionExportDialogSummary('pdf', 'native Japanese summary', 'en'),
    'PDF 1.7 · A4 portrait · fixed isometric projection · multiple pages',
  )
})

test('instruction export dialog keeps every static display token in its catalog', () => {
  const source = readFileSync(
    new URL('../src/components/InstructionExportDialog.tsx', import.meta.url),
    'utf8',
  )
  assert.match(source, /INSTRUCTION_EXPORT_COPY as TEXT/u)
  assert.match(source, /selectLocalizedText\(locale, text\)/u)
  assert.match(source, /formatInstructionExportDialogOption\(/u)
  assert.match(source, /formatInstructionExportDialogProgress\(/u)
  assert.match(source, /formatInstructionExportDialogCount\(/u)
  assert.match(source, /formatInstructionExportDialogRevision\(/u)
  assert.match(source, /instructionExportDialogSummary\(/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /locale\s*[!=]==?\s*['"](?:ja|en)['"]/u)
  assert.doesNotMatch(source, /['"](?:step|page|notice)['"]/u)
  assert.doesNotMatch(source, />\s*revision\s*\{/u)
  assert.doesNotMatch(source, />\s*×\s*</u)
  assert.doesNotMatch(source, /\{\s*['"] — ['"]\s*\}/u)
  assert.doesNotMatch(source, /\\u00a0/u)
})

function assertLocalizedLeaves(value: unknown, path = 'root') {
  assert.equal(typeof value, 'object', path)
  assert.notEqual(value, null, path)
  const record = value as Readonly<Record<string, unknown>>
  const keys = Object.keys(record)
  if (keys.length === 2 && keys[0] === 'ja' && keys[1] === 'en') {
    assert.equal(typeof record.ja, 'string', `${path}.ja`)
    assert.equal(typeof record.en, 'string', `${path}.en`)
    return
  }
  for (const [key, child] of Object.entries(record)) {
    assertLocalizedLeaves(child, path === 'root' ? key : `${path}.${key}`)
  }
}

function assertDeeplyFrozen(value: unknown, path = 'root') {
  if (
    (typeof value !== 'object' && typeof value !== 'function')
    || value === null
  ) return
  assert.equal(Object.isFrozen(value), true, path)
  for (const [key, child] of Object.entries(value)) {
    assertDeeplyFrozen(child, `${path}.${key}`)
  }
}

function placeholderMap(value: unknown) {
  const result: Record<string, Record<'ja' | 'en', string[]>> = {}
  visit(value, '', result)
  return result
}

function visit(
  value: unknown,
  path: string,
  result: Record<string, Record<'ja' | 'en', string[]>>,
) {
  if (typeof value !== 'object' || value === null) return
  const record = value as Readonly<Record<string, unknown>>
  const keys = Object.keys(record)
  if (
    keys.length === 2
    && keys[0] === 'ja'
    && keys[1] === 'en'
    && typeof record.ja === 'string'
    && typeof record.en === 'string'
  ) {
    const ja = placeholders(record.ja)
    const en = placeholders(record.en)
    if (ja.length > 0 || en.length > 0) result[path] = { ja, en }
    return
  }
  for (const [key, child] of Object.entries(record)) {
    visit(child, path ? `${path}.${key}` : key, result)
  }
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
