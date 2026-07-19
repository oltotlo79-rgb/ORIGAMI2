import assert from 'node:assert/strict'
import test from 'node:test'

import {
  APP_ERROR_CODES,
  appErrorLocalizedText,
} from '../src/lib/appMessages.ts'

test('application errors use a complete frozen code allowlist', () => {
  assert.equal(Object.isFrozen(APP_ERROR_CODES), true)
  assert.deepEqual(APP_ERROR_CODES, [
    'unexpected_failure',
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
  ])

  for (const code of APP_ERROR_CODES) {
    const message = appErrorLocalizedText(code)
    assert.equal(Object.isFrozen(message), true, code)
    assert.ok(message.ja.length > 0, code)
    assert.ok(message.en.length > 0, code)
    assert.doesNotMatch(message.ja, /\{(?:error|message|path)\}/u, code)
    assert.doesNotMatch(message.en, /\{(?:error|message|path)\}/u, code)
  }
})

test('a forged error code fails closed without reflecting its value', () => {
  const hostileCode = String.raw`C:\Users\alice\作品\private-project.ori`
  const message = appErrorLocalizedText(
    hostileCode as (typeof APP_ERROR_CODES)[number],
  )

  assert.deepEqual(message, appErrorLocalizedText('unexpected_failure'))
  assert.doesNotMatch(message.ja, /alice|private-project/iu)
  assert.doesNotMatch(message.en, /alice|private-project/iu)
})
