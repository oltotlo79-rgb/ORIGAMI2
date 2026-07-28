import { invoke } from '@tauri-apps/api/core'
import { DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1 } from './deterministicTranscendentalModel.ts'

export const BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1 = 1
export const BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1 =
  'ori_beginner_skeleton_endpoint_binary64_ecmascript_round_tenths_v1'
export {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    as BEGINNER_SKELETON_TRANSCENDENTAL_MODEL_ID_V1,
}

const RESPONSE_KEYS = [
  'schema_version',
  'endpoint_model_id',
  'transcendental_model_id',
  'request_start_x_mm',
  'request_start_y_mm',
  'request_length_mm',
  'request_angle_degrees',
  'endpoint_x_mm',
  'endpoint_y_mm',
  'endpoint_x_bits_hex',
  'endpoint_y_bits_hex',
  'start_tenths_mm',
  'end_tenths_mm',
  'authorizes_project_mutation',
] as const

export type BeginnerSkeletonEndpointInputV1 = Readonly<{
  startXMm: number
  startYMm: number
  lengthMm: number
  angleDegrees: number
}>

export type BeginnerSkeletonEndpointResponseV1 = Readonly<{
  schema_version: typeof BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1
  endpoint_model_id: typeof BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1
  transcendental_model_id:
    typeof DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
  request_start_x_mm: number
  request_start_y_mm: number
  request_length_mm: number
  request_angle_degrees: number
  endpoint_x_mm: number
  endpoint_y_mm: number
  endpoint_x_bits_hex: string
  endpoint_y_bits_hex: string
  start_tenths_mm: readonly [number, number]
  end_tenths_mm: readonly [number, number]
  authorizes_project_mutation: false
}>

export type BeginnerSkeletonEndpointNativeInvoke = (
  command: string,
  arguments_?: Readonly<Record<string, unknown>>,
) => unknown

export type BeginnerSkeletonEndpointTransport = Readonly<{
  resolve(
    input: BeginnerSkeletonEndpointInputV1,
  ): Promise<BeginnerSkeletonEndpointResponseV1>
}>

export function createBeginnerSkeletonEndpointTransport(
  nativeInvoke: BeginnerSkeletonEndpointNativeInvoke = invoke,
): BeginnerSkeletonEndpointTransport {
  return Object.freeze({
    async resolve(input) {
      const normalized = normalizeInput(input)
      if (!normalized) throw new Error('invalid_beginner_skeleton_endpoint_request')
      const value = await nativeInvoke(
        'resolve_beginner_skeleton_endpoint_v1',
        {
          request: {
            schemaVersion: BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1,
            endpointModelId: BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1,
            transcendentalModelId:
              DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            ...normalized,
          },
        },
      )
      const parsed = parseBeginnerSkeletonEndpointResponseV1(
        value,
        normalized,
      )
      if (!parsed) throw new Error('invalid_beginner_skeleton_endpoint_response')
      return parsed
    },
  })
}

const DEFAULT_TRANSPORT = createBeginnerSkeletonEndpointTransport()

export function resolveBeginnerSkeletonEndpointV1(
  input: BeginnerSkeletonEndpointInputV1,
) {
  return DEFAULT_TRANSPORT.resolve(input)
}

export function parseBeginnerSkeletonEndpointResponseV1(
  value: unknown,
  expectedInput: BeginnerSkeletonEndpointInputV1,
): BeginnerSkeletonEndpointResponseV1 | null {
  const expected = normalizeInput(expectedInput)
  const record = exactDataRecord(value, RESPONSE_KEYS)
  if (!expected || !record) return null
  const startTenthsMm = snapshotTenthsPoint(record.start_tenths_mm)
  const endTenthsMm = snapshotTenthsPoint(record.end_tenths_mm)
  const expectedStartTenthsMm = snapshotRoundedTenthsPoint(
    record.request_start_x_mm,
    record.request_start_y_mm,
  )
  const expectedEndTenthsMm = snapshotRoundedTenthsPoint(
    record.endpoint_x_mm,
    record.endpoint_y_mm,
  )
  if (
    record.schema_version !== BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1
    || record.endpoint_model_id !== BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1
    || record.transcendental_model_id
      !== DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    || record.authorizes_project_mutation !== false
    || !sameFiniteNumber(record.request_start_x_mm, expected.startXMm)
    || !sameFiniteNumber(record.request_start_y_mm, expected.startYMm)
    || !sameFiniteNumber(record.request_length_mm, expected.lengthMm)
    || !sameFiniteNumber(record.request_angle_degrees, expected.angleDegrees)
    || !isFiniteCanonicalNumber(record.endpoint_x_mm)
    || !isFiniteCanonicalNumber(record.endpoint_y_mm)
    || !isBinary64Hex(record.endpoint_x_bits_hex)
    || !isBinary64Hex(record.endpoint_y_bits_hex)
    || binary64Hex(record.endpoint_x_mm) !== record.endpoint_x_bits_hex
    || binary64Hex(record.endpoint_y_mm) !== record.endpoint_y_bits_hex
    || !startTenthsMm
    || !endTenthsMm
    || !expectedStartTenthsMm
    || !expectedEndTenthsMm
    || !sameTenthsPoint(startTenthsMm, expectedStartTenthsMm)
    || !sameTenthsPoint(endTenthsMm, expectedEndTenthsMm)
    || (
      startTenthsMm[0] === endTenthsMm[0]
      && startTenthsMm[1] === endTenthsMm[1]
    )
  ) return null
  return Object.freeze({
    schema_version: record.schema_version,
    endpoint_model_id: record.endpoint_model_id,
    transcendental_model_id: record.transcendental_model_id,
    request_start_x_mm: record.request_start_x_mm,
    request_start_y_mm: record.request_start_y_mm,
    request_length_mm: record.request_length_mm,
    request_angle_degrees: record.request_angle_degrees,
    endpoint_x_mm: record.endpoint_x_mm,
    endpoint_y_mm: record.endpoint_y_mm,
    endpoint_x_bits_hex: record.endpoint_x_bits_hex,
    endpoint_y_bits_hex: record.endpoint_y_bits_hex,
    start_tenths_mm: Object.freeze(startTenthsMm),
    end_tenths_mm: Object.freeze(endTenthsMm),
    authorizes_project_mutation: false,
  })
}

function normalizeInput(
  input: BeginnerSkeletonEndpointInputV1,
): BeginnerSkeletonEndpointInputV1 | null {
  const startXMm = normalizeZero(input.startXMm)
  const startYMm = normalizeZero(input.startYMm)
  const lengthMm = normalizeZero(input.lengthMm)
  const angleDegrees = normalizeZero(input.angleDegrees)
  if (
    ![startXMm, startYMm, lengthMm, angleDegrees].every(Number.isFinite)
    || Math.abs(startXMm) > 10_000
    || Math.abs(startYMm) > 10_000
    || lengthMm < 0.1
    || lengthMm > 10_000
    || Math.abs(angleDegrees) > 360
  ) return null
  return Object.freeze({ startXMm, startYMm, lengthMm, angleDegrees })
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
    const ownKeys = Reflect.ownKeys(descriptors)
    if (
      ownKeys.length !== expectedKeys.length
      || ownKeys.some((key) => typeof key !== 'string')
      || expectedKeys.some((key) => !Object.hasOwn(descriptors, key))
    ) return null
    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of expectedKeys) {
      const descriptor = descriptors[key]
      if (
        !descriptor
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

function sameFiniteNumber(value: unknown, expected: number): value is number {
  return isFiniteCanonicalNumber(value) && Object.is(value, expected)
}

function isFiniteCanonicalNumber(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && !Object.is(value, -0)
}

function isBinary64Hex(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{16}$/u.test(value)
}

function snapshotTenthsPoint(value: unknown): [number, number] | null {
  try {
    if (
      !Array.isArray(value)
      || Object.getPrototypeOf(value) !== Array.prototype
    ) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const ownKeys = Reflect.ownKeys(descriptors)
    if (
      ownKeys.length !== 3
      || ownKeys.some((key) => typeof key !== 'string')
      || !Object.hasOwn(descriptors, '0')
      || !Object.hasOwn(descriptors, '1')
      || !Object.hasOwn(descriptors, 'length')
    ) return null
    const descriptorMap = descriptors as unknown as
      Record<string, PropertyDescriptor | undefined>
    const length = descriptorMap.length
    const first = descriptorMap[0]
    const second = descriptorMap[1]
    if (
      !length
      || !('value' in length)
      || length.value !== 2
      || !first
      || !('value' in first)
      || !first.enumerable
      || !second
      || !('value' in second)
      || !second.enumerable
      || !isTenthsCoordinate(first.value)
      || !isTenthsCoordinate(second.value)
    ) return null
    return [first.value, second.value]
  } catch {
    return null
  }
}

function isTenthsCoordinate(value: unknown): value is number {
  return typeof value === 'number'
    && !Object.is(value, -0)
    && Number.isInteger(value)
    && value >= -100_000
    && value <= 100_000
}

function snapshotRoundedTenthsPoint(
  xMm: unknown,
  yMm: unknown,
): [number, number] | null {
  const x = ecmascriptRoundTenthsCoordinate(xMm)
  const y = ecmascriptRoundTenthsCoordinate(yMm)
  return x === null || y === null ? null : [x, y]
}

function ecmascriptRoundTenthsCoordinate(value: unknown): number | null {
  if (!isFiniteCanonicalNumber(value)) return null
  const rounded = Math.round(value * 10)
  const canonical = Object.is(rounded, -0) ? 0 : rounded
  return isTenthsCoordinate(canonical) ? canonical : null
}

function sameTenthsPoint(
  actual: readonly [number, number],
  expected: readonly [number, number],
) {
  return actual[0] === expected[0] && actual[1] === expected[1]
}

function binary64Hex(value: number) {
  const bytes = new ArrayBuffer(8)
  const view = new DataView(bytes)
  view.setFloat64(0, value, false)
  return `${view.getUint32(0, false).toString(16).padStart(8, '0')}${
    view.getUint32(4, false).toString(16).padStart(8, '0')
  }`
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}
