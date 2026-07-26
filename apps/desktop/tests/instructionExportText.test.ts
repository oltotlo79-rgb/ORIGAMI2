import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { selectLocalizedText } from '../src/lib/i18n.ts'
import {
  INSTRUCTION_EXPORT_ERROR_TEXT,
  INSTRUCTION_EXPORT_FORMAT_LABEL_TEXT,
  INSTRUCTION_EXPORT_PHASE_TEXT,
  INSTRUCTION_EXPORT_PRESENTATION_TEXT,
  INSTRUCTION_EXPORT_WARNING_TEXT,
} from '../src/lib/instructionExportText.ts'

const catalogs = [
  INSTRUCTION_EXPORT_ERROR_TEXT,
  INSTRUCTION_EXPORT_WARNING_TEXT,
  INSTRUCTION_EXPORT_FORMAT_LABEL_TEXT,
  INSTRUCTION_EXPORT_PHASE_TEXT,
  INSTRUCTION_EXPORT_PRESENTATION_TEXT,
] as const

test('instruction export presentation catalogs are closed and deeply frozen', () => {
  assert.equal(Object.keys(INSTRUCTION_EXPORT_ERROR_TEXT).length, 17)
  assert.equal(Object.keys(INSTRUCTION_EXPORT_WARNING_TEXT).length, 4)
  assert.equal(Object.keys(INSTRUCTION_EXPORT_FORMAT_LABEL_TEXT).length, 2)
  assert.equal(Object.keys(INSTRUCTION_EXPORT_PHASE_TEXT).length, 4)
  assert.equal(Object.keys(INSTRUCTION_EXPORT_PRESENTATION_TEXT).length, 6)

  for (const catalog of catalogs) {
    assert.equal(Object.isFrozen(catalog), true)
    for (const text of Object.values(catalog)) {
      assert.deepEqual(Object.keys(text), ['ja', 'en'])
      assert.equal(Object.isFrozen(text), true)
      assert.equal(typeof text.ja, 'string')
      assert.equal(typeof text.en, 'string')
      assert.deepEqual(placeholders(text.ja), placeholders(text.en))
    }
  }
})

test('instruction export catalogs preserve exact trusted text and Japanese fallback', () => {
  assert.deepEqual(INSTRUCTION_EXPORT_ERROR_TEXT.project_changed, {
    ja: '生成を開始した後に編集内容が変わりました。現在の編集内容から作り直してください。',
    en: 'The project changed after generation started. Rebuild from the current edits.',
  })
  assert.deepEqual(INSTRUCTION_EXPORT_WARNING_TEXT.discrete_step_endpoints_only, {
    ja: '各手順は保存済みの終端姿勢のみを表し、手順間の連続動作は出力されません。',
    en: 'Each step shows only its saved endpoint pose; continuous motion between steps is not exported.',
  })
  assert.deepEqual(INSTRUCTION_EXPORT_FORMAT_LABEL_TEXT.svg_zip, {
    ja: 'SVG画像 ZIP',
    en: 'SVG images ZIP',
  })
  assert.deepEqual(INSTRUCTION_EXPORT_PHASE_TEXT.building_document, {
    ja: 'ページとファイルを生成しています',
    en: 'Generating pages and files',
  })
  assert.equal(
    selectLocalizedText(
      'unsupported-locale',
      INSTRUCTION_EXPORT_PRESENTATION_TEXT.unknownWarning,
    ),
    '折り図の制約を識別できません。',
  )
  assert.equal(
    selectLocalizedText(
      'unsupported-locale',
      INSTRUCTION_EXPORT_PRESENTATION_TEXT.numberLocale,
    ),
    'ja-JP',
  )
})

test('instruction export delegates locale choice and formatting to i18n helpers', () => {
  const source = readFileSync(
    new URL('../src/lib/instructionExport.ts', import.meta.url),
    'utf8',
  )
  assert.doesNotMatch(source, /locale\s*[!=]==?\s*['"](?:ja|en)['"]/u)
  assert.doesNotMatch(source, /['"](?:ja|en)['"]\s*\?/u)
  assert.doesNotMatch(source, /\[\s*locale\s*\]/u)
  assert.match(source, /\bselectLocalizedText\(/u)
  assert.match(source, /\bformatLocalizedText\(/u)
  assert.match(source, /\bINSTRUCTION_EXPORT_ERROR_TEXT\b/u)
  assert.match(source, /\bINSTRUCTION_EXPORT_WARNING_TEXT\b/u)
  assert.match(source, /\bINSTRUCTION_EXPORT_FORMAT_LABEL_TEXT\b/u)
  assert.match(source, /\bINSTRUCTION_EXPORT_PHASE_TEXT\b/u)
  assert.match(source, /\bINSTRUCTION_EXPORT_PRESENTATION_TEXT\b/u)
})

function placeholders(value: string): readonly string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1]!)
    .sort()
}
