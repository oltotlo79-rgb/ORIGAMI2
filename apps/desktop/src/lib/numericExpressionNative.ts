import { invoke } from '@tauri-apps/api/core'

export const NUMERIC_EXPRESSION_SCHEMA =
  'origami2.numeric-expression-evaluation.v1'
export const MIN_NUMERIC_EXPRESSION_PRECISION_BITS = 32
export const MAX_NUMERIC_EXPRESSION_PRECISION_BITS = 512
export const MAX_NUMERIC_EXPRESSION_SOURCE_BYTES = 4_096
export const MAX_NUMERIC_EXPRESSION_OPERATIONS = 20_000
export const USER_INPUT_NUMERIC_EXPRESSION_PRECISION_BITS = 192

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

export type AdoptedMillimetreExpression = Readonly<{
  source: string
  value: number
  evaluation: NumericExpressionEvaluation
}>

export type AdoptedFiniteExpression = Readonly<{
  source: string
  value: number
  evaluation: NumericExpressionEvaluation
}>

type UserInputEvaluationJob = Readonly<{
  source: string
  transport: NumericExpressionNativeTransport
  resolve(value: AdoptedMillimetreExpression): void
  reject(error: NumericExpressionNativeError | unknown): void
}>

let userInputEvaluationRunning = false
let latestPendingUserInputEvaluation: UserInputEvaluationJob | null = null

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

export function evaluatePositiveMillimetreExpression(
  source: string,
  transport: NumericExpressionNativeTransport =
    createNumericExpressionNativeTransport(),
): Promise<AdoptedMillimetreExpression> {
  if (!validSource(source)) {
    return Promise.reject(new NumericExpressionNativeError('invalid_request'))
  }
  return new Promise((resolve, reject) => {
    const job: UserInputEvaluationJob = {
      source,
      transport,
      resolve,
      reject,
    }
    if (!userInputEvaluationRunning) {
      userInputEvaluationRunning = true
      void runUserInputEvaluation(job)
      return
    }
    latestPendingUserInputEvaluation?.reject(
      new NumericExpressionNativeError('stale_response'),
    )
    latestPendingUserInputEvaluation = job
  })
}

export async function evaluateFiniteNumericExpression(
  source: string,
  transport: NumericExpressionNativeTransport =
    createNumericExpressionNativeTransport(),
): Promise<AdoptedFiniteExpression> {
  if (!validSource(source)) {
    throw new NumericExpressionNativeError('invalid_request')
  }
  const evaluation = await transport.evaluate(
    source,
    USER_INPUT_NUMERIC_EXPRESSION_PRECISION_BITS,
  )
  const value = adoptFiniteAdjacentInterval(
    evaluation.lowerBound,
    evaluation.upperBound,
  )
  if (value === null) {
    throw new NumericExpressionNativeError('result_out_of_range')
  }
  return Object.freeze({ source, value, evaluation })
}

async function runUserInputEvaluation(job: UserInputEvaluationJob) {
  try {
    const evaluation = await job.transport.evaluate(
      job.source,
      USER_INPUT_NUMERIC_EXPRESSION_PRECISION_BITS,
    )
    const value = adoptPositiveAdjacentInterval(
      evaluation.lowerBound,
      evaluation.upperBound,
    )
    if (value === null) {
      throw new NumericExpressionNativeError('result_out_of_range')
    }
    job.resolve(Object.freeze({ source: job.source, value, evaluation }))
  } catch (error) {
    job.reject(error)
  } finally {
    const next = latestPendingUserInputEvaluation
    latestPendingUserInputEvaluation = null
    if (next) {
      void runUserInputEvaluation(next)
    } else {
      userInputEvaluationRunning = false
    }
  }
}

export function adoptPositiveAdjacentInterval(
  lower: number,
  upper: number,
): number | null {
  if (
    !Number.isFinite(lower)
    || !Number.isFinite(upper)
    || lower <= 0
    || lower > upper
    || Object.is(lower, -0)
    || Object.is(upper, -0)
  ) return null
  if (lower === upper) return lower
  if (nextUpPositive(lower) !== upper) return null
  return lower
}

export function adoptFiniteAdjacentInterval(
  lower: number,
  upper: number,
): number | null {
  if (
    !Number.isFinite(lower)
    || !Number.isFinite(upper)
    || lower > upper
  ) return null
  if (lower === upper) return lower === 0 ? 0 : lower
  if (nextUp(lower) !== upper) return null
  return lower === 0 ? 0 : lower
}

function nextUp(value: number) {
  if (Object.is(value, -0)) return Number.MIN_VALUE
  if (value >= 0) return nextUpPositive(value)
  return -nextDownPositive(-value)
}

function nextDownPositive(value: number) {
  if (value === Number.POSITIVE_INFINITY) return Number.MAX_VALUE
  if (value <= 0 || !Number.isFinite(value)) return Number.NaN
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  view.setFloat64(0, value)
  view.setBigUint64(0, view.getBigUint64(0) - 1n)
  return view.getFloat64(0)
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
  if (
    typeof value !== 'string'
    || value.length > MAX_NUMERIC_EXPRESSION_SOURCE_BYTES
    || hasUnpairedSurrogate(value)
  ) return false
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

function nextUpPositive(value: number): number | null {
  if (!Number.isFinite(value) || value <= 0) return null
  const buffer = new ArrayBuffer(8)
  const view = new DataView(buffer)
  view.setFloat64(0, value, false)
  const high = view.getUint32(0, false)
  const low = view.getUint32(4, false)
  if (low === 0xffff_ffff) {
    if (high === 0x7fef_ffff) return Number.POSITIVE_INFINITY
    view.setUint32(0, high + 1, false)
    view.setUint32(4, 0, false)
  } else {
    view.setUint32(4, low + 1, false)
  }
  const result = view.getFloat64(0, false)
  return Number.isFinite(result) ? result : null
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

export function numericExpressionNativeErrorCategory(
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
