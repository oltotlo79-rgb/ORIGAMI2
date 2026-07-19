import { useSyncExternalStore } from 'react'

export const LOCALE_STORAGE_KEY = 'origami2.locale'
export const SUPPORTED_LOCALES = ['ja', 'en'] as const
export const DEFAULT_LOCALE = 'ja' as const

export type Locale = (typeof SUPPORTED_LOCALES)[number]

export type LocaleSnapshot = Readonly<{
  locale: Locale
}>

export type LocaleEnvironment = Readonly<{
  readStoredLocale: () => unknown
  writeStoredLocale: (locale: Locale) => void
  applyDocumentLanguage: (locale: Locale) => void
}>

export type LocaleStore = Readonly<{
  initialize: () => LocaleSnapshot
  getSnapshot: () => LocaleSnapshot
  getServerSnapshot: () => LocaleSnapshot
  subscribe: (listener: () => void) => () => void
  setLocale: (locale: unknown) => boolean
  dispose: () => void
}>

export type LocalizedText = Readonly<Record<Locale, string>>
export type MessageVariable = string | number
export type MessageVariables = Readonly<Record<string, MessageVariable>>

const LOCALE_SNAPSHOTS: Readonly<Record<Locale, LocaleSnapshot>> =
  Object.freeze({
    ja: Object.freeze({ locale: 'ja' }),
    en: Object.freeze({ locale: 'en' }),
  })

const EMPTY_MESSAGE_VARIABLES: MessageVariables = Object.freeze({})
const MESSAGE_PLACEHOLDER = /\{([A-Za-z][A-Za-z0-9_]*)\}/gu

export function isLocale(value: unknown): value is Locale {
  return value === 'ja' || value === 'en'
}

export function selectLocalizedText(
  locale: unknown,
  text: LocalizedText,
): string {
  return text[isLocale(locale) ? locale : DEFAULT_LOCALE]
}

/**
 * Formats plain UI text. Only own data properties with identifier-like keys
 * are read; accessors, inherited values, objects, and non-finite numbers are
 * left as visible placeholders.
 */
export function formatMessage(
  template: string,
  variables: MessageVariables = EMPTY_MESSAGE_VARIABLES,
): string {
  const container: unknown = variables
  if (
    container === null
    || (typeof container !== 'object' && typeof container !== 'function')
  ) {
    return template
  }

  return template.replace(
    MESSAGE_PLACEHOLDER,
    (placeholder, key: string) => {
      let descriptor: PropertyDescriptor | undefined
      try {
        descriptor = Object.getOwnPropertyDescriptor(container, key)
      } catch {
        return placeholder
      }
      if (!descriptor || !('value' in descriptor)) return placeholder

      const value: unknown = descriptor.value
      if (typeof value === 'string') return value
      if (typeof value === 'number' && Number.isFinite(value)) {
        return String(value)
      }
      return placeholder
    },
  )
}

export function formatLocalizedText(
  locale: unknown,
  text: LocalizedText,
  variables: MessageVariables = EMPTY_MESSAGE_VARIABLES,
): string {
  return formatMessage(selectLocalizedText(locale, text), variables)
}

export function createLocaleStore(
  environment: LocaleEnvironment,
): LocaleStore {
  let initialized = false
  let snapshot = LOCALE_SNAPSHOTS[DEFAULT_LOCALE]
  const listeners = new Set<() => void>()

  const notify = () => {
    for (const listener of [...listeners]) listener()
  }

  const applyDocumentLanguage = (locale: Locale) => {
    try {
      environment.applyDocumentLanguage(locale)
    } catch {
      // Language settings must never prevent the editor from starting.
    }
  }

  const initialize = () => {
    if (initialized) return snapshot

    let storedLocale: unknown = null
    try {
      storedLocale = environment.readStoredLocale()
    } catch {
      storedLocale = null
    }
    const locale = isLocale(storedLocale) ? storedLocale : DEFAULT_LOCALE
    snapshot = LOCALE_SNAPSHOTS[locale]
    initialized = true
    applyDocumentLanguage(locale)
    return snapshot
  }

  const setLocale = (locale: unknown) => {
    if (!isLocale(locale)) return false
    initialize()

    try {
      environment.writeStoredLocale(locale)
    } catch {
      // The active session still changes when persistence is unavailable.
    }

    if (snapshot.locale === locale) return true
    snapshot = LOCALE_SNAPSHOTS[locale]
    applyDocumentLanguage(locale)
    notify()
    return true
  }

  const dispose = () => {
    listeners.clear()
    initialized = false
    snapshot = LOCALE_SNAPSHOTS[DEFAULT_LOCALE]
  }

  return Object.freeze({
    initialize,
    getSnapshot: () => initialize(),
    getServerSnapshot: () => LOCALE_SNAPSHOTS[DEFAULT_LOCALE],
    subscribe(listener: () => void) {
      initialize()
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    setLocale,
    dispose,
  })
}

const browserLocaleEnvironment: LocaleEnvironment = {
  readStoredLocale() {
    if (typeof window === 'undefined') return null
    return window.localStorage.getItem(LOCALE_STORAGE_KEY)
  },
  writeStoredLocale(locale) {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale)
  },
  applyDocumentLanguage(locale) {
    if (typeof document === 'undefined') return
    document.documentElement.lang = locale
  },
}

export const localeStore = createLocaleStore(browserLocaleEnvironment)

export function initializeLocaleStore() {
  return localeStore.initialize()
}

export function useLocale(store: LocaleStore = localeStore): Locale {
  return useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getServerSnapshot,
  ).locale
}
