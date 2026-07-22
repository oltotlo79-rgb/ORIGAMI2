const STORAGE_KEY = 'origami2.grid-division-preference.v1'

type StorageReader = Pick<Storage, 'getItem'>
type StorageWriter = Pick<Storage, 'setItem'>
type StorageHost = Readonly<{ localStorage: Storage }>

export type GridPreferenceV1 = Readonly<{
  divisions: number | null
  diagonals: boolean
}>

export function loadGridDivisionPreference(storage: StorageReader): GridPreferenceV1 | null {
  try {
    const raw = storage.getItem(STORAGE_KEY)
    if (raw === null || raw.length > 96) return null
    const value: unknown = JSON.parse(raw)
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
      || Object.getPrototypeOf(value) !== Object.prototype
    ) return null
    const record = value as Record<string, unknown>
    if (
      Object.keys(record).length !== 3
      || record.version !== 1
      || typeof record.diagonals !== 'boolean'
      || (record.divisions !== null && (
        !Number.isSafeInteger(record.divisions)
        || (record.divisions as number) < 2
        || (record.divisions as number) > 63
      ))
      || (record.diagonals && record.divisions === null)
    ) return null
    return {
      divisions: record.divisions as number | null,
      diagonals: record.diagonals,
    }
  } catch {
    return null
  }
}

export function saveGridDivisionPreference(
  storage: StorageWriter,
  preference: GridPreferenceV1,
) {
  try {
    if (
      preference === null
      || typeof preference !== 'object'
      || Array.isArray(preference)
      || Object.getPrototypeOf(preference) !== Object.prototype
      || Object.keys(preference).length !== 2
      || typeof preference.diagonals !== 'boolean'
      || (preference.divisions !== null && (
        !Number.isSafeInteger(preference.divisions)
        || preference.divisions < 2
        || preference.divisions > 63
      ))
      || (preference.diagonals && preference.divisions === null)
    ) return false
    storage.setItem(STORAGE_KEY, JSON.stringify({ version: 1, ...preference }))
    return true
  } catch {
    return false
  }
}

export function updateGridPreferenceInput(value: string, diagonals: boolean) {
  if (!/^\d{0,2}$/u.test(value)) return null
  return { input: value, diagonals: value === '' ? false : diagonals }
}

export function loadGridDivisionPreferenceFromHost(host: StorageHost) {
  try {
    return loadGridDivisionPreference(host.localStorage)
  } catch {
    return null
  }
}

export function saveGridDivisionPreferenceToHost(
  host: StorageHost,
  preference: GridPreferenceV1,
) {
  try {
    return saveGridDivisionPreference(host.localStorage, preference)
  } catch {
    return false
  }
}
