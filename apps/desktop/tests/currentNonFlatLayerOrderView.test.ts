import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import {
  CURRENT_NON_FLAT_LAYER_ORDER_GRAPH_POSE_MODEL_ID_V1,
  CURRENT_NON_FLAT_LAYER_ORDER_TREE_POSE_MODEL_ID_V1,
  CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1,
  MAX_NON_FLAT_VIEW_EXACT_MAGNITUDE_BYTES_V1,
  normalizeCurrentNonFlatLayerOrderViewV1 as normalize,
} from '../src/lib/currentNonFlatLayerOrderView.ts'

function uuid(seed: number) {
  const hex = seed.toString(16).padStart(12, '0')
  return `00000000-0000-4000-8000-${hex}`
}

const INSTANCE = uuid(1)
const PROJECT = uuid(2)
const FACE_A = uuid(11)
const FACE_B = uuid(12)
const EDGE_1 = uuid(21)
const EDGE_2 = uuid(22)
const DIGEST_A = 'a'.repeat(64)
const DIGEST_B = 'b'.repeat(64)
const DIGEST_C = 'c'.repeat(64)
const DIGEST_D = 'e'.repeat(64)
const FINGERPRINT = 'd'.repeat(64)

const ONE = {
  sign: 'positive',
  numeratorMagnitudeHex: '01',
  denominatorMagnitudeHex: '01',
}
const ZERO = {
  sign: 'zero',
  numeratorMagnitudeHex: '',
  denominatorMagnitudeHex: '01',
}
const MINUS_ONE = {
  sign: 'negative',
  numeratorMagnitudeHex: '01',
  denominatorMagnitudeHex: '01',
}

function identityAffine() {
  return {
    m00: { ...ONE },
    m01: { ...ZERO },
    m10: { ...MINUS_ONE },
    m11: { ...ONE },
    tx: { ...ZERO },
    ty: { ...MINUS_ONE },
  }
}

function exactPoint(u: number, v: number) {
  const rational = (value: number) => value === 0
    ? { ...ZERO }
    : {
      sign: value > 0 ? 'positive' : 'negative',
      numeratorMagnitudeHex: Math.abs(value).toString(16).padStart(2, '0'),
      denominatorMagnitudeHex: '01',
    }
  return { u: rational(u), v: rational(v) }
}

/** The plane axes fixed by each dropped world axis. */
const PLANE_AXES: Record<string, [string, string]> = {
  x: ['y', 'z'],
  y: ['x', 'z'],
  z: ['x', 'y'],
}

type Options = Readonly<{
  droppedAxis?: 'x' | 'y' | 'z'
  cells?: 0 | 1 | 2
  poseModelId?: string
}>

function response(options: Options = {}) {
  const axis = options.droppedAxis ?? 'z'
  const plane = PLANE_AXES[axis] as [string, string]
  const cellCount = options.cells ?? 1
  const cell = (key: string, digest: string) => ({
    cellKeySha256: key,
    exactBoundarySha256: digest,
    lowerFaceId: FACE_A,
    upperFaceId: FACE_B,
    projection: {
      droppedWorldAxis: axis,
      planeAxes: [...plane],
      roundedBoundaryUvMm: [[1, 3], [5, 3], [1, 8]],
      exactBoundaryUv: [exactPoint(1, 3), exactPoint(-5, 3), exactPoint(1, 0)],
    },
  })
  const cells = cellCount === 0
    ? []
    : cellCount === 1
      ? [cell(DIGEST_C, DIGEST_A)]
      : [cell(DIGEST_C, DIGEST_A), cell(DIGEST_D, DIGEST_B)]
  return {
    version: 1,
    modelId: CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1,
    projectInstanceId: INSTANCE,
    projectId: PROJECT,
    revision: 12,
    foldModelFingerprintSha256: FINGERPRINT,
    pose: {
      modelId: options.poseModelId
        ?? CURRENT_NON_FLAT_LAYER_ORDER_TREE_POSE_MODEL_ID_V1,
      generation: '7',
      fixedFaceId: FACE_A,
      hingeAngles: [
        { edgeId: EDGE_1, angleDegrees: 73.5 },
        { edgeId: EDGE_2, angleDegrees: 0 },
      ],
    },
    faces: [
      {
        faceId: FACE_A,
        faceKeySha256: DIGEST_A,
        worldOuterBoundaryXyzMm: [[0, 0, 0], [10, 0, 0], [10, 5, 2]],
        projection: {
          droppedWorldAxis: axis,
          planeAxes: [...plane],
          sourceToPlaneProjectionExact: identityAffine(),
        },
      },
      {
        faceId: FACE_B,
        faceKeySha256: DIGEST_B,
        worldOuterBoundaryXyzMm: [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
        projection: {
          droppedWorldAxis: axis,
          planeAxes: [...plane],
          sourceToPlaneProjectionExact: identityAffine(),
        },
      },
    ],
    cells,
    work: {
      testedFacePairs: 1,
      materialFaceCount: 2,
      sourceOverlapCellsAuthenticated: 0,
      overlapCellCount: cells.length,
      facePairOrderCount: cells.length,
      worldBoundaryPointCount: 6,
      exactBoundaryPointCount: cells.length * 3,
    },
    readOnly: true,
    authorizesProjectMutation: false,
  }
}

type Response = ReturnType<typeof response>

/** Applies one mutation to a fresh fixture and returns the value to parse. */
function forge(
  change: (value: Response) => unknown,
  options: Options = {},
): unknown {
  const value = response(options)
  const replaced = change(value)
  return replaced === undefined ? value : replaced
}

function rejects(name: string, change: (value: Response) => unknown) {
  test(`13.2 rejects ${name}`, () => {
    assert.equal(normalize(forge(change)), null)
  })
}

// -- 13.1 positive ---------------------------------------------------------

test('13.1 accepts a complete tree response', () => {
  const parsed = normalize(response())
  assert.ok(parsed)
  assert.equal(parsed.modelId, CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1)
  assert.equal(parsed.pose.modelId, CURRENT_NON_FLAT_LAYER_ORDER_TREE_POSE_MODEL_ID_V1)
  assert.equal(parsed.faces.length, 2)
  assert.equal(parsed.cells.length, 1)
  assert.equal(parsed.pose.generation, '7')
  assert.equal(parsed.readOnly, true)
  assert.equal(parsed.authorizesProjectMutation, false)
})

test('13.1 accepts a complete graph response', () => {
  const parsed = normalize(
    response({ poseModelId: CURRENT_NON_FLAT_LAYER_ORDER_GRAPH_POSE_MODEL_ID_V1 }),
  )
  assert.ok(parsed)
  assert.equal(
    parsed.pose.modelId,
    CURRENT_NON_FLAT_LAYER_ORDER_GRAPH_POSE_MODEL_ID_V1,
  )
})

for (const axis of ['x', 'y', 'z'] as const) {
  test(`13.1 accepts a response with dropped world axis ${axis}`, () => {
    const parsed = normalize(response({ droppedAxis: axis }))
    assert.ok(parsed)
    assert.equal(parsed.faces[0]?.projection.droppedWorldAxis, axis)
    assert.deepEqual(
      [...(parsed.cells[0]?.projection.planeAxes ?? [])],
      PLANE_AXES[axis],
    )
  })
}

test('13.1 accepts a zero-cell response', () => {
  const parsed = normalize(response({ cells: 0 }))
  assert.ok(parsed)
  assert.equal(parsed.cells.length, 0)
  assert.equal(parsed.work.overlapCellCount, 0)
})

test('13.1 accepts negative, zero, and positive rationals', () => {
  const parsed = normalize(response())
  assert.ok(parsed)
  const affine = parsed.faces[0]?.projection.sourceToPlaneProjectionExact
  assert.equal(affine?.m00.sign, 'positive')
  assert.equal(affine?.m01.sign, 'zero')
  assert.equal(affine?.m10.sign, 'negative')
  assert.equal(affine?.m01.numeratorMagnitudeHex, '')
  assert.equal(affine?.m01.denominatorMagnitudeHex, '01')
})

test('13.1 accepts two canonically ordered cells', () => {
  const parsed = normalize(response({ cells: 2 }))
  assert.ok(parsed)
  assert.equal(parsed.cells.length, 2)
  assert.equal(parsed.cells[0]?.cellKeySha256, DIGEST_C)
  assert.equal(parsed.cells[1]?.cellKeySha256, DIGEST_D)
})

/**
 * Magnitude bytes consumed by the zero-cell fixture: two faces whose affine is
 * `1, 0, -1, 1, 0, -1` over the denominator `01`.
 */
const ZERO_CELL_MAGNITUDE_BYTES = 2 * (2 + 1 + 2 + 2 + 1 + 2)

test('13.1 accepts an exact magnitude total at the aggregate cap', () => {
  const value = response({ cells: 0 })
  const deficit = MAX_NON_FLAT_VIEW_EXACT_MAGNITUDE_BYTES_V1
    - ZERO_CELL_MAGNITUDE_BYTES
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .numeratorMagnitudeHex = `01${'ff'.repeat(deficit)}`
  const parsed = normalize(value)
  assert.ok(parsed)
})

test('13.1 returns a detached deeply frozen value', () => {
  const raw = response()
  const parsed = normalize(raw)
  assert.ok(parsed)
  assert.equal(Object.isFrozen(parsed), true)
  assert.equal(Object.isFrozen(parsed.faces), true)
  assert.equal(Object.isFrozen(parsed.faces[0]), true)
  assert.equal(Object.isFrozen(parsed.faces[0]?.worldOuterBoundaryXyzMm), true)
  assert.equal(Object.isFrozen(parsed.faces[0]?.worldOuterBoundaryXyzMm[0]), true)
  assert.equal(Object.isFrozen(parsed.cells[0]?.projection.exactBoundaryUv[0]), true)
  assert.equal(Object.isFrozen(parsed.pose.hingeAngles[0]), true)
})

test('13.1 keeps the output stable after the input is mutated', () => {
  const raw = response()
  const parsed = normalize(raw)
  assert.ok(parsed)
  raw.faces[0]!.faceId = FACE_B
  raw.cells[0]!.projection.roundedBoundaryUvMm[0]![0] = 999
  raw.pose.generation = '9'
  assert.equal(parsed.faces[0]?.faceId, FACE_A)
  assert.equal(parsed.cells[0]?.projection.roundedBoundaryUvMm[0]?.[0], 1)
  assert.equal(parsed.pose.generation, '7')
})

// -- 13.2 negative: root level ---------------------------------------------

rejects('a null payload', () => null)
rejects('an array payload', () => [])
rejects('a string payload', () => 'view')
test('13.2 rejects an undefined payload', () => {
  assert.equal(normalize(undefined), null)
})
rejects('a wrong version', (value) => {
  value.version = 2
})
rejects('an unknown layer model ID', (value) => {
  value.modelId = 'other_model'
})
rejects('readOnly false', (value) => {
  value.readOnly = false
})
rejects('authorizesProjectMutation true', (value) => {
  value.authorizesProjectMutation = true
})
rejects('a missing root field', (value) => {
  const { work, ...rest } = value
  void work
  return rest
})
rejects('an extra root field', (value) => ({ ...value, extra: 1 }))
rejects('an inherited root field', (value) => {
  const { faces, ...rest } = value
  return Object.assign(Object.create({ faces }), rest)
})
rejects('a root symbol own property', (value) => {
  const hostile: Record<string | symbol, unknown> = { ...value }
  hostile[Symbol('extra')] = 1
  return hostile
})
rejects('an uppercase fingerprint digest', (value) => {
  value.foldModelFingerprintSha256 = 'A'.repeat(64)
})
rejects('a short fingerprint digest', (value) => {
  value.foldModelFingerprintSha256 = 'a'.repeat(63)
})
rejects('a long fingerprint digest', (value) => {
  value.foldModelFingerprintSha256 = 'a'.repeat(65)
})
rejects('an invalid project instance UUID', (value) => {
  value.projectInstanceId = 'not-a-uuid'
})
rejects('a nil project UUID', (value) => {
  value.projectId = '00000000-0000-0000-0000-000000000000'
})
rejects('a negative-zero revision', (value) => {
  value.revision = -0
})
rejects('an unsafe integer revision', (value) => {
  value.revision = Number.MAX_SAFE_INTEGER + 2
})
rejects('a NaN revision', (value) => {
  value.revision = Number.NaN
})
rejects('an Infinity revision', (value) => {
  value.revision = Number.POSITIVE_INFINITY
})
rejects('a -Infinity revision', (value) => {
  value.revision = Number.NEGATIVE_INFINITY
})
rejects('a string revision', (value) => {
  value.revision = '12' as unknown as number
})

// -- 13.2 negative: pose ---------------------------------------------------

rejects('a zero generation', (value) => {
  value.pose.generation = '0'
})
rejects('a leading-zero generation', (value) => {
  value.pose.generation = '007'
})
rejects('a generation above u64::MAX', (value) => {
  value.pose.generation = '18446744073709551616'
})
rejects('an unknown pose model ID', (value) => {
  value.pose.modelId = 'graph_pose_v1'
})
rejects('a pose with only flat hinge endpoints', (value) => {
  value.pose.hingeAngles = [
    { edgeId: EDGE_1, angleDegrees: 0 },
    { edgeId: EDGE_2, angleDegrees: 180 },
  ]
})
rejects('out-of-order hinge angles', (value) => {
  value.pose.hingeAngles = [
    { edgeId: EDGE_2, angleDegrees: 73.5 },
    { edgeId: EDGE_1, angleDegrees: 0 },
  ]
})
rejects('duplicate hinge edge IDs', (value) => {
  value.pose.hingeAngles = [
    { edgeId: EDGE_1, angleDegrees: 73.5 },
    { edgeId: EDGE_1, angleDegrees: 12 },
  ]
})
rejects('an empty hinge vector', (value) => {
  value.pose.hingeAngles = []
})
rejects('an out-of-range hinge angle', (value) => {
  value.pose.hingeAngles[0]!.angleDegrees = 181
})
rejects('a negative hinge angle', (value) => {
  value.pose.hingeAngles[0]!.angleDegrees = -1
})
rejects('a negative-zero hinge angle', (value) => {
  value.pose.hingeAngles[0]!.angleDegrees = -0
})
rejects('a NaN hinge angle', (value) => {
  value.pose.hingeAngles[0]!.angleDegrees = Number.NaN
})
rejects('an invalid hinge edge UUID', (value) => {
  value.pose.hingeAngles[0]!.edgeId = 'edge-1'
})
rejects('an extra pose field', (value) => {
  ;(value.pose as unknown as Record<string, unknown>).extra = 1
})
rejects('a missing pose field', (value) => {
  delete (value.pose as unknown as Record<string, unknown>).fixedFaceId
})
rejects('an inherited pose field', (value) => {
  const { hingeAngles, ...rest } = value.pose
  value.pose = Object.assign(
    Object.create({ hingeAngles }),
    rest,
  ) as Response['pose']
})
rejects('an accessor pose field', (value) => {
  Object.defineProperty(value.pose, 'generation', {
    configurable: true,
    enumerable: true,
    get: () => '7',
  })
})

// -- 13.2 negative: faces --------------------------------------------------

rejects('out-of-order faces', (value) => {
  value.faces = [value.faces[1]!, value.faces[0]!]
})
rejects('duplicate face IDs', (value) => {
  value.faces[1]!.faceId = FACE_A
})
rejects('mismatched plane axes', (value) => {
  value.faces[0]!.projection.planeAxes = ['y', 'z']
})
rejects('an unknown dropped world axis', (value) => {
  value.faces[0]!.projection.droppedWorldAxis = 'w'
})
rejects('a two-point world polygon', (value) => {
  value.faces[0]!.worldOuterBoundaryXyzMm = [[0, 0, 0], [1, 0, 0]]
})
rejects('a one-point world polygon', (value) => {
  value.faces[0]!.worldOuterBoundaryXyzMm = [[0, 0, 0]]
})
rejects('an empty world polygon', (value) => {
  value.faces[0]!.worldOuterBoundaryXyzMm = []
})
rejects('an Infinity world coordinate', (value) => {
  value.faces[0]!.worldOuterBoundaryXyzMm[0]![0] = Number.POSITIVE_INFINITY
})
rejects('a -Infinity world coordinate', (value) => {
  value.faces[0]!.worldOuterBoundaryXyzMm[0]![0] = Number.NEGATIVE_INFINITY
})
rejects('a NaN world coordinate', (value) => {
  value.faces[0]!.worldOuterBoundaryXyzMm[0]![0] = Number.NaN
})
rejects('a negative-zero world coordinate', (value) => {
  value.faces[0]!.worldOuterBoundaryXyzMm[0]![0] = -0
})
rejects('a four-component world point', (value) => {
  ;(value.faces[0]!.worldOuterBoundaryXyzMm as unknown as number[][])[0] = [
    0,
    0,
    0,
    0,
  ]
})
rejects('a face array hole', (value) => {
  const holed = [value.faces[0]!, value.faces[1]!]
  delete holed[1]
  value.faces = holed as Response['faces']
})
rejects('a named extra property on the face array', (value) => {
  ;(value.faces as unknown as Record<string, unknown>).extra = true
})
rejects('a symbol extra property on the face array', (value) => {
  ;(value.faces as unknown as Record<symbol, unknown>)[Symbol('extra')] = true
})
rejects('an index accessor on the face array', (value) => {
  Object.defineProperty(value.faces, '0', {
    configurable: true,
    enumerable: true,
    get: () => value.faces[1],
  })
})
rejects('a duplicate face key digest', (value) => {
  value.faces[1]!.faceKeySha256 = DIGEST_A
})
rejects('a missing face field', (value) => {
  delete (value.faces[0] as unknown as Record<string, unknown>).faceKeySha256
})
rejects('an extra face field', (value) => {
  ;(value.faces[0] as unknown as Record<string, unknown>).extra = 1
})
rejects('an extra projection field', (value) => {
  ;(value.faces[0]!.projection as unknown as Record<string, unknown>).extra = 1
})

// -- 13.2 negative: exact rationals ----------------------------------------

rejects('a malformed rational sign', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00.sign = 'plus'
})
rejects('a nonzero sign with an empty numerator', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00 = {
    sign: 'positive',
    numeratorMagnitudeHex: '',
    denominatorMagnitudeHex: '01',
  }
})
rejects('a zero sign with a nonzero numerator', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m01 = {
    sign: 'zero',
    numeratorMagnitudeHex: '01',
    denominatorMagnitudeHex: '01',
  }
})
rejects('a zero rational whose denominator is not 01', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m01
    .denominatorMagnitudeHex = '02'
})
rejects('an empty denominator', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .denominatorMagnitudeHex = ''
})
rejects('a zero denominator', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .denominatorMagnitudeHex = '00'
})
rejects('an odd-length numerator hex', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .numeratorMagnitudeHex = '001'
})
rejects('an uppercase numerator hex', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .numeratorMagnitudeHex = 'AB'
})
rejects('a leading-zero numerator hex', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .numeratorMagnitudeHex = '0001'
})
rejects('a leading-zero denominator hex', (value) => {
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .denominatorMagnitudeHex = '0001'
})
rejects('a numeric numerator magnitude', (value) => {
  ;(value.faces[0]!.projection.sourceToPlaneProjectionExact
    .m00 as unknown as Record<string, unknown>).numeratorMagnitudeHex = 1
})

test('13.2 rejects an exact magnitude total one byte over the aggregate cap', () => {
  const value = response({ cells: 0 })
  const deficit = MAX_NON_FLAT_VIEW_EXACT_MAGNITUDE_BYTES_V1
    - ZERO_CELL_MAGNITUDE_BYTES
  value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
    .numeratorMagnitudeHex = `01${'ff'.repeat(deficit + 1)}`
  assert.equal(normalize(value), null)
})

// -- 13.2 negative: cells --------------------------------------------------

rejects('a cell whose lower and upper face are the same', (value) => {
  value.cells[0]!.upperFaceId = value.cells[0]!.lowerFaceId
})
rejects('a cell referencing an unknown lower face', (value) => {
  value.cells[0]!.lowerFaceId = uuid(99)
})
rejects('a cell referencing an unknown upper face', (value) => {
  value.cells[0]!.upperFaceId = uuid(99)
})
rejects('a cell whose dropped axis differs from its faces', (value) => {
  value.cells[0]!.projection.droppedWorldAxis = 'x'
  value.cells[0]!.projection.planeAxes = ['y', 'z']
})
rejects('a cell whose plane axes contradict its dropped axis', (value) => {
  value.cells[0]!.projection.planeAxes = ['y', 'x']
})
rejects('an exact and rounded point count mismatch', (value) => {
  value.cells[0]!.projection.roundedBoundaryUvMm.pop()
})
rejects('a two-point cell boundary', (value) => {
  value.cells[0]!.projection.roundedBoundaryUvMm.pop()
  value.cells[0]!.projection.exactBoundaryUv.pop()
  value.work.exactBoundaryPointCount = 2
})
rejects('out-of-order cell keys', (value) => {
  const two = response({ cells: 2 })
  two.cells = [two.cells[1]!, two.cells[0]!]
  return two
})
rejects('duplicate cell keys', (value) => {
  const two = response({ cells: 2 })
  two.cells[1]!.cellKeySha256 = DIGEST_C
  return two
})
rejects('an extra cell field', (value) => {
  ;(value.cells[0] as unknown as Record<string, unknown>).extra = 1
})
rejects('a NaN rounded cell coordinate', (value) => {
  value.cells[0]!.projection.roundedBoundaryUvMm[0]![0] = Number.NaN
})

// -- 13.2 negative: work counts --------------------------------------------

rejects('a material face count mismatch', (value) => {
  value.work.materialFaceCount = 3
})
rejects('an overlap cell count mismatch', (value) => {
  value.work.overlapCellCount = 2
})
rejects('a face pair order count mismatch', (value) => {
  value.work.facePairOrderCount = 2
})
rejects('a world boundary point count mismatch', (value) => {
  value.work.worldBoundaryPointCount = 5
})
rejects('an exact boundary point count mismatch', (value) => {
  value.work.exactBoundaryPointCount = 4
})
rejects('a negative tested face pair count', (value) => {
  value.work.testedFacePairs = -1
})
rejects('an unsafe tested face pair count', (value) => {
  value.work.testedFacePairs = Number.MAX_SAFE_INTEGER + 2
})
rejects('an extra work field', (value) => {
  ;(value.work as unknown as Record<string, unknown>).extra = 1
})

// -- 13.2 negative: reflection hostility -----------------------------------

test('13.2 rejects a getter field without ever running it', () => {
  let reads = 0
  const value = response()
  const hostile: Record<string, unknown> = { ...value }
  Object.defineProperty(hostile, 'readOnly', {
    configurable: true,
    enumerable: true,
    get: () => {
      reads += 1
      return true
    },
  })
  assert.equal(normalize(hostile), null)
  assert.equal(reads, 0)
})

test('13.2 rejects a nested getter field without ever running it', () => {
  let reads = 0
  const value = response()
  Object.defineProperty(value.faces[0]!, 'faceKeySha256', {
    configurable: true,
    enumerable: true,
    get: () => {
      reads += 1
      return DIGEST_A
    },
  })
  assert.equal(normalize(value), null)
  assert.equal(reads, 0)
})

test('13.2 rejects a setter-only field without ever running it', () => {
  let writes = 0
  const value = response()
  const hostile: Record<string, unknown> = { ...value }
  Object.defineProperty(hostile, 'revision', {
    configurable: true,
    enumerable: true,
    set: () => {
      writes += 1
    },
  })
  assert.equal(normalize(hostile), null)
  assert.equal(writes, 0)
})

test('13.2 rejects a throwing ownKeys trap without surfacing it', () => {
  const hostile = new Proxy(response(), {
    ownKeys() {
      throw new Error('ownKeys trap')
    },
  })
  assert.equal(normalize(hostile), null)
})

test('13.2 rejects a throwing getOwnPropertyDescriptor trap', () => {
  const target = response()
  const hostile = new Proxy(target, {
    ownKeys: () => Reflect.ownKeys(target),
    getOwnPropertyDescriptor(_, key) {
      if (key === 'faces') throw new Error('descriptor trap')
      return Reflect.getOwnPropertyDescriptor(target, key)
    },
  })
  assert.equal(normalize(hostile), null)
})

test('13.2 rejects a throwing nested get trap', () => {
  const value = response()
  const target = value.faces[0]!
  value.faces[0] = new Proxy(target, {
    get() {
      throw new Error('get trap')
    },
    getOwnPropertyDescriptor() {
      throw new Error('descriptor trap')
    },
  })
  assert.equal(normalize(value), null)
})

// -- 3.1 source hygiene ----------------------------------------------------

test('3.1 keeps the viewer sources free of literal NUL bytes', () => {
  const sources = [
    '../src/lib/currentNonFlatLayerOrderView.ts',
    '../src/components/CurrentNonFlatLayerOrderViewer.tsx',
    '../src/lib/currentNonFlatLayerOrderViewerText.ts',
    './currentNonFlatLayerOrderView.test.ts',
    './currentNonFlatLayerOrderViewer.dom.test.tsx',
  ]
  for (const source of sources) {
    const path = fileURLToPath(new URL(source, import.meta.url))
    const bytes = readFileSync(path)
    assert.equal(bytes.indexOf(0), -1, `${source} contains a literal NUL byte`)
  }
})
