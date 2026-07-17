import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import {
  MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
  findFoldPreviewNarrowPhaseInteractions,
  prepareFoldPreviewNarrowPhase,
} from '../src/lib/foldPreviewNarrowCollision.ts'

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

test('prepared analysis is equivalent to the one-shot compatibility API', () => {
  const faces = [face('left'), face('right')]
  const transforms = new Map([
    ['left', new Matrix4()],
    ['right', new Matrix4().makeTranslation(1, 0, 0)],
  ])
  const adjacencies = [{
    edgeId: 'hinge',
    firstFaceId: 'left',
    secondFaceId: 'right',
  }]
  const prepared = prepareFoldPreviewNarrowPhase(faces, adjacencies)
  assert.ok(prepared)
  assert.deepEqual(
    prepared.analyze(transforms, 0.1),
    findFoldPreviewNarrowPhaseInteractions(
      faces,
      transforms,
      0.1,
      adjacencies,
    ),
  )
})

test('prepared geometry is a deep snapshot of faces and adjacencies', () => {
  const faces = [
    { id: 'left', polygon: square.map((point) => ({ ...point })) },
    { id: 'right', polygon: square.map((point) => ({ ...point })) },
  ]
  const adjacencies = [{
    edgeId: 'hinge',
    firstFaceId: 'left',
    secondFaceId: 'right',
  }]
  const prepared = prepareFoldPreviewNarrowPhase(faces, adjacencies)
  assert.ok(prepared)
  const transforms = new Map([
    ['left', new Matrix4()],
    ['right', new Matrix4()],
  ])
  const expected = prepared.analyze(transforms, 0.1)
  assert.ok(expected)

  faces[0].id = 'mutated-left'
  faces[0].polygon[0].x = 100
  faces[1].polygon.reverse()
  faces.reverse()
  adjacencies[0].edgeId = 'mutated-hinge'
  adjacencies[0].firstFaceId = 'mutated-left'
  adjacencies.push({
    edgeId: 'extra',
    firstFaceId: 'mutated-left',
    secondFaceId: 'right',
  })

  assert.deepEqual(prepared.analyze(transforms, 0.1), expected)
  const exposedHingeIds = expected.interactions[0]?.hingeEdgeIds as string[]
  exposedHingeIds[0] = 'mutated-output'
  assert.deepEqual(
    prepared.analyze(transforms, 0.1)?.interactions[0]?.hingeEdgeIds,
    ['hinge'],
  )
})

test('prepared analysis recomputes the current pose and thickness on every call', () => {
  const prepared = prepareFoldPreviewNarrowPhase(
    [face('fixed'), face('moving')],
    [],
  )
  assert.ok(prepared)
  const movingTransform = new Matrix4()
  const transforms = new Map([
    ['fixed', new Matrix4()],
    ['moving', movingTransform],
  ])

  const overlapping = prepared.analyze(transforms, 0.1)
  assert.ok(overlapping)
  assert.equal(overlapping.interactions[0]?.geometryClass, 'penetrating')

  movingTransform.makeTranslation(3, 0, 0)
  const separatedPose = prepared.analyze(transforms, 0.1)
  assert.ok(separatedPose)
  assert.equal(separatedPose.broadPhaseCandidates, 0)
  assert.deepEqual(separatedPose.interactions, [])

  movingTransform.makeTranslation(0, 0.2, 0)
  const thinPaper = prepared.analyze(transforms, 0.1)
  const thickPaper = prepared.analyze(transforms, 0.5)
  assert.ok(thinPaper && thickPaper)
  assert.equal(thinPaper.broadPhaseCandidates, 0)
  assert.equal(
    thickPaper.interactions[0]?.geometryClass,
    'penetrating',
  )
})

test('prepared analysis fails closed for missing, extra, and non-rigid poses', () => {
  const prepared = prepareFoldPreviewNarrowPhase([face('a'), face('b')], [])
  assert.ok(prepared)
  assert.equal(prepared.analyze(
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4()],
    ]),
    Number.NaN,
  ), null)
  assert.equal(prepared.analyze(
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4()],
    ]),
    -0.1,
  ), null)
  assert.equal(prepared.analyze(
    new Map([['a', new Matrix4()]]),
    0.1,
  ), null)
  assert.equal(prepared.analyze(
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4()],
      ['extra', new Matrix4()],
    ]),
    0.1,
  ), null)
  assert.equal(prepared.analyze(
    new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4().makeScale(1.001, 1, 1)],
    ]),
    0.1,
  ), null)
  const projective = new Matrix4()
  projective.elements[3] = 0.01
  assert.equal(prepared.analyze(
    new Map([
      ['a', new Matrix4()],
      ['b', projective],
    ]),
    0.1,
  ), null)
})

test('preparation rejects malformed static geometry and adjacency snapshots', () => {
  assert.equal(prepareFoldPreviewNarrowPhase(
    [face('duplicate'), face('duplicate')],
    [],
  ), null)
  assert.equal(prepareFoldPreviewNarrowPhase(
    [face('degenerate', [
      { x: 0, z: 0 },
      { x: 1, z: 0 },
      { x: 2, z: 0 },
    ])],
    [],
  ), null)
  assert.equal(prepareFoldPreviewNarrowPhase(
    [face('a'), face('b')],
    [{
      edgeId: 'unknown-face',
      firstFaceId: 'a',
      secondFaceId: 'missing',
    }],
  ), null)
  assert.equal(prepareFoldPreviewNarrowPhase(
    [face('a'), face('b')],
    [
      { edgeId: 'duplicate-edge', firstFaceId: 'a', secondFaceId: 'b' },
      { edgeId: 'duplicate-edge', firstFaceId: 'b', secondFaceId: 'a' },
    ],
  ), null)
  assert.equal(prepareFoldPreviewNarrowPhase(
    [face(
      'over-preparation-limit',
      Array(MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES + 1)
        .fill({ x: 0, z: 0 }),
    )],
    [],
  ), null)
})

test('one-shot analysis preserves lazy zero-thickness handling', () => {
  const degeneratePolygon = [
    { x: 0, z: 0 },
    { x: 1, z: 0 },
    { x: 2, z: 0 },
  ] as const
  const faces = [
    face('a', degeneratePolygon),
    face('b', degeneratePolygon),
  ]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4()],
  ])
  const oneShot = findFoldPreviewNarrowPhaseInteractions(
    faces,
    transforms,
    0,
    [],
  )
  assert.ok(oneShot)
  assert.equal(oneShot.interactions[0]?.geometryClass, 'indeterminate')
  assert.equal(prepareFoldPreviewNarrowPhase(faces, []), null)
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
