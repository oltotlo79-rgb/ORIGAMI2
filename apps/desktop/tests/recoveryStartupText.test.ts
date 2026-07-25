import assert from 'node:assert/strict'
import test from 'node:test'
import { RECOVERY_STARTUP_TEXT } from '../src/lib/recoveryStartupText.ts'

test('recovery startup catalog is closed, deeply frozen, and bilingual', () => {
  assert.deepEqual(Object.keys(RECOVERY_STARTUP_TEXT), [
    'eyebrow', 'checkingTitle', 'failedTitle', 'checkingDescription',
    'failedDescription', 'retrying', 'retry',
  ])
  assert.equal(Object.isFrozen(RECOVERY_STARTUP_TEXT), true)
  for (const text of Object.values(RECOVERY_STARTUP_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.deepEqual(RECOVERY_STARTUP_TEXT.retrying, {
    ja: '再確認中…',
    en: 'Checking again…',
  })
})
