import type { LocalizedText } from './i18n.ts'

export const KEYBOARD_SHORTCUT_TEXT: Readonly<Record<
  | 'summary' | 'groupAriaLabel' | 'description' | 'keyAriaLabel'
  | 'useAltAriaLabel' | 'useShiftAriaLabel' | 'currentAriaLabel'
  | 'reset' | 'invalid' | 'conflict' | 'platformJoin',
  LocalizedText
>> = Object.freeze({
  summary: Object.freeze({ ja: 'ショートカット', en: 'Shortcuts' }),
  groupAriaLabel: Object.freeze({ ja: 'ショートカット設定', en: 'Shortcut settings' }),
  description: Object.freeze({
    ja: 'Ctrl/Cmdを共通の主キーとして設定します。WindowsのCtrl+Yは「やり直す」として常に利用できます。',
    en: 'Ctrl/Cmd is the shared primary key. Ctrl+Y is always available for Redo on Windows.',
  }),
  keyAriaLabel: Object.freeze({ ja: '{command}のキー', en: '{command} key' }),
  useAltAriaLabel: Object.freeze({ ja: '{command}でAltを使う', en: 'Use Alt for {command}' }),
  useShiftAriaLabel: Object.freeze({ ja: '{command}でShiftを使う', en: 'Use Shift for {command}' }),
  currentAriaLabel: Object.freeze({
    ja: '{command}の現在のショートカット',
    en: 'Current shortcut for {command}',
  }),
  reset: Object.freeze({ ja: '標準設定に戻す', en: 'Restore defaults' }),
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
