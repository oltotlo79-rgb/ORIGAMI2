import { invoke } from '@tauri-apps/api/core'

import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export const HISTORY_LIMIT_SCHEMA_VERSION = 1 as const
export const MIN_HISTORY_ENTRY_LIMIT = 1
export const MAX_HISTORY_ENTRY_LIMIT = 128

const SETTINGS_KEYS = [
  'schemaVersion',
  'projectInstanceId',
  'projectId',
  'revision',
  'historyEntryLimit',
] as const

const EXPECTED_BINDING_KEYS = [
  'expectedProjectInstanceId',
  'expectedProjectId',
  'expectedRevision',
] as const

const SET_REQUEST_KEYS = [
  'schemaVersion',
  ...EXPECTED_BINDING_KEYS,
  'historyEntryLimit',
] as const

export type HistoryLimitSettings = Readonly<{
  schemaVersion: typeof HISTORY_LIMIT_SCHEMA_VERSION
  projectInstanceId: string
  projectId: string
  revision: number
  historyEntryLimit: number
}>

export type HistoryLimitExpectedProjectBinding = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
}>

export type SetHistoryEntryLimitRequest =
  & HistoryLimitExpectedProjectBinding
  & Readonly<{
    schemaVersion: typeof HISTORY_LIMIT_SCHEMA_VERSION
    historyEntryLimit: number
  }>

export type HistoryLimitNativeInvoke = (
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) => unknown

export type HistoryLimitClient = Readonly<{
  get(
    expected: HistoryLimitExpectedProjectBinding,
  ): Promise<HistoryLimitSettings>
  set(request: SetHistoryEntryLimitRequest): Promise<HistoryLimitSettings>
}>

export type HistoryLimitClientErrorCode =
  | 'invalid_request'
  | 'native_unavailable'
  | 'invalid_response'
  | 'stale_response'

const ERROR_MESSAGES: Readonly<Record<HistoryLimitClientErrorCode, string>> = {
  invalid_request: '履歴設定の変更条件が正しくありません。',
  native_unavailable: '履歴設定をデスクトップ機能で処理できませんでした。',
  invalid_response: '履歴設定の応答を確認できませんでした。',
  stale_response: '現在とは異なるプロジェクト状態の履歴設定応答を拒否しました。',
}

/**
 * A redacted boundary error. Native rejection values and malformed DTO data
 * are never retained as a message, cause, or public field.
 */
export class HistoryLimitClientError extends Error {
  readonly code: HistoryLimitClientErrorCode

  constructor(code: HistoryLimitClientErrorCode) {
    super(ERROR_MESSAGES[code])
    this.name = 'HistoryLimitClientError'
    this.code = code
  }
}

export function createHistoryLimitClient(
  nativeInvoke: HistoryLimitNativeInvoke = defaultNativeInvoke,
): HistoryLimitClient {
  return Object.freeze({
    async get(expected) {
      const binding = normalizeExpectedBinding(expected)
      if (!binding) throw new HistoryLimitClientError('invalid_request')

      let raw: unknown
      try {
        raw = await Promise.resolve(
          nativeInvoke('get_history_entry_limit'),
        )
      } catch {
        throw new HistoryLimitClientError('native_unavailable')
      }

      const settings = parseHistoryLimitSettings(raw)
      if (!settings) throw new HistoryLimitClientError('invalid_response')
      if (!settingsMatchExpectedBinding(settings, binding)) {
        throw new HistoryLimitClientError('stale_response')
      }
      return settings
    },

    async set(candidate) {
      const request = normalizeSetRequest(candidate)
      if (!request) throw new HistoryLimitClientError('invalid_request')

      let raw: unknown
      try {
        raw = await Promise.resolve(nativeInvoke(
          'set_history_entry_limit',
          Object.freeze({ request }),
        ))
      } catch {
        throw new HistoryLimitClientError('native_unavailable')
      }

      const settings = parseHistoryLimitSettings(raw)
      if (!settings) throw new HistoryLimitClientError('invalid_response')
      if (
        !settingsMatchExpectedBinding(settings, request)
        || settings.historyEntryLimit !== request.historyEntryLimit
      ) {
        throw new HistoryLimitClientError('stale_response')
      }
      return settings
    },
  })
}

export const historyLimitClient = createHistoryLimitClient()

export function getHistoryEntryLimit(
  expected: HistoryLimitExpectedProjectBinding,
): Promise<HistoryLimitSettings> {
  return historyLimitClient.get(expected)
}

export function setHistoryEntryLimit(
  request: SetHistoryEntryLimitRequest,
): Promise<HistoryLimitSettings> {
  return historyLimitClient.set(request)
}

/**
 * Strictly admits an exact, data-only V1 settings DTO and returns a detached,
 * frozen value. Accessors, symbols, class instances, and hostile proxies are
 * rejected without reading application data through property access.
 */
export function parseHistoryLimitSettings(
  value: unknown,
): HistoryLimitSettings | null {
  const record = exactDataRecord(value, SETTINGS_KEYS)
  if (
    !record
    || record.schemaVersion !== HISTORY_LIMIT_SCHEMA_VERSION
    || !isCanonicalNonNilUuid(record.projectInstanceId)
    || !isCanonicalNonNilUuid(record.projectId)
    || !isRevision(record.revision)
    || !isHistoryEntryLimit(record.historyEntryLimit)
  ) return null

  return Object.freeze({
    schemaVersion: HISTORY_LIMIT_SCHEMA_VERSION,
    projectInstanceId: record.projectInstanceId,
    projectId: record.projectId,
    revision: record.revision,
    historyEntryLimit: record.historyEntryLimit,
  })
}

export function historyLimitSettingsMatchExpectedBinding(
  settings: unknown,
  expected: unknown,
): boolean {
  const parsedSettings = parseHistoryLimitSettings(settings)
  const parsedExpected = normalizeExpectedBinding(expected)
  return parsedSettings !== null
    && parsedExpected !== null
    && settingsMatchExpectedBinding(parsedSettings, parsedExpected)
}

export function isHistoryEntryLimit(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= MIN_HISTORY_ENTRY_LIMIT
    && value <= MAX_HISTORY_ENTRY_LIMIT
}

function normalizeExpectedBinding(
  value: unknown,
): HistoryLimitExpectedProjectBinding | null {
  const record = exactDataRecord(value, EXPECTED_BINDING_KEYS)
  if (
    !record
    || !isCanonicalNonNilUuid(record.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(record.expectedProjectId)
    || !isRevision(record.expectedRevision)
  ) return null

  return Object.freeze({
    expectedProjectInstanceId: record.expectedProjectInstanceId,
    expectedProjectId: record.expectedProjectId,
    expectedRevision: record.expectedRevision,
  })
}

function normalizeSetRequest(
  value: unknown,
): SetHistoryEntryLimitRequest | null {
  const record = exactDataRecord(value, SET_REQUEST_KEYS)
  if (
    !record
    || record.schemaVersion !== HISTORY_LIMIT_SCHEMA_VERSION
    || !isCanonicalNonNilUuid(record.expectedProjectInstanceId)
    || !isCanonicalNonNilUuid(record.expectedProjectId)
    || !isRevision(record.expectedRevision)
    || !isHistoryEntryLimit(record.historyEntryLimit)
  ) return null

  return Object.freeze({
    schemaVersion: HISTORY_LIMIT_SCHEMA_VERSION,
    expectedProjectInstanceId: record.expectedProjectInstanceId,
    expectedProjectId: record.expectedProjectId,
    expectedRevision: record.expectedRevision,
    historyEntryLimit: record.historyEntryLimit,
  })
}

function settingsMatchExpectedBinding(
  settings: HistoryLimitSettings,
  expected: HistoryLimitExpectedProjectBinding,
): boolean {
  return settings.projectInstanceId === expected.expectedProjectInstanceId
    && settings.projectId === expected.expectedProjectId
    && settings.revision === expected.expectedRevision
}

function isRevision(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= 0
    && !Object.is(value, -0)
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  try {
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
      actualKeys.length !== expectedKeys.length
      || actualKeys.some((key) => typeof key !== 'string')
      || expectedKeys.some((key) => !Object.hasOwn(descriptors, key))
    ) return null

    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of expectedKeys) {
      const descriptor = descriptors[key]
      if (
        descriptor === undefined
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      snapshot[key] = descriptor.value
    }
    return snapshot as Readonly<Record<Keys[number], unknown>>
  } catch {
    return null
  }
}

function defaultNativeInvoke(
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) {
  return invoke<unknown>(command, arguments_)
}
