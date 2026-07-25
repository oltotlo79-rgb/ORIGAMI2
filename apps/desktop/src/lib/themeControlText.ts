import type { LocalizedText } from './i18n.ts'

export const THEME_CONTROL_TEXT: Readonly<Record<
  | 'label'
  | 'ariaLabel'
  | 'system'
  | 'light'
  | 'dark'
  | 'effectiveAriaLabel'
  | 'current',
  LocalizedText
>> = Object.freeze({
  label: Object.freeze({ ja: 'テーマ', en: 'Theme' }),
  ariaLabel: Object.freeze({ ja: '表示テーマ', en: 'Display theme' }),
  system: Object.freeze({
    ja: 'OS設定に合わせる',
    en: 'Match OS setting',
  }),
  light: Object.freeze({ ja: 'ライト', en: 'Light' }),
  dark: Object.freeze({ ja: 'ダーク', en: 'Dark' }),
  effectiveAriaLabel: Object.freeze({
    ja: '現在の実効テーマ',
    en: 'Current effective theme',
  }),
  current: Object.freeze({ ja: '現在:', en: 'Current:' }),
})
