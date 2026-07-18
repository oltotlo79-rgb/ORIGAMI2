import assert from 'node:assert/strict'
import test from 'node:test'

import {
  deriveFoldPreviewTwoBodyCorrectionCandidate,
  FOLD_PREVIEW_TWO_BODY_CORRECTION_CANDIDATE_VERSION,
  MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS,
  MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS,
  type FoldPreviewTwoBodyCorrectionCandidate,
} from '../src/lib/foldPreviewTwoBodyCorrectionCandidate.ts'
import type {
  FoldPreviewTreeTerminalFullScanBinding,
} from '../src/lib/foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from '../src/lib/foldPreviewTreeScenePose.ts'
import {
  MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
} from '../src/lib/foldPreviewNarrowCollision.ts'

type Point = Readonly<{ x: number; y: number; z: number }>
type Body = 'stationary' | 'moving'

type ConstraintSpec = Readonly<{
  normal: Point
  firstBody?: Body
  secondBody?: Body
  escapeDistance?: number
  toleratedGap?: number
  geometryClass?: 'touching' | 'penetrating'
}>

const X = Object.freeze({ x: 1, y: 0, z: 0 })
const NEGATIVE_X = Object.freeze({ x: -1, y: 0, z: 0 })
const Y = Object.freeze({ x: 0, y: 1, z: 0 })
const Z = Object.freeze({ x: 0, y: 0, z: 1 })

test('normal sign and moving side produce the translation direction', () => {
  const cases = [
    {
      spec: { normal: X },
      axis: 'x' as const,
      sign: 1,
      movingSide: 'second',
    },
    {
      spec: { normal: NEGATIVE_X },
      axis: 'x' as const,
      sign: -1,
      movingSide: 'second',
    },
    {
      spec: {
        normal: Y,
        firstBody: 'moving' as const,
        secondBody: 'stationary' as const,
      },
      axis: 'y' as const,
      sign: -1,
      movingSide: 'first',
    },
  ]

  for (const current of cases) {
    const candidate = deriveFoldPreviewTwoBodyCorrectionCandidate(
      bindingFor([current.spec]),
      0.25,
      10,
    )
    assert.ok(candidate)
    assert.equal(Math.sign(candidate.translation[current.axis]), current.sign)
    assert.equal(candidate.constraints[0].movingSide, current.movingSide)
    assert.equal(
      Math.sign(candidate.constraints[0].direction[current.axis]),
      current.sign,
    )
    assertCandidateSatisfiesEveryConstraint(candidate)
  }
})

test('one, two, and three orthogonal constraints select exact active dimensions', () => {
  const specs = [
    { normal: X, escapeDistance: 1 },
    { normal: Y, escapeDistance: 2 },
    { normal: Z, escapeDistance: 3 },
  ] as const
  const expectedEvaluations = [1, 3, 7]

  for (let count = 1; count <= 3; count += 1) {
    const candidate = deriveFoldPreviewTwoBodyCorrectionCandidate(
      bindingFor(specs.slice(0, count)),
      0.5,
      10,
    )
    assert.ok(candidate)
    assert.equal(candidate.solver.activeSetSize, count)
    assert.deepEqual(
      candidate.solver.activeConstraintIndices,
      Array.from({ length: count }, (_, index) => index),
    )
    assert.equal(
      candidate.solver.evaluatedActiveSetCount,
      expectedEvaluations[count - 1],
    )
    for (let index = 0; index < count; index += 1) {
      const axis = ['x', 'y', 'z'][index] as keyof Point
      assert.ok(
        candidate.translation[axis]
          >= candidate.constraints[index].solverTargetProjection,
      )
    }
    assertCandidateSatisfiesEveryConstraint(candidate)
  }
})

test('parallel constraints reduce to the strongest while opposite demands conflict', () => {
  const redundant = deriveFoldPreviewTwoBodyCorrectionCandidate(
    bindingFor([
      { normal: X, escapeDistance: 1 },
      { normal: X, escapeDistance: 2 },
    ]),
    0.25,
    10,
  )
  assert.ok(redundant)
  assert.deepEqual(redundant.solver.activeConstraintIndices, [1])
  assert.equal(redundant.solver.activeSetSize, 1)
  assert.ok(
    redundant.translation.x
      >= redundant.constraints[1].solverTargetProjection,
  )
  assertCandidateSatisfiesEveryConstraint(redundant)

  assert.equal(
    deriveFoldPreviewTwoBodyCorrectionCandidate(
      bindingFor([
        { normal: X, escapeDistance: 1 },
        { normal: NEGATIVE_X, escapeDistance: 1 },
      ]),
      0.25,
      10,
    ),
    null,
  )
})

test('clearance and maximum translation fail closed at every numeric boundary', () => {
  const binding = bindingFor([{ normal: X, escapeDistance: 1 }])
  const valid = deriveFoldPreviewTwoBodyCorrectionCandidate(
    binding,
    0.25,
    10,
  )
  assert.ok(valid)
  assert.equal(valid.clearance, 0.25)
  assert.equal(valid.maximumTranslation, 10)
  assert.ok(valid.constraints[0].requiredProjection > 1)
  assert.ok(
    valid.constraints[0].solverTargetProjection
      > valid.constraints[0].requiredProjection,
  )

  for (const [clearance, maximumTranslation] of [
    [0, 10],
    [-1, 10],
    [Number.NaN, 10],
    [Number.POSITIVE_INFINITY, 10],
    [0.25, 0],
    [0.25, -1],
    [0.25, Number.NaN],
    [0.25, Number.POSITIVE_INFINITY],
    [2, 1],
  ]) {
    assert.equal(
      deriveFoldPreviewTwoBodyCorrectionCandidate(
        binding,
        clearance,
        maximumTranslation,
      ),
      null,
    )
  }

  // The exact required projection is insufficient because the solver target
  // is the next representable value on the safe side.
  assert.equal(
    deriveFoldPreviewTwoBodyCorrectionCandidate(binding, 0.25, 1.25),
    null,
  )

  // Each orthogonal projection fits, but their combined norm exceeds max.
  assert.equal(
    deriveFoldPreviewTwoBodyCorrectionCandidate(
      bindingFor([
        {
          normal: X,
          escapeDistance: 0,
          geometryClass: 'touching',
        },
        {
          normal: Y,
          escapeDistance: 0,
          geometryClass: 'touching',
        },
      ]),
      1,
      1.1,
    ),
    null,
  )
})

test('raw, incomplete, same-body, and forged bindings are never solver inputs', () => {
  const binding = bindingFor([{ normal: X }])
  const incompleteEvidence = {
    ...binding.evidence,
    kind: 'unavailable',
    reasons: ['witness_limit_exceeded'],
    witnessSamples: [],
  }
  const invalidBindings = [
    binding.evidence,
    bindingFor([{
      normal: X,
      firstBody: 'stationary',
      secondBody: 'stationary',
    }]),
    { ...binding, version: 'future-binding' },
    { ...binding, sourcePose: 'analyzed_input_pose' },
    { ...binding, requestIdentityBound: false },
    { ...binding, evidence: incompleteEvidence },
    {
      ...binding,
      evidence: { ...binding.evidence, requestIdentityBound: true },
    },
    {
      ...binding,
      safety: { ...binding.safety, autoApplicable: true },
    },
    {
      ...binding,
      safety: {
        ...binding.safety,
        twoBodyTranslationInputEligible: false,
      },
    },
  ]

  for (const invalid of invalidBindings) {
    assert.equal(
      deriveFoldPreviewTwoBodyCorrectionCandidate(
        invalid as FoldPreviewTreeTerminalFullScanBinding,
        0.25,
        10,
      ),
      null,
    )
  }
})

test('partition, witness index, identity, and coverage corruption is rejected', () => {
  const binding = bindingFor([
    { normal: X },
    { normal: Y },
  ])
  const [firstSample, secondSample] = binding.evidence.witnessSamples
  const invalidBindings = [
    {
      ...binding,
      partition: {
        ...binding.partition,
        witnessRelations: binding.partition.witnessRelations.map(
          (relation, index) => ({
            ...relation,
            witnessIndex: index + 1,
          }),
        ),
      },
    },
    {
      ...binding,
      partition: {
        ...binding.partition,
        witnessRelations: binding.partition.witnessRelations.slice(0, 1),
      },
    },
    {
      ...binding,
      partition: {
        ...binding.partition,
        movingFaceIds: ['unknown-moving-face'],
      },
    },
    {
      ...binding,
      partition: {
        ...binding.partition,
        movingFaceIds: [
          ...binding.partition.movingFaceIds,
          binding.identity.fixedFaceId,
        ],
      },
    },
    {
      ...binding,
      partition: {
        ...binding.partition,
        stationaryFaceIds: binding.partition.stationaryFaceIds.filter(
          (faceId) => faceId !== binding.identity.fixedFaceId,
        ),
      },
    },
    {
      ...binding,
      evidence: {
        ...binding.evidence,
        exactTransversalProofWork: {
          ...binding.evidence.exactTransversalProofWork,
          skippedByLimit: 1,
        },
      },
    },
    {
      ...binding,
      evidence: {
        ...binding.evidence,
        exactTransversalProofWork: {
          ...binding.evidence.exactTransversalProofWork,
          attempted:
            MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS + 1,
        },
      },
    },
    {
      ...binding,
      evidence: {
        ...binding.evidence,
        coverage: {
          ...binding.evidence.coverage,
          availablePairCount:
            binding.evidence.coverage.availablePairCount + 1,
        },
      },
    },
    {
      ...binding,
      evidence: {
        ...binding.evidence,
        coverage: {
          ...binding.evidence.coverage,
          broadPhaseCandidateCount: 0,
        },
      },
    },
    {
      ...binding,
      evidence: {
        ...binding.evidence,
        coverage: {
          ...binding.evidence.coverage,
          allCollisionConstraintsRepresented: false,
        },
      },
    },
    {
      ...binding,
      evidence: {
        ...binding.evidence,
        witnessSamples: [
          firstSample,
          {
            ...secondSample,
            firstFaceId: firstSample.firstFaceId,
            secondFaceId: firstSample.secondFaceId,
            firstTriangleIndex: firstSample.firstTriangleIndex,
            secondTriangleIndex: firstSample.secondTriangleIndex,
          },
        ],
      },
    },
  ]

  for (const invalid of invalidBindings) {
    assert.equal(
      deriveFoldPreviewTwoBodyCorrectionCandidate(
        invalid as FoldPreviewTreeTerminalFullScanBinding,
        0.25,
        10,
      ),
      null,
    )
  }
})

test('directed arithmetic and interval proofs cover cancellation and dot rounding', () => {
  const cancellation = deriveFoldPreviewTwoBodyCorrectionCandidate(
    bindingFor([{
      normal: X,
      escapeDistance: 1,
      toleratedGap: 1e16,
      geometryClass: 'touching',
    }], 1e16),
    0.5,
    10,
  )
  assert.ok(cancellation)
  assert.ok(cancellation.constraints[0].requiredProjection > 1.5)
  assert.ok(
    cancellation.translation.x
      - cancellation.constraints[0].solverTargetProjection
      < 1e-14,
  )
  assertCandidateSatisfiesEveryConstraint(cancellation)

  const auditedNormal = {
    x: -0.6065796062703668,
    y: -0.7916308813210694,
    z: -0.07336026850900894,
  }
  assert.equal(Math.hypot(
    auditedNormal.x,
    auditedNormal.y,
    auditedNormal.z,
  ), 1)
  const roundedDot = deriveFoldPreviewTwoBodyCorrectionCandidate(
    bindingFor([{
      normal: auditedNormal,
      escapeDistance: 0,
      geometryClass: 'touching',
    }]),
    0.0009479939972014623,
    1,
  )
  assert.ok(roundedDot)
  assertCandidateSatisfiesEveryConstraint(roundedDot)

  const largeTarget = deriveFoldPreviewTwoBodyCorrectionCandidate(
    bindingFor([{ normal: X, escapeDistance: 2e12 }]),
    1,
    3e12,
  )
  assert.ok(largeTarget)
  assertCandidateSatisfiesEveryConstraint(largeTarget)
})

test('pose keys and every retained witness component are revalidated', () => {
  const binding = bindingFor([{ normal: X }])
  const sample = binding.evidence.witnessSamples[0]
  const witness = sample.witness
  const invalidWitnesses = [
    { ...witness, firstSupport: [] },
    {
      ...witness,
      positionRegion: {
        ...witness.positionRegion,
        generators: [{ x: 0.25, y: 0, z: 0 }],
      },
    },
    {
      ...witness,
      localSeparationHint: {
        ...witness.localSeparationHint,
        translation: { x: Number.POSITIVE_INFINITY, y: 0, z: 0 },
      },
    },
  ]
  const invalidBindings: unknown[] = [
    {
      ...binding,
      identity: {
        ...binding.identity,
        request: {
          ...binding.identity.request,
          sourcePoseRequestKey: 'forged-source-pose-key',
        },
      },
    },
    {
      ...binding,
      identity: {
        ...binding.identity,
        blockingPoseRequestKey: 'forged-blocking-pose-key',
      },
    },
    ...invalidWitnesses.map((invalidWitness) => ({
      ...binding,
      evidence: {
        ...binding.evidence,
        witnessSamples: [{ ...sample, witness: invalidWitness }],
      },
    })),
  ]
  for (const invalid of invalidBindings) {
    assert.equal(
      deriveFoldPreviewTwoBodyCorrectionCandidate(
        invalid as FoldPreviewTreeTerminalFullScanBinding,
        0.25,
        10,
      ),
      null,
    )
  }
})

test('output is deterministic, detached, deeply frozen, and explicitly unsafe', () => {
  const binding = bindingFor([
    { normal: X, escapeDistance: 1 },
    { normal: Y, escapeDistance: 2 },
    { normal: Z, escapeDistance: 3 },
  ])
  const first = deriveFoldPreviewTwoBodyCorrectionCandidate(
    binding,
    0.25,
    10,
  )
  const second = deriveFoldPreviewTwoBodyCorrectionCandidate(
    binding,
    0.25,
    10,
  )
  assert.ok(first && second)
  assert.deepEqual(second, first)
  assert.notStrictEqual(second, first)
  assert.equal(
    first.version,
    FOLD_PREVIEW_TWO_BODY_CORRECTION_CANDIDATE_VERSION,
  )
  assert.equal(first.kind, 'unverified_two_body_translation_candidate')
  assert.equal(first.solver.method, 'certified_outward_active_set_3d_v1')
  assert.equal(
    first.solver.seedMethod,
    'minimum_norm_kkt_active_set_3d_v1',
  )
  assert.deepEqual(first.sourceIdentity, {
    bindingVersion: 'tree_single_hinge_terminal_full_scan_binding_v1',
    projectId: 'correction-project',
    revision: 7,
    fixedFaceId: 'fixed-face',
    selectedHingeEdgeId: 'selected-hinge',
    contextKey: 'correction-context',
    sourcePoseRequestKey: binding.identity.request.sourcePoseRequestKey,
    blockingPoseRequestKey: binding.identity.blockingPoseRequestKey,
    generation: 11,
    requestSequence: 13,
    blockingSampleTime: 0.5,
    selectedAngleDegrees: 50,
    collisionThickness: 0.02,
  })
  assert.deepEqual(first.safety, {
    sourcePairConstraintsSatisfied: true,
    nonAdjacentScopeOnly: true,
    hingeAdjacentPairsIncluded: false,
    wholeSceneConstraintsRepresented: false,
    legalCorrectionPoseGenerated: false,
    staticCandidateRevalidated: false,
    continuousCandidatePathCertified: false,
    autoApplicable: false,
  })
  assertCandidateSatisfiesEveryConstraint(first)
  assertDeeplyFrozen(first)
})

test('source partition preserves validated order and is detached from later forgery', () => {
  const binding = bindingFor([
    { normal: X },
    { normal: Y },
  ])
  const stationaryInput =
    binding.partition.stationaryFaceIds as string[]
  const movingInput = binding.partition.movingFaceIds as string[]
  stationaryInput.reverse()
  movingInput.reverse()

  const candidate = deriveFoldPreviewTwoBodyCorrectionCandidate(
    binding,
    0.25,
    10,
  )
  assert.ok(candidate)
  assert.deepEqual(candidate.sourcePartition, {
    version: 'rerooted_selected_hinge_partition_v1',
    stationaryFaceIds: [
      'stationary-first-1',
      'stationary-first-0',
      'fixed-face',
    ],
    movingFaceIds: [
      'moving-second-1',
      'moving-second-0',
      'moving-anchor',
    ],
  })
  assert.notStrictEqual(candidate.sourcePartition, binding.partition)
  assert.notStrictEqual(
    candidate.sourcePartition.stationaryFaceIds,
    stationaryInput,
  )
  assert.notStrictEqual(
    candidate.sourcePartition.movingFaceIds,
    movingInput,
  )
  assertDeeplyFrozen(candidate.sourcePartition)

  stationaryInput[0] = 'forged-overlap'
  movingInput[0] = 'forged-overlap'
  assert.deepEqual(candidate.sourcePartition, {
    version: 'rerooted_selected_hinge_partition_v1',
    stationaryFaceIds: [
      'stationary-first-1',
      'stationary-first-0',
      'fixed-face',
    ],
    movingFaceIds: [
      'moving-second-1',
      'moving-second-0',
      'moving-anchor',
    ],
  })
  assert.equal(
    deriveFoldPreviewTwoBodyCorrectionCandidate(binding, 0.25, 10),
    null,
  )
})

test('public input is snapshotted once and hostile proxies fail closed', () => {
  const binding = bindingFor([{ normal: X }])
  const topLevelReads = new Map<PropertyKey, number>()
  const guardedPoint = oneReadPoint(X)
  const sourceSample = binding.evidence.witnessSamples[0]
  const guardedFirstSupport = oneReadArray(
    sourceSample.witness.firstSupport,
  )
  const guardedSecondSupport = oneReadArray(
    sourceSample.witness.secondSupport,
  )
  const guardedGenerators = oneReadArray(
    sourceSample.witness.positionRegion.generators,
  )
  const guardedHintTranslation = oneReadPoint(
    sourceSample.witness.localSeparationHint.translation,
  )
  const guardedSamples = oneReadArray([{
    ...sourceSample,
    witness: {
      ...sourceSample.witness,
      normal: {
        ...sourceSample.witness.normal,
        vector: guardedPoint.value,
      },
      firstSupport: guardedFirstSupport.values,
      secondSupport: guardedSecondSupport.values,
      positionRegion: {
        ...sourceSample.witness.positionRegion,
        generators: guardedGenerators.values,
      },
      localSeparationHint: {
        ...sourceSample.witness.localSeparationHint,
        translation: guardedHintTranslation.value,
      },
    },
  }])
  const guardedBinding = new Proxy({
    ...binding,
    evidence: {
      ...binding.evidence,
      witnessSamples: guardedSamples.values,
    },
  }, {
    get(target, property, receiver) {
      const next = (topLevelReads.get(property) ?? 0) + 1
      topLevelReads.set(property, next)
      if (next > 1) throw new Error(`top-level reread: ${String(property)}`)
      return Reflect.get(target, property, receiver)
    },
  })
  const candidate = deriveFoldPreviewTwoBodyCorrectionCandidate(
    guardedBinding,
    0.25,
    10,
  )
  assert.ok(candidate)
  for (const property of [
    'version',
    'sourcePose',
    'requestIdentityBound',
    'identity',
    'blockingSampleTime',
    'selectedAngleDegrees',
    'collisionThickness',
    'angleVectors',
    'partition',
    'evidence',
    'safety',
  ]) {
    assert.equal(topLevelReads.get(property), 1, property)
  }
  assert.deepEqual(guardedPoint.reads(), { x: 1, y: 1, z: 1 })
  assert.equal(guardedSamples.lengthReads(), 1)
  assert.deepEqual(guardedSamples.indexReads(), [1])
  for (const guarded of [
    guardedFirstSupport,
    guardedSecondSupport,
    guardedGenerators,
  ]) {
    assert.equal(guarded.lengthReads(), 1)
    assert.deepEqual(guarded.indexReads(), [1])
  }
  assert.deepEqual(
    guardedHintTranslation.reads(),
    { x: 1, y: 1, z: 1 },
  )

  const throwing = new Proxy(binding, {
    get() {
      throw new Error('hostile binding')
    },
  })
  assert.equal(
    deriveFoldPreviewTwoBodyCorrectionCandidate(throwing, 0.25, 10),
    null,
  )
  const revocable = Proxy.revocable(binding, {})
  revocable.revoke()
  assert.equal(
    deriveFoldPreviewTwoBodyCorrectionCandidate(
      revocable.proxy,
      0.25,
      10,
    ),
    null,
  )
})

test('sixteen constraints evaluate the exact 696 active-set ceiling', () => {
  assert.equal(MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS, 16)
  assert.equal(MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS, 696)
  const specs = Array.from(
    { length: MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS },
    () => ({ normal: X, escapeDistance: 1 }),
  )
  const candidate = deriveFoldPreviewTwoBodyCorrectionCandidate(
    bindingFor(specs),
    0.25,
    10,
  )
  assert.ok(candidate)
  assert.equal(
    candidate.constraints.length,
    MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_CONSTRAINTS,
  )
  assert.equal(
    candidate.solver.evaluatedActiveSetCount,
    MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS,
  )
  assert.equal(
    candidate.solver.maximumActiveSetCount,
    MAX_FOLD_PREVIEW_TWO_BODY_CORRECTION_ACTIVE_SETS,
  )
  assert.deepEqual(candidate.solver.activeConstraintIndices, [0])
  assertCandidateSatisfiesEveryConstraint(candidate)

  assert.equal(
    deriveFoldPreviewTwoBodyCorrectionCandidate(
      bindingFor([
        ...specs,
        { normal: X, escapeDistance: 1 },
      ]),
      0.25,
      10,
    ),
    null,
  )
})

function bindingFor(
  specs: readonly ConstraintSpec[],
  numericalMargin = 0,
): FoldPreviewTreeTerminalFullScanBinding {
  const stationaryFaceIds = new Set<string>(['fixed-face'])
  const movingFaceIds = new Set<string>(['moving-anchor'])
  const witnessSamples:
    FoldPreviewTreeTerminalFullScanBinding['evidence']['witnessSamples'] = []
  const witnessRelations:
    FoldPreviewTreeTerminalFullScanBinding['partition']['witnessRelations'] = []
  let touchingPairCount = 0
  let penetratingPairCount = 0
  let sameBodyWitnessCount = 0

  for (let index = 0; index < specs.length; index += 1) {
    const spec = specs[index]
    const firstBody = spec.firstBody ?? 'stationary'
    const secondBody = spec.secondBody ?? 'moving'
    const firstFaceId = `${firstBody}-first-${index}`
    const secondFaceId = `${secondBody}-second-${index}`
    if (firstBody === 'stationary') {
      stationaryFaceIds.add(firstFaceId)
    } else {
      movingFaceIds.add(firstFaceId)
    }
    if (secondBody === 'stationary') {
      stationaryFaceIds.add(secondFaceId)
    } else {
      movingFaceIds.add(secondFaceId)
    }
    const escapeDistance = spec.escapeDistance ?? 1
    const toleratedGap = spec.toleratedGap ?? 0
    const geometryClass = spec.geometryClass
      ?? (escapeDistance > 0 ? 'penetrating' : 'touching')
    if (geometryClass === 'penetrating') {
      penetratingPairCount += 1
    } else {
      touchingPairCount += 1
    }
    const normal = { ...spec.normal }
    witnessSamples.push({
      firstFaceId,
      secondFaceId,
      relation: 'non_adjacent',
      firstTriangleIndex: index,
      secondTriangleIndex: 0,
      geometryClass,
      witness: {
        algorithm: 'triangle_prism_sat_witness_v1',
        geometryClass,
        numericalMargin,
        normal: {
          vector: normal,
          convention: 'moves_second_away_from_first',
          uniqueness: 'unique',
        },
        escapeDistance,
        toleratedGap,
        firstSupport: [{ x: 0, y: 0, z: 0 }],
        secondSupport: [{ x: 1, y: 0, z: 0 }],
        positionRegion: {
          kind: 'support_midpoint_hull_v1',
          sourcePose: 'analyzed_input_pose',
          generators: [{ x: 0.5, y: 0, z: 0 }],
        },
        localSeparationHint: {
          translation: {
            x: normal.x * escapeDistance,
            y: normal.y * escapeDistance,
            z: normal.z * escapeDistance,
          },
          distance: escapeDistance,
          scope: 'selected_triangle_prism_pair_only',
          autoApplicable: false,
        },
      },
    })
    const relation = firstBody === secondBody
      ? firstBody === 'stationary'
        ? 'stationary_internal'
        : 'moving_internal'
      : 'cross_partition'
    if (relation !== 'cross_partition') sameBodyWitnessCount += 1
    witnessRelations.push({
      witnessIndex: index,
      firstBody,
      secondBody,
      relation,
    })
  }

  const allWitnessesCrossPartition = sameBodyWitnessCount === 0
  const count = specs.length
  const angleVectors = {
    start: [
      { edgeId: 'selected-hinge', angleDegrees: 0 },
      { edgeId: 'frozen-hinge', angleDegrees: 30 },
    ],
    target: [
      { edgeId: 'selected-hinge', angleDegrees: 100 },
      { edgeId: 'frozen-hinge', angleDegrees: 30 },
    ],
    sample: [
      { edgeId: 'selected-hinge', angleDegrees: 50 },
      { edgeId: 'frozen-hinge', angleDegrees: 30 },
    ],
  } as const
  const poseIdentity = {
    projectId: 'correction-project',
    revision: 7,
    kind: 'fold_graph',
  } as const
  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    poseIdentity,
    'fixed-face',
    0.02,
    angleVectors.start,
  )
  const blockingPoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    poseIdentity,
    'fixed-face',
    0.02,
    angleVectors.sample,
  )
  assert.ok(sourcePoseRequestKey && blockingPoseRequestKey)
  return {
    version: 'tree_single_hinge_terminal_full_scan_binding_v1',
    sourcePose: 'blocking_evaluate_point_pose',
    requestIdentityBound: true,
    identity: {
      projectId: 'correction-project',
      revision: 7,
      revisionBinding: 'project_response_source_equal_v1',
      fixedFaceId: 'fixed-face',
      selectedHingeEdgeId: 'selected-hinge',
      request: {
        contextKey: 'correction-context',
        sourcePoseRequestKey,
        generation: 11,
        requestSequence: 13,
      },
      blockingPoseRequestKey,
    },
    blockingSampleTime: 0.5,
    selectedAngleDegrees: 50,
    collisionThickness: 0.02,
    angleVectors,
    partition: {
      version: 'rerooted_selected_hinge_partition_v1',
      stationaryFaceIds: [...stationaryFaceIds],
      movingFaceIds: [...movingFaceIds],
      witnessRelations,
    },
    evidence: {
      kind: 'complete',
      algorithm: 'full_non_adjacent_prism_witness_scan_v2',
      sourcePose: 'analyzed_input_pose',
      requestIdentityBound: false,
      collisionThickness: 0.02,
      numericalMargin,
      exactTransversalProofWork: {
        algorithm: 'binary64_transversal_triangle_intersection_v1',
        maximumAttempts:
          MAX_FOLD_PREVIEW_EXACT_TRANSVERSAL_PROOF_ATTEMPTS,
        attempted: 0,
        skippedByLimit: 0,
      },
      autoApplicable: false,
      coverage: {
        scope: 'all_broad_phase_non_adjacent_triangle_pairs_full_scan_v2',
        broadPhaseCandidateCount: count,
        expectedTrianglePairCount: count,
        trianglePairTests: count,
        aabbRejectedPairCount: 0,
        satTests: count,
        satSeparatedPairCount: 0,
        allowedSharedVertexPairCount: 0,
        touchingPairCount,
        penetratingPairCount,
        indeterminatePairCount: 0,
        eligiblePairCount: count,
        attemptedPairCount: count,
        availablePairCount: count,
        unavailablePairCount: 0,
        omittedByLimitCount: 0,
        authoritativePairScanComplete: true,
        allCollisionConstraintsRepresented: true,
      },
      witnessSamples,
    },
    safety: {
      nonAdjacentScopeOnly: true,
      hingeAdjacentPairsIncluded: false,
      allWitnessesCrossPartition,
      sameBodyWitnessCount,
      twoBodyTranslationInputEligible: allWitnessesCrossPartition,
      wholeSceneConstraintsRepresented: false,
      legalCorrectionPoseGenerated: false,
      staticCandidateRevalidated: false,
      continuousCandidatePathCertified: false,
      autoApplicable: false,
    },
  }
}

function assertCandidateSatisfiesEveryConstraint(
  candidate: FoldPreviewTwoBodyCorrectionCandidate,
) {
  assert.ok(candidate.magnitude > 0)
  assert.ok(
    candidate.magnitude <= candidate.certifiedMagnitudeUpperBound,
  )
  assert.ok(
    candidate.certifiedMagnitudeUpperBound <= candidate.maximumTranslation,
  )
  assert.equal(candidate.constraints.length > 0, true)
  for (const constraint of candidate.constraints) {
    const achieved = constraint.direction.x * candidate.translation.x
      + constraint.direction.y * candidate.translation.y
      + constraint.direction.z * candidate.translation.z
    assert.equal(constraint.achievedProjection, achieved)
    assert.ok(
      constraint.achievedProjection >= constraint.solverTargetProjection,
    )
    assert.ok(
      constraint.certifiedProjectionLowerBound
        < constraint.achievedProjection,
    )
    assert.ok(
      constraint.certifiedProjectionLowerBound
        > constraint.requiredProjection,
    )
    assert.ok(
      constraint.solverTargetProjection > constraint.requiredProjection,
    )
  }
}

function assertDeeplyFrozen(value: unknown, seen = new Set<object>()) {
  if (typeof value !== 'object' || value === null || seen.has(value)) return
  seen.add(value)
  assert.ok(Object.isFrozen(value))
  for (const key of Reflect.ownKeys(value)) {
    assertDeeplyFrozen(
      (value as Record<PropertyKey, unknown>)[key],
      seen,
    )
  }
}

function oneReadPoint(value: Point) {
  const reads = { x: 0, y: 0, z: 0 }
  const result = {}
  for (const axis of ['x', 'y', 'z'] as const) {
    Object.defineProperty(result, axis, {
      enumerable: true,
      get() {
        reads[axis] += 1
        if (reads[axis] > 1) throw new Error(`point reread: ${axis}`)
        return value[axis]
      },
    })
  }
  return {
    value: result as Point,
    reads: () => ({ ...reads }),
  }
}

function oneReadArray<T>(values: readonly T[]) {
  let lengthReads = 0
  const indexReads = Array.from({ length: values.length }, () => 0)
  const guarded = new Proxy([...values], {
    get(target, property, receiver) {
      if (property === 'length') {
        lengthReads += 1
        if (lengthReads > 1) throw new Error('array length reread')
      } else if (
        typeof property === 'string'
        && /^\d+$/u.test(property)
      ) {
        const index = Number(property)
        indexReads[index] += 1
        if (indexReads[index] > 1) {
          throw new Error(`array index reread: ${index}`)
        }
      }
      return Reflect.get(target, property, receiver)
    },
  })
  return {
    values: guarded,
    lengthReads: () => lengthReads,
    indexReads: () => [...indexReads],
  }
}
