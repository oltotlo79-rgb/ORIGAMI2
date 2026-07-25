import assert from 'node:assert/strict'
import test from 'node:test'
import { formatLocalizedText } from '../src/lib/i18n.ts'
import {
  KEYBOARD_SHORTCUT_COMMAND_LABELS,
  KEYBOARD_SHORTCUT_TEXT,
} from '../src/lib/keyboardShortcutControlText.ts'

test('keyboard shortcut catalog is closed, deeply frozen, and preserves placeholders', () => {
  assert.deepEqual(Object.keys(KEYBOARD_SHORTCUT_TEXT), [
    'summary', 'groupAriaLabel', 'description', 'keyAriaLabel',
    'useAltAriaLabel', 'useShiftAriaLabel', 'currentAriaLabel',
    'reset', 'invalid', 'conflict', 'platformJoin',
  ])
  assert.equal(Object.isFrozen(KEYBOARD_SHORTCUT_TEXT), true)
  for (const text of Object.values(KEYBOARD_SHORTCUT_TEXT)) assert.equal(Object.isFrozen(text), true)
  assert.equal(formatLocalizedText('en', KEYBOARD_SHORTCUT_TEXT.conflict, {
    command: 'Undo', conflictingCommand: 'Redo', platforms: 'Windows / macOS',
  }), 'Undo conflicts with Redo (Windows / macOS).')
  assert.deepEqual(KEYBOARD_SHORTCUT_COMMAND_LABELS, {
    new: { ja: '新規', en: 'New' },
    open: { ja: '開く', en: 'Open' },
    save: { ja: '保存', en: 'Save' },
    save_as: { ja: '別名保存', en: 'Save as' },
    undo: { ja: '元に戻す', en: 'Undo' },
    redo: { ja: 'やり直す', en: 'Redo' },
  })
  assert.equal(Object.isFrozen(KEYBOARD_SHORTCUT_COMMAND_LABELS), true)
  for (const text of Object.values(KEYBOARD_SHORTCUT_COMMAND_LABELS)) {
    assert.equal(Object.isFrozen(text), true)
  }
})
