import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const appSource = readFileSync(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)

const APP_FAILURE_CODES = [
  'window_close_status_invalid',
  'topology_analysis_failed',
  'native_edit_failed',
  'validation_failed',
  'file_operation_failed',
  'fold_read_failed',
  'fold_cleanup_failed',
  'fold_import_failed',
  'svg_read_failed',
  'svg_cleanup_failed',
  'svg_boundary_validation_failed',
  'svg_import_failed',
  'crease_export_prepare_failed',
  'crease_export_cleanup_failed',
  'crease_export_save_failed',
  'benchmark_failed',
] as const

test('App routes every general failure through a fixed structured code', () => {
  for (const code of APP_FAILURE_CODES) {
    assert.match(
      appSource,
      new RegExp(`appErrorLocalizedText\\('${code}'\\)`, 'u'),
      code,
    )
  }

  assert.doesNotMatch(appSource, /String\(error\)/u)
  assert.doesNotMatch(appSource, /const message = String\(error\)/u)
  assert.doesNotMatch(appSource, /\{\s*error:\s*message\s*\}/u)
  assert.doesNotMatch(
    appSource,
    /translated\.get\(message\)\s*\?\?\s*\{\s*ja:\s*message/u,
  )
})

test('specialized error paths retain their bounded category translators', () => {
  assert.match(
    appSource,
    /instructionExportErrorMessage\(error, locale\)/u,
  )
  assert.match(
    appSource,
    /numericExpressionNativeErrorCategory\(error\)/u,
  )
  assert.match(
    appSource,
    /newProjectExpressionErrorMessage\(error, 'ja'\)[\s\S]*newProjectExpressionErrorMessage\(error, 'en'\)/u,
  )
})

test('diagnostic classifications survive redaction', () => {
  assert.match(
    appSource,
    /reportUnexpected\('app\.topology_analysis'\)[\s\S]*appErrorLocalizedText\('topology_analysis_failed'\)/u,
  )
  assert.match(
    appSource,
    /reportValidationUnexpected\(\)[\s\S]*setValidation\(null\)[\s\S]*appErrorLocalizedText\('validation_failed'\)/u,
  )
  assert.match(
    appSource,
    /reportUnexpected\('app\.benchmark'\)[\s\S]*appErrorLocalizedText\('benchmark_failed'\)/u,
  )
})

test('unknown display vocabularies fail closed and file paths stay out of UI', () => {
  assert.doesNotMatch(appSource, /nativeSnapshot\?\.current_path/u)
  assert.match(
    appSource,
    /const label = labels\[tool\][\s\S]*ja: '不明なツール',[\s\S]*en: 'Unknown tool'/u,
  )
  assert.match(
    appSource,
    /const label = labels\[code\][\s\S]*ja: '不明な幾何検証問題',[\s\S]*en: 'Unknown geometry validation issue'/u,
  )
  assert.doesNotMatch(
    appSource,
    /return label \? selectLocalizedText\(locale, label\) : (?:tool|code)/u,
  )
})

test('explicit project names and generated file names remain visible', () => {
  assert.match(appSource, /\{nativeSnapshot\?\.name \?\? text\(/u)
  assert.match(appSource, /\{ name: snapshot\.name \}/u)
  assert.match(appSource, /\{ name: response\.project\.name \}/u)
  assert.equal(
    appSource.match(/\{ fileName: preview\.suggested_file_name \}/gu)?.length,
    2,
  )
})
