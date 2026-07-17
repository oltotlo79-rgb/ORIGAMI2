import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import {
  findFoldPreviewNarrowPhaseInteractions,
  MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  prepareFoldPreviewNarrowPhase,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const TRIANGLE = Object.freeze([
  Object.freeze({ x: 0, z: 0 }),
  Object.freeze({ x: 2, z: 0 }),
  Object.freeze({ x: 0, z: 2 }),
])
const SQUARE = Object.freeze([
  Object.freeze({ x: 0, z: 0 }),
  Object.freeze({ x: 2, z: 0 }),
  Object.freeze({ x: 2, z: 2 }),
  Object.freeze({ x: 0, z: 2 }),
])

test('a definitive penetration exposes one bounded authoritative pair sample', () => {
  const result = analyze([face('a'), face('b')])

  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
  assert.deepEqual(result.witnessCoverage, {
    scope: 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1',
    eligiblePairCount: 1,
    attemptedPairCount: 1,
    unavailablePairCount: 0,
    omittedByLimitCount: 0,
    authoritativePairScanComplete: true,
  })
  assert.equal(result.witnessSamples.length, 1)
  const sample = result.witnessSamples[0]
  assert.equal(sample.firstFaceId, 'a')
  assert.equal(sample.secondFaceId, 'b')
  assert.equal(sample.relation, 'non_adjacent')
  assert.equal(sample.firstTriangleIndex, 0)
  assert.equal(sample.secondTriangleIndex, 0)
  assert.equal(sample.geometryClass, 'penetrating')
  assert.equal(sample.witness.geometryClass, sample.geometryClass)
  assert.equal(sample.witness.localSeparationHint.autoApplicable, false)
  assert.ok(Object.isFrozen(result.witnessSamples))
  assert.ok(Object.isFrozen(result.witnessCoverage))
  assertDeeplyFrozen(sample)
})

test('face contact remains touching and carries its exact triangle identity', () => {
  const faces = [face('a'), face('b')]
  const result = analyze(faces, new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, 0.2, 0)],
  ]))

  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'touching')
  assert.equal(result.witnessSamples.length, 1)
  assert.equal(result.witnessSamples[0].geometryClass, 'touching')
  assert.equal(result.witnessSamples[0].witness.escapeDistance, 0)
  assert.equal(result.witnessCoverage.eligiblePairCount, 1)
  assert.equal(result.witnessCoverage.attemptedPairCount, 1)
  assert.equal(result.witnessCoverage.unavailablePairCount, 0)
  assert.equal(result.witnessCoverage.omittedByLimitCount, 0)
  assert.equal(result.witnessCoverage.authoritativePairScanComplete, true)
})

test('first-penetration exit is explicit about incomplete pair coverage', () => {
  const result = analyze([face('a', SQUARE), face('b', SQUARE)])

  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
  assert.equal(result.trianglePairTests, 1)
  assert.equal(result.witnessSamples.length, 1)
  assert.equal(result.witnessCoverage.eligiblePairCount, 1)
  assert.equal(result.witnessCoverage.attemptedPairCount, 1)
  assert.equal(result.witnessCoverage.authoritativePairScanComplete, false)
})

test('hinge-adjacent and zero-thickness interactions expose no Phase B sample', () => {
  const adjacency: FoldPreviewCollisionAdjacency = {
    edgeId: 'hinge',
    firstFaceId: 'a',
    secondFaceId: 'b',
  }
  const hinge = analyze([face('a'), face('b')], undefined, 0.2, [adjacency])
  const zeroThickness = analyze([face('a'), face('b')], undefined, 0)

  assert.ok(hinge && zeroThickness)
  assert.equal(hinge.interactions[0]?.relation, 'hinge_adjacent')
  assert.deepEqual(hinge.witnessSamples, [])
  assert.equal(hinge.witnessCoverage.eligiblePairCount, 0)
  assert.equal(hinge.witnessCoverage.attemptedPairCount, 0)
  assert.equal(hinge.witnessCoverage.authoritativePairScanComplete, false)
  assert.equal(zeroThickness.interactions[0]?.geometryClass, 'indeterminate')
  assert.deepEqual(zeroThickness.witnessSamples, [])
  assert.deepEqual(zeroThickness.witnessCoverage, {
    scope: 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1',
    eligiblePairCount: 0,
    attemptedPairCount: 0,
    unavailablePairCount: 0,
    omittedByLimitCount: 0,
    authoritativePairScanComplete: false,
  })
})

test('a pose with no non-adjacent SAT scan never claims complete coverage', () => {
  const result = analyze([face('a'), face('b')], new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(10, 0, 0)],
  ]))

  assert.ok(result)
  assert.equal(result.broadPhaseCandidates, 0)
  assert.deepEqual(result.witnessSamples, [])
  assert.equal(result.witnessCoverage.eligiblePairCount, 0)
  assert.equal(result.witnessCoverage.authoritativePairScanComplete, false)
})

test('penetration samples outrank earlier touching pairs at the global cap', () => {
  const touchingPolygon = convexIntegerPolygon()
  const faces = [
    face('a', touchingPolygon),
    face('b', touchingPolygon),
    face('c'),
    face('d'),
  ]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, 0.2, 0)],
    ['c', new Matrix4().makeTranslation(100, 0, 0)],
    ['d', new Matrix4().makeTranslation(100, 0, 0)],
  ])
  const result = analyze(faces, transforms)
  const permuted = analyze(
    [faces[3], faces[1], faces[0], faces[2]],
    new Map([
      ['d', transforms.get('d')!.clone()],
      ['c', transforms.get('c')!.clone()],
      ['b', transforms.get('b')!.clone()],
      ['a', transforms.get('a')!.clone()],
    ]),
  )

  assert.ok(result && permuted)
  assert.deepEqual(
    result.interactions.map((interaction) => [
      interaction.firstFaceId,
      interaction.secondFaceId,
      interaction.geometryClass,
    ]),
    [
      ['a', 'b', 'touching'],
      ['c', 'd', 'penetrating'],
    ],
  )
  assert.equal(
    result.witnessSamples.length,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  assert.deepEqual(
    [
      result.witnessSamples[0].firstFaceId,
      result.witnessSamples[0].secondFaceId,
      result.witnessSamples[0].geometryClass,
    ],
    ['c', 'd', 'penetrating'],
  )
  assert.ok(
    result.witnessSamples.slice(1).every(
      (sample) => sample.geometryClass === 'touching',
    ),
  )
  assert.ok(result.witnessCoverage.eligiblePairCount > 16)
  assert.equal(result.witnessCoverage.attemptedPairCount, 16)
  assert.equal(result.witnessCoverage.unavailablePairCount, 0)
  assert.equal(
    result.witnessCoverage.eligiblePairCount,
    result.witnessCoverage.attemptedPairCount
      + result.witnessCoverage.omittedByLimitCount,
  )
  assert.equal(result.witnessCoverage.authoritativePairScanComplete, true)
  assert.deepEqual(permuted.witnessSamples, result.witnessSamples)
  assert.deepEqual(permuted.witnessCoverage, result.witnessCoverage)
})

test('a conservative witness miss never changes authoritative classification', () => {
  const faces = [face('a'), face('b')]
  const result = analyze(faces, new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeRotationY(5e-11)],
  ]))

  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
  assert.deepEqual(result.witnessSamples, [])
  assert.equal(result.witnessCoverage.eligiblePairCount, 1)
  assert.equal(result.witnessCoverage.attemptedPairCount, 1)
  assert.equal(result.witnessCoverage.unavailablePairCount, 1)
  assert.equal(result.witnessCoverage.omittedByLimitCount, 0)
})

test('an indeterminate authoritative pair is never eligible for a witness', () => {
  const faces = [face('a'), face('b')]
  const result = analyze(faces, new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeRotationY(1e-14)],
  ]))

  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'indeterminate')
  assert.deepEqual(result.witnessSamples, [])
  assert.equal(result.witnessCoverage.eligiblePairCount, 0)
  assert.equal(result.witnessCoverage.attemptedPairCount, 0)
  assert.equal(result.witnessCoverage.unavailablePairCount, 0)
  assert.equal(result.witnessCoverage.omittedByLimitCount, 0)
  assert.equal(result.witnessCoverage.authoritativePairScanComplete, true)
})

test('touching samples use a separate fixed cap without truncating SAT scans', () => {
  const polygon = convexIntegerPolygon()
  const faces = [face('a', polygon), face('b', polygon)]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, 0.2, 0)],
  ])
  const first = analyze(faces, transforms)
  const permuted = analyze(
    [faces[1], faces[0]],
    new Map([
      ['b', transforms.get('b')!.clone()],
      ['a', transforms.get('a')!.clone()],
    ]),
  )

  assert.ok(first && permuted)
  assert.equal(first.interactions[0]?.geometryClass, 'touching')
  assert.equal(
    first.witnessCoverage.attemptedPairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  assert.ok(first.witnessCoverage.eligiblePairCount > 16)
  assert.equal(
    first.witnessCoverage.eligiblePairCount,
    first.witnessCoverage.attemptedPairCount
      + first.witnessCoverage.omittedByLimitCount,
  )
  assert.equal(
    first.witnessCoverage.attemptedPairCount,
    first.witnessSamples.length
      + first.witnessCoverage.unavailablePairCount,
  )
  assert.ok(first.witnessSamples.length <= 16)
  assert.ok(first.witnessCoverage.omittedByLimitCount > 0)
  assert.equal(first.witnessCoverage.authoritativePairScanComplete, true)
  assert.deepEqual(permuted.witnessSamples, first.witnessSamples)
  assert.deepEqual(permuted.witnessCoverage, first.witnessCoverage)
})

test('prepared and one-shot analysis return the same witness snapshot', () => {
  const faces = [face('a'), face('b')]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4()],
  ])
  const prepared = prepareFoldPreviewNarrowPhase(faces, [])
  const oneShot = analyze(faces, transforms)

  assert.ok(prepared && oneShot)
  assert.deepEqual(prepared.analyze(transforms, 0.2), oneShot)
})

function analyze(
  faces: readonly FoldPreviewCollisionPoseFace[],
  transforms = new Map(faces.map((item) => [item.id, new Matrix4()])),
  thickness = 0.2,
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
  polygon: FoldPreviewCollisionPoseFace['polygon'] = TRIANGLE,
): FoldPreviewCollisionPoseFace {
  return { id, polygon }
}

function convexIntegerPolygon(): FoldPreviewCollisionPoseFace['polygon'] {
  return Object.freeze([
    ...Array.from({ length: 19 }, (_, index) => {
      const x = index - 9
      return Object.freeze({ x, z: x * x })
    }),
    Object.freeze({ x: 9, z: 100 }),
    Object.freeze({ x: -9, z: 100 }),
  ])
}

function assertDeeplyFrozen(value: unknown): void {
  if (typeof value !== 'object' || value === null) return
  assert.ok(Object.isFrozen(value))
  for (const nested of Object.values(value)) assertDeeplyFrozen(nested)
}
