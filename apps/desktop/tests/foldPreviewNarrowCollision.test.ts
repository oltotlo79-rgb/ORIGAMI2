import assert from 'node:assert/strict'
import test from 'node:test'

import { Matrix4, Vector3 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoseFace,
} from '../src/lib/foldPreviewCollision.ts'
import type {
  FoldPreviewHingeContactConstraint,
} from '../src/lib/foldPreviewHingeCollision.ts'
import {
  summarizeFoldPreviewCollision,
} from '../src/lib/foldPreviewCollisionPresentation.ts'
import {
  FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
  MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
  MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
  MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
  findFoldPreviewNarrowPhaseInteractions,
  prepareFoldPreviewNarrowPhase,
  type FoldPreviewNarrowPhaseAnalysisJob,
  type FoldPreviewNarrowPhaseAnalysisJobStep,
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

test('zero-thickness coplanar area overlap is an explicit penetration', () => {
  const result = analyze([face('a'), face('b')], undefined, 0)
  assert.ok(result)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
  assert.equal(result.trianglePairTests, 1)
  assert.equal(result.satTests, 1)
  assert.equal(result.witnessSamples.length, 0)
  assert.equal(result.witnessCoverage.unavailablePairCount, 1)
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

test('one-shot analysis snapshots a stateful transform map once before broad phase', () => {
  const faces = [face('a'), face('b')]
  const reads = new Map<string, number>()
  let sizeReads = 0
  const transforms = {
    get size() {
      sizeReads += 1
      return faces.length
    },
    get(faceId: string) {
      const readCount = reads.get(faceId) ?? 0
      reads.set(faceId, readCount + 1)
      return faceId === 'b' && readCount > 0
        ? new Matrix4().makeTranslation(100, 0, 0)
        : new Matrix4()
    },
  } as ReadonlyMap<string, Matrix4>

  const result = findFoldPreviewNarrowPhaseInteractions(
    faces,
    transforms,
    0.1,
    [],
  )

  assert.ok(result)
  assert.equal(sizeReads, 1)
  assert.deepEqual([...reads], [['a', 1], ['b', 1]])
  assert.equal(result.broadPhaseCandidates, 1)
  assert.equal(result.interactions[0]?.geometryClass, 'penetrating')
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
  assertDeeplyFrozen(expected)
  const exposedHingeIds = expected.interactions[0]?.hingeEdgeIds as string[]
  assert.throws(() => {
    exposedHingeIds[0] = 'mutated-output'
  }, TypeError)
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

test('one-shot zero-thickness analysis rejects malformed polygons', () => {
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
  assert.equal(findFoldPreviewNarrowPhaseInteractions(
    faces,
    transforms,
    0,
    [],
  ), null)
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

test('resumable narrow scans preserve synchronous output for every chunk size', () => {
  const faces = [face('a'), face('b'), face('c'), face('d')]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, 0.1, 0)],
    ['c', new Matrix4().makeTranslation(100, 0, 0)],
    ['d', new Matrix4().makeTranslation(100, 0, 0)],
  ])
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const synchronous = analyzer.analyze(transforms, 0.1)
  assert.ok(synchronous)
  assert.deepEqual(
    synchronous,
    findFoldPreviewNarrowPhaseInteractions(faces, transforms, 0.1, []),
  )
  assert.equal(synchronous.trianglePairTests, 5)
  assert.equal(synchronous.satTests, 5)
  assert.deepEqual(
    synchronous.interactions.map((interaction) => [
      interaction.firstFaceId,
      interaction.secondFaceId,
      interaction.geometryClass,
    ]),
    [
      ['a', 'b', 'touching'],
      ['c', 'd', 'penetrating'],
    ],
  )
  assert.deepEqual(
    synchronous.witnessSamples.map((sample) => [
      sample.firstFaceId,
      sample.secondFaceId,
      sample.geometryClass,
    ]),
    [
      ['c', 'd', 'penetrating'],
      ['a', 'b', 'touching'],
      ['a', 'b', 'touching'],
      ['a', 'b', 'touching'],
      ['a', 'b', 'touching'],
    ],
  )
  assertDeeplyFrozen(synchronous)
  assert.throws(() => {
    (synchronous.interactions as unknown[]).pop()
  }, TypeError)
  assert.throws(() => {
    const coverage = synchronous.witnessCoverage as unknown as {
      eligiblePairCount: number
    }
    coverage.eligiblePairCount = 999
  }, TypeError)
  assert.throws(() => {
    const vector = synchronous.witnessSamples[0].witness.normal.vector as unknown as { x: number }
    vector.x = 999
  }, TypeError)

  for (const workBudget of [
    1,
    2,
    17,
    MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS
      + MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  ]) {
    const job = analyzer.createAnalysisJob(transforms, 0.1)
    assert.ok(job)
    assert.deepEqual(job.workBounds, {
      entireStepTimeBounded: false,
      synchronousFactoryPreparation: true,
      synchronousHingePolicyFinalization: true,
      synchronousResultFinalization: true,
      potentialTrianglePairCount: 8,
      maximumTrianglePairTests: 8,
      maximumWitnessDerivations: 8,
      maximumTotalWorkUnits: 16,
    })
    const terminal = drainAnalysisJob(job, workBudget)
    assert.equal(terminal.kind, 'complete')
    assert.deepEqual(terminal.result, synchronous)
    assert.equal(terminal.work.trianglePairTests, 5)
    assert.equal(terminal.work.witnessDerivations, 5)
    assert.strictEqual(job.step(1), terminal)
    job.cancel()
    assert.strictEqual(job.step(17), terminal)
    assertFrozenJobStep(terminal)
  }
})

test('zero-thickness analysis uses the same bounded pair cursor as positive thickness', () => {
  const faces = [face('a'), face('b')]
  const transforms = new Map(faces.map((item) => [item.id, new Matrix4()]))
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const synchronous = analyzer.analyze(transforms, 0)
  const job = analyzer.createAnalysisJob(transforms, 0)
  assert.ok(synchronous && job)
  assert.deepEqual(job.workBounds, {
    entireStepTimeBounded: false,
    synchronousFactoryPreparation: true,
    synchronousHingePolicyFinalization: true,
    synchronousResultFinalization: true,
    potentialTrianglePairCount: 4,
    maximumTrianglePairTests: 4,
    maximumWitnessDerivations: 4,
    maximumTotalWorkUnits: 8,
  })
  const terminal = drainAnalysisJob(job, 1)
  assert.equal(terminal.kind, 'complete')
  assert.deepEqual(terminal.result, synchronous)
  assert.deepEqual(terminal.work, {
    totalWorkUnits: 2,
    trianglePairTests: 1,
    witnessDerivations: 1,
  })
  assert.strictEqual(job.step(1), terminal)
  assertFrozenJobStep(terminal)
})

test('exact fallback budget is shared across candidates and closes limit plus one as indeterminate', () => {
  const atLimit = exactFallbackFixture(
    MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
  )
  const atLimitAnalyzer = prepareFoldPreviewNarrowPhase(atLimit.faces, [])
  assert.ok(atLimitAnalyzer)
  const atLimitResult = atLimitAnalyzer.analyze(atLimit.transforms, 0)
  assert.ok(atLimitResult)
  assert.deepEqual(atLimitResult.exactTransversalProofWork, {
    algorithm: 'binary64_transversal_triangle_intersection_v1',
    maximumAttempts:
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    attempted: MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    skippedByLimit: 0,
  })
  assert.equal(
    atLimitResult.interactions.every(
      ({ geometryClass }) => geometryClass === 'penetrating',
    ),
    true,
  )

  const overLimit = exactFallbackFixture(
    MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS + 1,
    true,
  )
  const analyzer = prepareFoldPreviewNarrowPhase(overLimit.faces, [])
  assert.ok(analyzer)
  const synchronous = analyzer.analyze(overLimit.transforms, 0)
  const oneShot = findFoldPreviewNarrowPhaseInteractions(
    overLimit.faces,
    overLimit.transforms,
    0,
    [],
  )
  assert.ok(synchronous && oneShot)
  assert.deepEqual(oneShot, synchronous)
  assert.deepEqual(synchronous.exactTransversalProofWork, {
    algorithm: 'binary64_transversal_triangle_intersection_v1',
    maximumAttempts:
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    attempted: MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    skippedByLimit: 1,
  })
  assert.equal(
    synchronous.interactions[
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS
    ]?.geometryClass,
    'indeterminate',
  )
  assert.equal(
    synchronous.interactions.at(-1)?.geometryClass,
    'penetrating',
    'a later definitive SAT penetration must still raise severity',
  )
  const presentation = summarizeFoldPreviewCollision(synchronous)
  assert.equal(presentation.indeterminateInteractions, 1)
  assert.equal(presentation.nonAdjacentPenetrations, 257)

  for (const chunkSize of [1, 17, 10_000]) {
    const job = analyzer.createAnalysisJob(overLimit.transforms, 0)
    assert.ok(job)
    const terminal = drainAnalysisJob(job, chunkSize)
    assert.equal(terminal.kind, 'complete')
    assert.deepEqual(terminal.result, synchronous)
    assert.deepEqual(
      terminal.exactTransversalProofWork,
      synchronous.exactTransversalProofWork,
    )
  }
})

test('exact fallback accounting resets per analysis and cancellation cannot revive or leak it', () => {
  const fixture = exactFallbackFixture(2)
  const analyzer = prepareFoldPreviewNarrowPhase(fixture.faces, [])
  assert.ok(analyzer)

  const first = analyzer.createAnalysisJob(fixture.transforms, 0)
  assert.ok(first)
  const afterOnePair = first.step(1)
  assert.equal(afterOnePair.kind, 'pending')
  assert.deepEqual(afterOnePair.exactTransversalProofWork, {
    algorithm: 'binary64_transversal_triangle_intersection_v1',
    maximumAttempts:
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    attempted: 1,
    skippedByLimit: 0,
  })
  first.cancel()
  const cancelled = first.step(1)
  assert.equal(cancelled.kind, 'cancelled')
  assert.deepEqual(
    cancelled.exactTransversalProofWork,
    afterOnePair.exactTransversalProofWork,
  )
  assert.strictEqual(first.step(1), cancelled)

  const replacement = analyzer.createAnalysisJob(fixture.transforms, 0)
  assert.ok(replacement)
  const replacementFirstPair = replacement.step(1)
  assert.equal(replacementFirstPair.kind, 'pending')
  assert.deepEqual(
    replacementFirstPair.exactTransversalProofWork,
    afterOnePair.exactTransversalProofWork,
    'a replacement analysis starts from a fresh zero budget',
  )
  const replacementTerminal = replacement.step(100)
  assert.equal(replacementTerminal.kind, 'complete')
  assert.equal(replacementTerminal.exactTransversalProofWork.attempted, 2)
  assert.equal(replacementTerminal.exactTransversalProofWork.skippedByLimit, 0)

  const synchronousFirst = analyzer.analyze(fixture.transforms, 0)
  const synchronousSecond = analyzer.analyze(fixture.transforms, 0)
  assert.ok(synchronousFirst && synchronousSecond)
  assert.deepEqual(
    synchronousSecond.exactTransversalProofWork,
    synchronousFirst.exactTransversalProofWork,
  )
  assert.equal(synchronousSecond.exactTransversalProofWork.attempted, 2)
})

test('exact fallback exhaustion cannot be reclassified as allowed hinge contact', () => {
  const fixture = exactFallbackFixture(
    MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
  )
  const offset =
    (MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS + 1) * 10
  const start = {
    vertexId: 'limit-hinge-start',
    x: offset,
    z: -2,
  } as const
  const end = {
    vertexId: 'limit-hinge-end',
    x: offset,
    z: 2,
  } as const
  fixture.faces.push(
    face('zz-limit-hinge-left', [
      start,
      { vertexId: 'limit-hinge-left-tip', x: offset - 2, z: 0 },
      end,
    ]),
    face('zz-limit-hinge-right', [
      end,
      { vertexId: 'limit-hinge-right-tip', x: offset + 2, z: 0 },
      start,
    ]),
  )
  fixture.transforms.set('zz-limit-hinge-left', new Matrix4())
  fixture.transforms.set('zz-limit-hinge-right', new Matrix4())
  const adjacency: FoldPreviewCollisionAdjacency = {
    edgeId: 'limit-hinge',
    firstFaceId: 'zz-limit-hinge-left',
    secondFaceId: 'zz-limit-hinge-right',
  }
  const constraint: FoldPreviewHingeContactConstraint = {
    edgeId: 'limit-hinge',
    leftFaceId: 'zz-limit-hinge-left',
    rightFaceId: 'zz-limit-hinge-right',
    start,
    end,
    thicknessRule: 'centered_mid_surface_v1',
  }
  const analyzer = prepareFoldPreviewNarrowPhase(
    fixture.faces,
    [adjacency],
    [constraint],
  )
  assert.ok(analyzer)
  const result = analyzer.analyze(fixture.transforms, 0)
  assert.ok(result)
  assert.deepEqual(result.exactTransversalProofWork, {
    algorithm: 'binary64_transversal_triangle_intersection_v1',
    maximumAttempts:
      MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    attempted: MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
    skippedByLimit: 1,
  })
  const hinge = result.interactions.find(
    ({ firstFaceId, secondFaceId }) =>
      firstFaceId === 'zz-limit-hinge-left'
      && secondFaceId === 'zz-limit-hinge-right',
  )
  assert.ok(hinge)
  assert.equal(hinge.geometryClass, 'indeterminate')
  assert.deepEqual(hinge.hingeDecision, {
    kind: 'indeterminate',
    hingeEdgeIds: ['limit-hinge'],
    reason: 'numerical_geometry',
  })
  const presentation = summarizeFoldPreviewCollision(result)
  assert.equal(presentation.indeterminateInteractions, 1)
  assert.equal(
    presentation.faceSeverities.get('zz-limit-hinge-left'),
    'indeterminate',
  )
})

test('cancellation is stable in pair-scan and witness cursor phases', () => {
  const faces = [face('a'), face('b')]
  const transforms = new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(0, 0.1, 0)],
  ])
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)

  const pairJob = analyzer.createAnalysisJob(transforms, 0.1)
  assert.ok(pairJob)
  const firstPair = pairJob.step(1)
  assert.equal(firstPair.kind, 'pending')
  assert.equal(firstPair.phase, 'triangle_pair_scan')
  pairJob.cancel()
  const pairCancelled = pairJob.step(1)
  assert.equal(pairCancelled.kind, 'cancelled')
  assert.deepEqual(pairCancelled.work, firstPair.work)
  assert.strictEqual(pairJob.step(17), pairCancelled)
  assertFrozenJobStep(pairCancelled)

  const witnessJob = analyzer.createAnalysisJob(transforms, 0.1)
  assert.ok(witnessJob)
  const afterPairs = witnessJob.step(4)
  assert.equal(afterPairs.kind, 'pending')
  assert.equal(afterPairs.phase, 'witness_derivation')
  const afterWitness = witnessJob.step(1)
  assert.equal(afterWitness.kind, 'pending')
  assert.equal(afterWitness.phase, 'witness_derivation')
  witnessJob.cancel()
  const witnessCancelled = witnessJob.step(1)
  assert.equal(witnessCancelled.kind, 'cancelled')
  assert.deepEqual(witnessCancelled.work, afterWitness.work)
  assert.strictEqual(witnessJob.step(17), witnessCancelled)
  assertFrozenJobStep(witnessCancelled)
})

test('witness-derivation reentry publishes one charged cancelled terminal', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createAnalysisJob(
    new Map(faces.map((item) => [item.id, new Matrix4()])),
    0.1,
  )
  assert.ok(job)
  const afterPair = job.step(1)
  assert.equal(afterPair.kind, 'pending')
  assert.equal(afterPair.phase, 'witness_derivation')

  const originalIsFinite = Number.isFinite
  let reentered = false
  let nested: FoldPreviewNarrowPhaseAnalysisJobStep | null = null
  Number.isFinite = function isFinite(value: unknown) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalIsFinite(value)
  }
  let outer: FoldPreviewNarrowPhaseAnalysisJobStep
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
  assert.strictEqual(job.step(1), outer)
  assertFrozenJobStep(outer)
})

test('a witness-helper throw is contained as one unavailable charged attempt', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createAnalysisJob(
    new Map(faces.map((item) => [item.id, new Matrix4()])),
    0.1,
  )
  assert.ok(job)
  const afterPair = job.step(1)
  assert.equal(afterPair.kind, 'pending')
  assert.equal(afterPair.phase, 'witness_derivation')

  const originalIsFinite = Number.isFinite
  Number.isFinite = function isFinite() {
    throw new Error('witness derivation failure')
  }
  let terminal: FoldPreviewNarrowPhaseAnalysisJobStep
  try {
    terminal = job.step(1)
  } finally {
    Number.isFinite = originalIsFinite
  }

  assert.equal(terminal.kind, 'complete')
  assert.deepEqual(terminal.work, {
    totalWorkUnits: 2,
    trianglePairTests: 1,
    witnessDerivations: 1,
  })
  assert.deepEqual(terminal.result.witnessSamples, [])
  assert.equal(terminal.result.witnessCoverage.attemptedPairCount, 1)
  assert.equal(terminal.result.witnessCoverage.unavailablePairCount, 1)
  assert.strictEqual(job.step(1), terminal)
  assertDeeplyFrozen(terminal)
})

test('invalid narrow-scan budgets fail closed with one terminal value', () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const transforms = new Map(faces.map((item) => [item.id, new Matrix4()]))

  for (const workBudget of [
    0,
    -1,
    0.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.NEGATIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1,
  ]) {
    const job = analyzer.createAnalysisJob(transforms, 0.1)
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
    assertFrozenJobStep(terminal)
  }
})

test('reentrant narrow-scan steps cancel without an outer overwrite', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createAnalysisJob(
    new Map(faces.map((item) => [item.id, new Matrix4()])),
    0.1,
  )
  assert.ok(job)

  const originalDot = Vector3.prototype.dot
  let reentered = false
  let nested: FoldPreviewNarrowPhaseAnalysisJobStep | null = null
  Vector3.prototype.dot = function dot(vector: Vector3) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalDot.call(this, vector)
  }
  let outer: FoldPreviewNarrowPhaseAnalysisJobStep
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
  assertFrozenJobStep(outer)
})

test('budget-validation reentry cancels before any cursor unit is charged', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createAnalysisJob(
    new Map(faces.map((item) => [item.id, new Matrix4()])),
    0.1,
  )
  assert.ok(job)

  const originalIsSafeInteger = Number.isSafeInteger
  let reentered = false
  let nested: FoldPreviewNarrowPhaseAnalysisJobStep | null = null
  Number.isSafeInteger = function isSafeInteger(value: unknown) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalIsSafeInteger(value)
  }
  let outer: FoldPreviewNarrowPhaseAnalysisJobStep
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
  assertFrozenJobStep(outer)
})

test('classifier exceptions fail closed after charging the visited pair', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createAnalysisJob(
    new Map(faces.map((item) => [item.id, new Matrix4()])),
    0.1,
  )
  assert.ok(job)

  const originalDot = Vector3.prototype.dot
  Vector3.prototype.dot = function dot() {
    throw new Error('classifier failure')
  }
  let terminal: FoldPreviewNarrowPhaseAnalysisJobStep
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
  assertFrozenJobStep(terminal)
})

test('cancellation outranks throws during validation and charged pair work', {
  concurrency: false,
}, () => {
  const faces = [face('a'), face('b')]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const transforms = new Map(
    faces.map((item) => [item.id, new Matrix4()]),
  )

  {
    const job = analyzer.createAnalysisJob(transforms, 0.1)
    assert.ok(job)
    const originalIsSafeInteger = Number.isSafeInteger
    Number.isSafeInteger = function isSafeInteger() {
      job.cancel()
      throw new Error('validation cancellation')
    }
    let terminal: FoldPreviewNarrowPhaseAnalysisJobStep
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
    assertFrozenJobStep(terminal)
  }

  {
    const job = analyzer.createAnalysisJob(transforms, 0.1)
    assert.ok(job)
    const originalDot = Vector3.prototype.dot
    Vector3.prototype.dot = function dot() {
      job.cancel()
      throw new Error('pair cancellation')
    }
    let terminal: FoldPreviewNarrowPhaseAnalysisJobStep
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
    assertFrozenJobStep(terminal)
  }
})

test('hinge-policy finalization preserves output but is explicitly outside cursor timing', () => {
  const fixture = hingeFixture()
  const synchronous = fixture.analyzer.analyze(fixture.transforms, 0.1)
  assert.ok(synchronous)
  const job = fixture.analyzer.createAnalysisJob(fixture.transforms, 0.1)
  assert.ok(job)
  assert.deepEqual(job.workBounds, {
    entireStepTimeBounded: false,
    synchronousFactoryPreparation: true,
    synchronousHingePolicyFinalization: true,
    synchronousResultFinalization: true,
    potentialTrianglePairCount: 1,
    maximumTrianglePairTests: 1,
    maximumWitnessDerivations: 0,
    maximumTotalWorkUnits: 1,
  })
  const terminal = job.step(1)
  assert.equal(terminal.kind, 'complete')
  assert.deepEqual(terminal.result, synchronous)
  assert.deepEqual(terminal.work, {
    totalWorkUnits: 1,
    trianglePairTests: 1,
    witnessDerivations: 0,
  })
  assert.equal(
    terminal.result.interactions[0]?.hingeDecision?.kind,
    'allowed_by_hinge_model',
  )
  assert.strictEqual(job.step(1), terminal)
})

test('hinge-policy cancellation, reentry, and exceptions are stable and fail closed', {
  concurrency: false,
}, () => {
  const originalDistanceTo = Vector3.prototype.distanceTo

  {
    const fixture = hingeFixture()
    const job = fixture.analyzer.createAnalysisJob(fixture.transforms, 0.1)
    assert.ok(job)
    let nested: FoldPreviewNarrowPhaseAnalysisJobStep | null = null
    Vector3.prototype.distanceTo = function distanceTo(vector: Vector3) {
      if (!nested) nested = job.step(1)
      return originalDistanceTo.call(this, vector)
    }
    let outer: FoldPreviewNarrowPhaseAnalysisJobStep
    try {
      outer = job.step(1)
    } finally {
      Vector3.prototype.distanceTo = originalDistanceTo
    }
    assert.ok(nested)
    assert.equal(nested.kind, 'cancelled')
    assert.strictEqual(outer, nested)
    assert.deepEqual(outer.work, {
      totalWorkUnits: 1,
      trianglePairTests: 1,
      witnessDerivations: 0,
    })
    assert.strictEqual(job.step(1), outer)
  }

  {
    const fixture = hingeFixture()
    const job = fixture.analyzer.createAnalysisJob(fixture.transforms, 0.1)
    assert.ok(job)
    Vector3.prototype.distanceTo = function distanceTo(vector: Vector3) {
      job.cancel()
      return originalDistanceTo.call(this, vector)
    }
    let terminal: FoldPreviewNarrowPhaseAnalysisJobStep
    try {
      terminal = job.step(1)
    } finally {
      Vector3.prototype.distanceTo = originalDistanceTo
    }
    assert.equal(terminal.kind, 'cancelled')
    assert.deepEqual(terminal.work, {
      totalWorkUnits: 1,
      trianglePairTests: 1,
      witnessDerivations: 0,
    })
    assert.strictEqual(job.step(1), terminal)
  }

  {
    const fixture = hingeFixture()
    const job = fixture.analyzer.createAnalysisJob(fixture.transforms, 0.1)
    assert.ok(job)
    Vector3.prototype.distanceTo = function distanceTo() {
      throw new Error('hinge policy failure')
    }
    let terminal: FoldPreviewNarrowPhaseAnalysisJobStep
    try {
      terminal = job.step(1)
    } finally {
      Vector3.prototype.distanceTo = originalDistanceTo
    }
    assert.equal(terminal.kind, 'indeterminate')
    assert.equal(terminal.reason, 'scan_error')
    assert.deepEqual(terminal.work, {
      totalWorkUnits: 1,
      trianglePairTests: 1,
      witnessDerivations: 0,
    })
    assert.strictEqual(job.step(1), terminal)
  }
})

test('the analysis-job factory snapshots stateful transforms once and rejects hostile maps', () => {
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
      return oneReadIdentityMatrix(`analysis.${faceId}`)
    },
  } as ReadonlyMap<string, Matrix4>
  const job = analyzer.createAnalysisJob(transforms, 0.1)
  assert.ok(job)
  assert.equal(sizeReads, 1)
  assert.deepEqual([...reads], [['a', 1], ['b', 1]])
  const terminal = drainAnalysisJob(job, 1)
  assert.equal(terminal.kind, 'complete')
  assert.equal(sizeReads, 1)
  assert.deepEqual([...reads], [['a', 1], ['b', 1]])

  const throwing = new Proxy(new Map<string, Matrix4>(), {
    get() {
      throw new Error('hostile map')
    },
  }) as ReadonlyMap<string, Matrix4>
  assert.equal(analyzer.createAnalysisJob(throwing, 0.1), null)

  const revocable = Proxy.revocable(new Map<string, Matrix4>(), {})
  revocable.revoke()
  assert.equal(analyzer.createAnalysisJob(revocable.proxy, 0.1), null)
})

test('potential work above one million still succeeds after an early penetration', () => {
  const faces = [
    face('a', regularPolygon(1_003)),
    face('b', regularPolygon(1_002)),
  ]
  const transforms = new Map(faces.map((item) => [item.id, new Matrix4()]))
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const synchronous = analyzer.analyze(transforms, 0.2)
  assert.ok(synchronous)
  assert.equal(synchronous.trianglePairTests, 1)
  assert.equal(synchronous.satTests, 1)

  const job = analyzer.createAnalysisJob(transforms, 0.2)
  assert.ok(job)
  assert.equal(job.workBounds.potentialTrianglePairCount, 1_001_000)
  assert.equal(
    job.workBounds.maximumTrianglePairTests,
    MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
  )
  const terminal = drainAnalysisJob(job, 1)
  assert.equal(terminal.kind, 'complete')
  assert.deepEqual(terminal.result, synchronous)
  assert.deepEqual(terminal.work, {
    totalWorkUnits: 2,
    trianglePairTests: 1,
    witnessDerivations: 1,
  })
})

test('the actual one-million pair limit terminates before another pair is charged', () => {
  const faces = [
    face('a', regularPolygon(103)),
    face('b', regularPolygon(9_903)),
  ]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createAnalysisJob(new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(190, 0, 190)],
  ]), 0.2)
  assert.ok(job)
  assert.equal(job.workBounds.potentialTrianglePairCount, 1_000_001)

  const terminal = job.step(MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS)
  assert.equal(terminal.kind, 'indeterminate')
  assert.equal(terminal.reason, 'work_limit_exceeded')
  assert.deepEqual(terminal.work, {
    totalWorkUnits: MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
    trianglePairTests: MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
    witnessDerivations: 0,
  })
  assert.strictEqual(job.step(1), terminal)
  assertFrozenJobStep(terminal)
})

test('exactly one million separated pair visits complete instead of hitting the limit', () => {
  const faces = [
    face('a', regularPolygon(1_002)),
    face('b', regularPolygon(1_002)),
  ]
  const analyzer = prepareFoldPreviewNarrowPhase(faces, [])
  assert.ok(analyzer)
  const job = analyzer.createAnalysisJob(new Map([
    ['a', new Matrix4()],
    ['b', new Matrix4().makeTranslation(190, 0, 190)],
  ]), 0.2)
  assert.ok(job)
  assert.equal(
    job.workBounds.potentialTrianglePairCount,
    MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
  )

  const terminal = job.step(MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS)
  assert.equal(terminal.kind, 'complete')
  assert.deepEqual(terminal.work, {
    totalWorkUnits: MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
    trianglePairTests: MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
    witnessDerivations: 0,
  })
  assert.equal(terminal.result.trianglePairTests, 1_000_000)
  assert.equal(terminal.result.satTests, 4)
  assert.deepEqual(terminal.result.interactions, [])
  assert.strictEqual(job.step(1), terminal)
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

function exactFallbackFixture(
  pairCount: number,
  appendDefinitivePenetration = false,
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

function drainAnalysisJob(
  job: FoldPreviewNarrowPhaseAnalysisJob,
  workBudget: number,
): FoldPreviewNarrowPhaseAnalysisJobStep {
  assert.equal(job.workBounds.entireStepTimeBounded, false)
  assert.equal(job.workBounds.synchronousFactoryPreparation, true)
  assert.equal(job.workBounds.synchronousHingePolicyFinalization, true)
  assert.equal(job.workBounds.synchronousResultFinalization, true)
  assert.equal(
    job.workBounds.maximumTrianglePairTests,
    Math.min(
      job.workBounds.potentialTrianglePairCount,
      MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS,
    ),
  )
  assert.equal(
    job.workBounds.maximumTotalWorkUnits,
    job.workBounds.maximumTrianglePairTests
      + job.workBounds.maximumWitnessDerivations,
  )
  assert.ok(Object.isFrozen(job.workBounds))
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
      FOLD_PREVIEW_NARROW_PHASE_ANALYSIS_JOB_VERSION,
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
        <= step.workBounds.maximumTrianglePairTests,
    )
    assert.ok(
      step.work.witnessDerivations
        <= step.workBounds.maximumWitnessDerivations,
    )
    assert.ok(
      step.work.totalWorkUnits <= step.workBounds.maximumTotalWorkUnits,
    )
    if (step.kind !== 'pending') return step
    assert.ok(totalDelta > 0, 'a pending step must make bounded cursor progress')
    previous = step.work
    previousExact = step.exactTransversalProofWork
  }
  assert.fail('narrow analysis job did not reach a terminal result')
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

function hingeFixture() {
  const start = { vertexId: 'start', x: 0, z: 0 } as const
  const end = { vertexId: 'end', x: 0, z: 1 } as const
  const faces: readonly FoldPreviewCollisionPoseFace[] = [
    {
      id: 'left',
      polygon: [
        start,
        { vertexId: 'left-tip', x: -1, z: 0.5 },
        end,
      ],
    },
    {
      id: 'right',
      polygon: [
        end,
        { vertexId: 'right-tip', x: 1, z: 0.5 },
        start,
      ],
    },
  ]
  const adjacencies: readonly FoldPreviewCollisionAdjacency[] = [{
    edgeId: 'hinge',
    firstFaceId: 'left',
    secondFaceId: 'right',
  }]
  const constraints: readonly FoldPreviewHingeContactConstraint[] = [{
    edgeId: 'hinge',
    leftFaceId: 'left',
    rightFaceId: 'right',
    start,
    end,
    thicknessRule: 'centered_mid_surface_v1',
  }]
  const analyzer = prepareFoldPreviewNarrowPhase(
    faces,
    adjacencies,
    constraints,
  )
  assert.ok(analyzer)
  return {
    analyzer,
    transforms: new Map(faces.map((item) => [item.id, new Matrix4()])),
  }
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

function assertFrozenJobStep(
  step: FoldPreviewNarrowPhaseAnalysisJobStep,
) {
  assertDeeplyFrozen(step)
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
