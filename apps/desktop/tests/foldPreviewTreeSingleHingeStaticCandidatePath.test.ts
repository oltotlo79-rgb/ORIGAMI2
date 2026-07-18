import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createFoldPreviewTreeSingleHingeStaticCandidatePathJob,
  FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
  isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext,
  type FoldPreviewTreeSingleHingeStaticCandidatePathJob,
  type FoldPreviewTreeSingleHingeStaticCandidatePathOptions,
  type FoldPreviewTreeSingleHingeStaticCandidatePathStep,
} from '../src/lib/foldPreviewTreeSingleHingeStaticCandidatePath.ts'
import {
  deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates,
} from '../src/lib/foldPreviewTreeSingleHingeStaticCorrectionCandidates.ts'
import {
  prepareFoldPreviewTreeSingleHingeContinuousCollision,
  type FoldPreviewTreeSingleHingeContinuousAnalyzer,
} from '../src/lib/foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  prepareFoldPreviewTreeMotionContext,
  replaceFoldPreviewTreeMotionSelectedAngle,
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

test('the source vector reaches the ordinary static candidate with bound identities', () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  const result = runPath(job)
  assert.equal(result.kind, 'certified')
  if (result.kind !== 'certified') return

  const certificate = result.certificate
  const staticCandidate = fixture.staticCandidates.candidates[0]
  assert.ok(staticCandidate)
  assert.equal(
    certificate.version,
    FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CANDIDATE_PATH_VERSION,
  )
  assert.equal(
    certificate.kind,
    'continuously_certified_static_candidate',
  )
  assert.equal(certificate.sourceIdentity.projectId, 'stationary-branch-project')
  assert.equal(certificate.sourceIdentity.revision, 1)
  assert.equal(certificate.sourceIdentity.fixedFaceId, 'root')
  assert.equal(certificate.sourceIdentity.selectedHingeEdgeId, 'selected')
  assert.equal(
    certificate.sourceIdentity.contextKey,
    fixture.context.contextKey,
  )
  assert.equal(
    certificate.sourceIdentity.sourcePoseRequestKey,
    fixture.sourcePoseRequestKey,
  )
  assert.equal(
    certificate.sourceIdentity.blockingPoseRequestKey,
    fixture.binding.identity.blockingPoseRequestKey,
  )
  assert.equal(certificate.sourceIdentity.generation, 7)
  assert.equal(certificate.sourceIdentity.requestSequence, 11)
  assert.equal(
    certificate.sourceIdentity.blockingSampleTime,
    fixture.binding.blockingSampleTime,
  )
  assert.equal(certificate.sourceIdentity.sourceSelectedAngleDegrees, 0)
  assert.equal(
    certificate.sourceIdentity.blockingSelectedAngleDegrees,
    fixture.binding.selectedAngleDegrees,
  )
  assert.equal(
    certificate.sourceIdentity.collisionThickness,
    COLLISION_THICKNESS,
  )
  assert.deepEqual(certificate.sourceIdentity.sourcePartition, {
    version: 'rerooted_selected_hinge_partition_v1',
    stationaryFaceIds: ['root', 'obstacle'],
    movingFaceIds: ['moving'],
  })
  assert.deepEqual(certificate.selectedCandidate, {
    rank: staticCandidate.rank,
    sourceSeedRank: staticCandidate.sourceSeedRank,
    source: staticCandidate.source,
    poseRequestKey: staticCandidate.pose.poseRequestKey,
    selectedAngleDegrees: staticCandidate.pose.selectedAngleDegrees,
    appliedAngles: staticCandidate.pose.appliedAngles,
  })
  assert.deepEqual(certificate.path.sourceAngles, fixture.context.appliedAngles)
  assert.deepEqual(
    certificate.path.targetAngles,
    staticCandidate.pose.appliedAngles,
  )
  assert.notStrictEqual(
    certificate.path.sourceAngles,
    fixture.context.appliedAngles,
  )
  assert.notStrictEqual(
    certificate.path.targetAngles,
    staticCandidate.pose.appliedAngles,
  )
  assert.equal(
    certificate.path.sourceSelectedAngleDegrees,
    fixture.context.selectedAngleDegrees,
  )
  assert.equal(
    certificate.path.targetSelectedAngleDegrees,
    staticCandidate.pose.selectedAngleDegrees,
  )
  assert.equal(
    certificate.path.sourcePoseRequestKey,
    fixture.sourcePoseRequestKey,
  )
  assert.equal(
    certificate.path.targetPoseRequestKey,
    staticCandidate.pose.poseRequestKey,
  )
  assert.equal(certificate.path.certifiedSafeThrough, 1)
  assert.equal(certificate.path.stopTime, 1)
  assert.deepEqual(certificate.path.stats, {
    intervalTests: 11,
    pointTests: 7,
    pointCacheHits: 0,
    maximumDepthReached: 4,
  })
  assert.deepEqual(certificate.precedingAttempts, [])
  assert.deepEqual(certificate.aggregateStats, certificate.path.stats)
  assert.deepEqual(certificate.workBounds, {
    entireStepTimeBounded: false,
    synchronousFactoryPreparation: true,
    synchronousChildJobPreparation: true,
    synchronousResultFinalization: true,
    candidateCount: 1,
    maximumCumulativeIntervalTests: 2_048,
    maximumCumulativeIntervalPairVisits: 1_000_000,
    maximumCumulativePointTriangleTests: 1_000_000,
    terminalEvidenceFullScanEnabled: false,
  })
  assert.deepEqual(certificate.staticAnalysis, staticCandidate.staticAnalysis)
  assert.notStrictEqual(
    certificate.staticAnalysis,
    staticCandidate.staticAnalysis,
  )
  assert.deepEqual(certificate.safety, {
    modelIdentityBound: true,
    sourcePoseIdentityVerified: true,
    candidatePoseIdentityVerified: true,
    partitionRevalidated: true,
    completeLegalAngleVectorGenerated: true,
    legalCorrectionPoseGenerated: true,
    collisionConstraintsRevalidated: true,
    hingeContactPolicySatisfied: true,
    wholeSceneStaticClear: true,
    staticCandidateRevalidated: true,
    continuousCandidatePathCertified: true,
    runtimeRequestBound: false,
    startScenePoseMatched: false,
    sceneApplied: false,
    autoApplicable: false,
  })
})

test('a rerooted negative sign certifies its genuine source-to-candidate path', () => {
  const fixture = correctionFixture({
    fixedFaceId: 'moving',
    startSelectedAngleDegrees: 30,
    frozenAngleDegrees: 135,
    blockingTargetAngleDegrees: 90,
  })
  assert.equal(
    fixture.context.tree.joints.find(
      (joint) => joint.hinge.edgeId === 'selected',
    )?.childRotationSign,
    -1,
  )
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  const result = runPath(job)
  assert.equal(result.kind, 'certified')
  if (result.kind !== 'certified') return

  const certificate = result.certificate
  assert.equal(certificate.sourceIdentity.fixedFaceId, 'moving')
  assert.equal(certificate.path.sourceSelectedAngleDegrees, 30)
  assert.ok(certificate.path.targetSelectedAngleDegrees > 30)
  assert.ok(
    certificate.path.targetSelectedAngleDegrees
      < fixture.binding.selectedAngleDegrees,
  )
  assert.equal(
    certificate.path.targetAngles.find(
      (angle) => angle.edgeId === 'frozen',
    )?.angleDegrees,
    135,
  )
  assert.deepEqual(certificate.path.stats, {
    intervalTests: 3,
    pointTests: 3,
    pointCacheHits: 0,
    maximumDepthReached: 1,
  })
  assert.equal(certificate.safety.continuousCandidatePathCertified, true)
  assert.equal(certificate.safety.sceneApplied, false)
  assert.equal(certificate.safety.autoApplicable, false)
})

test('the certified job starts at the source pose instead of the blocking pose', () => {
  const fixture = correctionFixture()
  const candidate = fixture.staticCandidates.candidates[0]
  assert.ok(candidate)
  const blockingAngles = replaceFoldPreviewTreeMotionSelectedAngle(
    fixture.context,
    fixture.binding.selectedAngleDegrees,
  )
  assert.ok(blockingAngles)
  const analyzer = prepareFoldPreviewTreeSingleHingeContinuousCollision(
    fixture.context.model,
    fixture.context.fixedFaceId,
    fixture.context.selectedHingeEdgeId,
  )
  assert.ok(analyzer)
  const fromBlocking = analyzer.createJob(
    blockingAngles,
    candidate.pose.selectedAngleDegrees,
    fixture.context.collisionThickness,
  )
  assert.ok(fromBlocking)
  const blockingResult = runContinuous(fromBlocking)
  assert.equal(blockingResult.kind, 'blocked')
  if (blockingResult.kind === 'blocked') {
    assert.equal(blockingResult.certifiedSafeThrough, 0)
    assert.equal(blockingResult.blockingSampleTime, 0)
  }

  const pathJob = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(pathJob)
  const result = runPath(pathJob)
  assert.equal(result.kind, 'certified')
  if (result.kind !== 'certified') return
  assert.equal(result.certificate.path.sourceSelectedAngleDegrees, 0)
  assert.notEqual(
    result.certificate.path.sourceSelectedAngleDegrees,
    fixture.binding.selectedAngleDegrees,
  )
  assert.deepEqual(
    result.certificate.path.sourceAngles,
    fixture.context.appliedAngles,
  )
})

test('step one is incremental and a terminal result retains one reference', () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  const first = job.step(1)
  assert.equal(first.kind, 'pending')
  if (first.kind === 'pending') {
    assert.equal(first.phase, 'candidate_analysis')
    assert.equal(first.activeCandidate.rank, 1)
    assert.equal(first.certifiedSafeThrough, 0)
    assert.equal(first.aggregateStats.intervalTests, 0)
    assert.equal(first.workBounds.candidateCount, 1)
    assert.strictEqual(first.workBounds, job.workBounds)
    assert.deepEqual(first.completedAttempts, [])
  }

  const second = job.step(1)
  assert.equal(second.kind, 'pending')
  if (second.kind === 'pending') {
    assert.equal(second.phase, 'candidate_analysis')
    assert.equal(second.aggregateStats.intervalTests, 1)
  }

  let terminal = second
  for (let index = 0; index < 100 && terminal.kind === 'pending'; index += 1) {
    terminal = job.step(1)
  }
  assert.equal(terminal.kind, 'certified')
  assert.strictEqual(job.step(1), terminal)
  assert.doesNotThrow(() => job.cancel())
  assert.strictEqual(job.step(32), terminal)
})

test('chunk sizes preserve the same certified path and aggregate work', () => {
  const fixture = correctionFixture()
  const expectedJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
    )
  assert.ok(expectedJob)
  const expected = runPath(expectedJob, Number.MAX_SAFE_INTEGER)
  assert.equal(expected.kind, 'certified')

  for (const workBudget of [1, 2, 17, Number.MAX_SAFE_INTEGER]) {
    const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
    )
    assert.ok(job)
    const terminal = runPath(job, workBudget)
    assert.deepEqual(terminal, expected)
    assert.strictEqual(job.step(1), terminal)
  }
})

test('later candidates are prepared only after an explicit phase boundary', () => {
  const fixture = correctionFixture({
    correctionClearance: 0.15,
    maximumTranslation: 2,
  })
  assert.equal(fixture.staticCandidates.candidates.length, 2)
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
    { maxIntervalTests: 1 },
  )
  assert.ok(job)
  assert.equal(job.workBounds.candidateCount, 2)

  const firstPrepared = job.step(1)
  assert.equal(firstPrepared.kind, 'pending')
  assert.equal(firstPrepared.phase, 'candidate_analysis')
  assert.equal(firstPrepared.activeCandidate.rank, 1)
  assert.equal(firstPrepared.aggregateStats.intervalTests, 0)

  const firstProgress = job.step(1)
  assert.equal(firstProgress.kind, 'pending')
  assert.equal(firstProgress.phase, 'candidate_analysis')
  assert.equal(firstProgress.activeCandidate.rank, 1)
  assert.equal(firstProgress.aggregateStats.intervalTests, 1)

  const secondBoundary = job.step(1)
  assert.equal(secondBoundary.kind, 'pending')
  assert.equal(secondBoundary.phase, 'candidate_preparation')
  assert.equal(secondBoundary.activeCandidate.rank, 2)
  assert.equal(secondBoundary.completedAttempts.length, 1)
  assert.equal(secondBoundary.aggregateStats.intervalTests, 1)

  const secondPrepared = job.step(1)
  assert.equal(secondPrepared.kind, 'pending')
  assert.equal(secondPrepared.phase, 'candidate_analysis')
  assert.equal(secondPrepared.activeCandidate.rank, 2)
  assert.equal(secondPrepared.aggregateStats.intervalTests, 1)

  const secondProgress = job.step(1)
  assert.equal(secondProgress.kind, 'pending')
  assert.equal(secondProgress.phase, 'candidate_analysis')
  assert.equal(secondProgress.activeCandidate.rank, 2)
  assert.equal(secondProgress.aggregateStats.intervalTests, 2)

  const exhausted = job.step(1)
  assert.equal(exhausted.kind, 'exhausted')
  assert.equal(exhausted.attempts.length, 2)
  assert.deepEqual(
    exhausted.attempts.map((attempt) => attempt.candidate.rank),
    [1, 2],
  )
  assert.equal(exhausted.aggregateStats.intervalTests, 2)
  assert.strictEqual(job.step(17), exhausted)

  const cancelledJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
      { maxIntervalTests: 1 },
    )
  assert.ok(cancelledJob)
  assert.equal(cancelledJob.step(1).kind, 'pending')
  assert.equal(cancelledJob.step(1).kind, 'pending')
  const boundary = cancelledJob.step(1)
  assert.equal(boundary.kind, 'pending')
  assert.equal(boundary.phase, 'candidate_preparation')
  cancelledJob.cancel()
  const cancelled = cancelledJob.step(1)
  assert.equal(cancelled.kind, 'cancelled')
  assert.equal(cancelled.completedAttempts.length, 1)
  assert.equal(cancelled.aggregateStats.intervalTests, 1)
})

test('candidate rollover ignores later Object.freeze replacement', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture({
    correctionClearance: 0.15,
    maximumTranslation: 2,
  })
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
    { maxIntervalTests: 1 },
  )
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')
  assert.equal(job.step(1).kind, 'pending')

  const originalFreeze = Object.freeze
  let completedAttemptSeen = false
  let zeroStatsFinalizationCalls = 0
  let nested: FoldPreviewTreeSingleHingeStaticCandidatePathStep | null = null
  Object.freeze = ((value: object) => {
    const record = value as Record<PropertyKey, unknown>
    if (
      record.kind === 'indeterminate'
      && record.candidate !== undefined
    ) {
      completedAttemptSeen = true
      nested ??= job.step(1)
      return value
    } else if (
      completedAttemptSeen
      && record.intervalTests === 0
      && record.pointTests === 0
      && record.pointCacheHits === 0
      && record.maximumDepthReached === 0
    ) {
      zeroStatsFinalizationCalls += 1
      nested ??= job.step(1)
    }
    return originalFreeze(value)
  }) as typeof Object.freeze
  let boundary: FoldPreviewTreeSingleHingeStaticCandidatePathStep
  try {
    boundary = job.step(1)
  } finally {
    Object.freeze = originalFreeze
  }

  assert.equal(completedAttemptSeen, false)
  assert.equal(zeroStatsFinalizationCalls, 0)
  assert.equal(nested, null)
  assert.equal(boundary.kind, 'pending')
  if (boundary.kind === 'pending') {
    assert.equal(boundary.phase, 'candidate_preparation')
    assert.equal(boundary.completedAttempts.length, 1)
    assert.equal(boundary.aggregateStats.intervalTests, 1)
  }
  assertDeeplyFrozen(boundary)
})

test('a tight interval cap exhausts the genuine candidate without partial success', () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
    { maxIntervalTests: 1 },
  )
  assert.ok(job)
  const result = runPath(job)
  assert.equal(result.kind, 'exhausted')
  if (result.kind !== 'exhausted') return
  assert.equal(result.attempts.length, 1)
  assert.equal(result.attempts[0]?.kind, 'indeterminate')
  assert.equal(result.attempts[0]?.candidate.rank, 1)
  assert.equal(result.aggregateStats.intervalTests, 1)
  assert.deepEqual(result.safety, {
    continuousCandidatePathCertified: false,
    sceneApplied: false,
    autoApplicable: false,
  })
  assert.strictEqual(job.step(1), result)
})

test('provenance clones, replacement contexts, hostile inputs, and invalid options are rejected', () => {
  const fixture = correctionFixture()
  const clone = Object.freeze({ ...fixture.staticCandidates })
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      clone,
    ),
    null,
  )
  const equivalentContext = prepareContext()
  assert.notStrictEqual(equivalentContext, fixture.context)
  assert.equal(equivalentContext.contextKey, fixture.context.contextKey)
  assert.equal(
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      equivalentContext,
      fixture.staticCandidates,
    ),
    null,
  )

  const hostileContext = new Proxy(fixture.context, {
    get() {
      throw new Error('context getter')
    },
  })
  const hostileCandidates = new Proxy(fixture.staticCandidates, {
    get() {
      throw new Error('candidate getter')
    },
  })
  const hostileOptions = new Proxy({}, {
    get() {
      throw new Error('option getter')
    },
  })
  const revokedContext = Proxy.revocable(fixture.context, {})
  const revokedCandidates = Proxy.revocable(fixture.staticCandidates, {})
  const revokedOptions = Proxy.revocable({}, {})
  revokedContext.revoke()
  revokedCandidates.revoke()
  revokedOptions.revoke()
  for (const [context, candidates, options] of [
    [hostileContext, fixture.staticCandidates, {}],
    [fixture.context, hostileCandidates, {}],
    [revokedContext.proxy, fixture.staticCandidates, {}],
    [fixture.context, revokedCandidates.proxy, {}],
    [fixture.context, fixture.staticCandidates, hostileOptions],
    [fixture.context, fixture.staticCandidates, revokedOptions.proxy],
  ] as const) {
    let result: unknown
    assert.doesNotThrow(() => {
      result = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
        context,
        candidates,
        options,
      )
    })
    assert.equal(result, null)
  }

  const invalidOptions: unknown[] = [
    null,
    [],
    { maxDepth: -1 },
    { maxDepth: 53 },
    { maxDepth: 1.5 },
    { maxIntervalTests: 0 },
    { maxIntervalTests: 1_000_001 },
    { minTimeSpan: 0 },
    { minTimeSpan: 2 },
    { minTimeSpan: Number.NaN },
    { maxIntervalPairVisits: 0 },
    { maxPointTriangleTests: 0 },
  ]
  for (const options of invalidOptions) {
    assert.equal(
      createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
        fixture.context,
        fixture.staticCandidates,
        options as FoldPreviewTreeSingleHingeStaticCandidatePathOptions,
      ),
      null,
    )
  }
})

test('cancellation and invalid work budgets become permanent fail-closed terminals', () => {
  const fixture = correctionFixture()
  const cancelledJob = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(cancelledJob)
  assert.equal(cancelledJob.step(1).kind, 'pending')
  assert.equal(cancelledJob.step(1).kind, 'pending')
  cancelledJob.cancel()
  const cancelled = cancelledJob.step(1)
  assert.equal(cancelled.kind, 'cancelled')
  if (cancelled.kind === 'cancelled') {
    assert.ok(cancelled.aggregateStats.intervalTests > 0)
    assert.equal(
      cancelled.safety.continuousCandidatePathCertified,
      false,
    )
  }
  assert.strictEqual(cancelledJob.step(32), cancelled)

  for (const budget of [
    0,
    -1,
    1.5,
    Number.NaN,
    Number.POSITIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1,
  ]) {
    const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
    )
    assert.ok(job)
    const result = job.step(budget)
    assert.equal(result.kind, 'indeterminate')
    if (result.kind === 'indeterminate') {
      assert.equal(result.reason, 'invalid_work_budget')
      assert.deepEqual(result.completedAttempts, [])
      assert.deepEqual(result.aggregateStats, {
        intervalTests: 0,
        pointTests: 0,
        pointCacheHits: 0,
        maximumDepthReached: 0,
      })
      assert.equal(
        result.safety.continuousCandidatePathCertified,
        false,
      )
    }
    assert.strictEqual(job.step(1), result)
  }
})

test('budget-validation reentry cancels before interval work is charged', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')

  const originalIsSafeInteger = Number.isSafeInteger
  let nested: FoldPreviewTreeSingleHingeStaticCandidatePathStep | null = null
  let reentered = false
  Number.isSafeInteger = function isSafeInteger(value: unknown) {
    if (!reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalIsSafeInteger(value)
  }
  let outer: FoldPreviewTreeSingleHingeStaticCandidatePathStep
  try {
    outer = job.step(1)
  } finally {
    Number.isSafeInteger = originalIsSafeInteger
  }

  assert.equal(reentered, true)
  assert.ok(nested)
  assert.equal(nested.kind, 'cancelled')
  assert.strictEqual(outer, nested)
  assert.equal(outer.aggregateStats.intervalTests, 0)
  assert.strictEqual(job.step(1), outer)
})

test('child wrapper reentry cancels before interval work is charged', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')

  const originalFreeze = Object.freeze
  const originalHypot = Math.hypot
  let nested: FoldPreviewTreeSingleHingeStaticCandidatePathStep | null = null
  let initialPointWasSafe = false
  let reentered = false
  Object.freeze = ((value: object) => {
    if (
      (value as Record<PropertyKey, unknown>).kind === 'safe'
    ) {
      initialPointWasSafe = true
    }
    return originalFreeze(value)
  }) as typeof Object.freeze
  Math.hypot = (...values: number[]) => {
    if (initialPointWasSafe && !reentered) {
      reentered = true
      nested = job.step(1)
    }
    return originalHypot(...values)
  }
  let outer: FoldPreviewTreeSingleHingeStaticCandidatePathStep
  try {
    outer = job.step(1)
  } finally {
    Math.hypot = originalHypot
    Object.freeze = originalFreeze
  }

  assert.equal(reentered, true)
  assert.ok(nested)
  assert.equal(nested.kind, 'cancelled')
  assert.strictEqual(outer, nested)
  assert.equal(outer.aggregateStats.intervalTests, 0)
  assert.strictEqual(job.step(1), outer)
  assertDeeplyFrozen(outer)
})

test('child cancellation outranks a charged classifier throw', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')

  const originalFreeze = Object.freeze
  const originalHypot = Math.hypot
  let initialPointWasSafe = false
  Object.freeze = ((value: object) => {
    if (
      (value as Record<PropertyKey, unknown>).kind === 'safe'
    ) {
      initialPointWasSafe = true
    }
    return originalFreeze(value)
  }) as typeof Object.freeze
  Math.hypot = (...values: number[]) => {
    if (initialPointWasSafe) {
      job.cancel()
      throw new Error('charged path cancellation')
    }
    return originalHypot(...values)
  }
  let terminal: FoldPreviewTreeSingleHingeStaticCandidatePathStep
  try {
    terminal = job.step(1)
  } finally {
    Math.hypot = originalHypot
    Object.freeze = originalFreeze
  }

  assert.equal(terminal.kind, 'cancelled')
  assert.equal(terminal.aggregateStats.intervalTests, 1)
  assert.strictEqual(job.step(1), terminal)
  assertDeeplyFrozen(terminal)
})

test('pending finalization ignores later Object.freeze replacement', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)

  const originalFreeze = Object.freeze
  let nested: FoldPreviewTreeSingleHingeStaticCandidatePathStep | null = null
  let reentered = false
  Object.freeze = ((value: object) => {
    const record = value as Record<PropertyKey, unknown>
    if (
      !reentered
      && record.kind === 'pending'
      && record.phase === 'candidate_analysis'
    ) {
      reentered = true
      nested = job.step(1)
      return value
    }
    return originalFreeze(value)
  }) as typeof Object.freeze
  let outer: FoldPreviewTreeSingleHingeStaticCandidatePathStep
  try {
    outer = job.step(1)
  } finally {
    Object.freeze = originalFreeze
  }

  assert.equal(reentered, false)
  assert.equal(nested, null)
  assert.equal(outer.kind, 'pending')
  if (outer.kind === 'pending') {
    assert.equal(outer.phase, 'candidate_analysis')
    assert.equal(outer.aggregateStats.intervalTests, 0)
  }
  assertDeeplyFrozen(outer)
})

test('certificate finalization ignores later intrinsic replacement', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')

  const originalFreeze = Object.freeze
  const originalAdd = WeakSet.prototype.add
  const originalHas = WeakSet.prototype.has
  const originalApply = Reflect.apply
  const originalOwnKeys = Reflect.ownKeys
  const replacementCalls = {
    add: 0,
    has: 0,
    apply: 0,
    ownKeys: 0,
    freeze: 0,
  }
  WeakSet.prototype.add = function replacementAdd(
    this: WeakSet<object>,
  ) {
    replacementCalls.add += 1
    return this
  } as typeof WeakSet.prototype.add
  WeakSet.prototype.has = function replacementHas() {
    replacementCalls.has += 1
    return true
  } as typeof WeakSet.prototype.has
  Reflect.apply = (function replacementApply() {
    replacementCalls.apply += 1
    throw new Error('replacement Reflect.apply called')
  }) as typeof Reflect.apply
  Reflect.ownKeys = (function replacementOwnKeys() {
    replacementCalls.ownKeys += 1
    return []
  }) as typeof Reflect.ownKeys
  Object.freeze = ((value: object) => {
    const record = value as Record<PropertyKey, unknown>
    if (
      record.kind === 'continuously_certified_static_candidate'
    ) {
      replacementCalls.freeze += 1
      return value
    }
    return originalFreeze(value)
  }) as typeof Object.freeze
  let outer: FoldPreviewTreeSingleHingeStaticCandidatePathStep
  try {
    outer = job.step(Number.MAX_SAFE_INTEGER)
  } finally {
    Object.freeze = originalFreeze
    Reflect.ownKeys = originalOwnKeys
    Reflect.apply = originalApply
    WeakSet.prototype.add = originalAdd
    WeakSet.prototype.has = originalHas
  }

  assert.deepEqual(replacementCalls, {
    add: 0,
    has: 0,
    apply: 0,
    ownKeys: 0,
    freeze: 0,
  })
  assert.equal(outer.kind, 'certified')
  if (outer.kind !== 'certified') return
  assertDeeplyFrozen(outer.certificate)
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
      fixture.context,
      outer.certificate,
    ),
    true,
  )
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
      fixture.context,
      structuredClone(outer.certificate),
    ),
    false,
  )
  assert.strictEqual(job.step(1), outer)
})

test('certificate provenance ignores later WeakMap method replacement', {
  concurrency: false,
}, () => {
  const fixture = correctionFixture()
  const job = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(job)
  assert.equal(job.step(1).kind, 'pending')

  const originalSet = WeakMap.prototype.set
  let dynamicSetCalled = false
  let nested: FoldPreviewTreeSingleHingeStaticCandidatePathStep | null = null
  WeakMap.prototype.set = (function replacedSet() {
    dynamicSetCalled = true
    nested ??= job.step(1)
    throw new Error('replaced WeakMap.set')
  }) as typeof WeakMap.prototype.set
  let terminal: FoldPreviewTreeSingleHingeStaticCandidatePathStep
  try {
    terminal = job.step(Number.MAX_SAFE_INTEGER)
  } finally {
    WeakMap.prototype.set = originalSet
  }

  assert.equal(dynamicSetCalled, false)
  assert.equal(nested, null)
  assert.equal(terminal.kind, 'certified')
  if (terminal.kind !== 'certified') return

  const originalGet = WeakMap.prototype.get
  let dynamicGetCalled = false
  WeakMap.prototype.get = (function replacedGet() {
    dynamicGetCalled = true
    throw new Error('replaced WeakMap.get')
  }) as typeof WeakMap.prototype.get
  try {
    assert.equal(
      isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
        fixture.context,
        terminal.certificate,
      ),
      true,
    )
  } finally {
    WeakMap.prototype.get = originalGet
  }
  assert.equal(dynamicGetCalled, false)
  assert.strictEqual(job.step(1), terminal)
})

test('certificate provenance binds only the exact clear result and context', () => {
  const fixture = correctionFixture()
  const certifiedJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
    )
  assert.ok(certifiedJob)
  const result = runPath(certifiedJob)
  assert.equal(result.kind, 'certified')
  if (result.kind !== 'certified') return
  const certificate = result.certificate

  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
      fixture.context,
      certificate,
    ),
    true,
  )
  const outerClone = Object.freeze({ ...result })
  assert.notStrictEqual(outerClone, result)
  assert.strictEqual(outerClone.certificate, certificate)
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
      fixture.context,
      outerClone.certificate,
    ),
    true,
  )
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
      fixture.context,
      result,
    ),
    false,
  )

  const equivalentContext = prepareContext()
  assert.notStrictEqual(equivalentContext, fixture.context)
  assert.equal(equivalentContext.contextKey, fixture.context.contextKey)
  assert.equal(
    isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
      equivalentContext,
      certificate,
    ),
    false,
  )
  for (const clone of [
    { ...certificate },
    Object.freeze({ ...certificate }),
    Object.create(certificate) as unknown,
  ]) {
    assert.equal(
      isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
        fixture.context,
        clone,
      ),
      false,
    )
  }

  let hostileValueReadCount = 0
  const hostileValue = new Proxy(certificate, {
    get() {
      hostileValueReadCount += 1
      throw new Error('certificate getter')
    },
  })
  const revokedValue = Proxy.revocable(certificate, {})
  revokedValue.revoke()
  for (const value of [
    null,
    undefined,
    false,
    0,
    '',
    Symbol('certificate'),
    () => undefined,
    hostileValue,
    revokedValue.proxy,
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(
        isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
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
  const revokedContext = Proxy.revocable(fixture.context, {})
  revokedContext.revoke()
  for (const context of [
    null,
    undefined,
    false,
    0,
    '',
    hostileContext,
    revokedContext.proxy,
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(
        isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
          context as FoldPreviewTreeMotionContext,
          certificate,
        ),
        false,
      )
    })
  }
  assert.equal(hostileContextReadCount, 0)

  const pendingJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
    )
  const exhaustedJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
      { maxIntervalTests: 1 },
    )
  const cancelledJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
    )
  const indeterminateJob =
    createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
      fixture.context,
      fixture.staticCandidates,
    )
  assert.ok(
    pendingJob
      && exhaustedJob
      && cancelledJob
      && indeterminateJob,
  )
  const pending = pendingJob.step(1)
  const exhausted = runPath(exhaustedJob)
  cancelledJob.cancel()
  const cancelled = cancelledJob.step(1)
  const indeterminate = indeterminateJob.step(0)
  assert.equal(pending.kind, 'pending')
  assert.equal(exhausted.kind, 'exhausted')
  assert.equal(cancelled.kind, 'cancelled')
  assert.equal(indeterminate.kind, 'indeterminate')
  for (const nonClear of [
    pending,
    exhausted,
    cancelled,
    indeterminate,
  ]) {
    assert.equal(
      isFoldPreviewTreeSingleHingeStaticCandidatePathCertificateBoundToContext(
        fixture.context,
        nonClear,
      ),
      false,
    )
  }
})

test('certificates are deterministic, detached, deeply frozen, and never apply a scene', () => {
  const fixture = correctionFixture()
  const firstJob = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  const secondJob = createFoldPreviewTreeSingleHingeStaticCandidatePathJob(
    fixture.context,
    fixture.staticCandidates,
  )
  assert.ok(firstJob && secondJob)
  const first = runPath(firstJob)
  const second = runPath(secondJob)
  assert.equal(first.kind, 'certified')
  assert.equal(second.kind, 'certified')
  assert.deepEqual(second, first)
  assert.notStrictEqual(second, first)
  assertDeeplyFrozen(first)
  assertDeeplyFrozen(second)
  if (first.kind !== 'certified' || second.kind !== 'certified') return
  assert.notStrictEqual(second.certificate, first.certificate)
  assert.notStrictEqual(
    first.certificate.selectedCandidate,
    fixture.staticCandidates.candidates[0],
  )
  assert.equal(first.certificate.safety.runtimeRequestBound, false)
  assert.equal(first.certificate.safety.startScenePoseMatched, false)
  assert.equal(first.certificate.safety.sceneApplied, false)
  assert.equal(first.certificate.safety.autoApplicable, false)
  assert.equal(
    fixture.staticCandidates.safety.continuousCandidatePathCertified,
    false,
  )
  assert.equal(fixture.staticCandidates.safety.sceneApplied, false)
  assert.equal(fixture.staticCandidates.safety.autoApplicable, false)
})

type FixtureOptions = Readonly<{
  fixedFaceId?: 'root' | 'moving' | 'obstacle'
  startSelectedAngleDegrees?: number
  frozenAngleDegrees?: number
  blockingTargetAngleDegrees?: number
  correctionClearance?: number
  maximumTranslation?: number
  maximumAngleDeltaDegrees?: number
}>

function correctionFixture(options: FixtureOptions = {}) {
  const context = prepareContext(options)
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
  const blockingJob = analyzer.createJob(
    context.appliedAngles,
    options.blockingTargetAngleDegrees ?? 120,
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
  assert.ok(blockingJob)
  const blockingResult = runContinuous(blockingJob)
  assert.equal(blockingResult.kind, 'blocked', JSON.stringify(blockingResult))
  const binding = blockingResult.kind === 'blocked'
    ? blockingResult.blocker?.blockingSample?.terminalFullScanBinding
    : null
  assert.ok(binding)
  const staticCandidates =
    deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
      context,
      binding,
      options.correctionClearance ?? CLEARANCE,
      options.maximumTranslation ?? MAXIMUM_TRANSLATION,
      options.maximumAngleDeltaDegrees
        ?? MAXIMUM_ANGLE_DELTA_DEGREES,
    )
  assert.ok(staticCandidates)
  return {
    context,
    sourcePoseRequestKey,
    analyzer,
    binding,
    staticCandidates,
  }
}

function prepareContext({
  fixedFaceId = 'root',
  startSelectedAngleDegrees = 0,
  frozenAngleDegrees = 90,
}: FixtureOptions = {}): FoldPreviewTreeMotionContext {
  const context = prepareFoldPreviewTreeMotionContext({
    model: stationaryBranchCollisionModel(),
    fixedFaceId,
    selectedHingeEdgeId: 'selected',
    appliedAngles: [
      {
        edgeId: 'selected',
        angleDegrees: startSelectedAngleDegrees,
      },
      { edgeId: 'frozen', angleDegrees: frozenAngleDegrees },
    ],
    collisionThickness: COLLISION_THICKNESS,
    visualThickness: COLLISION_THICKNESS,
  })
  assert.ok(context)
  return context
}

function runContinuous(
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

function runPath(
  job: FoldPreviewTreeSingleHingeStaticCandidatePathJob,
  workBudget = 32,
) {
  for (let index = 0; index < 1_000; index += 1) {
    const result = job.step(workBudget)
    if (result.kind !== 'pending') return result
  }
  throw new Error('static candidate path job did not terminate')
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
