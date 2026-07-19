import { useState, useSyncExternalStore } from 'react'

import {
  KEYBOARD_SHORTCUT_COMMAND_LABELS,
  KEYBOARD_SHORTCUT_COMMANDS,
  KEYBOARD_SHORTCUT_KEYS,
  keyboardShortcutDisplayValue,
  keyboardShortcutStore,
  type KeyboardShortcutCommand,
  type KeyboardShortcutKey,
  type KeyboardShortcutStore,
  type PortableKeyboardShortcut,
  type SetKeyboardShortcutResult,
} from '../lib/keyboardShortcutSettings'

type KeyboardShortcutControlProps = Readonly<{
  store?: KeyboardShortcutStore
}>

export function KeyboardShortcutControl({
  store = keyboardShortcutStore,
}: KeyboardShortcutControlProps) {
  const snapshot = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  )
  const [error, setError] = useState<string | null>(null)

  const update = (
    command: KeyboardShortcutCommand,
    candidate: PortableKeyboardShortcut,
  ) => {
    const result = store.setShortcut(command, candidate)
    setError(shortcutResultError(result))
  }

  return (
    <details className="keyboard-shortcut-control">
      <summary>ショートカット</summary>
      <div
        className="keyboard-shortcut-menu"
        role="group"
        aria-label="ショートカット設定"
      >
        <p>
          Ctrl/Cmdを共通の主キーとして設定します。
          WindowsのCtrl+Yは「やり直す」として常に利用できます。
        </p>
        <div className="keyboard-shortcut-list">
          {KEYBOARD_SHORTCUT_COMMANDS.map((command) => {
            const value = snapshot[command]
            const label = KEYBOARD_SHORTCUT_COMMAND_LABELS[command]
            return (
              <fieldset key={command}>
                <legend>{label}</legend>
                <span className="keyboard-shortcut-primary">Ctrl/Cmd +</span>
                <label>
                  <span className="visually-hidden">{label}のキー</span>
                  <select
                    aria-label={`${label}のキー`}
                    value={value.key}
                    onChange={(event) => update(command, {
                      ...value,
                      key: event.currentTarget.value as KeyboardShortcutKey,
                    })}
                  >
                    {KEYBOARD_SHORTCUT_KEYS.map((key) => (
                      <option key={key} value={key}>
                        {key.toUpperCase()}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <input
                    type="checkbox"
                    aria-label={`${label}でAltを使う`}
                    checked={value.alt}
                    onChange={(event) => update(command, {
                      ...value,
                      alt: event.currentTarget.checked,
                    })}
                  />
                  Alt
                </label>
                <label>
                  <input
                    type="checkbox"
                    aria-label={`${label}でShiftを使う`}
                    checked={value.shift}
                    onChange={(event) => update(command, {
                      ...value,
                      shift: event.currentTarget.checked,
                    })}
                  />
                  Shift
                </label>
                <output aria-label={`${label}の現在のショートカット`}>
                  {keyboardShortcutDisplayValue(command, snapshot)}
                </output>
              </fieldset>
            )
          })}
        </div>
        {error && (
          <p className="keyboard-shortcut-error" role="alert">
            {error}
          </p>
        )}
        <button
          type="button"
          onClick={() => {
            store.reset()
            setError(null)
          }}
        >
          標準設定に戻す
        </button>
      </div>
    </details>
  )
}

function shortcutResultError(
  result: SetKeyboardShortcutResult,
): string | null {
  if (result.ok) return null
  if (result.reason === 'invalid') {
    return 'このショートカットは設定できません。'
  }
  const platforms = result.conflict.platforms
    .map((platform) => platform === 'windows' ? 'Windows' : 'macOS')
    .join('・')
  return `${KEYBOARD_SHORTCUT_COMMAND_LABELS[result.conflict.command]}は`
    + `${KEYBOARD_SHORTCUT_COMMAND_LABELS[result.conflict.conflictingCommand]}`
    + `と重複します（${platforms}）。`
}
