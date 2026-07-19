import assert from 'node:assert/strict'
import test from 'node:test'

import {
  DEFAULT_KEYBOARD_SHORTCUTS,
  KEYBOARD_SHORTCUT_COMMANDS,
  KEYBOARD_SHORTCUT_STORAGE_KEY,
  KEYBOARD_SHORTCUT_VERSION,
  createKeyboardShortcutStore,
  decodeKeyboardShortcuts,
  encodeKeyboardShortcuts,
  findKeyboardShortcutConflict,
  keyboardShortcutAriaValue,
  keyboardShortcutDisplayValue,
  resolveConfiguredKeyboardShortcut,
  type KeyboardShortcutEnvironment,
  type KeyboardShortcutEvent,
  type KeyboardShortcutSnapshot,
} from '../src/lib/keyboardShortcutSettings.ts'

test('portable defaults preserve Windows and macOS standard commands', () => {
  const windows = {
    ctrlKey: true,
    metaKey: false,
  }
  const macos = {
    ctrlKey: false,
    metaKey: true,
  }
  for (const modifiers of [windows, macos]) {
    assert.equal(resolve(key('n', modifiers)), 'new')
    assert.equal(resolve(key('o', modifiers)), 'open')
    assert.equal(resolve(key('s', modifiers)), 'save')
    assert.equal(resolve(key('s', { ...modifiers, shiftKey: true })), 'save_as')
    assert.equal(resolve(key('z', modifiers)), 'undo')
    assert.equal(resolve(key('z', { ...modifiers, shiftKey: true })), 'redo')
  }
  assert.equal(resolve(key('y')), 'redo')
  assert.equal(
    resolve(key('y', { ctrlKey: false, metaKey: true })),
    null,
  )
})

test('configured chords support allowlisted keys and exact modifiers', () => {
  const fixture = environment()
  const store = createKeyboardShortcutStore(fixture.value)
  assert.equal(store.setShortcut('new', {
    key: 'f12',
    alt: true,
    shift: true,
  }).ok, true)
  assert.equal(
    resolveConfiguredKeyboardShortcut(
      key('F12', { altKey: true, shiftKey: true }),
      store.getSnapshot(),
    ),
    'new',
  )
  assert.equal(
    resolveConfiguredKeyboardShortcut(
      key('F12', { altKey: false, shiftKey: true }),
      store.getSnapshot(),
    ),
    null,
  )
  assert.equal(resolve(key('n'), store.getSnapshot()), null)
})

test('transformed Alt and Shift keys use a bounded KeyboardEvent.code fallback', () => {
  const fixture = environment()
  const store = createKeyboardShortcutStore(fixture.value)
  assert.equal(store.setShortcut('new', {
    key: '1',
    alt: false,
    shift: true,
  }).ok, true)
  assert.equal(
    resolveConfiguredKeyboardShortcut(
      key('!', { code: 'Digit1', shiftKey: true }),
      store.getSnapshot(),
    ),
    'new',
  )
  assert.equal(
    resolveConfiguredKeyboardShortcut(
      key('!', { shiftKey: true }),
      store.getSnapshot(),
    ),
    null,
  )

  assert.equal(store.setShortcut('new', {
    key: 'n',
    alt: true,
    shift: false,
  }).ok, true)
  assert.equal(
    resolveConfiguredKeyboardShortcut(
      key('Dead', { code: 'KeyN', altKey: true }),
      store.getSnapshot(),
    ),
    'new',
  )
  assert.equal(
    resolveConfiguredKeyboardShortcut(
      key('a', { code: 'KeyN', altKey: true }),
      store.getSnapshot(),
    ),
    null,
  )
  assert.equal(
    resolveConfiguredKeyboardShortcut(
      key('Escape', { code: 'KeyN' }),
      store.getSnapshot(),
    ),
    null,
  )
})

test('duplicates are rejected atomically on both platforms', () => {
  const fixture = environment()
  const store = createKeyboardShortcutStore(fixture.value)
  const before = store.getSnapshot()
  const result = store.setShortcut('open', before.new)
  assert.equal(result.ok, false)
  assert.equal(result.reason, 'duplicate')
  if (result.reason === 'duplicate') {
    assert.equal(result.conflict.command, 'open')
    assert.equal(result.conflict.conflictingCommand, 'new')
    assert.deepEqual(result.conflict.platforms, ['windows', 'macos'])
    assert.equal(Object.isFrozen(result.conflict.platforms), true)
  }
  assert.equal(store.getSnapshot(), before)
  assert.equal(fixture.writes.length, 0)
})

test('the fixed Windows Ctrl+Y redo alias participates in conflicts', () => {
  const conflict = findKeyboardShortcutConflict(
    DEFAULT_KEYBOARD_SHORTCUTS,
    'open',
    { key: 'y', alt: false, shift: false },
  )
  assert.equal(conflict?.conflictingCommand, 'redo')
  assert.deepEqual(conflict?.platforms, ['windows'])

  const fixture = environment()
  const store = createKeyboardShortcutStore(fixture.value)
  assert.equal(store.setShortcut('open', {
    key: 'y',
    alt: false,
    shift: false,
  }).ok, false)
  assert.equal(store.setShortcut('redo', {
    key: 'y',
    alt: false,
    shift: false,
  }).ok, true)
})

test('strict versioned persistence round trips only canonical assignments', () => {
  const encoded = encodeKeyboardShortcuts(DEFAULT_KEYBOARD_SHORTCUTS)
  const wire = JSON.parse(encoded)
  assert.deepEqual(Object.keys(wire), ['version', 'assignments'])
  assert.equal(wire.version, KEYBOARD_SHORTCUT_VERSION)
  assert.deepEqual(Object.keys(wire.assignments), KEYBOARD_SHORTCUT_COMMANDS)
  assert.deepEqual(decodeKeyboardShortcuts(encoded), DEFAULT_KEYBOARD_SHORTCUTS)
  assert.equal(KEYBOARD_SHORTCUT_STORAGE_KEY, 'origami2.keyboard-shortcuts')
})

test('old malformed duplicated oversized and hostile settings fail closed', () => {
  const valid = JSON.parse(encodeKeyboardShortcuts(DEFAULT_KEYBOARD_SHORTCUTS))
  for (const candidate of [
    null,
    '',
    '{',
    '[]',
    '{}',
    JSON.stringify({ ...valid, version: 0 }),
    JSON.stringify({ ...valid, extra: true }),
    JSON.stringify({
      ...valid,
      assignments: { ...valid.assignments, extra: valid.assignments.new },
    }),
    JSON.stringify({
      ...valid,
      assignments: {
        ...valid.assignments,
        open: valid.assignments.new,
      },
    }),
    JSON.stringify({
      ...valid,
      assignments: {
        ...valid.assignments,
        open: { key: 'y', alt: false, shift: false },
      },
    }),
    JSON.stringify({
      ...valid,
      assignments: {
        ...valid.assignments,
        new: { key: 'escape', alt: false, shift: false },
      },
    }),
    JSON.stringify({
      ...valid,
      assignments: {
        ...valid.assignments,
        new: { key: 'n', alt: false, shift: false, surprise: true },
      },
    }),
    'x'.repeat(2_049),
    Object.defineProperty({}, 'length', {
      get() {
        throw new Error('hostile')
      },
    }),
  ]) {
    assert.equal(decodeKeyboardShortcuts(candidate), null)
  }
})

test('storage failures do not block editing reset or subscriptions', () => {
  const store = createKeyboardShortcutStore({
    readStoredShortcuts() {
      throw new Error('blocked')
    },
    writeStoredShortcuts() {
      throw new Error('blocked')
    },
  })
  let notifications = 0
  store.subscribe(() => {
    notifications += 1
  })
  assert.equal(store.setShortcut('new', {
    key: 'p',
    alt: true,
    shift: false,
  }).ok, true)
  assert.equal(store.getSnapshot().new.key, 'p')
  store.reset()
  assert.equal(store.getSnapshot(), DEFAULT_KEYBOARD_SHORTCUTS)
  assert.equal(notifications, 2)
})

test('a throwing observer cannot break state or later observers', () => {
  const fixture = environment()
  const store = createKeyboardShortcutStore(fixture.value)
  let notifications = 0
  store.subscribe(() => {
    throw new Error('hostile observer')
  })
  store.subscribe(() => {
    notifications += 1
  })

  assert.doesNotThrow(() => {
    store.setShortcut('new', {
      key: 'p',
      alt: false,
      shift: false,
    })
  })
  assert.equal(store.getSnapshot().new.key, 'p')
  assert.equal(notifications, 1)
})

test('invalid setters and hostile events never mutate or dispatch', () => {
  const fixture = environment()
  const store = createKeyboardShortcutStore(fixture.value)
  const before = store.getSnapshot()
  for (const candidate of [
    null,
    {},
    { key: 'n', alt: false },
    { key: 'escape', alt: false, shift: false },
    { key: 'N', alt: false, shift: false },
    { key: 'n', alt: 0, shift: false },
    Object.defineProperty({}, 'key', {
      get() {
        throw new Error('hostile')
      },
    }),
    new Proxy({
      key: 'n',
      alt: false,
      shift: false,
    }, {
      get() {
        throw new Error('hostile')
      },
    }),
  ]) {
    assert.equal(store.setShortcut('new', candidate).ok, false)
  }
  assert.equal(store.setShortcut('unknown', before.new).ok, false)
  assert.equal(store.getSnapshot(), before)
  assert.equal(fixture.writes.length, 0)

  for (const event of [
    null,
    {},
    key('n', { repeat: true }),
    key('n', { isComposing: true }),
    key('n', { ctrlKey: false }),
    key('n', { metaKey: true }),
    key('n', { shiftKey: true }),
    { ...key('n'), key: 'n'.repeat(17) },
    key('!', { code: 'KeyN'.repeat(17), shiftKey: true }),
    Object.defineProperty({}, 'key', {
      get() {
        throw new Error('hostile')
      },
    }),
  ]) {
    assert.equal(
      resolveConfiguredKeyboardShortcut(event, before),
      null,
    )
  }
})

test('ARIA and visible labels follow the active setting and redo alias', () => {
  assert.equal(
    keyboardShortcutAriaValue('save_as', DEFAULT_KEYBOARD_SHORTCUTS),
    'Control+Shift+S Meta+Shift+S',
  )
  assert.equal(
    keyboardShortcutDisplayValue('save_as', DEFAULT_KEYBOARD_SHORTCUTS),
    'Ctrl/Cmd+Shift+S',
  )
  assert.equal(
    keyboardShortcutAriaValue('redo', DEFAULT_KEYBOARD_SHORTCUTS),
    'Control+Shift+Z Meta+Shift+Z Control+Y',
  )
  assert.equal(
    keyboardShortcutDisplayValue('redo', DEFAULT_KEYBOARD_SHORTCUTS),
    'Ctrl/Cmd+Shift+Z / Ctrl+Y',
  )
})

test('dispose drops listeners and reloads the current stored profile', () => {
  const fixture = environment()
  const store = createKeyboardShortcutStore(fixture.value)
  let notifications = 0
  store.subscribe(() => {
    notifications += 1
  })
  store.setShortcut('new', { key: 'p', alt: false, shift: false })
  assert.equal(notifications, 1)
  store.dispose()

  const changed: KeyboardShortcutSnapshot = {
    ...DEFAULT_KEYBOARD_SHORTCUTS,
    open: { key: 'l', alt: true, shift: false },
  }
  fixture.stored = encodeKeyboardShortcuts(changed)
  assert.equal(store.initialize().open.key, 'l')
  store.setShortcut('open', { key: 'k', alt: true, shift: false })
  assert.equal(notifications, 1)
})

function resolve(
  event: KeyboardShortcutEvent,
  snapshot = DEFAULT_KEYBOARD_SHORTCUTS,
) {
  return resolveConfiguredKeyboardShortcut(event, snapshot)
}

function key(
  value: string,
  overrides: Partial<KeyboardShortcutEvent> = {},
): KeyboardShortcutEvent {
  return {
    key: value,
    altKey: false,
    ctrlKey: true,
    metaKey: false,
    shiftKey: false,
    repeat: false,
    isComposing: false,
    ...overrides,
  }
}

function environment(): {
  value: KeyboardShortcutEnvironment
  writes: string[]
  stored: unknown
} {
  const fixture = {
    writes: [] as string[],
    stored: null as unknown,
    value: null as unknown as KeyboardShortcutEnvironment,
  }
  fixture.value = {
    readStoredShortcuts: () => fixture.stored,
    writeStoredShortcuts(serialized) {
      fixture.writes.push(serialized)
      fixture.stored = serialized
    },
  }
  return fixture
}
