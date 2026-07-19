import { useSyncExternalStore } from 'react'

import {
  isThemePreference,
  themeStore,
  type ThemeStore,
} from '../lib/theme'
import {
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n'

type ThemeControlProps = Readonly<{
  store?: ThemeStore
  localeStore?: LocaleStore
}>

export function ThemeControl({
  store = themeStore,
  localeStore: localeStore_ = localeStore,
}: ThemeControlProps) {
  const locale = useLocale(localeStore_)
  const snapshot = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  )
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)

  return (
    <label className="theme-control">
      <span className="theme-control-label">{text(THEME_TEXT.label)}</span>
      <select
        aria-label={text(THEME_TEXT.ariaLabel)}
        value={snapshot.preference}
        onChange={(event) => {
          const preference = event.currentTarget.value
          if (isThemePreference(preference)) {
            store.setPreference(preference)
          }
        }}
      >
        <option value="system">{text(THEME_TEXT.system)}</option>
        <option value="light">{text(THEME_TEXT.light)}</option>
        <option value="dark">{text(THEME_TEXT.dark)}</option>
      </select>
      <output
        className="theme-effective"
        role="status"
        aria-label={text(THEME_TEXT.effectiveAriaLabel)}
        aria-live="polite"
      >
        {text(THEME_TEXT.current)}
        {' '}
        {snapshot.effectiveTheme === 'dark'
          ? text(THEME_TEXT.dark)
          : text(THEME_TEXT.light)}
      </output>
    </label>
  )
}

const THEME_TEXT = Object.freeze({
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
