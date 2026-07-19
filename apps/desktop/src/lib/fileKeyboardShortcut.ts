const MAX_KEY_TOKEN_LENGTH = 64

export type FileKeyboardShortcutCommand =
  | 'new'
  | 'open'
  | 'save'
  | 'save_as'

export type FileKeyboardShortcutEvent = Readonly<{
  key: string
  altKey: boolean
  ctrlKey: boolean
  metaKey: boolean
  shiftKey: boolean
  repeat: boolean
  isComposing: boolean
}>

/**
 * Resolves platform-standard project file shortcuts without inspecting the
 * host operating system. Ctrl and Meta deliberately share the same mapping so
 * Windows and macOS builds exercise one deterministic implementation.
 *
 * The caller remains responsible for ignoring editable targets and for
 * checking whether the resolved command is currently available.
 */
export function resolveFileKeyboardShortcut(
  value: unknown,
): FileKeyboardShortcutCommand | null {
  try {
    if (!isRecord(value)) return null
    const key = value.key
    const altKey = value.altKey
    const ctrlKey = value.ctrlKey
    const metaKey = value.metaKey
    const shiftKey = value.shiftKey
    const repeat = value.repeat
    const isComposing = value.isComposing
    if (
      !validKey(key)
      || typeof altKey !== 'boolean'
      || typeof ctrlKey !== 'boolean'
      || typeof metaKey !== 'boolean'
      || typeof shiftKey !== 'boolean'
      || typeof repeat !== 'boolean'
      || typeof isComposing !== 'boolean'
      || altKey
      || repeat
      || isComposing
      || ctrlKey === metaKey
    ) return null

    switch (key.toLowerCase()) {
      case 'n':
        return shiftKey ? null : 'new'
      case 'o':
        return shiftKey ? null : 'open'
      case 's':
        return shiftKey ? 'save_as' : 'save'
      default:
        return null
    }
  } catch {
    return null
  }
}

function validKey(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_KEY_TOKEN_LENGTH
}

function isRecord(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === 'object' && value !== null
}
