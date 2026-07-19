import {
  isLocale,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n'

type LanguageControlProps = Readonly<{
  store?: LocaleStore
}>

const LABEL = Object.freeze({
  ja: '表示言語',
  en: 'Display language',
})

export function LanguageControl({
  store = localeStore,
}: LanguageControlProps) {
  const locale = useLocale(store)
  const label = selectLocalizedText(locale, LABEL)

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
        <option value="ja" lang="ja">日本語</option>
        <option value="en" lang="en">English</option>
      </select>
    </label>
  )
}
