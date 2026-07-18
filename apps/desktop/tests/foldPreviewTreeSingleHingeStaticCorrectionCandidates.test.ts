import assert from 'node:assert/strict'
import test from 'node:test'
import { Vector3 } from 'three'

import {
  createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob,
  deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates,
  FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION,
  FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION,
  isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext,
  MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS,
  type FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob,
  type FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep,
} from '../src/lib/foldPreviewTreeSingleHingeStaticCorrectionCandidates.ts'
import {
  prepareFoldPreviewTreeSingleHingeContinuousCollision,
  type FoldPreviewTreeSingleHingeContinuousAnalyzer,
  type FoldPreviewTreeTerminalFullScanBinding,
} from '../src/lib/foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  prepareFoldPreviewTreeMotionContext,
  type FoldPreviewTreeMotionContext,
} from '../src/lib/foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from '../src/lib/foldPreviewTreeScenePose.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from '../src/lib/foldPreviewModel.ts'

const COLLISION_THICKNESS = 0.02
const CLEARANCE = 0.005
const MAXIMUM_TRANSLATION = 0.01
const MAXIMUM_ANGLE_DELTA_DEGREES = 30
const EXPECTED_BLOCKING_ANGLE_DEGREES = 117.5335693359375
const EXPECTED_CORRECTION_ANGLE_DEGREES = 117.02300314111429

test('a genuine terminal binding produces a legal whole-scene static-clear angle', () => {
  const fixture = correctionFixture()
  const result = deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
    fixture.context,
    fixture.binding,
    CLEARANCE,
    MAXIMUM_TRANSLATION,
    MAXIMUM_ANGLE_DELTA_DEGREES,
  )
  assert.ok(result)
  assert.equal(
    result.version,
    FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION,
  )
  assert.equal(
    result.kind,
    'statically_revalidated_single_hinge_correction_candidates',
  )
  assert.ok(result.candidates.length > 0)

  const candidate = result.candidates[0]
  assert.ok(candidate)
  assert.ok(
    Math.abs(
      candidate.pose.selectedAngleDegrees
        - EXPECTED_CORRECTION_ANGLE_DEGREES,
    ) < 1e-10,
  )
  assert.equal(candidate.pose.appliedAngles.length, 2)
  assert.deepEqual(
    candidate.pose.appliedAngles
      .filter((angle) => angle.edgeId !== 'selected'),
    [{ edgeId: 'frozen', angleDegrees: 90 }],
  )
  assert.equal(
    candidate.pose.appliedAngles.find(
      (angle) => angle.edgeId === 'selected',
    )?.angleDegrees,
    candidate.pose.selectedAngleDegrees,
  )
  assert.equal(
    candidate.pose.poseRequestKey,
    createFoldPreviewTreeSceneCollisionPoseKey(
      fixture.context.model,
      fixture.context.fixedFaceId,
      fixture.context.collisionThickness,
      candidate.pose.appliedAngles,
    ),
  )
  assert.ok(candidate.fit.signedDeltaDegrees < 0)
  assert.ok(candidate.fit.improvementRatio > 0)
  assert.ok(candidate.fit.residualSquared >= 0)
  assert.ok(candidate.fit.residualRms >= 0)
  assert.equal(candidate.staticAnalysis.interactionCount, 2)
  assert.equal(candidate.staticAnalysis.allowedHingeInteractionCount, 2)
  assert.equal(
    candidate.staticAnalysis.broadPhaseNonAdjacentCandidateCount,
    0,
  )
  assert.equal(
    candidate.staticAnalysis.broadPhaseHingeAdjacentCandidateCount,
    2,
  )
  assert.equal(
    candidate.staticAnalysis.fullScanBroadPhaseCandidateCount,
    0,
  )
  assert.equal(candidate.staticAnalysis.fullScanExpectedTrianglePairCount, 0)
  assert.equal(candidate.staticAnalysis.fullScanTrianglePairTests, 0)
  assert.deepEqual(result.staticValidationWork, {
    strategy: 'full_non_adjacent_then_hinge_policy_v1',
    maximumTrianglePairVisits:
      MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS,
    plannedTrianglePairVisitUpperBound: 24,
    actualTrianglePairVisits: 8,
    fullScanCount: 1,
    narrowScanCount: 1,
  })
  assert.deepEqual(candidate.safety, {
    modelIdentityBound: true,
    completeLegalAngleVectorGenerated: true,
    legalCorrectionPoseGenerated: true,
    collisionConstraintsRevalidated: true,
    hingeContactPolicySatisfied: true,
    wholeSceneStaticClear: true,
    staticCandidateRevalidated: true,
    continuousCandidatePathCertified: false,
    sceneApplied: false,
    autoApplicable: false,
  })
})

test('identity, pose keys, and the rerooted partition remain exactly bound', () => {
  const fixture = correctionFixture()
  const result = deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
    fixture.context,
    fixture.binding,
    CLEARANCE,
    MAXIMUM_TRANSLATION,
    MAXIMUM_ANGLE_DELTA_DEGREES,
  )
  assert.ok(result)
  assert.equal(
    fixture.binding.selectedAngleDegrees,
    EXPECTED_BLOCKING_ANGLE_DEGREES,
  )
  assert.deepEqual(result.sourcePartition, {
    version: 'rerooted_selected_hinge_partition_v1',
    stationaryFaceIds: ['root', 'obstacle'],
    movingFaceIds: ['moving'],
  })
  assert.equal(result.sourceIdentity.projectId, fixture.context.model.projectId)
  assert.equal(result.sourceIdentity.revision, fixture.context.model.revision)
  assert.equal(result.sourceIdentity.fixedFaceId, 'root')
  assert.equal(result.sourceIdentity.selectedHingeEdgeId, 'selected')
  assert.equal(result.sourceIdentity.contextKey, fixture.context.contextKey)
  assert.equal(
    result.sourceIdentity.sourcePoseRequestKey,
    fixture.sourcePoseRequestKey,
  )
  assert.equal(
    result.sourceIdentity.blockingPoseRequestKey,
    fixture.binding.identity.blockingPoseRequestKey,
  )
  assert.equal(result.sourceIdentity.generation, 7)
  assert.equal(result.sourceIdentity.requestSequence, 11)
  assert.equal(
    result.sourceIdentity.blockingSampleTime,
    fixture.binding.blockingSampleTime,
  )
  assert.equal(
    result.sourceIdentity.blockingSelectedAngleDegrees,
    fixture.binding.selectedAngleDegrees,
  )
  assert.equal(
    result.sourceIdentity.sourceSelectedAngleDegrees,
    fixture.context.selectedAngleDegrees,
  )
  assert.equal(
    result.sourceIdentity.collisionThickness,
    COLLISION_THICKNESS,
  )
  assert.equal(result.commonTranslation.clearance, CLEARANCE)
  assert.equal(
    result.commonTranslation.maximumTranslation,
    MAXIMUM_TRANSLATION,
  )
  assert.equal(result.commonTranslation.constraintCount, 3)
  assert.ok(result.commonTranslation.magnitude > 0)
  assert.ok(
    result.commonTranslation.magnitude
      <= result.commonTranslation.certifiedMagnitudeUpperBound,
  )
  assert.ok(
    result.commonTranslation.certifiedMagnitudeUpperBound
      <= MAXIMUM_TRANSLATION,
  )
  assert.equal(result.rotationFit.childRotationSign, 1)
  assert.equal(result.rotationFit.movingPointCount, 4)
  assert.equal(result.rotationFit.seedCount, result.candidates.length)
  assert.deepEqual(result.rotationFit.worldAxis, {
    point: { x: 0.25, y: 0, z: 0 },
    direction: { x: 0, y: 0, z: 1 },
  })
})

test('successful result provenance is bound to the exact authentic context', () => {
  const fixture = correctionFixture()
  const result = deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
    fixture.context,
    fixture.binding,
    CLEARANCE,
    MAXIMUM_TRANSLATION,
    MAXIMUM_ANGLE_DELTA_DEGREES,
  )
  assert.ok(result)
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
      fixture.context,
      result,
    ),
    true,
  )

  const equivalentContext = prepareFoldPreviewTreeMotionContext({
    model: stationaryBranchCollisionModel(),
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    appliedAngles: [
      { edgeId: 'selected', angleDegrees: 0 },
      { edgeId: 'frozen', angleDegrees: 90 },
    ],
    collisionThickness: COLLISION_THICKNESS,
    visualThickness: COLLISION_THICKNESS,
  })
  assert.ok(equivalentContext)
  assert.notStrictEqual(equivalentContext, fixture.context)
  assert.equal(equivalentContext.contextKey, fixture.context.contextKey)
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
      equivalentContext,
      result,
    ),
    false,
  )

  for (const clone of [
    { ...result },
    Object.freeze({ ...result }),
    Object.create(result) as unknown,
  ]) {
    assert.equal(
      isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
        fixture.context,
        clone,
      ),
      false,
    )
  }

  let hostileValueReadCount = 0
  const hostileValue = new Proxy({}, {
    get() {
      hostileValueReadCount += 1
      throw new Error('value getter')
    },
  })
  const revokedValue = Proxy.revocable({}, {})
  revokedValue.revoke()
  for (const value of [
    null,
    undefined,
    false,
    0,
    '',
    Symbol('value'),
    () => undefined,
    hostileValue,
    revokedValue.proxy,
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(
        isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
          fixture.context,
          value,
        ),
        false,
      )
    })
  }
  assert.equal(hostileValueReadCount, 0)

  let hostileContextReadCount = 0
  const hostileContext = new Proxy(fixture.context, {
    get() {
      hostileContextReadCount += 1
      throw new Error('context getter')
    },
  })
  const invalidContexts: unknown[] = [
    null,
    undefined,
    false,
    0,
    '',
    hostileContext,
  ]
  for (const context of invalidContexts) {
    assert.doesNotThrow(() => {
      assert.equal(
        isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
          context as FoldPreviewTreeMotionContext,
          result,
        ),
        false,
      )
    })
  }
  assert.equal(hostileContextReadCount, 0)
})

test('a rerooted negative rotation sign yields the matching static-clear correction', () => {
  const fixture = correctionFixture({
    fixedFaceId: 'moving',
    startSelectedAngleDegrees: 30,
    frozenAngleDegrees: 135,
    targetSelectedAngleDegrees: 90,
  })
  const selectedJoint = fixture.context.tree.joints.find(
    (joint) => joint.hinge.edgeId === 'selected',
  )
  assert.equal(selectedJoint?.childRotationSign, -1)
  assert.deepEqual(fixture.binding.partition.stationaryFaceIds, ['moving'])
  assert.deepEqual(
    fixture.binding.partition.movingFaceIds,
    ['root', 'obstacle'],
  )

  const result = deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
    fixture.context,
    fixture.binding,
    CLEARANCE,
    MAXIMUM_TRANSLATION,
    MAXIMUM_ANGLE_DELTA_DEGREES,
  )
  assert.ok(result)
  assert.equal(result.rotationFit.childRotationSign, -1)
  assert.ok(result.candidates.length > 0)
  const candidate = result.candidates[0]
  assert.ok(candidate)
  assert.ok(
    candidate.pose.selectedAngleDegrees
      < result.sourceIdentity.blockingSelectedAngleDegrees,
  )
  assert.ok(candidate.fit.signedDeltaDegrees < 0)
  assert.ok(candidate.fit.signedRotationRadians > 0)
  assert.equal(
    candidate.pose.appliedAngles.find(
      (angle) => angle.edgeId === 'frozen',
    )?.angleDegrees,
    135,
  )
  assert.equal(candidate.safety.wholeSceneStaticClear, true)
  assert.equal(candidate.safety.continuousCandidatePathCertified, false)
})

test('an under-sized translation remains colliding and returns no candidate', () => {
  const fixture = correctionFixture()
  assert.equal(
    deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
      fixture.context,
      fixture.binding,
      1e-9,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    ),
    null,
  )
})

test('stale context and forged identity, partition, or pose keys fail closed', () => {
  const fixture = correctionFixture()
  const staleContext = prepareFoldPreviewTreeMotionContext({
    model: stationaryBranchCollisionModel(),
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'selected',
    appliedAngles: [
      { edgeId: 'selected', angleDegrees: 1 },
      { edgeId: 'frozen', angleDegrees: 90 },
    ],
    collisionThickness: COLLISION_THICKNESS,
    visualThickness: COLLISION_THICKNESS,
  })
  assert.ok(staleContext)
  assert.equal(staleContext.contextKey, fixture.context.contextKey)

  const forgedContext = {
    ...fixture.context,
  } as FoldPreviewTreeMotionContext
  const forgedBindings = [
    {
      ...fixture.binding,
      identity: {
        ...fixture.binding.identity,
        projectId: `${fixture.binding.identity.projectId}-forged`,
      },
    },
    {
      ...fixture.binding,
      identity: {
        ...fixture.binding.identity,
        request: {
          ...fixture.binding.identity.request,
          contextKey: `${fixture.context.contextKey}:forged`,
        },
      },
    },
    {
      ...fixture.binding,
      identity: {
        ...fixture.binding.identity,
        request: {
          ...fixture.binding.identity.request,
          sourcePoseRequestKey: 'forged-source-pose-key',
        },
      },
    },
    {
      ...fixture.binding,
      identity: {
        ...fixture.binding.identity,
        blockingPoseRequestKey: 'forged-blocking-pose-key',
      },
    },
    {
      ...fixture.binding,
      partition: {
        ...fixture.binding.partition,
        movingFaceIds: [
          ...fixture.binding.partition.movingFaceIds,
          fixture.context.fixedFaceId,
        ],
      },
    },
    {
      ...fixture.binding,
      partition: {
        ...fixture.binding.partition,
        stationaryFaceIds: ['root'],
      },
    },
  ] as FoldPreviewTreeTerminalFullScanBinding[]

  for (const [context, binding] of [
    [staleContext, fixture.binding],
    [forgedContext, fixture.binding],
    ...forgedBindings.map((binding) => [fixture.context, binding]),
  ] as const) {
    assert.equal(
      deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
        context,
        binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      ),
      null,
    )
  }
})

test('numeric work and angle bounds reject invalid or insufficient requests', () => {
  const fixture = correctionFixture()
  const invalidArguments = [
    [0, MAXIMUM_TRANSLATION, MAXIMUM_ANGLE_DELTA_DEGREES],
    [-1, MAXIMUM_TRANSLATION, MAXIMUM_ANGLE_DELTA_DEGREES],
    [Number.NaN, MAXIMUM_TRANSLATION, MAXIMUM_ANGLE_DELTA_DEGREES],
    [CLEARANCE, 0, MAXIMUM_ANGLE_DELTA_DEGREES],
    [CLEARANCE, 0.001, MAXIMUM_ANGLE_DELTA_DEGREES],
    [CLEARANCE, Number.POSITIVE_INFINITY, MAXIMUM_ANGLE_DELTA_DEGREES],
    [CLEARANCE, MAXIMUM_TRANSLATION, 0],
    [CLEARANCE, MAXIMUM_TRANSLATION, 181],
    [CLEARANCE, MAXIMUM_TRANSLATION, Number.NaN],
  ] as const
  for (const [
    clearance,
    maximumTranslation,
    maximumAngleDeltaDegrees,
  ] of invalidArguments) {
    assert.equal(
      deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
        fixture.context,
        fixture.binding,
        clearance,
        maximumTranslation,
        maximumAngleDeltaDegrees,
      ),
      null,
    )
  }
})

test('output is deterministic, detached, deeply frozen, and never applied', () => {
  const fixture = correctionFixture()
  const first = deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
    fixture.context,
    fixture.binding,
    CLEARANCE,
    MAXIMUM_TRANSLATION,
    MAXIMUM_ANGLE_DELTA_DEGREES,
  )
  const second = deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
    fixture.context,
    fixture.binding,
    CLEARANCE,
    MAXIMUM_TRANSLATION,
    MAXIMUM_ANGLE_DELTA_DEGREES,
  )
  assert.ok(first && second)
  assert.deepEqual(second, first)
  assert.notStrictEqual(second, first)
  assert.notStrictEqual(first.sourcePartition, fixture.binding.partition)
  assert.notStrictEqual(
    first.sourcePartition.stationaryFaceIds,
    fixture.binding.partition.stationaryFaceIds,
  )
  assert.notStrictEqual(
    first.candidates[0]?.pose.appliedAngles,
    fixture.context.appliedAngles,
  )
  assert.equal(first.safety.continuousCandidatePathCertified, false)
  assert.equal(first.safety.sceneApplied, false)
  assert.equal(first.safety.autoApplicable, false)
  for (const candidate of first.candidates) {
    assert.equal(candidate.safety.continuousCandidatePathCertified, false)
    assert.equal(candidate.safety.sceneApplied, false)
    assert.equal(candidate.safety.autoApplicable, false)
  }
  assertDeeplyFrozen(first)
  assertDeeplyFrozen(second)
})

test('throwing and revoked public inputs return null without escaping errors', () => {
  const fixture = correctionFixture()
  const throwingContext = new Proxy(fixture.context, {
    get() {
      throw new Error('context getter')
    },
  })
  const throwingBinding = new Proxy(fixture.binding, {
    get() {
      throw new Error('binding getter')
    },
  })
  const revokedContext = Proxy.revocable(fixture.context, {})
  const revokedBinding = Proxy.revocable(fixture.binding, {})
  revokedContext.revoke()
  revokedBinding.revoke()

  for (const [context, binding] of [
    [throwingContext, fixture.binding],
    [fixture.context, throwingBinding],
    [revokedContext.proxy, fixture.binding],
    [fixture.context, revokedBinding.proxy],
  ] as const) {
    let result: unknown
    assert.doesNotThrow(() => {
      result = deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
        context,
        binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      )
    })
    assert.equal(result, null)
  }
})

test('resumable jobs preserve synchronous output for tiny and large chunks', () => {
  const fixture = correctionFixture()
  const synchronous =
    deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
      fixture.context,
      fixture.binding,
      CLEARANCE,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(synchronous)

  for (const workBudget of [1, 2, 17, Number.MAX_SAFE_INTEGER]) {
    const job =
      createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
        fixture.context,
        fixture.binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      )
    assert.ok(job)
    assert.deepEqual(job.workBounds, {
      entireStepTimeBounded: false,
      synchronousFactoryPreparation: true,
      synchronousCandidatePosePreparation: true,
      synchronousChildJobPreparation: true,
      synchronousHingePolicyFinalization: true,
      synchronousResultFinalization: true,
      candidateSeedCount: 1,
      maximumTrianglePairTests:
        MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS,
      plannedTrianglePairVisitUpperBound: 24,
      maximumWitnessDerivations: 32,
      maximumTotalWorkUnits: 56,
    })
    const pendingSteps:
      FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep[] = []
    const terminal = runStaticCorrectionJob(
      job,
      workBudget,
      pendingSteps,
    )
    assert.equal(terminal.kind, 'complete')
    assert.equal(
      terminal.version,
      FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_JOB_VERSION,
    )
    assert.deepEqual(terminal.result, synchronous)
    assert.notStrictEqual(terminal.result, synchronous)
    assert.deepEqual(terminal.work, {
      totalWorkUnits: 8,
      trianglePairTests: 8,
      witnessDerivations: 0,
    })
    assert.strictEqual(terminal.workBounds, job.workBounds)
    assert.equal(
      isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
        fixture.context,
        terminal.result,
      ),
      true,
    )
    for (const pending of pendingSteps) {
      assert.equal(pending.kind, 'pending')
      assert.equal('result' in pending, false)
      assert.equal('candidates' in pending, false)
      assertDeeplyFrozen(pending)
    }
    assert.strictEqual(job.step(1), terminal)
    job.cancel()
    assert.strictEqual(job.step(2), terminal)
    assertDeeplyFrozen(terminal)
  }
})

test('full scan completes before narrow scan and every phase leaves a cancellation window', () => {
  const fixture = correctionFixture()
  const job =
    createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
      fixture.context,
      fixture.binding,
      CLEARANCE,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(job)

  const fullPrepared = job.step(Number.MAX_SAFE_INTEGER)
  assert.equal(fullPrepared.kind, 'pending')
  assert.equal(fullPrepared.phase, 'full_scan')
  assert.deepEqual(fullPrepared.work, {
    totalWorkUnits: 0,
    trianglePairTests: 0,
    witnessDerivations: 0,
  })
  const fullComplete = job.step(Number.MAX_SAFE_INTEGER)
  assert.equal(fullComplete.kind, 'pending')
  assert.equal(fullComplete.phase, 'narrow_scan_preparation')
  assert.deepEqual(fullComplete.work, fullPrepared.work)
  const narrowPrepared = job.step(Number.MAX_SAFE_INTEGER)
  assert.equal(narrowPrepared.kind, 'pending')
  assert.equal(narrowPrepared.phase, 'narrow_scan')
  assert.deepEqual(narrowPrepared.work, fullComplete.work)
  const terminal = job.step(Number.MAX_SAFE_INTEGER)
  assert.equal(terminal.kind, 'complete')
  assert.equal(terminal.result.staticValidationWork.fullScanCount, 1)
  assert.equal(terminal.result.staticValidationWork.narrowScanCount, 1)
})

test('multiple seeds retain source order and separate every child phase', () => {
  const fixture = correctionFixture()
  const job =
    createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
      fixture.context,
      fixture.binding,
      0.15,
      2,
      30,
    )
  assert.ok(job)
  assert.equal(job.workBounds.candidateSeedCount, 2)

  const pending:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep[] = []
  const terminal = runStaticCorrectionJob(
    job,
    Number.MAX_SAFE_INTEGER,
    pending,
  )
  assert.equal(terminal.kind, 'complete')
  assert.deepEqual(
    pending.map((step) => step.kind === 'pending' ? step.phase : null),
    [
      'full_scan',
      'narrow_scan_preparation',
      'narrow_scan',
      'full_scan_preparation',
      'full_scan',
      'narrow_scan_preparation',
      'narrow_scan',
    ],
  )
  assert.deepEqual(
    terminal.result.candidates.map((candidate) => ({
      rank: candidate.rank,
      sourceSeedRank: candidate.sourceSeedRank,
    })),
    [
      { rank: 1, sourceSeedRank: 1 },
      { rank: 2, sourceSeedRank: 2 },
    ],
  )
  assert.deepEqual(terminal.result.staticValidationWork, {
    strategy: 'full_non_adjacent_then_hinge_policy_v1',
    maximumTrianglePairVisits:
      MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS,
    plannedTrianglePairVisitUpperBound: 48,
    actualTrianglePairVisits: 16,
    fullScanCount: 2,
    narrowScanCount: 2,
  })
  assert.deepEqual(
    terminal.result,
    deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
      fixture.context,
      fixture.binding,
      0.15,
      2,
      30,
    ),
  )
})

test('a colliding full scan skips its narrow child and exposes no partial candidate', () => {
  const fixture = correctionFixture()
  const job =
    createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
      fixture.context,
      fixture.binding,
      1e-9,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(job)
  const pending:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep[] = []
  const terminal = runStaticCorrectionJob(job, 1, pending)
  assert.equal(terminal.kind, 'exhausted')
  assert.ok(pending.length > 0)
  assert.ok(pending.every((step) =>
    step.kind === 'pending'
    && step.phase !== 'narrow_scan_preparation'
    && step.phase !== 'narrow_scan'))
  assert.equal('result' in terminal, false)
  assert.equal('candidates' in terminal, false)
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
      fixture.context,
      terminal,
    ),
    false,
  )
  assert.strictEqual(job.step(17), terminal)
  assertDeeplyFrozen(terminal)
})

test('cancellation is stable before and during every outer phase', () => {
  const fixture = correctionFixture()
  for (const advanceCount of [0, 1, 2, 3]) {
    const job =
      createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
        fixture.context,
        fixture.binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      )
    assert.ok(job)
    for (let index = 0; index < advanceCount; index += 1) {
      const step = job.step(Number.MAX_SAFE_INTEGER)
      assert.equal(step.kind, 'pending')
    }
    job.cancel()
    const terminal = job.step(1)
    assert.equal(terminal.kind, 'cancelled')
    assert.equal('result' in terminal, false)
    assert.equal('candidates' in terminal, false)
    assert.equal(
      isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
        fixture.context,
        terminal,
      ),
      false,
    )
    assert.strictEqual(job.step(17), terminal)
    job.cancel()
    assert.strictEqual(job.step(2), terminal)
    assertDeeplyFrozen(terminal)
  }
})

test('invalid budgets are zero-work stable terminals', () => {
  const fixture = correctionFixture()
  for (const workBudget of [
    0,
    -1,
    0.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1,
  ]) {
    const job =
      createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
        fixture.context,
        fixture.binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
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

test('budget-validation reentry cancels before child work is charged', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job =
    createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
      fixture.context,
      fixture.binding,
      CLEARANCE,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')

  const originalIsSafeInteger = Number.isSafeInteger
  let nested:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep | null = null
  let reentered = false
  Number.isSafeInteger = function isSafeInteger(value: unknown) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalIsSafeInteger(value)
  }
  let outer: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep
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
})

test('charged child reentry is included before the parent cancellation publishes', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job =
    createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
      fixture.context,
      fixture.binding,
      CLEARANCE,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(job)
  assert.equal(job.step(17).kind, 'pending')
  assert.equal(job.step(17).kind, 'pending')
  const narrowPrepared = job.step(17)
  assert.equal(narrowPrepared.kind, 'pending')
  assert.equal(narrowPrepared.phase, 'narrow_scan')

  const originalDot = Vector3.prototype.dot
  let nested:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep | null = null
  let reentered = false
  Vector3.prototype.dot = function dot(vector: Vector3) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalDot.call(this, vector)
  }
  let outer: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep
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

test('child cancellation outranks a charged classifier throw', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job =
    createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
      fixture.context,
      fixture.binding,
      CLEARANCE,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(job)
  assert.equal(job.step(17).kind, 'pending')
  assert.equal(job.step(17).kind, 'pending')
  assert.equal(job.step(17).kind, 'pending')

  const originalDot = Vector3.prototype.dot
  Vector3.prototype.dot = function dot() {
    job.cancel()
    throw new Error('charged child cancellation')
  }
  let terminal:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep
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
})

test('result finalization reentry cannot publish or authorize partial success', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job =
    createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
      fixture.context,
      fixture.binding,
      CLEARANCE,
      MAXIMUM_TRANSLATION,
      MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(job)
  assert.equal(job.step(Number.MAX_SAFE_INTEGER).kind, 'pending')
  assert.equal(job.step(Number.MAX_SAFE_INTEGER).kind, 'pending')
  assert.equal(job.step(Number.MAX_SAFE_INTEGER).kind, 'pending')

  const originalFreeze = Object.freeze
  let nested:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep | null = null
  let unpublishedResult: unknown = null
  let reentered = false
  Object.freeze = ((value: object) => {
    const record = value as Record<PropertyKey, unknown>
    if (
      !reentered
      && record.kind
        === 'statically_revalidated_single_hinge_correction_candidates'
    ) {
      reentered = true
      unpublishedResult = value
      nested = job.step(1)
    }
    return originalFreeze(value)
  }) as typeof Object.freeze
  let outer: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep
  try {
    outer = job.step(Number.MAX_SAFE_INTEGER)
  } finally {
    Object.freeze = originalFreeze
  }

  assert.equal(reentered, true)
  assert.ok(nested)
  assert.equal(nested.kind, 'cancelled')
  assert.strictEqual(outer, nested)
  assert.equal('result' in outer, false)
  assert.ok(unpublishedResult)
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCorrectionCandidatesBoundToContext(
      fixture.context,
      unpublishedResult,
    ),
    false,
  )
  assertDeeplyFrozen(unpublishedResult)
  assert.strictEqual(job.step(1), outer)
})

test('job and synchronous boundaries reject every non-authentic context or binding', () => {
  const fixture = correctionFixture()
  const otherFixture = correctionFixture()
  let hostileContextReadCount = 0
  const hostileContext = new Proxy(fixture.context, {
    get() {
      hostileContextReadCount += 1
      throw new Error('context getter')
    },
  })
  const forgedContexts = [
    { ...fixture.context },
    Object.create(fixture.context),
    hostileContext,
  ] as FoldPreviewTreeMotionContext[]
  const forgedBindings = [
    { ...fixture.binding },
    Object.freeze({ ...fixture.binding }),
    Object.create(fixture.binding),
    otherFixture.binding,
  ] as FoldPreviewTreeTerminalFullScanBinding[]

  for (const context of forgedContexts) {
    assert.equal(
      createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
        context,
        fixture.binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      ),
      null,
    )
    assert.equal(
      deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
        context,
        fixture.binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      ),
      null,
    )
  }
  assert.equal(hostileContextReadCount, 0)
  for (const binding of forgedBindings) {
    assert.equal(
      createFoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob(
        fixture.context,
        binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      ),
      null,
    )
    assert.equal(
      deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
        fixture.context,
        binding,
        CLEARANCE,
        MAXIMUM_TRANSLATION,
        MAXIMUM_ANGLE_DELTA_DEGREES,
      ),
      null,
    )
  }
})

function runStaticCorrectionJob(
  job: FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJob,
  workBudget: number,
  pendingSteps:
    FoldPreviewTreeSingleHingeStaticCorrectionCandidatesJobStep[] = [],
) {
  for (let index = 0; index < 10_000; index += 1) {
    const step = job.step(workBudget)
    if (step.kind !== 'pending') return step
    pendingSteps.push(step)
  }
  throw new Error('static correction candidates job did not terminate')
}

function correctionFixture({
  fixedFaceId = 'root',
  startSelectedAngleDegrees = 0,
  frozenAngleDegrees = 90,
  targetSelectedAngleDegrees = 120,
}: Readonly<{
  fixedFaceId?: 'root' | 'moving' | 'obstacle'
  startSelectedAngleDegrees?: number
  frozenAngleDegrees?: number
  targetSelectedAngleDegrees?: number
}> = {}) {
  const model = stationaryBranchCollisionModel()
  const startAngles = [
    {
      edgeId: 'selected',
      angleDegrees: startSelectedAngleDegrees,
    },
    { edgeId: 'frozen', angleDegrees: frozenAngleDegrees },
  ] as const
  const context = prepareFoldPreviewTreeMotionContext({
    model,
    fixedFaceId,
    selectedHingeEdgeId: 'selected',
    appliedAngles: startAngles,
    collisionThickness: COLLISION_THICKNESS,
    visualThickness: COLLISION_THICKNESS,
  })
  assert.ok(context)
  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    context.model,
    context.fixedFaceId,
    context.collisionThickness,
    context.appliedAngles,
  )
  assert.ok(sourcePoseRequestKey)
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    context.model,
    context.fixedFaceId,
    context.selectedHingeEdgeId,
  )
  assert.ok(analyzer)
  const job = analyzer.createJob(
    context.appliedAngles,
    targetSelectedAngleDegrees,
    context.collisionThickness,
    {
      maxDepth: 18,
      minTimeSpan: 2 ** -22,
      maxIntervalTests: 10_000,
      requestIdentity: {
        contextKey: context.contextKey,
        sourcePoseRequestKey,
        generation: 7,
        requestSequence: 11,
      },
    },
  )
  assert.ok(job)
  const result = run(job)
  assert.equal(result.kind, 'blocked', JSON.stringify(result))
  const binding = result.kind === 'blocked'
    ? result.blocker?.blockingSample?.terminalFullScanBinding
    : null
  assert.ok(binding)
  return {
    model,
    context,
    sourcePoseRequestKey,
    binding,
  }
}

function run(
  job: NonNullable<
    ReturnType<FoldPreviewTreeSingleHingeContinuousAnalyzer['createJob']>
  >,
) {
  for (let index = 0; index < 1_000; index += 1) {
    const result = job.step(32)
    if (result.kind !== 'pending') return result
  }
  throw new Error('tree single-hinge continuous job did not terminate')
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

function stationaryBranchCollisionModel(): FoldGraphPreviewModel {
  const movingAxisStart = { vertexId: 'ma', x: 0.25, z: 0 }
  const movingAxisEnd = { vertexId: 'mb', x: 0.25, z: 1 }
  const obstacleAxisStart = { vertexId: 'oa', x: 0, z: 0 }
  const obstacleAxisEnd = { vertexId: 'ob', x: 0, z: 1 }
  const root: FoldPreviewFaceModel = {
    id: 'root',
    polygon: [
      movingAxisStart,
      obstacleAxisStart,
      obstacleAxisEnd,
      movingAxisEnd,
    ],
  }
  const moving: FoldPreviewFaceModel = {
    id: 'moving',
    polygon: [
      movingAxisEnd,
      { vertexId: 'moving-top-right', x: 0.75, z: 1 },
      { vertexId: 'moving-bottom-right', x: 0.75, z: 0 },
      movingAxisStart,
    ],
  }
  const obstacle: FoldPreviewFaceModel = {
    id: 'obstacle',
    polygon: [
      obstacleAxisStart,
      { vertexId: 'obstacle-bottom-left', x: -0.5, z: 0 },
      { vertexId: 'obstacle-top-left', x: -0.5, z: 1 },
      obstacleAxisEnd,
    ],
  }
  const selected: FoldPreviewHingeModel = {
    edgeId: 'selected',
    leftFaceId: 'root',
    rightFaceId: 'moving',
    start: movingAxisStart,
    end: movingAxisEnd,
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const frozen: FoldPreviewHingeModel = {
    edgeId: 'frozen',
    leftFaceId: 'root',
    rightFaceId: 'obstacle',
    start: obstacleAxisStart,
    end: obstacleAxisEnd,
    axis: { x: 0, z: 1 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'stationary-branch-project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0.125, y: 0.5 },
    worldBounds: { minX: -0.5, minZ: 0, maxX: 0.75, maxZ: 1 },
    faces: [root, moving, obstacle],
    hinges: [selected, frozen],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'moving',
          hinge: selected,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'root',
          childFaceId: 'obstacle',
          hinge: frozen,
          childRotationSign: -1,
        },
      ],
    },
  }
}
