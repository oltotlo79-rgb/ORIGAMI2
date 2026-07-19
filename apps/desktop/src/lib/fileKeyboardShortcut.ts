import {
  DEFAULT_KEYBOARD_SHORTCUTS,
  resolveConfiguredKeyboardShortcut,
} from './keyboardShortcutSettings.ts'

export type FileKeyboardShortcutCommand =
  | 'new'
  | 'open'
  | 'save'
  | 'save_as'

export type FileKeyboardShortcutEvent = Readonly<{
  key: string
  code?: string
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
  const command = resolveConfiguredKeyboardShortcut(
    value,
    DEFAULT_KEYBOARD_SHORTCUTS,
  )
  return command === 'new'
    || command === 'open'
    || command === 'save'
    || command === 'save_as'
    ? command
    : null
}
