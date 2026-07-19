const MAX_STORED_SHORTCUT_BYTES = 2_048
const MAX_EVENT_KEY_LENGTH = 16
const MAX_EVENT_CODE_LENGTH = 16

export const KEYBOARD_SHORTCUT_STORAGE_KEY = 'origami2.keyboard-shortcuts'
export const KEYBOARD_SHORTCUT_VERSION = 1

export const KEYBOARD_SHORTCUT_COMMANDS = [
  'new',
  'open',
  'save',
  'save_as',
  'undo',
  'redo',
] as const

export type KeyboardShortcutCommand =
  (typeof KEYBOARD_SHORTCUT_COMMANDS)[number]

export const KEYBOARD_SHORTCUT_COMMAND_LABELS: Readonly<
  Record<KeyboardShortcutCommand, string>
> = Object.freeze({
  new: '新規',
  open: '開く',
  save: '保存',
  save_as: '別名保存',
  undo: '元に戻す',
  redo: 'やり直す',
})

export const KEYBOARD_SHORTCUT_KEYS = Object.freeze([
  'a',
  'b',
  'c',
  'd',
  'e',
  'f',
  'g',
  'h',
  'i',
  'j',
  'k',
  'l',
  'm',
  'n',
  'o',
  'p',
  'q',
  'r',
  's',
  't',
  'u',
  'v',
  'w',
  'x',
  'y',
  'z',
  '0',
  '1',
  '2',
  '3',
  '4',
  '5',
  '6',
  '7',
  '8',
  '9',
  'f1',
  'f2',
  'f3',
  'f4',
  'f5',
  'f6',
  'f7',
  'f8',
  'f9',
  'f10',
  'f11',
  'f12',
] as const)

export type KeyboardShortcutKey =
  (typeof KEYBOARD_SHORTCUT_KEYS)[number]

export type PortableKeyboardShortcut = Readonly<{
  key: KeyboardShortcutKey
  alt: boolean
  shift: boolean
}>

export type KeyboardShortcutSnapshot = Readonly<
  Record<KeyboardShortcutCommand, PortableKeyboardShortcut>
>

export type KeyboardShortcutPlatform = 'windows' | 'macos'

export type KeyboardShortcutConflict = Readonly<{
  command: KeyboardShortcutCommand
  conflictingCommand: KeyboardShortcutCommand
  platforms: readonly KeyboardShortcutPlatform[]
  shortcut: PortableKeyboardShortcut
}>

export type SetKeyboardShortcutResult =
  | Readonly<{
    ok: true
    snapshot: KeyboardShortcutSnapshot
  }>
  | Readonly<{
    ok: false
    reason: 'invalid'
    snapshot: KeyboardShortcutSnapshot
  }>
  | Readonly<{
    ok: false
    reason: 'duplicate'
    conflict: KeyboardShortcutConflict
    snapshot: KeyboardShortcutSnapshot
  }>

export type KeyboardShortcutEnvironment = Readonly<{
  readStoredShortcuts: () => unknown
  writeStoredShortcuts: (serialized: string) => void
}>

export type KeyboardShortcutStore = Readonly<{
  initialize: () => KeyboardShortcutSnapshot
  getSnapshot: () => KeyboardShortcutSnapshot
  getServerSnapshot: () => KeyboardShortcutSnapshot
  subscribe: (listener: () => void) => () => void
  setShortcut: (
    command: unknown,
    shortcut: unknown,
  ) => SetKeyboardShortcutResult
  reset: () => void
  dispose: () => void
}>

export type KeyboardShortcutEvent = Readonly<{
  key: string
  code?: string
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
  repeat: boolean
  isComposing: boolean
}>

export const DEFAULT_KEYBOARD_SHORTCUTS: KeyboardShortcutSnapshot =
  freezeSnapshot({
    new: shortcut('n'),
    open: shortcut('o'),
    save: shortcut('s'),
    save_as: shortcut('s', false, true),
    undo: shortcut('z'),
    redo: shortcut('z', false, true),
  })

export function createKeyboardShortcutStore(
  environment: KeyboardShortcutEnvironment,
): KeyboardShortcutStore {
  let initialized = false
  let snapshot = DEFAULT_KEYBOARD_SHORTCUTS
  const listeners = new Set<() => void>()

  const initialize = () => {
    if (initialized) return snapshot
    let stored: unknown = null
    try {
      stored = environment.readStoredShortcuts()
    } catch {
      stored = null
    }
    snapshot = decodeKeyboardShortcuts(stored) ?? DEFAULT_KEYBOARD_SHORTCUTS
    initialized = true
    return snapshot
  }

  const persist = () => {
    try {
      environment.writeStoredShortcuts(encodeKeyboardShortcuts(snapshot))
    } catch {
      // A usable in-memory setting remains active when storage is blocked.
    }
  }

  const replace = (next: KeyboardShortcutSnapshot) => {
    if (keyboardShortcutSnapshotsEqual(snapshot, next)) return
    snapshot = next
    persist()
    for (const listener of [...listeners]) {
      try {
        listener()
      } catch {
        // A broken observer must not prevent later observers from updating.
      }
    }
  }

  return Object.freeze({
    initialize,
    getSnapshot: initialize,
    getServerSnapshot: () => DEFAULT_KEYBOARD_SHORTCUTS,
    subscribe(listener: () => void) {
      initialize()
      if (typeof listener !== 'function') return () => undefined
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    setShortcut(command, candidate) {
      initialize()
      if (!isKeyboardShortcutCommand(command)) {
        return Object.freeze({
          ok: false,
          reason: 'invalid',
          snapshot,
        })
      }
      const normalized = normalizePortableShortcut(candidate)
      if (!normalized) {
        return Object.freeze({
          ok: false,
          reason: 'invalid',
          snapshot,
        })
      }
      const conflict = findKeyboardShortcutConflict(
        snapshot,
        command,
        normalized,
      )
      if (conflict) {
        return Object.freeze({
          ok: false,
          reason: 'duplicate',
          conflict,
          snapshot,
        })
      }
      const next = freezeSnapshot({
        ...snapshot,
        [command]: normalized,
      })
      replace(next)
      return Object.freeze({ ok: true, snapshot })
    },
    reset() {
      initialize()
      replace(DEFAULT_KEYBOARD_SHORTCUTS)
    },
    dispose() {
      listeners.clear()
      initialized = false
      snapshot = DEFAULT_KEYBOARD_SHORTCUTS
    },
  })
}

export function resolveConfiguredKeyboardShortcut(
  value: unknown,
  snapshot: KeyboardShortcutSnapshot,
): KeyboardShortcutCommand | null {
  try {
    if (
      !isRecord(value)
      || !isKeyboardShortcutSnapshot(snapshot)
      || keyboardShortcutSnapshotHasConflict(snapshot)
    ) return null
    if (
      typeof value.altKey !== 'boolean'
      || typeof value.ctrlKey !== 'boolean'
      || typeof value.metaKey !== 'boolean'
      || typeof value.shiftKey !== 'boolean'
      || typeof value.repeat !== 'boolean'
      || typeof value.isComposing !== 'boolean'
      || value.repeat
      || value.isComposing
      || value.ctrlKey === value.metaKey
    ) return null
    const key = normalizeEventKey(value.key)
      ?? (
        value.altKey || value.shiftKey
          ? normalizeEventCode(value.code)
          : null
      )
    if (!key) return null

    const eventShortcut = shortcut(key, value.altKey, value.shiftKey)
    for (const command of KEYBOARD_SHORTCUT_COMMANDS) {
      if (portableShortcutsEqual(snapshot[command], eventShortcut)) {
        return command
      }
    }

    if (
      value.ctrlKey
      && !value.metaKey
      && !value.altKey
      && !value.shiftKey
      && key === 'y'
    ) return 'redo'
    return null
  } catch {
    return null
  }
}

export function findKeyboardShortcutConflict(
  snapshot: KeyboardShortcutSnapshot,
  command: KeyboardShortcutCommand,
  candidate: PortableKeyboardShortcut,
): KeyboardShortcutConflict | null {
  if (
    !isKeyboardShortcutSnapshot(snapshot)
    || !isKeyboardShortcutCommand(command)
  ) return null
  const normalized = normalizePortableShortcut(candidate)
  if (!normalized) return null

  for (const conflictingCommand of KEYBOARD_SHORTCUT_COMMANDS) {
    if (conflictingCommand === command) continue
    const platforms = conflictingPlatforms(
      normalized,
      snapshot[conflictingCommand],
    )
    if (platforms.length > 0) {
      return freezeConflict({
        command,
        conflictingCommand,
        platforms,
        shortcut: normalized,
      })
    }
  }

  if (
    command !== 'redo'
    && normalized.key === 'y'
    && !normalized.alt
    && !normalized.shift
  ) {
    return freezeConflict({
      command,
      conflictingCommand: 'redo',
      platforms: ['windows'],
      shortcut: normalized,
    })
  }
  return null
}

export function encodeKeyboardShortcuts(
  snapshot: KeyboardShortcutSnapshot,
): string {
  if (!isKeyboardShortcutSnapshot(snapshot)) {
    throw new TypeError('invalid keyboard shortcut snapshot')
  }
  return JSON.stringify({
    version: KEYBOARD_SHORTCUT_VERSION,
    assignments: Object.fromEntries(
      KEYBOARD_SHORTCUT_COMMANDS.map((command) => [
        command,
        snapshot[command],
      ]),
    ),
  })
}

export function decodeKeyboardShortcuts(
  serialized: unknown,
): KeyboardShortcutSnapshot | null {
  if (
    typeof serialized !== 'string'
    || serialized.length === 0
    || serialized.length > MAX_STORED_SHORTCUT_BYTES
    || utf8ByteLength(serialized) > MAX_STORED_SHORTCUT_BYTES
  ) return null
  try {
    const wire: unknown = JSON.parse(serialized)
    if (
      !hasExactKeys(wire, ['version', 'assignments'])
      || wire.version !== KEYBOARD_SHORTCUT_VERSION
    ) return null
    const wireAssignments = wire.assignments
    if (!hasExactKeys(wireAssignments, KEYBOARD_SHORTCUT_COMMANDS)) return null

    const assignments = Object.fromEntries(
      KEYBOARD_SHORTCUT_COMMANDS.map((command) => [
        command,
        normalizePortableShortcut(wireAssignments[command]),
      ]),
    )
    if (!isKeyboardShortcutSnapshot(assignments)) return null
    const snapshot = freezeSnapshot(assignments)
    for (const command of KEYBOARD_SHORTCUT_COMMANDS) {
      if (findKeyboardShortcutConflict(snapshot, command, snapshot[command])) {
        return null
      }
    }
    return snapshot
  } catch {
    return null
  }
}

export function keyboardShortcutAriaValue(
  command: KeyboardShortcutCommand,
  snapshot: KeyboardShortcutSnapshot,
): string {
  if (!isKeyboardShortcutCommand(command) || !isKeyboardShortcutSnapshot(snapshot)) {
    return ''
  }
  const portable = snapshot[command]
  const values = [
    ariaChord('Control', portable),
    ariaChord('Meta', portable),
  ]
  if (command === 'redo') values.push('Control+Y')
  return [...new Set(values)].join(' ')
}

export function keyboardShortcutDisplayValue(
  command: KeyboardShortcutCommand,
  snapshot: KeyboardShortcutSnapshot,
): string {
  if (!isKeyboardShortcutCommand(command) || !isKeyboardShortcutSnapshot(snapshot)) {
    return ''
  }
  const value = snapshot[command]
  const modifiers = ['Ctrl/Cmd']
  if (value.alt) modifiers.push('Alt')
  if (value.shift) modifiers.push('Shift')
  modifiers.push(displayKey(value.key))
  const primary = modifiers.join('+')
  return command === 'redo' && primary !== 'Ctrl/Cmd+Y'
    ? `${primary} / Ctrl+Y`
    : primary
}

function normalizePortableShortcut(
  value: unknown,
): PortableKeyboardShortcut | null {
  try {
    if (
      !hasExactKeys(value, ['key', 'alt', 'shift'])
      || typeof value.key !== 'string'
      || !isKeyboardShortcutKey(value.key)
      || typeof value.alt !== 'boolean'
      || typeof value.shift !== 'boolean'
    ) return null
    return shortcut(value.key, value.alt, value.shift)
  } catch {
    return null
  }
}

function normalizeEventKey(value: unknown): KeyboardShortcutKey | null {
  if (
    typeof value !== 'string'
    || value.length === 0
    || value.length > MAX_EVENT_KEY_LENGTH
  ) return null
  const normalized = value.toLowerCase()
  return isKeyboardShortcutKey(normalized) ? normalized : null
}

function normalizeEventCode(value: unknown): KeyboardShortcutKey | null {
  if (
    typeof value !== 'string'
    || value.length === 0
    || value.length > MAX_EVENT_CODE_LENGTH
  ) return null
  const letter = /^Key([A-Z])$/u.exec(value)
  if (letter) return letter[1].toLowerCase() as KeyboardShortcutKey
  const digit = /^Digit([0-9])$/u.exec(value)
  if (digit) return digit[1] as KeyboardShortcutKey
  const functionKey = /^F([1-9]|1[0-2])$/u.exec(value)
  return functionKey
    ? `f${functionKey[1]}` as KeyboardShortcutKey
    : null
}

function shortcut(
  key: KeyboardShortcutKey,
  alt = false,
  shift = false,
): PortableKeyboardShortcut {
  return Object.freeze({ key, alt, shift })
}

function freezeSnapshot(
  value: Record<KeyboardShortcutCommand, PortableKeyboardShortcut>,
): KeyboardShortcutSnapshot {
  return Object.freeze(Object.fromEntries(
    KEYBOARD_SHORTCUT_COMMANDS.map((command) => [
      command,
      shortcut(
        value[command].key,
        value[command].alt,
        value[command].shift,
      ),
    ]),
  )) as KeyboardShortcutSnapshot
}

function freezeConflict(
  value: KeyboardShortcutConflict,
): KeyboardShortcutConflict {
  return Object.freeze({
    ...value,
    platforms: Object.freeze([...value.platforms]),
    shortcut: shortcut(
      value.shortcut.key,
      value.shortcut.alt,
      value.shortcut.shift,
    ),
  })
}

function isKeyboardShortcutSnapshot(
  value: unknown,
): value is KeyboardShortcutSnapshot {
  try {
    if (!hasExactKeys(value, KEYBOARD_SHORTCUT_COMMANDS)) return false
    return KEYBOARD_SHORTCUT_COMMANDS.every(
      (command) => normalizePortableShortcut(value[command]) !== null,
    )
  } catch {
    return false
  }
}

function isKeyboardShortcutCommand(
  value: unknown,
): value is KeyboardShortcutCommand {
  return typeof value === 'string'
    && (KEYBOARD_SHORTCUT_COMMANDS as readonly string[]).includes(value)
}

function isKeyboardShortcutKey(
  value: string,
): value is KeyboardShortcutKey {
  return (KEYBOARD_SHORTCUT_KEYS as readonly string[]).includes(value)
}

function conflictingPlatforms(
  left: PortableKeyboardShortcut,
  right: PortableKeyboardShortcut,
): readonly KeyboardShortcutPlatform[] {
  return portableShortcutsEqual(left, right)
    ? Object.freeze(['windows', 'macos'] as const)
    : Object.freeze([])
}

function portableShortcutsEqual(
  left: PortableKeyboardShortcut,
  right: PortableKeyboardShortcut,
) {
  return left.key === right.key
    && left.alt === right.alt
    && left.shift === right.shift
}

function ariaChord(
  primary: 'Control' | 'Meta',
  value: PortableKeyboardShortcut,
) {
  const tokens: string[] = [primary]
  if (value.alt) tokens.push('Alt')
  if (value.shift) tokens.push('Shift')
  tokens.push(displayKey(value.key))
  return tokens.join('+')
}

function displayKey(key: KeyboardShortcutKey) {
  return key.toUpperCase()
}

function keyboardShortcutSnapshotsEqual(
  left: KeyboardShortcutSnapshot,
  right: KeyboardShortcutSnapshot,
) {
  return KEYBOARD_SHORTCUT_COMMANDS.every(
    (command) => portableShortcutsEqual(left[command], right[command]),
  )
}

function keyboardShortcutSnapshotHasConflict(
  snapshot: KeyboardShortcutSnapshot,
) {
  return KEYBOARD_SHORTCUT_COMMANDS.some(
    (command) => findKeyboardShortcutConflict(
      snapshot,
      command,
      snapshot[command],
    ) !== null,
  )
}

function hasExactKeys<T extends string>(
  value: unknown,
  expected: readonly T[],
): value is Record<T, unknown> {
  try {
    if (!isRecord(value)) return false
    const keys = Object.keys(value)
    return keys.length === expected.length
      && expected.every((key) => keys.includes(key))
  } catch {
    return false
  }
}

function isRecord(value: unknown): value is Record<PropertyKey, unknown> {
  try {
    return typeof value === 'object' && value !== null && !Array.isArray(value)
  } catch {
    return false
  }
}

function utf8ByteLength(value: string) {
  let bytes = 0
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0
    bytes += codePoint <= 0x7f
      ? 1
      : codePoint <= 0x7ff
        ? 2
        : codePoint <= 0xffff ? 3 : 4
  }
  return bytes
}

const browserKeyboardShortcutEnvironment: KeyboardShortcutEnvironment = {
  readStoredShortcuts() {
    if (typeof window === 'undefined') return null
    return window.localStorage.getItem(KEYBOARD_SHORTCUT_STORAGE_KEY)
  },
  writeStoredShortcuts(serialized) {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(KEYBOARD_SHORTCUT_STORAGE_KEY, serialized)
  },
}

export const keyboardShortcutStore = createKeyboardShortcutStore(
  browserKeyboardShortcutEnvironment,
)
