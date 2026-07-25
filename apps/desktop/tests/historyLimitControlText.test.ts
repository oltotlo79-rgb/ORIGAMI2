import assert from 'node:assert/strict'
import test from 'node:test'
import { formatLocalizedText } from '../src/lib/i18n.ts'
import { HISTORY_LIMIT_CONTROL_TEXT } from '../src/lib/historyLimitControlText.ts'

test('history limit control catalog is closed, deeply frozen, and preserves count', () => {
  assert.deepEqual(Object.keys(HISTORY_LIMIT_CONTROL_TEXT), [
    'invalidValueError', 'applyError', 'legend', 'currentLimit',
    'currentLimitAriaLabel', 'entryCount', 'unavailable', 'inputLabel',
    'applying', 'apply', 'description',
  ])
  assert.equal(Object.isFrozen(HISTORY_LIMIT_CONTROL_TEXT), true)
  for (const text of Object.values(HISTORY_LIMIT_CONTROL_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.equal(formatLocalizedText('ja', HISTORY_LIMIT_CONTROL_TEXT.entryCount, {
    count: 128,
  }), '128件')
})
