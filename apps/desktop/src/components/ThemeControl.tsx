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
import { THEME_CONTROL_TEXT } from '../lib/themeControlText'

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
      <span className="theme-control-label">{text(THEME_CONTROL_TEXT.label)}</span>
      <select
        aria-label={text(THEME_CONTROL_TEXT.ariaLabel)}
        value={snapshot.preference}
        onChange={(event) => {
          const preference = event.currentTarget.value
          if (isThemePreference(preference)) {
            store.setPreference(preference)
          }
        }}
      >
        <option value="system">{text(THEME_CONTROL_TEXT.system)}</option>
        <option value="light">{text(THEME_CONTROL_TEXT.light)}</option>
        <option value="dark">{text(THEME_CONTROL_TEXT.dark)}</option>
      </select>
      <output
        className="theme-effective"
        role="status"
        aria-label={text(THEME_CONTROL_TEXT.effectiveAriaLabel)}
        aria-live="polite"
      >
        {text(THEME_CONTROL_TEXT.current)}
        {' '}
        {snapshot.effectiveTheme === 'dark'
          ? text(THEME_CONTROL_TEXT.dark)
          : text(THEME_CONTROL_TEXT.light)}
      </output>
    </label>
  )
}
