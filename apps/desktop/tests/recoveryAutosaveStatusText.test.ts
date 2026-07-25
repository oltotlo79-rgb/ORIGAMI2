import assert from 'node:assert/strict'
import test from 'node:test'

import {
  RECOVERY_AUTOSAVE_MONITOR_WARNING,
  RECOVERY_AUTOSAVE_MONITOR_WARNING_EN,
  RECOVERY_AUTOSAVE_PERSISTENCE_WARNING,
  RECOVERY_AUTOSAVE_PERSISTENCE_WARNING_EN,
  RECOVERY_AUTOSAVE_RECOVERED_NOTICE,
  RECOVERY_AUTOSAVE_RECOVERED_NOTICE_EN,
  RECOVERY_AUTOSAVE_STATUS_TEXT,
} from '../src/lib/recoveryAutosaveStatusText.ts'

test('recovery autosave status catalog is closed, deeply frozen, and preserves public constants', () => {
  assert.deepEqual(Object.keys(RECOVERY_AUTOSAVE_STATUS_TEXT), [
    'persistence',
    'monitor',
    'recovered',
  ])
  assert.equal(Object.isFrozen(RECOVERY_AUTOSAVE_STATUS_TEXT), true)
  for (const text of Object.values(RECOVERY_AUTOSAVE_STATUS_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.deepEqual(RECOVERY_AUTOSAVE_STATUS_TEXT.persistence, {
    ja: RECOVERY_AUTOSAVE_PERSISTENCE_WARNING,
    en: RECOVERY_AUTOSAVE_PERSISTENCE_WARNING_EN,
  })
  assert.deepEqual(RECOVERY_AUTOSAVE_STATUS_TEXT.monitor, {
    ja: RECOVERY_AUTOSAVE_MONITOR_WARNING,
    en: RECOVERY_AUTOSAVE_MONITOR_WARNING_EN,
  })
  assert.deepEqual(RECOVERY_AUTOSAVE_STATUS_TEXT.recovered, {
    ja: RECOVERY_AUTOSAVE_RECOVERED_NOTICE,
    en: RECOVERY_AUTOSAVE_RECOVERED_NOTICE_EN,
  })
})
