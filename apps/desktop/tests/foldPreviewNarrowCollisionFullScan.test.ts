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
  type FoldPreviewFullScanNonAdjacentWitnessCoverage,
  type FoldPreviewFullScanNonAdjacentWitnessSet,
} from '../src/lib/foldPreviewNarrowCollision.ts'

const THICKNESS = 0.2
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

test('v2 scans all four overlapping square pairs in traversal order while v1 stays early-exit', () => {
  const faces = [face('a', SQUARE), face('b', SQUARE)]
  const transforms = identityTransforms(faces)
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  const before = analyzer.analyze(transforms, THICKNESS)
  assert.ok(before)
  assert.equal(before.interactions[0]?.geometryClass, 'penetrating')
  assert.equal(before.trianglePairTests, 1)
  assert.equal(before.witnessCoverage.authoritativePairScanComplete, false)

  const full = analyzer.collectFullScanNonAdjacentWitnessSet(
    transforms,
    THICKNESS,
  )
  assert.ok(full)
  assert.equal(full.kind, 'complete')
  assert.deepEqual(
    full.witnessSamples.map((sample) => [
      sample.firstTriangleIndex,
      sample.secondTriangleIndex,
      sample.geometryClass,
    ]),
    [
      [0, 0, 'penetrating'],
      [0, 1, 'touching'],
      [1, 0, 'touching'],
      [1, 1, 'penetrating'],
    ],
  )
  assert.equal(full.coverage.expectedTrianglePairCount, 4)
  assert.equal(full.coverage.trianglePairTests, 4)
  assert.equal(full.coverage.satTests, 4)
  assert.equal(full.coverage.touchingPairCount, 2)
  assert.equal(full.coverage.penetratingPairCount, 2)
  assert.equal(full.coverage.allCollisionConstraintsRepresented, true)
  assertCoverageEquations(full)
  assertDeeplyFrozen(full)

  const after = analyzer.analyze(transforms, THICKNESS)
  assert.deepEqual(after, before)
  assert.deepEqual(
    after,
    findFoldPreviewNarrowPhaseInteractions(
      faces,
      transforms,
      THICKNESS,
      [],
    ),
  )
})

test('no broad-phase candidate returns an empty complete full scan', () => {
  const faces = [face('a'), face('b')]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(10, 0, 0)],
  ])
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  const result = analyzer.collectFullScanNonAdjacentWitnessSet(
    transforms,
    THICKNESS,
  )
  assert.ok(result)
  assert.equal(result.kind, 'complete')
  assert.deepEqual(result.witnessSamples, [])
  assert.deepEqual(result.coverage, {
    scope: 'all_broad_phase_non_adjacent_triangle_pairs_full_scan_v2',
    broadPhaseCandidateCount: 0,
    expectedTrianglePairCount: 0,
    trianglePairTests: 0,
    aabbRejectedPairCount: 0,
    satTests: 0,
    satSeparatedPairCount: 0,
    touchingPairCount: 0,
    penetratingPairCount: 0,
    indeterminatePairCount: 0,
    eligiblePairCount: 0,
    attemptedPairCount: 0,
    availablePairCount: 0,
    unavailablePairCount: 0,
    omittedByLimitCount: 0,
    authoritativePairScanComplete: true,
    allCollisionConstraintsRepresented: true,
  })
  assertCoverageEquations(result)
  assertDeeplyFrozen(result)
})

test('hinge-adjacent pairs are excluded and zero thickness is rejected', () => {
  const faces = [face('a'), face('b')]
  const transforms = identityTransforms(faces)
  const adjacency: FoldPreviewCollisionAdjacency = {
    edgeId: 'hinge',
    firstFaceId: 'a',
    secondFaceId: 'b',
  }
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [adjacency])
  assert.ok(analyzer)

  const ordinary = analyzer.analyze(transforms, THICKNESS)
  assert.ok(ordinary)
  assert.equal(ordinary.interactions[0]?.relation, 'hinge_adjacent')

  const result = analyzer.collectFullScanNonAdjacentWitnessSet(
    transforms,
    THICKNESS,
  )
  assert.ok(result)
  assert.equal(result.kind, 'complete')
  assert.equal(result.coverage.broadPhaseCandidateCount, 0)
  assert.equal(result.coverage.expectedTrianglePairCount, 0)
  assert.deepEqual(result.witnessSamples, [])
  assertCoverageEquations(result)
  assertDeeplyFrozen(result)

  assert.equal(
    analyzer.collectFullScanNonAdjacentWitnessSet(transforms, 0),
    null,
  )
})

test('more than sixteen eligible pairs withholds every partial witness', () => {
  const polygon = convexIntegerPolygon()
  const faces = [face('a', polygon), face('b', polygon)]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, THICKNESS, 0)],
  ])
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  const result = analyzer.collectFullScanNonAdjacentWitnessSet(
    transforms,
    THICKNESS,
  )
  assert.ok(result)
  assert.equal(result.kind, 'unavailable')
  assert.deepEqual(result.reasons, ['witness_limit_exceeded'])
  assert.deepEqual(result.witnessSamples, [])
  assert.ok(
    result.coverage.eligiblePairCount
      > MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  assert.equal(
    result.coverage.attemptedPairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  assert.equal(
    result.coverage.availablePairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  )
  assert.equal(result.coverage.unavailablePairCount, 0)
  assert.ok(result.coverage.omittedByLimitCount > 0)
  assert.equal(result.coverage.allCollisionConstraintsRepresented, false)
  assertCoverageEquations(result)
  assertDeeplyFrozen(result)
})

test('indeterminate pairs and conservative witness failures return unavailable without partial samples', () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  const cases = [
    {
      rotation: 1e-14,
      reason: 'indeterminate_pair',
      expected: {
        indeterminatePairCount: 1,
        penetratingPairCount: 0,
        eligiblePairCount: 0,
        attemptedPairCount: 0,
        availablePairCount: 0,
        unavailablePairCount: 0,
      },
    },
    {
      rotation: 5e-11,
      reason: 'witness_derivation_failed',
      expected: {
        indeterminatePairCount: 0,
        penetratingPairCount: 1,
        eligiblePairCount: 1,
        attemptedPairCount: 1,
        availablePairCount: 0,
        unavailablePairCount: 1,
      },
    },
  ] as const

  for (const current of cases) {
    const transforms = new Map([
      ['a', new Matrix4()],
      ['b', new Matrix4().makeRotationY(current.rotation)],
    ])
    const result = analyzer.collectFullScanNonAdjacentWitnessSet(
      transforms,
      THICKNESS,
    )
    assert.ok(result)
    assert.equal(result.kind, 'unavailable')
    assert.deepEqual(result.reasons, [current.reason])
    assert.deepEqual(result.witnessSamples, [])
    for (const [key, value] of Object.entries(current.expected)) {
      assert.equal(
        result.coverage[
          key as keyof FoldPreviewFullScanNonAdjacentWitnessCoverage
        ],
        value,
        `${current.reason}: ${key}`,
      )
    }
    assert.equal(result.coverage.allCollisionConstraintsRepresented, false)
    assertCoverageEquations(result)
    assertDeeplyFrozen(result)
  }
})

test('face and transform input order preserve full-scan candidate and triangle order', () => {
  const faces = [
    face('a', SQUARE),
    face('b', SQUARE),
    face('c', SQUARE),
    face('d', SQUARE),
  ]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, THICKNESS, 0)],
    ['c', new Matrix4().makeTranslation(100, 0, 0)],
    ['d', new Matrix4().makeTranslation(100, 0, 0)],
  ])
  const forwardAnalyzer = prepareFoldPreviewNarrowPhase(faces, [])
  const reverseAnalyzer = prepareFoldPreviewNarrowPhase(
    [...faces].reverse(),
    [],
  )
  assert.ok(forwardAnalyzer && reverseAnalyzer)

  const forward = forwardAnalyzer.collectFullScanNonAdjacentWitnessSet(
    transforms,
    THICKNESS,
  )
  const reverse = reverseAnalyzer.collectFullScanNonAdjacentWitnessSet(
    new Map([...transforms].reverse()),
    THICKNESS,
  )
  assert.ok(forward && reverse)
  assert.deepEqual(reverse, forward)
  assert.deepEqual(
    forward.witnessSamples.map((sample) => [
      sample.firstFaceId,
      sample.secondFaceId,
      sample.firstTriangleIndex,
      sample.secondTriangleIndex,
      sample.geometryClass,
    ]),
    [
      ['a', 'b', 0, 0, 'touching'],
      ['a', 'b', 0, 1, 'touching'],
      ['a', 'b', 1, 0, 'touching'],
      ['a', 'b', 1, 1, 'touching'],
      ['c', 'd', 0, 0, 'penetrating'],
      ['c', 'd', 0, 1, 'touching'],
      ['c', 'd', 1, 0, 'touching'],
      ['c', 'd', 1, 1, 'penetrating'],
    ],
  )
  assertCoverageEquations(forward)
  assertCoverageEquations(reverse)
  assertDeeplyFrozen(forward)
  assertDeeplyFrozen(reverse)
})

test('the v2 boundary snapshots each hostile transform and matrix element once', () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  const reads = new Map<string, number>()
  let sizeReads = 0
  const transforms = {
    get size() {
      sizeReads += 1
      return 2
    },
    get(faceId: string) {
      reads.set(faceId, (reads.get(faceId) ?? 0) + 1)
      return oneReadIdentityMatrix(faceId)
    },
  } as ReadonlyMap<string, Matrix4>

  const result = analyzer.collectFullScanNonAdjacentWitnessSet(
    transforms,
    THICKNESS,
  )
  assert.ok(result)
  assert.equal(result.kind, 'complete')
  assert.equal(result.coverage.penetratingPairCount, 1)
  assert.equal(result.witnessSamples.length, 1)
  assert.equal(sizeReads, 1)
  assert.deepEqual([...reads], [['a', 1], ['b', 1]])
  assertCoverageEquations(result)
  assertDeeplyFrozen(result)
})

test('a full scan above the one-million pair work cap fails closed', () => {
  const faces = [
    face('a', regularPolygon(1_003)),
    face('b', regularPolygon(1_002)),
  ]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  assert.equal(
    analyzer.collectFullScanNonAdjacentWitnessSet(
      identityTransforms(faces),
      THICKNESS,
    ),
    null,
  )
})

function face(
  id: string,
  polygon: FoldPreviewCollisionPoseFace['polygon'] = TRIANGLE,
): FoldPreviewCollisionPoseFace {
  return { id, polygon }
}

function identityTransforms(
  faces: readonly FoldPreviewCollisionPoseFace[],
) {
  return new Map(faces.map((current) => [current.id, new Matrix4()]))
}

function oneReadIdentityMatrix(label: string): Matrix4 {
  const identity = new Matrix4()
  const reads = Array.from({ length: 16 }, () => 0)
  let lengthReads = 0
  const elements = new Proxy(identity.elements, {
    get(target, property, receiver) {
      if (property === 'length') {
        lengthReads += 1
        assert.equal(lengthReads, 1, `${label}.elements.length`)
      }
      if (typeof property === 'string' && /^\d+$/.test(property)) {
        const index = Number(property)
        reads[index] += 1
        assert.equal(reads[index], 1, `${label}.elements[${index}]`)
      }
      return Reflect.get(target, property, receiver)
    },
  })
  let elementsReads = 0
  return Object.defineProperty({}, 'elements', {
    get() {
      elementsReads += 1
      assert.equal(elementsReads, 1, `${label}.elements`)
      return elements
    },
  }) as Matrix4
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

function regularPolygon(
  vertexCount: number,
): FoldPreviewCollisionPoseFace['polygon'] {
  return Object.freeze(Array.from({ length: vertexCount }, (_, index) => {
    const angle = index * Math.PI * 2 / vertexCount
    return Object.freeze({
      x: Math.cos(angle) * 100,
      z: Math.sin(angle) * 100,
    })
  }))
}

function assertCoverageEquations(
  result: FoldPreviewFullScanNonAdjacentWitnessSet,
) {
  assert.equal(result.algorithm, 'full_non_adjacent_prism_witness_scan_v2')
  assert.equal(result.sourcePose, 'analyzed_input_pose')
  assert.equal(result.requestIdentityBound, false)
  assert.equal(result.collisionThickness, THICKNESS)
  assert.equal(result.autoApplicable, false)
  const coverage = result.coverage
  assert.equal(
    coverage.expectedTrianglePairCount,
    coverage.trianglePairTests,
  )
  assert.equal(
    coverage.trianglePairTests,
    coverage.aabbRejectedPairCount + coverage.satTests,
  )
  assert.equal(
    coverage.satTests,
    coverage.satSeparatedPairCount
      + coverage.touchingPairCount
      + coverage.penetratingPairCount
      + coverage.indeterminatePairCount,
  )
  assert.equal(
    coverage.eligiblePairCount,
    coverage.touchingPairCount + coverage.penetratingPairCount,
  )
  assert.equal(
    coverage.eligiblePairCount,
    coverage.attemptedPairCount + coverage.omittedByLimitCount,
  )
  assert.equal(
    coverage.attemptedPairCount,
    coverage.availablePairCount + coverage.unavailablePairCount,
  )
  assert.equal(coverage.authoritativePairScanComplete, true)
  const expectedComplete = coverage.indeterminatePairCount === 0
    && coverage.unavailablePairCount === 0
    && coverage.omittedByLimitCount === 0
    && coverage.availablePairCount === coverage.eligiblePairCount
  assert.equal(
    coverage.allCollisionConstraintsRepresented,
    expectedComplete,
  )
  assert.equal(result.kind, expectedComplete ? 'complete' : 'unavailable')
  if (result.kind === 'complete') {
    assert.equal(
      result.witnessSamples.length,
      coverage.availablePairCount,
    )
  } else {
    assert.deepEqual(result.witnessSamples, [])
  }
}

function assertDeeplyFrozen(
  value: unknown,
  seen = new Set<object>(),
): void {
  if (typeof value !== 'object' || value === null || seen.has(value)) return
  seen.add(value)
  assert.ok(Object.isFrozen(value))
  for (const property of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[property],
      seen,
    )
  }
}
