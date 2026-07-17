import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import {
  rerootFoldPreviewTree,
  resolveSingleFoldAnchor,
} from '../src/lib/foldPreviewAnchoring.ts'
import {
  calculateFoldTreePose,
  calculateFoldTreePoseWithAngles,
  type FoldPreviewTreeKinematics,
} from '../src/lib/foldPreviewKinematics.ts'
import type {
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
  SingleFoldPreviewModel,
} from '../src/lib/foldPreviewModel.ts'

const firstHinge = hinge('hinge-1', 0, 1)
const secondHinge = hinge('hinge-2', 1, -1)

test('rerooting a chain reverses exactly the traversed joint signs', () => {
  const tree = chainTree()

  assert.strictEqual(rerootFoldPreviewTree(tree, 'west'), tree)
  assert.deepEqual(rerootFoldPreviewTree(tree, 'middle'), {
    kind: 'tree',
    rootFaceId: 'middle',
    joints: [
      {
        parentFaceId: 'middle',
        childFaceId: 'west',
        hinge: firstHinge,
        childRotationSign: -1,
      },
      {
        parentFaceId: 'middle',
        childFaceId: 'east',
        hinge: secondHinge,
        childRotationSign: -1,
      },
    ],
  })
  assert.deepEqual(rerootFoldPreviewTree(tree, 'east'), {
    kind: 'tree',
    rootFaceId: 'east',
    joints: [
      {
        parentFaceId: 'east',
        childFaceId: 'middle',
        hinge: secondHinge,
        childRotationSign: 1,
      },
      {
        parentFaceId: 'middle',
        childFaceId: 'west',
        hinge: firstHinge,
        childRotationSign: -1,
      },
    ],
  })
})

test('rerooted non-commuting poses differ only by the new fixed world frame', () => {
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
    assignment: 'valley',
    rotationSign: -1,
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
        childRotationSign: -1,
      },
    ],
  }
  const rerooted = rerootFoldPreviewTree(tree, 'leaf')
  const originalPose = calculateFoldTreePose(tree, 67)
  const rerootedPose = rerooted && calculateFoldTreePose(rerooted, 67)

  assert.ok(rerooted && originalPose && rerootedPose)
  const normalization = originalPose.faceTransforms.get('leaf')?.clone().invert()
  assert.ok(normalization)
  for (const [faceId, transform] of originalPose.faceTransforms) {
    const expected = normalization.clone().multiply(transform)
    const actual = rerootedPose.faceTransforms.get(faceId)
    assert.ok(actual)
    assertMatrix(actual, expected)
  }
  for (const joint of rerooted.joints) {
    const parent = rerootedPose.faceTransforms.get(joint.parentFaceId)
    const child = rerootedPose.faceTransforms.get(joint.childFaceId)
    assert.ok(parent && child)
    for (const point of [joint.hinge.start, joint.hinge.end]) {
      const restPoint = new Vector3(point.x, 0, point.z)
      assertPointEqual(restPoint.clone().applyMatrix4(parent), restPoint.clone().applyMatrix4(child))
    }
  }
})

test('rerooting preserves per-hinge angle ownership independently of input order', () => {
  const rerooted = rerootFoldPreviewTree(chainTree(), 'east')
  assert.ok(rerooted)
  const forward = calculateFoldTreePoseWithAngles(rerooted, {
    kind: 'per_hinge',
    angles: [
      { edgeId: 'hinge-1', angleDegrees: 25 },
      { edgeId: 'hinge-2', angleDegrees: 80 },
    ],
  })
  const reverse = calculateFoldTreePoseWithAngles(rerooted, {
    kind: 'per_hinge',
    angles: [
      { edgeId: 'hinge-2', angleDegrees: 80 },
      { edgeId: 'hinge-1', angleDegrees: 25 },
    ],
  })

  assert.ok(forward && reverse)
  for (const [faceId, transform] of forward.faceTransforms) {
    assertMatrix(transform, reverse.faceTransforms.get(faceId)!)
  }
})

test('rerooting rejects unknown roots and malformed source trees', () => {
  const tree = chainTree()
  assert.equal(rerootFoldPreviewTree(tree, 'unknown'), null)
  assert.equal(rerootFoldPreviewTree({ ...tree, rootFaceId: '' }, 'west'), null)
  assert.equal(rerootFoldPreviewTree({
    kind: 'tree',
    rootFaceId: 'root',
    joints: [{
      parentFaceId: 'missing',
      childFaceId: 'child',
      hinge: firstHinge,
      childRotationSign: 1,
    }],
  }, 'root'), null)
  assert.equal(rerootFoldPreviewTree({
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
  }, 'root'), null)
  assert.equal(rerootFoldPreviewTree({
    kind: 'tree',
    rootFaceId: 'root',
    joints: [
      {
        parentFaceId: 'root',
        childFaceId: 'first',
        hinge: firstHinge,
        childRotationSign: 1,
      },
      {
        parentFaceId: 'root',
        childFaceId: 'second',
        hinge: { ...secondHinge, edgeId: firstHinge.edgeId },
        childRotationSign: -1,
      },
    ],
  }, 'root'), null)
  assert.equal(rerootFoldPreviewTree({
    kind: 'tree',
    rootFaceId: 'root',
    joints: [{
      parentFaceId: 'root',
      childFaceId: 'child',
      hinge: { ...firstHinge, axis: { x: 1, z: 0 } },
      childRotationSign: 1,
    }],
  }, 'root'), null)
  assert.equal(rerootFoldPreviewTree({
    kind: 'tree',
    rootFaceId: 'root',
    joints: [{
      parentFaceId: 'root',
      childFaceId: 'child',
      hinge: firstHinge,
      childRotationSign: 0 as 1,
    }],
  }, 'root'), null)
})

test('single-fold anchoring flips only the moving-side rotation sign', () => {
  const model = singleFoldModel()
  assert.deepEqual(resolveSingleFoldAnchor(model, 'left'), {
    fixedFace: model.faces[0],
    movingFace: model.faces[1],
    movingRotationSign: 1,
  })
  assert.deepEqual(resolveSingleFoldAnchor(model, 'right'), {
    fixedFace: model.faces[1],
    movingFace: model.faces[0],
    movingRotationSign: -1,
  })
  assert.equal(resolveSingleFoldAnchor(model, 'unknown'), null)
  assert.equal(resolveSingleFoldAnchor(model, ''), null)
  assert.equal(resolveSingleFoldAnchor({
    ...model,
    faces: [model.faces[0], model.faces[0]],
  }, 'left'), null)

  const valleyModel: SingleFoldPreviewModel = {
    ...model,
    hinge: {
      ...model.hinge,
      assignment: 'valley',
      rotationSign: -1,
    },
  }
  assert.equal(resolveSingleFoldAnchor(valleyModel, 'left')?.movingRotationSign, -1)
  assert.equal(resolveSingleFoldAnchor(valleyModel, 'right')?.movingRotationSign, 1)
})

function chainTree(): FoldPreviewTreeKinematics {
  return {
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
}

function singleFoldModel(): SingleFoldPreviewModel {
  const left: FoldPreviewFaceModel = {
    id: 'left',
    polygon: [
      { vertexId: 'a', x: -1, z: -1 },
      { vertexId: 'b', x: 0, z: -1 },
      { vertexId: 'c', x: 0, z: 1 },
    ],
  }
  const right: FoldPreviewFaceModel = {
    id: 'right',
    polygon: [
      { vertexId: 'b', x: 0, z: -1 },
      { vertexId: 'd', x: 1, z: 1 },
      { vertexId: 'c', x: 0, z: 1 },
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
    hinge: hinge('single', 0, 1),
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

function assertMatrix(actual: Matrix4, expected: Matrix4) {
  for (let index = 0; index < actual.elements.length; index += 1) {
    assert.ok(
      Math.abs(actual.elements[index] - expected.elements[index]) < 1e-12,
      `matrix[${index}] ${actual.elements[index]} != ${expected.elements[index]}`,
    )
  }
}

function assertPointEqual(actual: Vector3, expected: Vector3) {
  assert.ok(actual.distanceTo(expected) < 1e-12, `${actual.toArray()} != ${expected.toArray()}`)
}
