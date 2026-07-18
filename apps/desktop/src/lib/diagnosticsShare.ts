import { invoke } from '@tauri-apps/api/core'

import {
  DIAGNOSTIC_SCOPES,
  MAX_SERIALIZED_DIAGNOSTICS_BYTES,
  REDACTED_DIAGNOSTICS_SCHEMA,
} from './diagnostics.ts'

const MAX_U32 = 0xffff_ffff
const UTF8_ENCODER = new TextEncoder()
const COUNT_BUCKETS = new Set([
  '0',
  '1',
  '2_4',
  '5_16',
  '17_64',
  '65_plus',
])

export type DiagnosticsSharePreview = Readonly<{
  preview_generation: number
  json: string
  byte_length: number
}>

export type DiagnosticsShareSaveResult = Readonly<{
  preview_generation: number
  byte_length: number
  canceled: boolean
}>

export type DiagnosticsShareTransport = Readonly<{
  invoke: (command: string, arguments_?: Readonly<Record<string, unknown>>) => unknown
}>

export type DiagnosticsShareClient = Readonly<{
  preparePreview: () => Promise<DiagnosticsSharePreview>
  savePreview: (
    preview: DiagnosticsSharePreview,
  ) => Promise<DiagnosticsShareSaveResult>
}>

export class DiagnosticsShareUnavailableError extends Error {
  constructor() {
    super('diagnostics share unavailable')
    this.name = 'DiagnosticsShareUnavailableError'
  }
}

export function createDiagnosticsShareClient(
  transport: DiagnosticsShareTransport,
): DiagnosticsShareClient {
  return Object.freeze({
    preparePreview: async () => {
      try {
        const value = await transport.invoke(
          'prepare_diagnostics_share_preview',
        )
        const preview = validatePreview(value)
        if (!preview) throw new DiagnosticsShareUnavailableError()
        return preview
      } catch {
        throw new DiagnosticsShareUnavailableError()
      }
    },
    savePreview: async (untrustedPreview) => {
      try {
        const preview = validatePreview(untrustedPreview)
        if (!preview) throw new DiagnosticsShareUnavailableError()
        const value = await transport.invoke(
          'save_diagnostics_share_preview',
          { previewGeneration: preview.preview_generation },
        )
        const result = validateSaveResult(
          value,
          preview.preview_generation,
          preview.byte_length,
        )
        if (!result) throw new DiagnosticsShareUnavailableError()
        return result
      } catch {
        throw new DiagnosticsShareUnavailableError()
      }
    },
  })
}

export function isDiagnosticsShareAvailable() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

const applicationDiagnosticsShare = createDiagnosticsShareClient({
  invoke: (command, arguments_) => invoke(command, arguments_),
})

export function prepareDiagnosticsSharePreview() {
  return applicationDiagnosticsShare.preparePreview()
}

export function saveDiagnosticsSharePreview(
  preview: DiagnosticsSharePreview,
) {
  return applicationDiagnosticsShare.savePreview(preview)
}

function validatePreview(value: unknown): DiagnosticsSharePreview | null {
  const fields = readExactDataFields(value, [
    'preview_generation',
    'json',
    'byte_length',
  ])
  if (!fields) return null
  const [generation, json, byteLength] = fields
  if (
    !isU32(generation)
    || typeof json !== 'string'
    || !isU32(byteLength)
    || byteLength > MAX_SERIALIZED_DIAGNOSTICS_BYTES
    || UTF8_ENCODER.encode(json).byteLength !== byteLength
    || !isCanonicalDiagnosticsJson(json)
  ) return null
  return Object.freeze({
    preview_generation: generation,
    json,
    byte_length: byteLength,
  })
}

function validateSaveResult(
  value: unknown,
  expectedGeneration: number,
  expectedByteLength: number,
): DiagnosticsShareSaveResult | null {
  const fields = readExactDataFields(value, [
    'preview_generation',
    'byte_length',
    'canceled',
  ])
  if (!fields) return null
  const [generation, byteLength, canceled] = fields
  if (
    !isU32(generation)
    || generation !== expectedGeneration
    || !isU32(byteLength)
    || byteLength !== expectedByteLength
    || typeof canceled !== 'boolean'
  ) return null
  return Object.freeze({
    preview_generation: generation,
    byte_length: byteLength,
    canceled,
  })
}

function isCanonicalDiagnosticsJson(json: string) {
  try {
    const parsed: unknown = JSON.parse(json)
    const root = readExactDataFields(parsed, ['schema', 'unexpected'])
    if (!root) return false
    const [schema, unexpected] = root
    if (
      schema !== REDACTED_DIAGNOSTICS_SCHEMA
      || !Array.isArray(unexpected)
      || unexpected.length !== DIAGNOSTIC_SCOPES.length
    ) return false
    for (let index = 0; index < DIAGNOSTIC_SCOPES.length; index += 1) {
      const entry = readExactDataFields(unexpected[index], [
        'scope',
        'count',
      ])
      if (
        !entry
        || entry[0] !== DIAGNOSTIC_SCOPES[index]
        || typeof entry[1] !== 'string'
        || !COUNT_BUCKETS.has(entry[1])
      ) return false
    }
    return JSON.stringify(parsed) === json
  } catch {
    return false
  }
}

function readExactDataFields(
  value: unknown,
  expectedKeys: readonly string[],
): unknown[] | null {
  try {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      return null
    }
    const keys = Reflect.ownKeys(value)
    if (
      keys.length !== expectedKeys.length
      || keys.some((key) => typeof key !== 'string')
      || expectedKeys.some((key) => !keys.includes(key))
    ) return null
    const fields: unknown[] = []
    for (const key of expectedKeys) {
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      if (
        !descriptor
        || !descriptor.enumerable
        || !Object.hasOwn(descriptor, 'value')
      ) return null
      fields.push(descriptor.value)
    }
    return fields
  } catch {
    return null
  }
}

function isU32(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isInteger(value)
    && value >= 0
    && value <= MAX_U32
    && !Object.is(value, -0)
}
