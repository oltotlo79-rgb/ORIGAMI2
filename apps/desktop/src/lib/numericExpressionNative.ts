import { invoke } from '@tauri-apps/api/core'

export const NUMERIC_EXPRESSION_SCHEMA =
  'origami2.numeric-expression-evaluation.v1'
export const MIN_NUMERIC_EXPRESSION_PRECISION_BITS = 32
export const MAX_NUMERIC_EXPRESSION_PRECISION_BITS = 512
export const MAX_NUMERIC_EXPRESSION_SOURCE_BYTES = 4_096
export const MAX_NUMERIC_EXPRESSION_OPERATIONS = 20_000

const MAX_DISPLAY_BYTES = 32
const RESPONSE_KEYS = [
  'schema',
  'source',
  'requestedPrecisionBits',
  'exact',
  'operations',
  'lowerBound',
  'upperBound',
  'lowerDisplay',
  'upperDisplay',
] as const

export type NumericExpressionEvaluation = Readonly<{
  schema: typeof NUMERIC_EXPRESSION_SCHEMA
  source: string
  requestedPrecisionBits: number
  exact: boolean
  operations: number
  lowerBound: number
  upperBound: number
  lowerDisplay: string
  upperDisplay: string
}>

export type NumericExpressionErrorCategory =
  | 'invalid_request'
  | 'invalid_expression'
  | 'resource_limit'
  | 'result_out_of_range'
  | 'internal_failure'
  | 'native_unavailable'
  | 'invalid_response'
  | 'stale_response'

export class NumericExpressionNativeError extends Error {
  readonly category: NumericExpressionErrorCategory

  constructor(category: NumericExpressionErrorCategory) {
    super(category)
    this.name = 'NumericExpressionNativeError'
    this.category = category
  }
}

export type NumericExpressionNativeInvoke = (
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) => unknown

export type NumericExpressionNativeTransport = Readonly<{
  evaluate(
    source: string,
    precisionBits: number,
  ): Promise<NumericExpressionEvaluation>
}>

export function createNumericExpressionNativeTransport(
  nativeInvoke: NumericExpressionNativeInvoke = defaultNativeInvoke,
): NumericExpressionNativeTransport {
  return Object.freeze({
    async evaluate(source, precisionBits) {
      if (!validRequest(source, precisionBits)) {
        throw new NumericExpressionNativeError('invalid_request')
      }

      let raw: unknown
      try {
        raw = await Promise.resolve(nativeInvoke(
          'evaluate_numeric_expression',
          {
            request: {
              source,
              precisionBits,
            },
          },
        ))
      } catch (error) {
        throw new NumericExpressionNativeError(
          numericExpressionNativeErrorCategory(error)
          ?? parseNativeErrorCategory(error)
          ?? 'native_unavailable',
        )
      }

      const response = parseNumericExpressionResponseDto(raw)
      if (!response) {
        throw new NumericExpressionNativeError('invalid_response')
      }
      if (
        response.source !== source
        || response.requestedPrecisionBits !== precisionBits
      ) {
        throw new NumericExpressionNativeError('stale_response')
      }
      return response
    },
  })
}

export function parseNumericExpressionResponseDto(
  value: unknown,
): NumericExpressionEvaluation | null {
  const record = exactDataRecord(value, RESPONSE_KEYS)
  if (
    !record
    || record.schema !== NUMERIC_EXPRESSION_SCHEMA
    || !validSource(record.source)
    || !isPrecisionBits(record.requestedPrecisionBits)
    || typeof record.exact !== 'boolean'
    || !isBoundedInteger(
      record.operations,
      0,
      MAX_NUMERIC_EXPRESSION_OPERATIONS,
    )
    || !isFiniteCanonicalNumber(record.lowerBound)
    || !isFiniteCanonicalNumber(record.upperBound)
    || record.lowerBound > record.upperBound
    || !isCanonicalDisplay(record.lowerDisplay, record.lowerBound)
    || !isCanonicalDisplay(record.upperDisplay, record.upperBound)
  ) return null

  return Object.freeze({
    schema: NUMERIC_EXPRESSION_SCHEMA,
    source: record.source,
    requestedPrecisionBits: record.requestedPrecisionBits,
    exact: record.exact,
    operations: record.operations,
    lowerBound: record.lowerBound,
    upperBound: record.upperBound,
    lowerDisplay: record.lowerDisplay,
    upperDisplay: record.upperDisplay,
  })
}

function validRequest(
  source: unknown,
  precisionBits: unknown,
): source is string {
  return validSource(source) && isPrecisionBits(precisionBits)
}

function validSource(value: unknown): value is string {
  if (typeof value !== 'string' || hasUnpairedSurrogate(value)) return false
  const byteLength = utf8ByteLength(value)
  return byteLength !== null
    && byteLength <= MAX_NUMERIC_EXPRESSION_SOURCE_BYTES
}

function isPrecisionBits(value: unknown): value is number {
  return isBoundedInteger(
    value,
    MIN_NUMERIC_EXPRESSION_PRECISION_BITS,
    MAX_NUMERIC_EXPRESSION_PRECISION_BITS,
  )
}

function isBoundedInteger(
  value: unknown,
  minimum: number,
  maximum: number,
): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= minimum
    && value <= maximum
}

function isFiniteCanonicalNumber(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && !Object.is(value, -0)
}

function isCanonicalDisplay(value: unknown, bound: number): value is string {
  if (typeof value !== 'string') return false
  const byteLength = utf8ByteLength(value)
  if (
    byteLength === null
    || byteLength > MAX_DISPLAY_BYTES
    || !/^-?[0-9]\.[0-9]{17}e-?[0-9]{1,3}$/u.test(value)
  ) return false
  return value === canonicalScientificDisplay(bound)
}

function canonicalScientificDisplay(value: number): string {
  return value.toExponential(17).replace('e+', 'e')
}

function parseNativeErrorCategory(
  value: unknown,
): Exclude<
  NumericExpressionErrorCategory,
  'native_unavailable' | 'invalid_response' | 'stale_response'
> | null {
  const record = exactDataRecord(value, ['category'] as const)
  if (!record) return null
  switch (record.category) {
    case 'invalid_request':
    case 'invalid_expression':
    case 'resource_limit':
    case 'result_out_of_range':
    case 'internal_failure':
      return record.category
    default:
      return null
  }
}

function numericExpressionNativeErrorCategory(
  value: unknown,
): NumericExpressionErrorCategory | null {
  try {
    if (!(value instanceof NumericExpressionNativeError)) return null
    switch (value.category) {
      case 'invalid_request':
      case 'invalid_expression':
      case 'resource_limit':
      case 'result_out_of_range':
      case 'internal_failure':
      case 'native_unavailable':
      case 'invalid_response':
      case 'stale_response':
        return value.category
      default:
        return null
    }
  } catch {
    return null
  }
}

function hasUnpairedSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index)
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      if (index + 1 >= value.length) return true
      const next = value.charCodeAt(index + 1)
      if (next < 0xdc00 || next > 0xdfff) return true
      index += 1
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      return true
    }
  }
  return false
}

function utf8ByteLength(value: string): number | null {
  try {
    return new TextEncoder().encode(value).byteLength
  } catch {
    return null
  }
}

function defaultNativeInvoke(
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) {
  if (!nativeRuntimeAvailable()) {
    throw new NumericExpressionNativeError('native_unavailable')
  }
  return invoke(command, arguments_)
}

function nativeRuntimeAvailable(): boolean {
  try {
    return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
  } catch {
    return false
  }
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
      || Object.getPrototypeOf(value) !== Object.prototype
      || Object.getOwnPropertySymbols(value).length !== 0
    ) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const keys = Object.keys(descriptors)
    if (
      keys.length !== expectedKeys.length
      || expectedKeys.some((key) => !Object.hasOwn(descriptors, key))
    ) return null

    const result = Object.create(null) as Record<string, unknown>
    for (const key of expectedKeys) {
      const descriptor = descriptors[key]
      if (
        descriptor === undefined
        || !('value' in descriptor)
        || !descriptor.enumerable
      ) return null
      result[key] = descriptor.value
    }
    return result as Readonly<Record<Keys[number], unknown>>
  } catch {
    return null
  }
}
