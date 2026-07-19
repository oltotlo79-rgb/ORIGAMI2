import assert from 'node:assert/strict'
import test from 'node:test'

import {
  resolveFileKeyboardShortcut,
  type FileKeyboardShortcutEvent,
} from '../src/lib/fileKeyboardShortcut.ts'

test('Windows Ctrl file shortcuts use the standard mapping', () => {
  assert.equal(resolveFileKeyboardShortcut(key('n')), 'new')
  assert.equal(resolveFileKeyboardShortcut(key('O')), 'open')
  assert.equal(resolveFileKeyboardShortcut(key('s')), 'save')
  assert.equal(
    resolveFileKeyboardShortcut(key('S', { shiftKey: true })),
    'save_as',
  )
})

test('macOS Meta mapping is identical and build-testable on Windows', () => {
  assert.equal(
    resolveFileKeyboardShortcut(key('n', { ctrlKey: false, metaKey: true })),
    'new',
  )
  assert.equal(
    resolveFileKeyboardShortcut(key('o', { ctrlKey: false, metaKey: true })),
    'open',
  )
  assert.equal(
    resolveFileKeyboardShortcut(key('s', { ctrlKey: false, metaKey: true })),
    'save',
  )
  assert.equal(
    resolveFileKeyboardShortcut(key('s', {
      ctrlKey: false,
      metaKey: true,
      shiftKey: true,
    })),
    'save_as',
  )
})

test('unsupported modifier combinations and unrelated keys are preserved', () => {
  assert.equal(resolveFileKeyboardShortcut(key('n', { shiftKey: true })), null)
  assert.equal(resolveFileKeyboardShortcut(key('o', { shiftKey: true })), null)
  assert.equal(resolveFileKeyboardShortcut(key('s', { altKey: true })), null)
  assert.equal(resolveFileKeyboardShortcut(key('s', { ctrlKey: false })), null)
  assert.equal(resolveFileKeyboardShortcut(key('s', { metaKey: true })), null)
  assert.equal(resolveFileKeyboardShortcut(key('z')), null)
  assert.equal(resolveFileKeyboardShortcut(key('F1')), null)
})

test('repeat, IME composition, malformed, and hostile values fail closed', () => {
  assert.equal(resolveFileKeyboardShortcut(key('n', { repeat: true })), null)
  assert.equal(resolveFileKeyboardShortcut(key('n', { isComposing: true })), null)
  assert.equal(resolveFileKeyboardShortcut(null), null)
  assert.equal(resolveFileKeyboardShortcut({}), null)
  assert.equal(resolveFileKeyboardShortcut({
    ...key('n'),
    ctrlKey: 'yes',
  }), null)
  assert.equal(resolveFileKeyboardShortcut({
    ...key('n'),
    key: 'n'.repeat(65),
  }), null)
  assert.equal(resolveFileKeyboardShortcut(Object.defineProperty({}, 'key', {
    get() {
      throw new Error('hostile getter')
    },
  })), null)
})

function key(
  keyValue: string,
  overrides: Partial<FileKeyboardShortcutEvent> = {},
): FileKeyboardShortcutEvent {
  return {
    key: keyValue,
    altKey: false,
    ctrlKey: true,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    isComposing: false,
    ...overrides,
  }
}
