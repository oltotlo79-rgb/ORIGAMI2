import {
  isLocale,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n'
import { LANGUAGE_CONTROL_TEXT } from '../lib/languageControlText.ts'

type LanguageControlProps = Readonly<{
  store?: LocaleStore
}>

export function LanguageControl({
  store = localeStore,
}: LanguageControlProps) {
  const locale = useLocale(store)
  const label = selectLocalizedText(locale, LANGUAGE_CONTROL_TEXT.label)

  return (
    <label className="language-control">
      <span className="language-control-label">{label}</span>
      <select
        aria-label={label}
        value={locale}
        onChange={(event) => {
          const nextLocale = event.currentTarget.value
          if (isLocale(nextLocale)) {
            store.setLocale(nextLocale)
          }
        }}
      >
        <option value="ja" lang="ja">{selectLocalizedText(locale, LANGUAGE_CONTROL_TEXT.japanese)}</option>
        <option value="en" lang="en">{selectLocalizedText(locale, LANGUAGE_CONTROL_TEXT.english)}</option>
      </select>
    </label>
  )
}
