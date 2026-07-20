import { invoke } from '@tauri-apps/api/core'

import {
  admitFoldTechniqueDocumentV1,
  type FoldTechniqueFileDocumentV1,
} from './foldTechniqueEditor.ts'
import type { Locale } from './i18n.ts'

export type FoldTechniqueFileResponseV1 = Readonly<{
  requestId: number
  canceled: boolean
  document: FoldTechniqueFileDocumentV1 | null
}>

export type FoldTechniqueFileClientErrorCode =
  | 'native_unavailable'
  | 'busy'
  | 'open_failed'
  | 'not_regular_file'
  | 'too_large'
  | 'read_failed'
  | 'invalid_document'
  | 'save_failed'
  | 'invalid_response'

const NATIVE_ERROR_CODES = Object.freeze({
  fold_technique_busy: 'busy',
  fold_technique_open_failed: 'open_failed',
  fold_technique_not_regular_file: 'not_regular_file',
  fold_technique_too_large: 'too_large',
  fold_technique_read_failed: 'read_failed',
  fold_technique_invalid_document: 'invalid_document',
  fold_technique_save_failed: 'save_failed',
} satisfies Record<string, FoldTechniqueFileClientErrorCode>)

export class FoldTechniqueFileClientError extends Error {
  readonly code: FoldTechniqueFileClientErrorCode

  constructor(code: FoldTechniqueFileClientErrorCode) {
    super(code)
    this.name = 'FoldTechniqueFileClientError'
    this.code = code
  }
}

export function isNativeFoldTechniqueFileAvailable(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export async function openFoldTechniqueFileV1(
  requestId: number,
  locale: Locale,
): Promise<FoldTechniqueFileResponseV1> {
  validateRequest(requestId, locale)
  ensureNativeAvailable()
  try {
    const response = await invoke<unknown>('open_fold_technique_file', {
      requestId,
      locale,
    })
    return normalizeFoldTechniqueFileResponseV1(response, requestId)
  } catch (error) {
    throw mapNativeError(error)
  }
}

export async function saveFoldTechniqueFileAsV1(
  requestId: number,
  locale: Locale,
  document: unknown,
): Promise<FoldTechniqueFileResponseV1> {
  validateRequest(requestId, locale)
  const admitted = admitFoldTechniqueDocumentV1(document)
  if (!admitted) throw new FoldTechniqueFileClientError('invalid_document')
  ensureNativeAvailable()
  try {
    const response = await invoke<unknown>('save_fold_technique_file_as', {
      requestId,
      locale,
      document: admitted,
    })
    return normalizeFoldTechniqueFileResponseV1(response, requestId)
  } catch (error) {
    throw mapNativeError(error)
  }
}

export function normalizeFoldTechniqueFileResponseV1(
  value: unknown,
  expectedRequestId: number,
): FoldTechniqueFileResponseV1 {
  const record = exactRecord(value, ['request_id', 'canceled', 'document'])
  const requestId = record.request_id
  const canceled = record.canceled
  if (
    !isRequestId(requestId)
    || requestId !== expectedRequestId
    || typeof canceled !== 'boolean'
  ) throw new FoldTechniqueFileClientError('invalid_response')
  if (canceled) {
    if (record.document !== null) {
      throw new FoldTechniqueFileClientError('invalid_response')
    }
    return Object.freeze({ requestId, canceled: true, document: null })
  }
  const document = admitFoldTechniqueDocumentV1(record.document)
  if (!document) throw new FoldTechniqueFileClientError('invalid_response')
  return Object.freeze({ requestId, canceled: false, document })
}

export function foldTechniqueFileClientErrorCode(
  error: unknown,
): FoldTechniqueFileClientErrorCode {
  return error instanceof FoldTechniqueFileClientError
    ? error.code
    : 'invalid_response'
}

function ensureNativeAvailable() {
  if (!isNativeFoldTechniqueFileAvailable()) {
    throw new FoldTechniqueFileClientError('native_unavailable')
  }
}

function validateRequest(requestId: number, locale: Locale) {
  if (!isRequestId(requestId) || (locale !== 'ja' && locale !== 'en')) {
    throw new FoldTechniqueFileClientError('invalid_response')
  }
}

function isRequestId(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && typeof value === 'number'
    && value >= 1
    && value <= 0xffff_ffff
}

function exactRecord(
  value: unknown,
  keys: readonly string[],
): Record<string, unknown> {
  try {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      throw new FoldTechniqueFileClientError('invalid_response')
    }
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) {
      throw new FoldTechniqueFileClientError('invalid_response')
    }
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const ownKeys = Reflect.ownKeys(descriptors)
    if (
      ownKeys.length !== keys.length
      || ownKeys.some((key) =>
        typeof key !== 'string' || !keys.includes(key))
    ) throw new FoldTechniqueFileClientError('invalid_response')
    const snapshot: Record<string, unknown> = Object.create(null)
    for (const key of keys) {
      const descriptor = descriptors[key]
      if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) {
        throw new FoldTechniqueFileClientError('invalid_response')
      }
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch (error) {
    if (error instanceof FoldTechniqueFileClientError) throw error
    throw new FoldTechniqueFileClientError('invalid_response')
  }
}

function mapNativeError(error: unknown): FoldTechniqueFileClientError {
  if (error instanceof FoldTechniqueFileClientError) return error
  if (typeof error === 'string' && Object.hasOwn(NATIVE_ERROR_CODES, error)) {
    const code = NATIVE_ERROR_CODES[
      error as keyof typeof NATIVE_ERROR_CODES
    ]
    return new FoldTechniqueFileClientError(code)
  }
  return new FoldTechniqueFileClientError('invalid_response')
}
