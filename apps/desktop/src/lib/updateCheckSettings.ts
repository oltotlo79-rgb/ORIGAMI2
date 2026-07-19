export const UPDATE_CHECK_SETTINGS_STORAGE_KEY =
  'origami2.update-check-settings'
export const UPDATE_CHECK_SETTINGS_VERSION = 1

const MAX_STORED_UPDATE_CHECK_SETTINGS_CODE_UNITS = 256

export type UpdateCheckSettingsSnapshot = Readonly<{
  enabled: boolean
}>

export const DEFAULT_UPDATE_CHECK_SETTINGS: UpdateCheckSettingsSnapshot =
  Object.freeze({ enabled: true })

export const DISABLED_UPDATE_CHECK_SETTINGS: UpdateCheckSettingsSnapshot =
  Object.freeze({ enabled: false })

export type UpdateCheckSettingsEnvironment = Readonly<{
  readStoredSettings: () => unknown
  writeStoredSettings: (serialized: string) => void
}>

export type UpdateCheckSettingsChangeResult =
  | Readonly<{
    ok: true
    persisted: boolean
    snapshot: UpdateCheckSettingsSnapshot
  }>
  | Readonly<{
    ok: false
    reason: 'invalid'
    snapshot: UpdateCheckSettingsSnapshot
  }>

export type UpdateCheckSettingsStore = Readonly<{
  initialize: () => UpdateCheckSettingsSnapshot
  getSnapshot: () => UpdateCheckSettingsSnapshot
  getServerSnapshot: () => UpdateCheckSettingsSnapshot
  subscribe: (listener: () => void) => () => void
  setEnabled: (enabled: unknown) => UpdateCheckSettingsChangeResult
  reset: () => UpdateCheckSettingsChangeResult
  dispose: () => void
}>

/**
 * Owns only the terminal-local preference. It never schedules or performs an
 * update check. A missing value uses the product default; unreadable or
 * malformed persisted data fails closed to disabled so a corrupt preference
 * cannot unexpectedly initiate future network access.
 */
export function createUpdateCheckSettingsStore(
  environment: UpdateCheckSettingsEnvironment,
): UpdateCheckSettingsStore {
  let initialized = false
  let snapshot = DEFAULT_UPDATE_CHECK_SETTINGS
  const listeners = new Set<() => void>()

  const initialize = () => {
    if (initialized) return snapshot

    let stored: unknown
    let storageReadable = true
    try {
      stored = environment.readStoredSettings()
    } catch {
      stored = null
      storageReadable = false
    }

    if (!storageReadable) {
      snapshot = DISABLED_UPDATE_CHECK_SETTINGS
    } else if (stored === null || stored === undefined) {
      snapshot = DEFAULT_UPDATE_CHECK_SETTINGS
    } else {
      snapshot = decodeUpdateCheckSettings(stored)
        ?? DISABLED_UPDATE_CHECK_SETTINGS
    }
    initialized = true
    return snapshot
  }

  const notify = () => {
    for (const listener of [...listeners]) {
      try {
        listener()
      } catch {
        // A broken observer cannot block saving or later observers.
      }
    }
  }

  const replace = (
    next: UpdateCheckSettingsSnapshot,
  ): UpdateCheckSettingsChangeResult => {
    initialize()
    const changed = snapshot.enabled !== next.enabled
    snapshot = next

    let persisted = false
    try {
      environment.writeStoredSettings(encodeUpdateCheckSettings(snapshot))
      persisted = true
    } catch {
      // The in-memory choice remains authoritative for this renderer session.
    }

    if (changed) notify()
    return Object.freeze({
      ok: true,
      persisted,
      snapshot,
    })
  }

  return Object.freeze({
    initialize,
    getSnapshot: initialize,
    getServerSnapshot: () => DEFAULT_UPDATE_CHECK_SETTINGS,
    subscribe(listener: () => void) {
      initialize()
      if (typeof listener !== 'function') return () => undefined
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    setEnabled(enabled: unknown) {
      initialize()
      if (typeof enabled !== 'boolean') {
        return Object.freeze({
          ok: false,
          reason: 'invalid',
          snapshot,
        })
      }
      return replace(
        enabled
          ? DEFAULT_UPDATE_CHECK_SETTINGS
          : DISABLED_UPDATE_CHECK_SETTINGS,
      )
    },
    reset() {
      return replace(DEFAULT_UPDATE_CHECK_SETTINGS)
    },
    dispose() {
      listeners.clear()
      initialized = false
      snapshot = DEFAULT_UPDATE_CHECK_SETTINGS
    },
  })
}

export function encodeUpdateCheckSettings(
  snapshot: UpdateCheckSettingsSnapshot,
): string {
  return JSON.stringify({
    version: UPDATE_CHECK_SETTINGS_VERSION,
    enabled: snapshot.enabled,
  })
}

export function decodeUpdateCheckSettings(
  serialized: unknown,
): UpdateCheckSettingsSnapshot | null {
  if (
    typeof serialized !== 'string'
    || serialized.length === 0
    || serialized.length > MAX_STORED_UPDATE_CHECK_SETTINGS_CODE_UNITS
  ) return null

  try {
    const parsed: unknown = JSON.parse(serialized)
    const record = exactDataRecord(parsed, ['version', 'enabled'])
    if (
      !record
      || record.version !== UPDATE_CHECK_SETTINGS_VERSION
      || typeof record.enabled !== 'boolean'
    ) return null
    return record.enabled
      ? DEFAULT_UPDATE_CHECK_SETTINGS
      : DISABLED_UPDATE_CHECK_SETTINGS
  } catch {
    return null
  }
}

export function isUpdateCheckSettingsSnapshot(
  value: unknown,
): value is UpdateCheckSettingsSnapshot {
  try {
    const record = exactDataRecord(value, ['enabled'])
    return record !== null && typeof record.enabled === 'boolean'
  } catch {
    return false
  }
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  keys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  if (
    value === null
    || typeof value !== 'object'
    || Array.isArray(value)
  ) return null
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const actualKeys = Reflect.ownKeys(descriptors)
  if (
    actualKeys.length !== keys.length
    || actualKeys.some((key) => typeof key !== 'string')
    || keys.some((key) => !Object.hasOwn(descriptors, key))
  ) return null

  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of keys) {
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot as Readonly<Record<Keys[number], unknown>>
}

const browserUpdateCheckSettingsEnvironment:
UpdateCheckSettingsEnvironment = Object.freeze({
  readStoredSettings() {
    if (typeof window === 'undefined') return null
    return window.localStorage.getItem(UPDATE_CHECK_SETTINGS_STORAGE_KEY)
  },
  writeStoredSettings(serialized) {
    if (typeof window === 'undefined') return
    window.localStorage.setItem(
      UPDATE_CHECK_SETTINGS_STORAGE_KEY,
      serialized,
    )
  },
})

export const updateCheckSettingsStore = createUpdateCheckSettingsStore(
  browserUpdateCheckSettingsEnvironment,
)
