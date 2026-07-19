export const THEME_STORAGE_KEY = 'origami2.theme'

export const THEME_PREFERENCES = [
  'system',
  'light',
  'dark',
] as const

export type ThemePreference = (typeof THEME_PREFERENCES)[number]
export type EffectiveTheme = Exclude<ThemePreference, 'system'>

export type ThemeSnapshot = Readonly<{
  preference: ThemePreference
  effectiveTheme: EffectiveTheme
}>

export type ThemeMediaChangeListener = (
  event: Readonly<{ matches: boolean }>,
) => void

export type ThemeMediaQuery = Readonly<{
  matches: boolean
  addEventListener: (
    type: 'change',
    listener: ThemeMediaChangeListener,
  ) => void
  removeEventListener: (
    type: 'change',
    listener: ThemeMediaChangeListener,
  ) => void
}>

export type ThemeEnvironment = Readonly<{
  readStoredPreference: () => unknown
  writeStoredPreference: (preference: ThemePreference) => void
  getSystemTheme: () => ThemeMediaQuery | null
  applyEffectiveTheme: (theme: EffectiveTheme) => void
}>

export type ThemeStore = Readonly<{
  initialize: () => ThemeSnapshot
  getSnapshot: () => ThemeSnapshot
  getServerSnapshot: () => ThemeSnapshot
  subscribe: (listener: () => void) => () => void
  setPreference: (preference: unknown) => boolean
  dispose: () => void
}>

const DEFAULT_THEME_SNAPSHOT: ThemeSnapshot = Object.freeze({
  preference: 'system',
  effectiveTheme: 'light',
})

export function isThemePreference(value: unknown): value is ThemePreference {
  return value === 'system' || value === 'light' || value === 'dark'
}

export function createThemeStore(
  environment: ThemeEnvironment,
): ThemeStore {
  let initialized = false
  let mediaQuery: ThemeMediaQuery | null = null
  let mediaListening = false
  let snapshot = DEFAULT_THEME_SNAPSHOT
  const listeners = new Set<() => void>()

  const notify = () => {
    for (const listener of [...listeners]) listener()
  }

  const applyEffectiveTheme = (theme: EffectiveTheme) => {
    try {
      environment.applyEffectiveTheme(theme)
    } catch {
      // A theme failure must never prevent the editor from starting.
    }
  }

  const detachSystemListener = () => {
    if (!mediaQuery || !mediaListening) return
    try {
      mediaQuery.removeEventListener('change', handleSystemThemeChange)
    } catch {
      // A partially implemented host may reject listener removal.
    } finally {
      mediaListening = false
    }
  }

  const attachSystemListener = () => {
    if (!mediaQuery || mediaListening || snapshot.preference !== 'system') {
      return
    }
    try {
      mediaQuery.addEventListener('change', handleSystemThemeChange)
      mediaListening = true
    } catch {
      mediaListening = false
    }
  }

  function handleSystemThemeChange(
    event: Readonly<{ matches: boolean }>,
  ) {
    if (!initialized || snapshot.preference !== 'system') return
    const effectiveTheme = event.matches ? 'dark' : 'light'
    if (snapshot.effectiveTheme === effectiveTheme) return
    snapshot = Object.freeze({
      preference: 'system',
      effectiveTheme,
    })
    applyEffectiveTheme(effectiveTheme)
    notify()
  }

  const initialize = () => {
    if (initialized) return snapshot

    let storedPreference: unknown = null
    try {
      storedPreference = environment.readStoredPreference()
    } catch {
      storedPreference = null
    }
    const preference = isThemePreference(storedPreference)
      ? storedPreference
      : 'system'

    try {
      mediaQuery = environment.getSystemTheme()
    } catch {
      mediaQuery = null
    }
    const effectiveTheme = preference === 'system'
      ? mediaQuery?.matches === true ? 'dark' : 'light'
      : preference
    snapshot = Object.freeze({ preference, effectiveTheme })
    initialized = true
    applyEffectiveTheme(effectiveTheme)
    attachSystemListener()
    return snapshot
  }

  const setPreference = (preference: unknown) => {
    if (!isThemePreference(preference)) return false
    initialize()

    try {
      environment.writeStoredPreference(preference)
    } catch {
      // The active session still changes when persistence is unavailable.
    }

    const effectiveTheme = preference === 'system'
      ? mediaQuery?.matches === true ? 'dark' : 'light'
      : preference
    const changed = snapshot.preference !== preference
      || snapshot.effectiveTheme !== effectiveTheme
    snapshot = Object.freeze({ preference, effectiveTheme })

    if (preference === 'system') {
      attachSystemListener()
    } else {
      detachSystemListener()
    }
    if (changed) {
      applyEffectiveTheme(effectiveTheme)
      notify()
    }
    return true
  }

  const dispose = () => {
    detachSystemListener()
    listeners.clear()
    mediaQuery = null
    initialized = false
    snapshot = DEFAULT_THEME_SNAPSHOT
  }

  return Object.freeze({
    initialize,
    getSnapshot: () => initialize(),
    getServerSnapshot: () => DEFAULT_THEME_SNAPSHOT,
    subscribe(listener: () => void) {
      initialize()
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    setPreference,
    dispose,
  })
}

const browserThemeEnvironment: ThemeEnvironment = {
  readStoredPreference() {
    if (typeof window === 'undefined') return null
    return window.localStorage.getItem(THEME_STORAGE_KEY)
  },
  writeStoredPreference(preference) {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(THEME_STORAGE_KEY, preference)
  },
  getSystemTheme() {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return null
    }
    return window.matchMedia('(prefers-color-scheme: dark)')
  },
  applyEffectiveTheme(theme) {
    if (typeof document === 'undefined') return
    document.documentElement.dataset.theme = theme
  },
}

export const themeStore = createThemeStore(browserThemeEnvironment)

export function initializeTheme() {
  return themeStore.initialize()
}
