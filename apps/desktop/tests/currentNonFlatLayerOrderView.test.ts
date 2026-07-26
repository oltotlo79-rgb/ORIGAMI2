import assert from 'node:assert/strict'
import test from 'node:test'

import {
  CURRENT_NON_FLAT_LAYER_ORDER_POSE_MODEL_ID_V1,
  CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1,
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

function identityAffine() {
  return {
    m00: { ...ONE },
    m01: { ...ZERO },
    m10: { ...ZERO },
    m11: { ...ONE },
    tx: { ...ZERO },
    ty: { ...ZERO },
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

function response() {
  return {
    version: 1,
    modelId: CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1,
    projectInstanceId: INSTANCE,
    projectId: PROJECT,
    revision: 12,
    foldModelFingerprintSha256: FINGERPRINT,
    pose: {
      modelId: CURRENT_NON_FLAT_LAYER_ORDER_POSE_MODEL_ID_V1,
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
          droppedWorldAxis: 'z',
          planeAxes: ['x', 'y'],
          sourceToPlaneProjectionExact: identityAffine(),
        },
      },
      {
        faceId: FACE_B,
        faceKeySha256: DIGEST_B,
        worldOuterBoundaryXyzMm: [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
        projection: {
          droppedWorldAxis: 'z',
          planeAxes: ['x', 'y'],
          sourceToPlaneProjectionExact: identityAffine(),
        },
      },
    ],
    cells: [
      {
        cellKeySha256: DIGEST_C,
        exactBoundarySha256: DIGEST_A,
        lowerFaceId: FACE_A,
        upperFaceId: FACE_B,
        projection: {
          droppedWorldAxis: 'z',
          planeAxes: ['x', 'y'],
          roundedBoundaryUvMm: [[1, 3], [5, 3], [1, 8]],
          exactBoundaryUv: [exactPoint(1, 3), exactPoint(5, 3), exactPoint(1, 8)],
        },
      },
    ],
    work: {
      testedFacePairs: 1,
      materialFaceCount: 2,
      sourceOverlapCellsAuthenticated: 0,
      overlapCellCount: 1,
      facePairOrderCount: 1,
      worldBoundaryPointCount: 6,
      exactBoundaryPointCount: 3,
    },
    readOnly: true,
    authorizesProjectMutation: false,
  }
}

test('the viewer parser accepts one complete response', () => {
  const parsed = normalize(response())
  assert.ok(parsed)
  assert.equal(parsed.faces.length, 2)
  assert.equal(parsed.cells.length, 1)
  assert.equal(parsed.pose.generation, '7')
  assert.equal(parsed.readOnly, true)
  assert.equal(parsed.authorizesProjectMutation, false)
  assert.equal(
    parsed.cells[0]?.projection.roundedBoundaryUvMm.length,
    parsed.cells[0]?.projection.exactBoundaryUv.length,
  )
})

test('the parser returns a detached deeply frozen value', () => {
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

  raw.faces[0]!.faceId = FACE_B
  raw.cells[0]!.projection.roundedBoundaryUvMm[0]![0] = 999
  raw.pose.generation = '9'
  assert.equal(parsed.faces[0]?.faceId, FACE_A)
  assert.equal(parsed.cells[0]?.projection.roundedBoundaryUvMm[0]?.[0], 1)
  assert.equal(parsed.pose.generation, '7')
})

test('the parser rejects malformed viewer responses', () => {
  const mutate = (
    change: (value: ReturnType<typeof response>) => unknown,
  ) => {
    const value = response()
    const replaced = change(value)
    return replaced === undefined ? value : replaced
  }
  const rejected: unknown[] = [
    null,
    [],
    'view',
    mutate((value) => {
      value.version = 2
      return undefined
    }),
    mutate((value) => {
      value.modelId = 'other_model'
      return undefined
    }),
    mutate((value) => {
      value.readOnly = false
      return undefined
    }),
    mutate((value) => {
      value.authorizesProjectMutation = true
      return undefined
    }),
    mutate((value) => {
      value.foldModelFingerprintSha256 = 'A'.repeat(64)
      return undefined
    }),
    mutate((value) => {
      value.projectInstanceId = 'not-a-uuid'
      return undefined
    }),
    mutate((value) => {
      value.revision = -0
      return undefined
    }),
    mutate((value) => {
      value.pose.generation = '0'
      return undefined
    }),
    mutate((value) => {
      value.pose.generation = '007'
      return undefined
    }),
    mutate((value) => {
      value.pose.modelId = 'graph_pose_v1'
      return undefined
    }),
    // Every hinge is a flat endpoint, so the pose is not a non-flat pose.
    mutate((value) => {
      value.pose.hingeAngles = [
        { edgeId: EDGE_1, angleDegrees: 0 },
        { edgeId: EDGE_2, angleDegrees: 180 },
      ]
      return undefined
    }),
    mutate((value) => {
      value.pose.hingeAngles = [
        { edgeId: EDGE_2, angleDegrees: 73.5 },
        { edgeId: EDGE_1, angleDegrees: 0 },
      ]
      return undefined
    }),
    mutate((value) => {
      value.pose.hingeAngles[0]!.angleDegrees = 181
      return undefined
    }),
    mutate((value) => {
      value.faces = [value.faces[1]!, value.faces[0]!]
      return undefined
    }),
    mutate((value) => {
      value.faces[0]!.projection.planeAxes = ['y', 'z']
      return undefined
    }),
    mutate((value) => {
      value.faces[0]!.projection.droppedWorldAxis = 'w'
      return undefined
    }),
    mutate((value) => {
      value.faces[0]!.worldOuterBoundaryXyzMm = [[0, 0, 0], [1, 0, 0]]
      return undefined
    }),
    mutate((value) => {
      value.faces[0]!.worldOuterBoundaryXyzMm[0]![0] = Number.POSITIVE_INFINITY
      return undefined
    }),
    mutate((value) => {
      value.faces[0]!.projection.sourceToPlaneProjectionExact.m00 = {
        sign: 'positive',
        numeratorMagnitudeHex: '',
        denominatorMagnitudeHex: '01',
      }
      return undefined
    }),
    mutate((value) => {
      value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
        .denominatorMagnitudeHex = ''
      return undefined
    }),
    mutate((value) => {
      value.faces[0]!.projection.sourceToPlaneProjectionExact.m00
        .numeratorMagnitudeHex = '001'
      return undefined
    }),
    mutate((value) => {
      value.cells[0]!.upperFaceId = value.cells[0]!.lowerFaceId
      return undefined
    }),
    mutate((value) => {
      value.cells[0]!.upperFaceId = uuid(99)
      return undefined
    }),
    mutate((value) => {
      value.cells[0]!.projection.droppedWorldAxis = 'x'
      value.cells[0]!.projection.planeAxes = ['y', 'z']
      return undefined
    }),
    mutate((value) => {
      value.cells[0]!.projection.roundedBoundaryUvMm.pop()
      return undefined
    }),
    mutate((value) => {
      value.work.materialFaceCount = 3
      return undefined
    }),
    mutate((value) => {
      value.work.worldBoundaryPointCount = 5
      return undefined
    }),
    mutate((value) => {
      value.work.exactBoundaryPointCount = 4
      return undefined
    }),
    // A getter must never be executed or accepted as an own data property.
    mutate((value) => {
      const hostile: Record<string, unknown> = { ...value }
      Object.defineProperty(hostile, 'readOnly', {
        enumerable: true,
        get: () => true,
      })
      return hostile
    }),
    // Inherited values must not satisfy a required field.
    mutate((value) => {
      const { faces, ...rest } = value
      return Object.assign(Object.create({ faces }), rest)
    }),
    // Extra own properties on an array are refused.
    mutate((value) => {
      const faces = value.faces as unknown as Record<string, unknown>
      faces.extra = true
      return undefined
    }),
  ]
  for (const [index, value] of rejected.entries()) {
    assert.equal(normalize(value), null, `rejected fixture ${index}`)
  }
})
