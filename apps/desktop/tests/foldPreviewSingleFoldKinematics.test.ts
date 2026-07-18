import assert from 'node:assert/strict'
import test from 'node:test'

import { Vector3 } from 'three'
import { calculateSingleFoldPose } from '../src/lib/foldPreviewSingleFoldKinematics.ts'
import type {
  FoldPreviewFaceModel,
  SingleFoldPreviewModel,
} from '../src/lib/foldPreviewModel.ts'

test('either single-fold face can be fixed while hinge points stay coincident', () => {
  const model = singleFoldModel()
  const leftFixed = calculateSingleFoldPose(model, 'left', 90)
  const rightFixed = calculateSingleFoldPose(model, 'right', 90)
  assert.ok(leftFixed && rightFixed)

  assertPoint(
    new Vector3(1, 0, 0).applyMatrix4(leftFixed.faceTransforms.get('right')!),
    [0, 1, 0],
  )
  assertPoint(
    new Vector3(-1, 0, 0).applyMatrix4(rightFixed.faceTransforms.get('left')!),
    [0, 1, 0],
  )
  assert.equal(leftFixed.signedAngleRadians, Math.PI / 2)
  assert.equal(rightFixed.signedAngleRadians, -Math.PI / 2)

  for (const pose of [leftFixed, rightFixed]) {
    for (const point of [model.hinge.start, model.hinge.end]) {
      const rest = new Vector3(point.x, 0, point.z)
      assertPoint(
        rest.clone().applyMatrix4(pose.faceTransforms.get(pose.fixedFaceId)!),
        rest.toArray(),
      )
      assertPoint(
        rest.clone().applyMatrix4(pose.faceTransforms.get(pose.movingFaceId)!),
        rest.toArray(),
      )
    }
  }
})

test('single-fold kinematics emits exact 90 and 180 degree collision poses', () => {
  const model = singleFoldModel()
  const ninety = calculateSingleFoldPose(model, 'left', 90)
  const flatFold = calculateSingleFoldPose(model, 'left', 180)
  const nearby = calculateSingleFoldPose(model, 'left', 89.999999)
  assert.ok(ninety && flatFold && nearby)
  const point = new Vector3(1, 0, 0)
  assert.deepEqual(
    point.clone().applyMatrix4(
      ninety.faceTransforms.get('right')!,
    ).toArray(),
    [0, 1, 0],
  )
  assert.deepEqual(
    point.clone().applyMatrix4(
      flatFold.faceTransforms.get('right')!,
    ).toArray(),
    [-1, 0, 0],
  )
  assert.notEqual(
    point.clone().applyMatrix4(
      nearby.faceTransforms.get('right')!,
    ).x,
    0,
  )
})

test('valley assignment reverses the canonical moving-side rotation', () => {
  const model = singleFoldModel()
  const valley: SingleFoldPreviewModel = {
    ...model,
    hinge: {
      ...model.hinge,
      assignment: 'valley',
      rotationSign: -1,
    },
  }
  const pose = calculateSingleFoldPose(valley, 'left', 90)
  assert.ok(pose)
  assert.equal(pose.signedAngleRadians, -Math.PI / 2)
  assertPoint(
    new Vector3(1, 0, 0).applyMatrix4(pose.faceTransforms.get('right')!),
    [0, -1, 0],
  )
})

test('single-fold poses are history-independent and return fresh transforms', () => {
  const model = singleFoldModel()
  const first = calculateSingleFoldPose(model, 'left', 67)
  const intermediate = calculateSingleFoldPose(model, 'left', 12)
  const repeated = calculateSingleFoldPose(model, 'left', 67)
  assert.ok(first && intermediate && repeated)
  assert.notStrictEqual(
    first.faceTransforms.get('right'),
    repeated.faceTransforms.get('right'),
  )
  assert.ok(first.faceTransforms.get('right')?.equals(
    repeated.faceTransforms.get('right')!,
  ))
})

test('invalid angles, anchors, and forged hinge geometry fail closed', () => {
  const model = singleFoldModel()
  for (const angle of [Number.NaN, Number.POSITIVE_INFINITY, -1, 181]) {
    assert.equal(calculateSingleFoldPose(model, 'left', angle), null)
  }
  assert.equal(calculateSingleFoldPose(model, 'missing', 45), null)
  assert.equal(calculateSingleFoldPose({
    ...model,
    hinge: {
      ...model.hinge,
      axis: { x: 1, z: 0 },
    },
  }, 'left', 45), null)
  assert.equal(calculateSingleFoldPose({
    ...model,
    hinge: {
      ...model.hinge,
      end: { ...model.hinge.start },
    },
  }, 'left', 45), null)
})

function singleFoldModel(): SingleFoldPreviewModel {
  const left: FoldPreviewFaceModel = {
    id: 'left',
    polygon: [
      { vertexId: 'start', x: 0, z: -1 },
      { vertexId: 'left', x: -1, z: 0 },
      { vertexId: 'end', x: 0, z: 1 },
    ],
  }
  const right: FoldPreviewFaceModel = {
    id: 'right',
    polygon: [
      { vertexId: 'end', x: 0, z: 1 },
      { vertexId: 'right', x: 1, z: 0 },
      { vertexId: 'start', x: 0, z: -1 },
    ],
  }
  return {
    kind: 'single_fold',
    projectId: 'project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: -1, minZ: -1, maxX: 1, maxZ: 1 },
    faces: [left, right],
    fixedFace: left,
    movingFace: right,
    hinge: {
      edgeId: 'hinge',
      leftFaceId: 'left',
      rightFaceId: 'right',
      start: left.polygon[0],
      end: left.polygon[2],
      axis: { x: 0, z: 1 },
      assignment: 'mountain',
      rotationSign: 1,
    },
  }
}

function assertPoint(actual: Vector3, expected: readonly number[]) {
  assert.equal(expected.length, 3)
  assert.ok(
    actual.distanceTo(new Vector3(expected[0], expected[1], expected[2])) < 1e-12,
    `${actual.toArray()} != ${expected}`,
  )
}
