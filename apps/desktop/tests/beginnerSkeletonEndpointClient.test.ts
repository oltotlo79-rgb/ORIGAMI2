import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1,
  BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1,
  BEGINNER_SKELETON_TRANSCENDENTAL_MODEL_ID_V1,
  createBeginnerSkeletonEndpointTransport,
  parseBeginnerSkeletonEndpointResponseV1,
  type BeginnerSkeletonEndpointInputV1,
} from '../src/lib/beginnerSkeletonEndpointClient.ts'

const INPUT: BeginnerSkeletonEndpointInputV1 = Object.freeze({
  startXMm: 1,
  startYMm: 2,
  lengthMm: 3,
  angleDegrees: 90,
})

function binary64Hex(value: number) {
  const bytes = new ArrayBuffer(8)
  const view = new DataView(bytes)
  view.setFloat64(0, value, false)
  return `${view.getUint32(0, false).toString(16).padStart(8, '0')}${
    view.getUint32(4, false).toString(16).padStart(8, '0')
  }`
}

function response(overrides: Readonly<Record<string, unknown>> = {}) {
  return {
    schema_version: BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1,
    endpoint_model_id: BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1,
    transcendental_model_id:
      BEGINNER_SKELETON_TRANSCENDENTAL_MODEL_ID_V1,
    request_start_x_mm: INPUT.startXMm,
    request_start_y_mm: INPUT.startYMm,
    request_length_mm: INPUT.lengthMm,
    request_angle_degrees: INPUT.angleDegrees,
    endpoint_x_mm: 1,
    endpoint_y_mm: 5,
    endpoint_x_bits_hex: binary64Hex(1),
    endpoint_y_bits_hex: binary64Hex(5),
    start_tenths_mm: [10, 20],
    end_tenths_mm: [10, 50],
    authorizes_project_mutation: false,
    ...overrides,
  }
}

function adjacent(value: number, direction: -1 | 1) {
  const bytes = new ArrayBuffer(8)
  const view = new DataView(bytes)
  view.setFloat64(0, value, false)
  const high = view.getUint32(0, false)
  const low = view.getUint32(4, false)
  if (direction < 0) {
    if (low === 0) {
      view.setUint32(0, high - 1, false)
      view.setUint32(4, 0xffff_ffff, false)
    } else {
      view.setUint32(4, low - 1, false)
    }
  } else if (low === 0xffff_ffff) {
    view.setUint32(0, high + 1, false)
    view.setUint32(4, 0, false)
  } else {
    view.setUint32(4, low + 1, false)
  }
  return view.getFloat64(0, false)
}

test('native endpoint transport sends only original scalars and frozen models', async () => {
  const calls: Array<readonly [
    string,
    Readonly<Record<string, unknown>> | undefined,
  ]> = []
  const transport = createBeginnerSkeletonEndpointTransport(
    (command, arguments_) => {
      calls.push([command, arguments_])
      return response()
    },
  )

  const result = await transport.resolve(INPUT)

  assert.deepEqual(calls, [[
    'resolve_beginner_skeleton_endpoint_v1',
    {
      request: {
        schemaVersion: BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1,
        endpointModelId: BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1,
        transcendentalModelId:
          BEGINNER_SKELETON_TRANSCENDENTAL_MODEL_ID_V1,
        startXMm: 1,
        startYMm: 2,
        lengthMm: 3,
        angleDegrees: 90,
      },
    },
  ]])
  assert.deepEqual(result.start_tenths_mm, [10, 20])
  assert.deepEqual(result.end_tenths_mm, [10, 50])
  assert.equal(result.authorizes_project_mutation, false)
  assert.equal(Object.isFrozen(result), true)
  assert.equal(Object.isFrozen(result.start_tenths_mm), true)
  assert.equal(Object.isFrozen(result.end_tenths_mm), true)
})

test('cardinal and adjacent angle request bits are echoed exactly', async () => {
  for (const angleDegrees of [adjacent(90, -1), 90, adjacent(90, 1)]) {
    const input = { ...INPUT, angleDegrees }
    const transport = createBeginnerSkeletonEndpointTransport(() =>
      response({ request_angle_degrees: angleDegrees }))
    const result = await transport.resolve(input)
    assert.equal(Object.is(result.request_angle_degrees, angleDegrees), true)
  }
})

test('strict endpoint parser rejects forged authority and malformed DTOs', () => {
  const base = response()
  const rejected = [
    { ...base, private_witness: true },
    { ...base, schema_version: 2 },
    { ...base, endpoint_model_id: 'forged' },
    { ...base, transcendental_model_id: 'forged' },
    { ...base, authorizes_project_mutation: true },
    { ...base, request_start_x_mm: 1.5 },
    { ...base, request_angle_degrees: 90.000_000_1 },
    { ...base, endpoint_x_mm: Number.NaN },
    { ...base, endpoint_y_mm: Number.POSITIVE_INFINITY },
    { ...base, endpoint_x_mm: -0, endpoint_x_bits_hex: binary64Hex(-0) },
    { ...base, endpoint_x_bits_hex: '0000000000000000' },
    { ...base, endpoint_x_bits_hex: '3FF0000000000000' },
    { ...base, endpoint_y_bits_hex: 'not-binary64' },
    { ...base, start_tenths_mm: [10] },
    { ...base, start_tenths_mm: [10, 20, 30] },
    { ...base, start_tenths_mm: [10.5, 20] },
    { ...base, start_tenths_mm: [100_001, 20] },
    { ...base, start_tenths_mm: [11, 20] },
    { ...base, end_tenths_mm: [10, 49] },
    { ...base, end_tenths_mm: [10, 20] },
    Object.fromEntries(
      Object.entries(base).filter(([key]) => key !== 'endpoint_model_id'),
    ),
    null,
    [],
  ]
  for (const value of rejected) {
    assert.equal(
      parseBeginnerSkeletonEndpointResponseV1(value, INPUT),
      null,
    )
  }
})

test('strict endpoint parser rejects negative-zero integer coordinates', () => {
  const zeroStartInput = { ...INPUT, startXMm: 0 }
  assert.equal(
    parseBeginnerSkeletonEndpointResponseV1(
      response({
        request_start_x_mm: 0,
        start_tenths_mm: [-0, 20],
      }),
      zeroStartInput,
    ),
    null,
  )
  assert.equal(
    parseBeginnerSkeletonEndpointResponseV1(
      response({
        endpoint_x_mm: 0,
        endpoint_x_bits_hex: binary64Hex(0),
        end_tenths_mm: [-0, 50],
      }),
      INPUT,
    ),
    null,
  )
})

test('strict parser rechecks negative-half ECMAScript tenths rounding', () => {
  const boundaryInput = {
    startXMm: -0.15,
    startYMm: -0.05,
    lengthMm: 0.1,
    angleDegrees: 0,
  }
  const boundaryResponse = response({
    request_start_x_mm: boundaryInput.startXMm,
    request_start_y_mm: boundaryInput.startYMm,
    request_length_mm: boundaryInput.lengthMm,
    request_angle_degrees: boundaryInput.angleDegrees,
    endpoint_x_mm: -0.05,
    endpoint_y_mm: -0.05,
    endpoint_x_bits_hex: binary64Hex(-0.05),
    endpoint_y_bits_hex: binary64Hex(-0.05),
    start_tenths_mm: [-1, 0],
    end_tenths_mm: [0, 0],
  })
  assert.ok(
    parseBeginnerSkeletonEndpointResponseV1(
      boundaryResponse,
      boundaryInput,
    ),
  )
  assert.equal(
    parseBeginnerSkeletonEndpointResponseV1(
      { ...boundaryResponse, start_tenths_mm: [-2, 0] },
      boundaryInput,
    ),
    null,
  )
  assert.equal(
    parseBeginnerSkeletonEndpointResponseV1(
      { ...boundaryResponse, end_tenths_mm: [-1, 0] },
      boundaryInput,
    ),
    null,
  )
})

test('strict parser snapshots own data without invoking hostile values', () => {
  let getterCalls = 0
  const accessor = response()
  Object.defineProperty(accessor, 'endpoint_x_mm', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 1
    },
  })
  assert.equal(
    parseBeginnerSkeletonEndpointResponseV1(accessor, INPUT),
    null,
  )
  assert.equal(getterCalls, 0)

  const inherited = Object.assign(Object.create({ inherited: true }), response())
  assert.equal(
    parseBeginnerSkeletonEndpointResponseV1(inherited, INPUT),
    null,
  )

  const arrayAccessor = response()
  const hostilePoint: unknown[] = [10, 20]
  Object.defineProperty(hostilePoint, '0', {
    enumerable: true,
    get() {
      getterCalls += 1
      return 10
    },
  })
  arrayAccessor.start_tenths_mm = hostilePoint
  assert.equal(
    parseBeginnerSkeletonEndpointResponseV1(arrayAccessor, INPUT),
    null,
  )
  assert.equal(getterCalls, 0)

  const throwingProxy = new Proxy(response(), {
    getPrototypeOf() {
      throw new Error('hostile prototype trap')
    },
    get() {
      throw new Error('hostile get trap')
    },
  })
  assert.doesNotThrow(() => {
    assert.equal(
      parseBeginnerSkeletonEndpointResponseV1(throwingProxy, INPUT),
      null,
    )
  })
})

test('invalid scalar requests fail before invoking native code', async () => {
  let calls = 0
  const transport = createBeginnerSkeletonEndpointTransport(() => {
    calls += 1
    return response()
  })
  for (const input of [
    { ...INPUT, startXMm: Number.NaN },
    { ...INPUT, startYMm: 10_001 },
    { ...INPUT, lengthMm: 0.099 },
    { ...INPUT, lengthMm: 10_001 },
    { ...INPUT, angleDegrees: 361 },
  ]) {
    await assert.rejects(transport.resolve(input))
  }
  assert.equal(calls, 0)
})

test('negative zero input is canonicalized before the native boundary', async () => {
  const calls: Array<Readonly<Record<string, unknown>> | undefined> = []
  const transport = createBeginnerSkeletonEndpointTransport(
    (_command, arguments_) => {
      calls.push(arguments_)
      return response({
        request_start_x_mm: 0,
        request_start_y_mm: 0,
        start_tenths_mm: [0, 0],
      })
    },
  )
  await transport.resolve({ ...INPUT, startXMm: -0, startYMm: -0 })
  assert.deepEqual(calls[0], {
    request: {
      schemaVersion: BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1,
      endpointModelId: BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1,
      transcendentalModelId:
        BEGINNER_SKELETON_TRANSCENDENTAL_MODEL_ID_V1,
      startXMm: 0,
      startYMm: 0,
      lengthMm: 3,
      angleDegrees: 90,
    },
  })
})

test('frontend and Rust use the single frozen transcendental model identifier', () => {
  const rust = readFileSync(
    new URL(
      '../../../crates/ori-numeric/src/deterministic_transcendental.rs',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(
    rust,
    new RegExp(
      `DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1[\\s\\S]{0,200}${
        BEGINNER_SKELETON_TRANSCENDENTAL_MODEL_ID_V1
      }`,
      'u',
    ),
  )
})
