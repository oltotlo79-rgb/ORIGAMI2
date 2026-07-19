import { useState, useSyncExternalStore } from 'react'

import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n'
import {
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
  localeStore?: LocaleStore
}>

export function KeyboardShortcutControl({
  store = keyboardShortcutStore,
  localeStore: localeStore_ = localeStore,
}: KeyboardShortcutControlProps) {
  const locale = useLocale(localeStore_)
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const snapshot = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  )
  const [shortcutResult, setShortcutResult] =
    useState<SetKeyboardShortcutResult | null>(null)
  const error = shortcutResult
    ? shortcutResultError(shortcutResult, locale)
    : null

  const update = (
    command: KeyboardShortcutCommand,
    candidate: PortableKeyboardShortcut,
  ) => {
    const result = store.setShortcut(command, candidate)
    setShortcutResult(result.ok ? null : result)
  }

  return (
    <details className="keyboard-shortcut-control">
      <summary>{text(KEYBOARD_SHORTCUT_TEXT.summary)}</summary>
      <div
        className="keyboard-shortcut-menu"
        role="group"
        aria-label={text(KEYBOARD_SHORTCUT_TEXT.groupAriaLabel)}
      >
        <p>
          {text(KEYBOARD_SHORTCUT_TEXT.description)}
        </p>
        <div className="keyboard-shortcut-list">
          {KEYBOARD_SHORTCUT_COMMANDS.map((command) => {
            const value = snapshot[command]
            const label = text(KEYBOARD_SHORTCUT_COMMAND_LABELS[command])
            const keyAriaLabel = formatLocalizedText(
              locale,
              KEYBOARD_SHORTCUT_TEXT.keyAriaLabel,
              { command: label },
            )
            return (
              <fieldset key={command}>
                <legend>{label}</legend>
                <span className="keyboard-shortcut-primary">Ctrl/Cmd +</span>
                <label>
                  <span className="visually-hidden">{keyAriaLabel}</span>
                  <select
                    aria-label={keyAriaLabel}
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
                    aria-label={formatLocalizedText(
                      locale,
                      KEYBOARD_SHORTCUT_TEXT.useAltAriaLabel,
                      { command: label },
                    )}
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
                    aria-label={formatLocalizedText(
                      locale,
                      KEYBOARD_SHORTCUT_TEXT.useShiftAriaLabel,
                      { command: label },
                    )}
                    checked={value.shift}
                    onChange={(event) => update(command, {
                      ...value,
                      shift: event.currentTarget.checked,
                    })}
                  />
                  Shift
                </label>
                <output aria-label={formatLocalizedText(
                  locale,
                  KEYBOARD_SHORTCUT_TEXT.currentAriaLabel,
                  { command: label },
                )}>
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
            setShortcutResult(null)
          }}
        >
          {text(KEYBOARD_SHORTCUT_TEXT.reset)}
        </button>
      </div>
    </details>
  )
}

function shortcutResultError(
  result: SetKeyboardShortcutResult,
  locale: Locale,
): string | null {
  if (result.ok) return null
  if (result.reason === 'invalid') {
    return selectLocalizedText(locale, KEYBOARD_SHORTCUT_TEXT.invalid)
  }
  const platforms = result.conflict.platforms
    .map((platform) => platform === 'windows' ? 'Windows' : 'macOS')
    .join(selectLocalizedText(locale, KEYBOARD_SHORTCUT_TEXT.platformJoin))
  return formatLocalizedText(locale, KEYBOARD_SHORTCUT_TEXT.conflict, {
    command: selectLocalizedText(
      locale,
      KEYBOARD_SHORTCUT_COMMAND_LABELS[result.conflict.command],
    ),
    conflictingCommand: selectLocalizedText(
      locale,
      KEYBOARD_SHORTCUT_COMMAND_LABELS[result.conflict.conflictingCommand],
    ),
    platforms,
  })
}

const KEYBOARD_SHORTCUT_COMMAND_LABELS: Readonly<
  Record<KeyboardShortcutCommand, LocalizedText>
> = Object.freeze({
  new: Object.freeze({ ja: '新規', en: 'New' }),
  open: Object.freeze({ ja: '開く', en: 'Open' }),
  save: Object.freeze({ ja: '保存', en: 'Save' }),
  save_as: Object.freeze({ ja: '別名保存', en: 'Save as' }),
  undo: Object.freeze({ ja: '元に戻す', en: 'Undo' }),
  redo: Object.freeze({ ja: 'やり直す', en: 'Redo' }),
})

const KEYBOARD_SHORTCUT_TEXT = Object.freeze({
  summary: Object.freeze({ ja: 'ショートカット', en: 'Shortcuts' }),
  groupAriaLabel: Object.freeze({
    ja: 'ショートカット設定',
    en: 'Shortcut settings',
  }),
  description: Object.freeze({
    ja: 'Ctrl/Cmdを共通の主キーとして設定します。WindowsのCtrl+Yは「やり直す」として常に利用できます。',
    en: 'Ctrl/Cmd is the shared primary key. Ctrl+Y is always available for Redo on Windows.',
  }),
  keyAriaLabel: Object.freeze({
    ja: '{command}のキー',
    en: '{command} key',
  }),
  useAltAriaLabel: Object.freeze({
    ja: '{command}でAltを使う',
    en: 'Use Alt for {command}',
  }),
  useShiftAriaLabel: Object.freeze({
    ja: '{command}でShiftを使う',
    en: 'Use Shift for {command}',
  }),
  currentAriaLabel: Object.freeze({
    ja: '{command}の現在のショートカット',
    en: 'Current shortcut for {command}',
  }),
  reset: Object.freeze({
    ja: '標準設定に戻す',
    en: 'Restore defaults',
  }),
  invalid: Object.freeze({
    ja: 'このショートカットは設定できません。',
    en: 'This shortcut cannot be assigned.',
  }),
  conflict: Object.freeze({
    ja: '{command}は{conflictingCommand}と重複します（{platforms}）。',
    en: '{command} conflicts with {conflictingCommand} ({platforms}).',
  }),
  platformJoin: Object.freeze({ ja: '・', en: ' / ' }),
})
