import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import { findFoldPreviewNarrowPhaseInteractions } from '../src/lib/foldPreviewNarrowCollision.ts'

const square = [
  { x: 0, z: 0 },
  { x: 1, z: 0 },
  { x: 1, z: 1 },
  { x: 0, z: 1 },
] as const

test('triangle prisms remove an AABB false positive', () => {
  const faces = [
    face('lower', [{ x: 0, z: 0 }, { x: 2, z: 0 }, { x: 0, z: 2 }]),
    face('upper', [{ x: 2, z: 2 }, { x: 2, z: 1.1 }, { x: 1.1, z: 2 }]),
  ]
  const result = analyze(faces)
  assert.ok(result)
  assert.equal(result.broadPhaseCandidates, 1)
  assert.deepEqual(result.interactions, [])
  assert.equal(result.trianglePairTests, 1)
  assert.equal(result.satTests, 1)
})

test('overlapping non-adjacent paper volumes are penetrating', () => {
  const result = analyze([face('a'), face('b')])
  assert.ok(result)
  assert.deepEqual(result.interactions, [{
    firstFaceId: 'a',
    secondFaceId: 'b',
    relation: 'non_adjacent',
    hingeEdgeIds: [],
    geometryClass: 'penetrating',
  }])
})

test('side and face contacts remain touching instead of becoming penetrations', () => {
  const sideContact = analyze(
    [face('a'), face('b')],
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4().makeTranslation(1, 0, 0)],
    ]),
  )
  const faceContact = analyze(
    [face('a'), face('b')],
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4().makeTranslation(0, 0.1, 0)],
    ]),
  )
  assert.ok(sideContact && faceContact)
  assert.equal(sideContact.interactions[0]?.geometryClass, 'touching')
  assert.equal(faceContact.interactions[0]?.geometryClass, 'touching')
})

test('crossing folded sheets produce a non-adjacent penetration', () => {
  const crossing = new Matrix4()
    .makeTranslation(0.5, 0, 0)
    .multiply(new Matrix4().makeRotationZ(Math.PI / 3))
  const result = analyze(
    [face('horizontal'), face('vertical')],
    new Map([
      ['horizontal', new Matrix4()],
      ['vertical', crossing],
    ]),
  )
  assert.ok(result)
  assert.equal(result.interactions.length, 1)
  assert.equal(result.interactions[0].geometryClass, 'penetrating')
})

test('shared hinges stay tagged for the later origami contact policy', () => {
  const adjacency: FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'hinge',
    firstFaceId: 'left',
    secondFaceId: 'right',
  }]
  const result = analyze(
    [face('left'), face('right')],
    new Map([
      ['left', new Matrix4()],
      ['right', new Matrix4().makeTranslation(1, 0, 0)],
    ]),
    0.1,
    adjacency,
  )
  assert.ok(result)
  assert.deepEqual(result.interactions, [{
    firstFaceId: 'left',
    secondFaceId: 'right',
    relation: 'hinge_adjacent',
    hingeEdgeIds: ['hinge'],
    geometryClass: 'touching',
  }])

  const unresolvedOverlap = analyze(
    [face('left'), face('right')],
    undefined,
    0.1,
    adjacency,
  )
  assert.ok(unresolvedOverlap)
  assert.equal(unresolvedOverlap.interactions[0]?.relation, 'hinge_adjacent')
  assert.equal(unresolvedOverlap.interactions[0]?.geometryClass, 'penetrating')
})

test('zero-thickness candidates remain explicitly indeterminate', () => {
  const result = analyze([face('a'), face('b')], undefined, 0)
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'indeterminate')
  assert.equal(result.trianglePairTests, 0)
  assert.equal(result.satTests, 0)
})

test('near-parallel numerical axes do not produce a false penetration claim', () => {
  const almostParallel = new Matrix4().makeRotationY(Number.EPSILON * 32)
  const result = analyze(
    [face('a'), face('b')],
    new Map([
      ['a', new Matrix4()],
      ['b', almostParallel],
    ]),
  )
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'indeterminate')
})

test('winding and a shared rigid world transform do not change classification', () => {
  const reversedSquare = [...square].reverse()
  const world = new Matrix4()
    .makeTranslation(4, -2, 3)
    .multiply(new Matrix4().makeRotationY(0.73))
  const result = analyze(
    [face('a'), face('b', reversedSquare)],
    new Map([
      ['a', world.clone()],
      ['b', world.clone()],
    ]),
  )
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
})

test('face and adjacency input order do not change narrow-phase output', () => {
  const faces = [face('b'), face('a')]
  const transforms = new Map([
    ['b', new Matrix4()],
    ['a', new Matrix4()],
  ])
  const adjacency = [{
    edgeId: 'edge',
    firstFaceId: 'b',
    secondFaceId: 'a',
  }]
  const forward = analyze(faces, transforms, 0.1, adjacency)
  const reversed = analyze(
    [...faces].reverse(),
    new Map([...transforms].reverse()),
    0.1,
    [{ ...adjacency[0], firstFaceId: 'a', secondFaceId: 'b' }],
  )
  assert.ok(forward && reversed)
  assert.deepEqual(reversed, forward)
})

test('partial poses, scaling, and singular face transforms fail closed', () => {
  const faces = [face('a'), face('b')]
  assert.equal(findFoldPreviewNarrowPhaseInteractions(
    faces,
    new Map([['a', new Matrix4()]]),
    0.1,
    [],
  ), null)
  assert.equal(analyze(
    faces,
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4().makeScale(0, 0, 0)],
    ]),
  ), null)
  assert.equal(analyze(
    faces,
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4().makeScale(1.001, 1, 1)],
    ]),
  ), null)
})

function analyze(
  faces: readonly FoldPreviewCollisionPoseFace[],
  transforms = new Map(faces.map((item) => [item.id, new Matrix4()])),
  thickness = 0.1,
  adjacencies: readonly FoldPreviewCollisionAdjacency[] = [],
) {
  return findFoldPreviewNarrowPhaseInteractions(
    faces,
    transforms,
    thickness,
    adjacencies,
  )
}

function face(
  id: string,
  polygon: FoldPreviewCollisionPoseFace['polygon'] = square,
): FoldPreviewCollisionPoseFace {
  return { id, polygon }
}
