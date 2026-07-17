import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import {
  calculateFoldTreePose,
  type FoldPreviewTreeKinematics,
} from '../src/lib/foldPreviewKinematics.ts'
import type { FoldPreviewHingeModel } from '../src/lib/foldPreviewModel.ts'

const firstHinge = hinge('hinge-1', 0, 1)
const secondHinge = hinge('hinge-2', 1, -1)

test('tree pose composes parent transforms and keeps both sides of every hinge coincident', () => {
  const tree: FoldPreviewTreeKinematics = {
    kind: 'tree',
    rootFaceId: 'west',
    joints: [
      {
        parentFaceId: 'west',
        childFaceId: 'middle',
        hinge: firstHinge,
        childRotationSign: 1,
      },
      {
        parentFaceId: 'middle',
        childFaceId: 'east',
        hinge: secondHinge,
        childRotationSign: -1,
      },
    ],
  }

  const pose = calculateFoldTreePose(tree, 90)

  assert.ok(pose)
  assert.equal(pose.faceTransforms.size, 3)
  assert.equal(pose.hingeTransforms.size, 2)
  const root = pose.faceTransforms.get('west')
  const middle = pose.faceTransforms.get('middle')
  const east = pose.faceTransforms.get('east')
  assert.ok(root && middle && east)
  assertPoint(new Vector3(0, 0, 0).applyMatrix4(root), [0, 0, 0])
  assertPoint(new Vector3(0, 0, 0).applyMatrix4(middle), [0, 0, 0])
  assertPoint(new Vector3(1, 0, 0).applyMatrix4(middle), [0, 1, 0])
  assertPoint(new Vector3(1, 0, 0).applyMatrix4(east), [0, 1, 0])
  assert.ok(pose.hingeTransforms.get('hinge-1')?.equals(root))
  assert.ok(pose.hingeTransforms.get('hinge-2')?.equals(middle))

  const middleStart = new Vector3(0.25, 0, 0).applyMatrix4(middle)
  const middleEnd = new Vector3(0.75, 0, 0).applyMatrix4(middle)
  assert.ok(Math.abs(middleStart.distanceTo(middleEnd) - 0.5) < 1e-12)
})

test('reversing traversal rotation changes the moving side but preserves the hinge axis', () => {
  const positive = oneJointTree(1)
  const negative = oneJointTree(-1)

  const positivePose = calculateFoldTreePose(positive, 90)
  const negativePose = calculateFoldTreePose(negative, 90)

  assert.ok(positivePose && negativePose)
  const point = new Vector3(1, 0, 0)
  assertPoint(point.clone().applyMatrix4(positivePose.faceTransforms.get('moving')!), [0, 1, 0])
  assertPoint(point.clone().applyMatrix4(negativePose.faceTransforms.get('moving')!), [0, -1, 0])
  const axisPoint = new Vector3(0, 0, 0.5)
  assertPoint(axisPoint.clone().applyMatrix4(positivePose.faceTransforms.get('moving')!), [0, 0, 0.5])
  assertPoint(axisPoint.clone().applyMatrix4(negativePose.faceTransforms.get('moving')!), [0, 0, 0.5])
})

test('invalid angles and non-topological joint orders fail closed', () => {
  assert.equal(calculateFoldTreePose(oneJointTree(1), Number.NaN), null)
  assert.equal(calculateFoldTreePose(oneJointTree(1), -1), null)
  assert.equal(calculateFoldTreePose(oneJointTree(1), 181), null)

  const childBeforeParent: FoldPreviewTreeKinematics = {
    kind: 'tree',
    rootFaceId: 'root',
    joints: [{
      parentFaceId: 'missing',
      childFaceId: 'child',
      hinge: firstHinge,
      childRotationSign: 1,
    }],
  }
  assert.equal(calculateFoldTreePose(childBeforeParent, 45), null)

  const duplicateChild: FoldPreviewTreeKinematics = {
    kind: 'tree',
    rootFaceId: 'root',
    joints: [
      {
        parentFaceId: 'root',
        childFaceId: 'child',
        hinge: firstHinge,
        childRotationSign: 1,
      },
      {
        parentFaceId: 'root',
        childFaceId: 'child',
        hinge: secondHinge,
        childRotationSign: -1,
      },
    ],
  }
  assert.equal(calculateFoldTreePose(duplicateChild, 45), null)

  const misaligned = oneJointTree(1)
  const forgedHinge = {
    ...misaligned.joints[0].hinge,
    axis: { x: 1, z: 0 },
  }
  assert.equal(calculateFoldTreePose({
    ...misaligned,
    joints: [{ ...misaligned.joints[0], hinge: forgedHinge }],
  }, 45), null)
})

test('non-commuting hinge rotations compose strictly parent before child', () => {
  const xAxisHinge: FoldPreviewHingeModel = {
    edgeId: 'x-axis',
    start: { vertexId: 'x-start', x: 0, z: 0 },
    end: { vertexId: 'x-end', x: 1, z: 0 },
    axis: { x: 1, z: 0 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const zAxisHinge: FoldPreviewHingeModel = {
    edgeId: 'z-axis',
    start: { vertexId: 'z-start', x: 1, z: 1 },
    end: { vertexId: 'z-end', x: 1, z: 2 },
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const tree: FoldPreviewTreeKinematics = {
    kind: 'tree',
    rootFaceId: 'root',
    joints: [
      {
        parentFaceId: 'root',
        childFaceId: 'middle',
        hinge: xAxisHinge,
        childRotationSign: 1,
      },
      {
        parentFaceId: 'middle',
        childFaceId: 'leaf',
        hinge: zAxisHinge,
        childRotationSign: 1,
      },
    ],
  }

  const pose = calculateFoldTreePose(tree, 90)

  assert.ok(pose)
  assertPoint(
    new Vector3(2, 0, 1).applyMatrix4(pose.faceTransforms.get('leaf')!),
    [1, -1, 1],
  )
})

test('poses are history-independent and sibling branches do not inherit each other', () => {
  const tree: FoldPreviewTreeKinematics = {
    kind: 'tree',
    rootFaceId: 'root',
    joints: [
      {
        parentFaceId: 'root',
        childFaceId: 'left-branch',
        hinge: firstHinge,
        childRotationSign: 1,
      },
      {
        parentFaceId: 'root',
        childFaceId: 'right-branch',
        hinge: secondHinge,
        childRotationSign: -1,
      },
    ],
  }
  const flat = calculateFoldTreePose(tree, 0)
  const firstNinety = calculateFoldTreePose(tree, 90)
  assert.ok(calculateFoldTreePose(tree, 30))
  const secondNinety = calculateFoldTreePose(tree, 90)

  assert.ok(flat && firstNinety && secondNinety)
  for (const transform of flat.faceTransforms.values()) {
    assert.ok(transform.equals(new Matrix4()))
  }
  assert.deepEqual(
    [...secondNinety.faceTransforms].map(([faceId, matrix]) => [faceId, matrix.elements]),
    [...firstNinety.faceTransforms].map(([faceId, matrix]) => [faceId, matrix.elements]),
  )
  const standaloneRight = calculateFoldTreePose({
    kind: 'tree',
    rootFaceId: 'root',
    joints: [{
      parentFaceId: 'root',
      childFaceId: 'right-branch',
      hinge: secondHinge,
      childRotationSign: -1,
    }],
  }, 90)
  assert.ok(standaloneRight)
  assert.deepEqual(
    secondNinety.faceTransforms.get('right-branch')?.elements,
    standaloneRight.faceTransforms.get('right-branch')?.elements,
  )
})

function oneJointTree(childRotationSign: 1 | -1): FoldPreviewTreeKinematics {
  return {
    kind: 'tree',
    rootFaceId: 'fixed',
    joints: [{
      parentFaceId: 'fixed',
      childFaceId: 'moving',
      hinge: hinge('one', 0, 1),
      childRotationSign,
    }],
  }
}

function hinge(
  edgeId: string,
  x: number,
  rotationSign: 1 | -1,
): FoldPreviewHingeModel {
  return {
    edgeId,
    start: { vertexId: `${edgeId}-start`, x, z: -1 },
    end: { vertexId: `${edgeId}-end`, x, z: 1 },
    axis: { x: 0, z: 1 },
    assignment: rotationSign === 1 ? 'mountain' : 'valley',
    rotationSign,
  }
}

function assertPoint(point: Vector3, expected: readonly [number, number, number]) {
  const actual = point.toArray()
  for (let index = 0; index < 3; index += 1) {
    assert.ok(Math.abs(actual[index] - expected[index]) < 1e-12, `${actual} != ${expected}`)
  }
}
