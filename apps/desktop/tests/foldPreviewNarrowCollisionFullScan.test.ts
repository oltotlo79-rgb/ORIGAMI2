import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import {
  FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
  findFoldPreviewNarrowPhaseInteractions,
  MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
  MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
  MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  prepareFoldPreviewNarrowPhase,
  type FoldPreviewFullScanNonAdjacentWitnessJob,
  type FoldPreviewFullScanNonAdjacentWitnessJobStep,
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

  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    transforms,
    THICKNESS,
  )
  assert.ok(job)
  assert.deepEqual(job.workBounds, {
    expectedTrianglePairCount: 0,
    maximumWitnessDerivations: 0,
    maximumTotalWorkUnits: 0,
  })
  const terminal = job.step(1)
  assert.equal(terminal.kind, 'complete')
  assert.deepEqual(terminal.result, result)
  assert.deepEqual(terminal.work, {
    totalWorkUnits: 0,
    trianglePairTests: 0,
    witnessDerivations: 0,
  })
  assert.strictEqual(terminal.workBounds, job.workBounds)
  assert.strictEqual(job.step(1), terminal)
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
  assert.equal(
    analyzer.createFullScanNonAdjacentWitnessSetJob(transforms, 0),
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

test('resumable full scans preserve synchronous output at every chunk size', () => {
  const polygon = convexIntegerPolygon()
  const faces = [face('a', polygon), face('b', polygon)]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, THICKNESS, 0)],
  ])
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const synchronous = analyzer.collectFullScanNonAdjacentWitnessSet(
    transforms,
    THICKNESS,
  )
  assert.ok(synchronous)

  for (const workBudget of [
    1,
    2,
    17,
    MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
      + MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  ]) {
    const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
      transforms,
      THICKNESS,
    )
    assert.ok(job)
    const terminal = drainFullScanJob(job, workBudget)
    assert.equal(terminal.kind, 'complete')
    assert.deepEqual(terminal.result, synchronous)
    assert.equal(
      terminal.work.trianglePairTests,
      terminal.result.coverage.trianglePairTests,
    )
    assert.equal(
      terminal.work.witnessDerivations,
      terminal.result.coverage.attemptedPairCount,
    )
    assert.strictEqual(job.step(1), terminal)
    job.cancel()
    assert.strictEqual(job.step(17), terminal)
    assertDeeplyFrozen(terminal)
  }
})

test('full scan shares the exact fallback cap and keeps later definitive penetration', () => {
  const fixture = exactFallbackFixture(
    MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS + 1,
    true,
  )
  const analyzer = prepareFoldPreviewNarrowPhase(fixture.faces, [])
  assert.ok(analyzer)
  const synchronous = analyzer.collectFullScanNonAdjacentWitnessSet(
    fixture.transforms,
    THICKNESS,
  )
  assert.ok(synchronous)
  assert.equal(synchronous.kind, 'unavailable')
  assert.deepEqual(synchronous.exactTransversalProofWork, {
    algorithm: 'binary64_transversal_triangle_intersection_v1',
    maximumAttempts:
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    attempted: MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    skippedByLimit: 1,
  })
  assert.equal(synchronous.coverage.indeterminatePairCount, 1)
  assert.ok(
    synchronous.coverage.penetratingPairCount
      > MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    'the post-cap definitive pair remains penetrating',
  )
  assert.ok(synchronous.reasons.includes('indeterminate_pair'))

  for (const chunkSize of [1, 17, 10_000]) {
    const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
      fixture.transforms,
      THICKNESS,
    )
    assert.ok(job)
    const terminal = drainFullScanJob(job, chunkSize)
    assert.equal(terminal.kind, 'complete')
    assert.deepEqual(terminal.result, synchronous)
    assert.deepEqual(
      terminal.exactTransversalProofWork,
      synchronous.exactTransversalProofWork,
    )
  }
})

test('cancellation is terminal during the triangle-pair scan', () => {
  const faces = [face('a', SQUARE), face('b', SQUARE)]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    identityTransforms(faces),
    THICKNESS,
  )
  assert.ok(job)

  const first = job.step(1)
  assert.equal(first.kind, 'pending')
  assert.equal(first.phase, 'triangle_pair_scan')
  assert.deepEqual(first.work, {
    totalWorkUnits: 1,
    trianglePairTests: 1,
    witnessDerivations: 0,
  })

  job.cancel()
  const cancelled = job.step(1)
  assert.equal(cancelled.kind, 'cancelled')
  assert.deepEqual(cancelled.work, first.work)
  assert.strictEqual(job.step(17), cancelled)
  job.cancel()
  assert.strictEqual(job.step(1), cancelled)
  assertDeeplyFrozen(cancelled)
})

test('cancellation is terminal during witness derivation', () => {
  const faces = [face('a', SQUARE), face('b', SQUARE)]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    identityTransforms(faces),
    THICKNESS,
  )
  assert.ok(job)

  const afterPairs = job.step(4)
  assert.equal(afterPairs.kind, 'pending')
  assert.equal(afterPairs.phase, 'witness_derivation')
  assert.deepEqual(afterPairs.work, {
    totalWorkUnits: 4,
    trianglePairTests: 4,
    witnessDerivations: 0,
  })
  const afterWitness = job.step(1)
  assert.equal(afterWitness.kind, 'pending')
  assert.equal(afterWitness.phase, 'witness_derivation')
  assert.deepEqual(afterWitness.work, {
    totalWorkUnits: 5,
    trianglePairTests: 4,
    witnessDerivations: 1,
  })

  job.cancel()
  const cancelled = job.step(1)
  assert.equal(cancelled.kind, 'cancelled')
  assert.deepEqual(cancelled.work, afterWitness.work)
  assert.strictEqual(job.step(17), cancelled)
  assertDeeplyFrozen(cancelled)
})

test('invalid work budgets fail closed with one stable terminal value', () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  for (const workBudget of [
    0,
    -1,
    0.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.NEGATIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1,
  ]) {
    const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
      identityTransforms(faces),
      THICKNESS,
    )
    assert.ok(job)
    const terminal = job.step(workBudget)
    assert.equal(terminal.kind, 'indeterminate')
    assert.equal(terminal.reason, 'invalid_work_budget')
    assert.deepEqual(terminal.work, {
      totalWorkUnits: 0,
      trianglePairTests: 0,
      witnessDerivations: 0,
    })
    assert.strictEqual(job.step(1), terminal)
    job.cancel()
    assert.strictEqual(job.step(1), terminal)
    assertDeeplyFrozen(terminal)
  }
})

test('a reentrant step cancels the mutable full scan without outer overwrite', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    identityTransforms(faces),
    THICKNESS,
  )
  assert.ok(job)

  const originalDot = Vector3.prototype.dot
  let reentered = false
  let nested: FoldPreviewFullScanNonAdjacentWitnessJobStep | null = null
  Vector3.prototype.dot = function dot(vector: Vector3) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalDot.call(this, vector)
  }
  let outer: FoldPreviewFullScanNonAdjacentWitnessJobStep
  try {
    outer = job.step(1)
  } finally {
    Vector3.prototype.dot = originalDot
  }

  assert.equal(reentered, true)
  assert.ok(nested)
  assert.equal(nested.kind, 'cancelled')
  assert.strictEqual(outer, nested)
  assert.deepEqual(outer.work, {
    totalWorkUnits: 1,
    trianglePairTests: 1,
    witnessDerivations: 0,
  })
  assert.strictEqual(job.step(1), outer)
  assertDeeplyFrozen(outer)
})

test('budget-validation reentry cancels a full scan before charging work', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    identityTransforms(faces),
    THICKNESS,
  )
  assert.ok(job)

  const originalIsSafeInteger = Number.isSafeInteger
  let reentered = false
  let nested: FoldPreviewFullScanNonAdjacentWitnessJobStep | null = null
  Number.isSafeInteger = function isSafeInteger(value: unknown) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalIsSafeInteger(value)
  }
  let outer: FoldPreviewFullScanNonAdjacentWitnessJobStep
  try {
    outer = job.step(1)
  } finally {
    Number.isSafeInteger = originalIsSafeInteger
  }

  assert.equal(reentered, true)
  assert.ok(nested)
  assert.equal(nested.kind, 'cancelled')
  assert.strictEqual(outer, nested)
  assert.deepEqual(outer.work, {
    totalWorkUnits: 0,
    trianglePairTests: 0,
    witnessDerivations: 0,
  })
  assert.strictEqual(job.step(1), outer)
  assertDeeplyFrozen(outer)
})

test('a step-time classifier error fails closed after charging one pair visit', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    identityTransforms(faces),
    THICKNESS,
  )
  assert.ok(job)

  const originalDot = Vector3.prototype.dot
  Vector3.prototype.dot = function dot() {
    throw new Error('classifier failure')
  }
  let terminal: FoldPreviewFullScanNonAdjacentWitnessJobStep
  try {
    terminal = job.step(1)
  } finally {
    Vector3.prototype.dot = originalDot
  }

  assert.equal(terminal.kind, 'indeterminate')
  assert.equal(terminal.reason, 'scan_error')
  assert.deepEqual(terminal.work, {
    totalWorkUnits: 1,
    trianglePairTests: 1,
    witnessDerivations: 0,
  })
  assert.strictEqual(terminal.workBounds, job.workBounds)
  assert.strictEqual(job.step(1), terminal)
  job.cancel()
  assert.strictEqual(job.step(1), terminal)
  assertDeeplyFrozen(terminal)
})

test('full-scan cancellation outranks validation and charged-work throws', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  {
    const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
      identityTransforms(faces),
      THICKNESS,
    )
    assert.ok(job)
    const originalIsSafeInteger = Number.isSafeInteger
    Number.isSafeInteger = function isSafeInteger() {
      job.cancel()
      throw new Error('validation cancellation')
    }
    let terminal: FoldPreviewFullScanNonAdjacentWitnessJobStep
    try {
      terminal = job.step(1)
    } finally {
      Number.isSafeInteger = originalIsSafeInteger
    }
    assert.equal(terminal.kind, 'cancelled')
    assert.deepEqual(terminal.work, {
      totalWorkUnits: 0,
      trianglePairTests: 0,
      witnessDerivations: 0,
    })
    assert.strictEqual(job.step(1), terminal)
    assertDeeplyFrozen(terminal)
  }

  {
    const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
      identityTransforms(faces),
      THICKNESS,
    )
    assert.ok(job)
    const originalDot = Vector3.prototype.dot
    Vector3.prototype.dot = function dot() {
      job.cancel()
      throw new Error('pair cancellation')
    }
    let terminal: FoldPreviewFullScanNonAdjacentWitnessJobStep
    try {
      terminal = job.step(1)
    } finally {
      Vector3.prototype.dot = originalDot
    }
    assert.equal(terminal.kind, 'cancelled')
    assert.deepEqual(terminal.work, {
      totalWorkUnits: 1,
      trianglePairTests: 1,
      witnessDerivations: 0,
    })
    assert.strictEqual(job.step(1), terminal)
    assertDeeplyFrozen(terminal)
  }
})

test('reentrant cancellation during witness derivation is terminal', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    identityTransforms(faces),
    THICKNESS,
  )
  assert.ok(job)
  const afterPair = job.step(1)
  assert.equal(afterPair.kind, 'pending')
  assert.equal(afterPair.phase, 'witness_derivation')

  const originalIsFinite = Number.isFinite
  let reentered = false
  let nested: FoldPreviewFullScanNonAdjacentWitnessJobStep | null = null
  Number.isFinite = function isFinite(value: unknown) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalIsFinite(value)
  }
  let outer: FoldPreviewFullScanNonAdjacentWitnessJobStep
  try {
    outer = job.step(1)
  } finally {
    Number.isFinite = originalIsFinite
  }

  assert.equal(reentered, true)
  assert.ok(nested)
  assert.equal(nested.kind, 'cancelled')
  assert.strictEqual(outer, nested)
  assert.deepEqual(outer.work, {
    totalWorkUnits: 2,
    trianglePairTests: 1,
    witnessDerivations: 1,
  })
  assert.strictEqual(outer.workBounds, job.workBounds)
  assert.strictEqual(job.step(1), outer)
  assertDeeplyFrozen(outer)
})

test('the resumable factory snapshots transforms once and rejects hostile maps', () => {
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
      return oneReadIdentityMatrix(`job.${faceId}`)
    },
  } as ReadonlyMap<string, Matrix4>
  const job = analyzer.createFullScanNonAdjacentWitnessSetJob(
    transforms,
    THICKNESS,
  )
  assert.ok(job)
  assert.equal(sizeReads, 1)
  assert.deepEqual([...reads], [['a', 1], ['b', 1]])
  const terminal = drainFullScanJob(job, 1)
  assert.equal(terminal.kind, 'complete')
  assert.equal(sizeReads, 1)
  assert.deepEqual([...reads], [['a', 1], ['b', 1]])

  const throwing = new Proxy(new Map<string, Matrix4>(), {
    get() {
      throw new Error('hostile map')
    },
  }) as ReadonlyMap<string, Matrix4>
  assert.equal(
    analyzer.createFullScanNonAdjacentWitnessSetJob(throwing, THICKNESS),
    null,
  )

  const revocable = Proxy.revocable(
    new Map<string, Matrix4>(),
    {},
  )
  revocable.revoke()
  assert.equal(
    analyzer.createFullScanNonAdjacentWitnessSetJob(
      revocable.proxy,
      THICKNESS,
    ),
    null,
  )
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
  assert.equal(
    analyzer.createFullScanNonAdjacentWitnessSetJob(
      identityTransforms(faces),
      THICKNESS,
    ),
    null,
  )
})

function drainFullScanJob(
  job: FoldPreviewFullScanNonAdjacentWitnessJob,
  workBudget: number,
): FoldPreviewFullScanNonAdjacentWitnessJobStep {
  assert.ok(Number.isSafeInteger(job.workBounds.expectedTrianglePairCount))
  assert.equal(
    job.workBounds.maximumWitnessDerivations,
    Math.min(
      job.workBounds.expectedTrianglePairCount,
      MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
    ),
  )
  assert.equal(
    job.workBounds.maximumTotalWorkUnits,
    job.workBounds.expectedTrianglePairCount
      + job.workBounds.maximumWitnessDerivations,
  )
  assertDeeplyFrozen(job.workBounds)
  let previous = {
    totalWorkUnits: 0,
    trianglePairTests: 0,
    witnessDerivations: 0,
  }
  let previousExact = {
    attempted: 0,
    skippedByLimit: 0,
  }
  for (let index = 0; index < 10_000; index += 1) {
    const step = job.step(workBudget)
    assert.equal(
      step.version,
      FOLD_PREVIEW_FULL_SCAN_NON_ADJACENT_WITNESS_JOB_VERSION,
    )
    assert.strictEqual(step.workBounds, job.workBounds)
    const totalDelta =
      step.work.totalWorkUnits - previous.totalWorkUnits
    const trianglePairDelta =
      step.work.trianglePairTests - previous.trianglePairTests
    const witnessDelta =
      step.work.witnessDerivations - previous.witnessDerivations
    const exactAttemptDelta =
      step.exactTransversalProofWork.attempted - previousExact.attempted
    const exactSkippedDelta =
      step.exactTransversalProofWork.skippedByLimit
      - previousExact.skippedByLimit
    assert.ok(totalDelta >= 0 && totalDelta <= workBudget)
    assert.ok(trianglePairDelta >= 0 && trianglePairDelta <= workBudget)
    assert.ok(witnessDelta >= 0 && witnessDelta <= workBudget)
    assert.ok(exactAttemptDelta >= 0)
    assert.ok(exactSkippedDelta >= 0)
    assert.ok(exactAttemptDelta + exactSkippedDelta <= trianglePairDelta)
    assert.equal(
      step.exactTransversalProofWork.maximumAttempts,
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    )
    assert.ok(
      step.exactTransversalProofWork.attempted
        <= MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    )
    assert.equal(totalDelta, trianglePairDelta + witnessDelta)
    assert.equal(
      step.work.totalWorkUnits,
      step.work.trianglePairTests + step.work.witnessDerivations,
    )
    assert.ok(
      step.work.trianglePairTests
        <= step.workBounds.expectedTrianglePairCount,
    )
    assert.ok(
      step.work.witnessDerivations
        <= step.workBounds.maximumWitnessDerivations,
    )
    assert.ok(
      step.work.totalWorkUnits
        <= step.workBounds.maximumTotalWorkUnits,
    )
    if (step.kind !== 'pending') return step
    assert.ok(totalDelta > 0, 'a pending step must make bounded progress')
    previous = step.work
    previousExact = step.exactTransversalProofWork
  }
  assert.fail('full-scan job did not reach a terminal result')
}

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

function exactFallbackFixture(
  pairCount: number,
  appendDefinitivePenetration: boolean,
) {
  const polygon = [
    { x: -2, z: -2 },
    { x: 2, z: -2 },
    { x: 0, z: 2 },
  ] as const
  const faces: FoldPreviewCollisionPoseFace[] = []
  const transforms = new Map<string, Matrix4>()
  const shallowRotation = new Matrix4().makeRotationX(
    Number.EPSILON * 64,
  )
  for (let index = 0; index < pairCount; index += 1) {
    const prefix = `exact-${String(index).padStart(4, '0')}`
    const offset = index * 10
    faces.push(
      face(`${prefix}-first`, polygon),
      face(`${prefix}-second`, polygon),
    )
    transforms.set(
      `${prefix}-first`,
      new Matrix4().makeTranslation(offset, 0, 0),
    )
    transforms.set(
      `${prefix}-second`,
      new Matrix4()
        .makeTranslation(offset, 0, 0)
        .multiply(shallowRotation),
    )
  }
  if (appendDefinitivePenetration) {
    const offset = (pairCount + 1) * 10
    faces.push(
      face('zz-definitive-first', polygon),
      face('zz-definitive-second', polygon),
    )
    transforms.set(
      'zz-definitive-first',
      new Matrix4().makeTranslation(offset, 0, 0),
    )
    transforms.set(
      'zz-definitive-second',
      new Matrix4().makeTranslation(offset, 0, 0),
    )
  }
  return { faces, transforms }
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
