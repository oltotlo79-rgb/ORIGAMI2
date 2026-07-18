import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import {
  makeFoldPreviewCanonicalAxisRotation,
  makeFoldPreviewCanonicalPivotMatrix,
} from '../src/lib/foldPreviewCanonicalRotation.ts'
import { calculateSingleFoldPose } from '../src/lib/foldPreviewSingleFoldKinematics.ts'

test('cardinal axis rotations use exact zero and unit sine/cosine values', () => {
  const axis = new Vector3(0, 0, 1)
  const cases = [
    [0, [1, 0, 0]],
    [Math.PI / 2, [0, 1, 0]],
    [-Math.PI / 2, [0, -1, 0]],
    [Math.PI, [-1, 0, 0]],
    [-Math.PI, [-1, 0, 0]],
  ] as const
  for (const [radians, expected] of cases) {
    const rotation = makeFoldPreviewCanonicalAxisRotation(axis, radians)
    assert.ok(rotation)
    assert.deepEqual(
      new Vector3(1, 0, 0).applyMatrix4(rotation).toArray(),
      expected,
    )
  }
})

test('a nearby non-cardinal angle is never rounded to exact 90 degrees', () => {
  const radians = 89.999999 * Math.PI / 180
  const rotation = makeFoldPreviewCanonicalAxisRotation(
    new Vector3(0, 0, 1),
    radians,
  )
  assert.ok(rotation)
  const transformed = new Vector3(1, 0, 0).applyMatrix4(rotation)
  assert.notEqual(transformed.x, 0)
  assert.notEqual(transformed.y, 1)
  assert.ok(transformed.x > 0)
})

test('single-fold scene and collision matrices are identical at cardinal angles', () => {
  const start = { vertexId: 'start', x: 200, z: 0 }
  const end = {
    vertexId: 'end',
    x: 0,
    z: 200 * Math.sqrt(3),
  }
  const fixedFace = {
    id: 'fixed',
    polygon: [
      start,
      { vertexId: 'fixed-corner', x: 0, z: 0 },
      end,
    ],
  }
  const movingFace = {
    id: 'moving',
    polygon: [
      end,
      { vertexId: 'moving-corner', x: 400, z: 400 },
      start,
    ],
  }
  const model = {
    kind: 'single_fold',
    projectId: 'project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 200, y: 200 },
    worldBounds: { minX: 0, minZ: 0, maxX: 400, maxZ: 400 },
    faces: [fixedFace, movingFace],
    fixedFace,
    movingFace,
    hinge: {
      edgeId: 'hinge',
      leftFaceId: fixedFace.id,
      rightFaceId: movingFace.id,
      start,
      end,
      axis: {
        x: end.x - start.x,
        z: end.z - start.z,
      },
      assignment: 'mountain',
      rotationSign: 1,
    },
  } as const
  const axis = new Vector3(
    end.x - start.x,
    0,
    end.z - start.z,
  ).normalize()

  for (const angle of [90, 180]) {
    const pose = calculateSingleFoldPose(model, fixedFace.id, angle)
    assert.ok(pose)
    const sceneMatrix = makeFoldPreviewCanonicalPivotMatrix(
      axis,
      { x: start.x, y: 0, z: start.z },
      pose.signedAngleRadians,
    )
    assert.ok(sceneMatrix)
    const sceneRestTransform = sceneMatrix.clone().multiply(
      new Matrix4().makeTranslation(-start.x, 0, -start.z),
    )
    assert.deepEqual(
      sceneRestTransform.elements,
      pose.faceTransforms.get(movingFace.id)?.elements,
      `${angle} degrees`,
    )
  }
})
