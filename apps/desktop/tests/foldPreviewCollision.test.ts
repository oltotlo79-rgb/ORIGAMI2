import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4 } from 'three'
import {
  findFoldPreviewBroadPhaseCandidates,
  findFoldPreviewPoseBroadPhaseCandidates,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
  type FoldPreviewCollisionFace,
} from '../src/lib/foldPreviewCollision.ts'

const square = Object.freeze([
  { x: 0, z: 0 },
  { x: 1, z: 0 },
  { x: 1, z: 1 },
  { x: 0, z: 1 },
])

test('separated face prisms do not become broad-phase candidates', () => {
  const result = findFoldPreviewBroadPhaseCandidates([
    face('left'),
    face('right', new Matrix4().makeTranslation(2, 0, 0)),
  ], [])
  assert.ok(result)
  assert.equal(result.bounds.length, 2)
  assert.deepEqual(result.candidates, [])
})

test('world transforms and paper thickness contribute to conservative bounds', () => {
  const rotated = new Matrix4()
    .makeTranslation(0.5, 0, 0)
    .multiply(new Matrix4().makeRotationZ(Math.PI / 2))
  const result = findFoldPreviewBroadPhaseCandidates([
    face('fixed', new Matrix4(), 0.1),
    face('moving', rotated, 0.1),
  ], [])
  assert.ok(result)
  assert.equal(result.candidates.length, 1)
  assert.equal(result.candidates[0].relation, 'non_adjacent')
  assert.ok(result.candidates[0].overlap.x > 0)
  assert.ok(result.candidates[0].overlap.y > 0)
})

test('touching bounds remain candidates for the later narrow phase', () => {
  const result = findFoldPreviewBroadPhaseCandidates([
    face('first'),
    face('second', new Matrix4().makeTranslation(1, 0, 0)),
  ], [])
  assert.ok(result)
  assert.equal(result.candidates.length, 1)
  assert.equal(result.candidates[0].touching, true)
  assert.equal(result.candidates[0].overlap.x, 0)
})

test('hinge neighbours are classified instead of unsafely discarded', () => {
  const result = findFoldPreviewBroadPhaseCandidates([
    face('a'),
    face('b'),
  ], [
    { edgeId: 'hinge-z', firstFaceId: 'b', secondFaceId: 'a' },
    { edgeId: 'hinge-a', firstFaceId: 'a', secondFaceId: 'b' },
  ])
  assert.ok(result)
  assert.deepEqual(result.candidates, [{
    firstFaceId: 'a',
    secondFaceId: 'b',
    relation: 'hinge_adjacent',
    hingeEdgeIds: ['hinge-a', 'hinge-z'],
    overlap: { x: 1, y: 0.1, z: 1 },
    touching: false,
  }])
})

test('face and adjacency permutations produce the same ordered result', () => {
  const faces = [
    face('c', new Matrix4().makeTranslation(0.5, 0, 0)),
    face('a'),
    face('b', new Matrix4().makeTranslation(0.25, 0, 0)),
  ]
  const adjacency = [
    { edgeId: 'edge-ca', firstFaceId: 'c', secondFaceId: 'a' },
    { edgeId: 'edge-ab', firstFaceId: 'a', secondFaceId: 'b' },
  ]
  const forward = findFoldPreviewBroadPhaseCandidates(faces, adjacency)
  const reversed = findFoldPreviewBroadPhaseCandidates(
    [...faces].reverse(),
    [...adjacency].reverse(),
  )
  assert.ok(forward)
  assert.ok(reversed)
  assert.deepEqual(reversed, forward)
})

test('malformed faces, transforms, and adjacency fail closed', () => {
  const perspective = new Matrix4()
  perspective.elements[3] = 0.25
  assert.equal(findFoldPreviewBroadPhaseCandidates([
    face('duplicate'),
    face('duplicate'),
  ], []), null)
  assert.equal(findFoldPreviewBroadPhaseCandidates([
    { ...face('bad-point'), polygon: [{ x: 0, z: 0 }, { x: 1, z: 0 }, { x: 0, z: Number.NaN }] },
  ], []), null)
  assert.equal(findFoldPreviewBroadPhaseCandidates([
    face('perspective', perspective),
  ], []), null)
  assert.equal(findFoldPreviewBroadPhaseCandidates([
    face('a'),
    face('b'),
  ], [
    { edgeId: 'unknown-face', firstFaceId: 'a', secondFaceId: 'missing' },
  ]), null)
})

test('the documented 10,000-face sparse workload stays bounded and deterministic', () => {
  const faces = Array.from({ length: MAX_FOLD_PREVIEW_COLLISION_FACES }, (_, index) =>
    face(
      `face-${index.toString().padStart(5, '0')}`,
      new Matrix4().makeTranslation(index * 2, 0, 0),
    ))
  const result = findFoldPreviewBroadPhaseCandidates(faces, [])
  assert.ok(result)
  assert.equal(result.bounds.length, MAX_FOLD_PREVIEW_COLLISION_FACES)
  assert.deepEqual(result.candidates, [])
  assert.equal(findFoldPreviewBroadPhaseCandidates([
    ...faces,
    face('over-limit'),
  ], []), null)
})

test('ordering uses stable code units and floating-point margin keeps near contacts', () => {
  const nearContact = new Matrix4().makeTranslation(1 + Number.EPSILON * 16, 0, 0)
  const result = findFoldPreviewBroadPhaseCandidates([
    face('ä', nearContact),
    face('z'),
  ], [])
  assert.ok(result)
  assert.deepEqual(result.bounds.map(({ faceId }) => faceId), ['z', 'ä'])
  assert.equal(result.candidates.length, 1)
  assert.equal(result.candidates[0].touching, true)
})

test('huge translations cannot relax affine validation and dense output fails closed', () => {
  const invalid = new Matrix4().makeTranslation(1e200, 0, 0)
  invalid.elements[3] = 1
  assert.equal(findFoldPreviewBroadPhaseCandidates([
    face('invalid', invalid),
  ], []), null)

  const dense = Array.from({ length: 449 }, (_, index) => face(`dense-${index}`))
  assert.equal(findFoldPreviewBroadPhaseCandidates(dense, []), null)
})

test('pose adapter requires an exact complete transform snapshot', () => {
  const faces = [
    { id: 'fixed', polygon: square },
    { id: 'moving', polygon: square },
  ]
  const complete = new Map([
    ['fixed', new Matrix4()],
    ['moving', new Matrix4().makeTranslation(2, 0, 0)],
  ])
  const result = findFoldPreviewPoseBroadPhaseCandidates(faces, complete, 0.1, [])
  assert.ok(result)
  assert.deepEqual(result.candidates, [])
  assert.equal(findFoldPreviewPoseBroadPhaseCandidates(
    faces,
    new Map([['fixed', new Matrix4()]]),
    0.1,
    [],
  ), null)
  assert.equal(findFoldPreviewPoseBroadPhaseCandidates(
    faces,
    new Map([
      ['fixed', new Matrix4()],
      ['stale', new Matrix4()],
    ]),
    0.1,
    [],
  ), null)
})

function face(
  faceId: string,
  transform = new Matrix4(),
  thickness = 0.1,
): FoldPreviewCollisionFace {
  return { faceId, polygon: square, transform, thickness }
}
