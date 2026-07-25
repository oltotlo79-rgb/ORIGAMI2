import assert from 'node:assert/strict'
import test from 'node:test'
import { RECOVERY_DIALOG_TEXT } from '../src/lib/recoveryDialogText.ts'

test('recovery dialog catalog is closed, deeply frozen, and bilingual', () => {
  assert.deepEqual(Object.keys(RECOVERY_DIALOG_TEXT), [
    'eyebrow', 'availableTitle', 'invalidTitle', 'availableDescription',
    'lastUpdated', 'caution', 'invalidDescription', 'actionError',
    'restoring', 'restore', 'checking', 'retry', 'discarding',
    'discard', 'noTimestamp', 'unavailable',
  ])
  assert.equal(Object.isFrozen(RECOVERY_DIALOG_TEXT), true)
  for (const text of Object.values(RECOVERY_DIALOG_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.deepEqual(RECOVERY_DIALOG_TEXT.availableTitle, {
    ja: '未保存の編集内容を復元しますか？',
    en: 'Restore unsaved edits?',
  })
})
