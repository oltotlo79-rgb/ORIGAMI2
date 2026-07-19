import {
  createLocaleStore,
  type Locale,
  type LocaleStore,
} from '../src/lib/i18n.ts'

export function localeFixture(locale: Locale): LocaleStore {
  const store = createLocaleStore({
    readStoredLocale: () => locale,
    writeStoredLocale: () => undefined,
    applyDocumentLanguage: () => undefined,
  })
  store.initialize()
  return store
}
