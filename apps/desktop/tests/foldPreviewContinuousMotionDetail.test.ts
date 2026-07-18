import assert from 'node:assert/strict'
import test from 'node:test'

import {
  describeFoldPreviewContinuousMotionDetail,
  type FoldPreviewMotionFaceLabel,
  type FoldPreviewTreeBlockingSampleDetailContext,
} from '../src/lib/foldPreviewContinuousMotionDetail.ts'
import type {
  FoldPreviewContinuousMotionRunnerState,
} from '../src/lib/foldPreviewContinuousMotionRunner.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from '../src/lib/foldPreviewTreeScenePose.ts'

const stats = {
  intervalTests: 4,
  pointTests: 7,
  pointCacheHits: 2,
  maximumDepthReached: 3,
}

const faceLabels: readonly FoldPreviewMotionFaceLabel[] = [
  { id: 'fixed-face', number: 1, label: '面 1（固定）' },
  { id: 'moving-face', number: 2, label: '面 2' },
]

test('forward blocked motion preserves the certified boundary and search bracket', () => {
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    requested: 100,
    applied: 50,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0.5,
      stopTime: 0.5,
      unsafeBracket: [0.5, 0.625],
      blockingSampleTime: 0.625,
      blocker: {
        firstFaceId: 'fixed-face',
        secondFaceId: 'moving-face',
        relation: 'non_adjacent',
        geometryClass: 'penetrating',
      },
      stats,
    },
  }), faceLabels)

  assert.ok(detail)
  assert.equal(detail.kind, 'blocked')
  assert.equal(detail.resultKind, 'blocked')
  assert.deepEqual(detail.path, {
    startDegrees: 0,
    requestedDegrees: 100,
    direction: 'increasing',
  })
  assert.equal(detail.displayDegrees, 50)
  assert.deepEqual(detail.certification, {
    kind: 'interval',
    throughProgress: 0.5,
    throughDegrees: 50,
  })
  assert.deepEqual(detail.bracket, {
    progress: [0.5, 0.625],
    anglesInPathOrder: [50, 62.5],
  })
  assert.equal(detail.certifiedSafeThrough, 0.5)
  assert.equal(detail.firstFaceNumber, 1)
  assert.equal(detail.secondFaceNumber, 2)
  assert.equal(detail.relation, 'non_adjacent')
  assert.equal(detail.geometryClass, 'penetrating')
  assert.equal(detail.hingeDecision, null)
  assert.match(detail.summaryText, /面 1（固定）/u)
  assert.match(detail.summaryText, /面 2/u)
})

test('reverse blocked motion keeps bracket angles in path order', () => {
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    start: 100,
    requested: 20,
    applied: 80,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0.25,
      stopTime: 0.25,
      unsafeBracket: [0.25, 0.5],
      blockingSampleTime: 0.5,
      blocker: {
        firstFaceId: 'moving-face',
        secondFaceId: 'fixed-face',
        relation: 'hinge_adjacent',
        geometryClass: 'touching',
        hingeDecisionKind: 'outside_hinge_contact',
      },
      stats,
    },
  }), faceLabels)

  assert.ok(detail)
  assert.equal(detail.path.direction, 'decreasing')
  assert.deepEqual(detail.bracket?.progress, [0.25, 0.5])
  assert.deepEqual(detail.bracket?.anglesInPathOrder, [80, 60])
  assert.equal(detail.firstFaceNumber, 2)
  assert.equal(detail.secondFaceNumber, 1)
  assert.equal(detail.relation, 'hinge_adjacent')
  assert.equal(detail.geometryClass, 'touching')
  assert.equal(detail.hingeDecision, 'outside_hinge_contact')
  assert.equal(detail.reasonCode, 'motion_blocked')
})

test('zero-width and positive-width time-zero brackets have distinct certification', () => {
  const blockedStart = describeFoldPreviewContinuousMotionDetail(state({
    requested: 90,
    applied: 0,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0,
      stopTime: 0,
      unsafeBracket: [0, 0],
      blockingSampleTime: 0,
      stats,
    },
  }))
  const blockedAfterStart = describeFoldPreviewContinuousMotionDetail(state({
    requested: 90,
    applied: 0,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0,
      stopTime: 0,
      unsafeBracket: [0, 0.1],
      blockingSampleTime: 0.1,
      stats,
    },
  }))

  assert.ok(blockedStart)
  assert.ok(blockedAfterStart)
  assert.deepEqual(blockedStart.certification, {
    kind: 'none',
    displayDegrees: 0,
  })
  assert.deepEqual(blockedAfterStart.certification, {
    kind: 'start_point_only',
    displayDegrees: 0,
  })
  assert.deepEqual(blockedStart.bracket?.anglesInPathOrder, [0, 0])
  assert.deepEqual(blockedAfterStart.bracket?.anglesInPathOrder, [0, 9])
  assert.notEqual(blockedStart.title, blockedAfterStart.title)
})

test('indeterminate motion reports an unresolved path without blocker metadata', () => {
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    start: 10,
    requested: 110,
    applied: 50,
    status: 'indeterminate',
    reason: 'work_limit',
    result: {
      kind: 'indeterminate',
      certifiedSafeThrough: 0.4,
      stopTime: 0.4,
      unresolvedBracket: [0.4, 0.5],
      reason: 'work_limit',
      stats,
    },
  }))

  assert.ok(detail)
  assert.equal(detail.kind, 'indeterminate')
  assert.equal(detail.resultKind, 'indeterminate')
  assert.equal(detail.reasonCode, 'work_limit')
  assert.deepEqual(detail.bracket?.anglesInPathOrder, [50, 60])
  assert.deepEqual(detail.certification, {
    kind: 'interval',
    throughProgress: 0.4,
    throughDegrees: 50,
  })
  assert.equal(detail.firstFaceNumber, null)
  assert.equal(detail.secondFaceNumber, null)
  assert.equal(detail.relation, null)
  assert.equal(detail.geometryClass, null)
  assert.equal(detail.hingeDecision, null)
})

test('an unmodeled layer offset has a dedicated stop explanation', () => {
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    start: 0,
    requested: 180,
    applied: 170,
    status: 'indeterminate',
    reason: 'hinge_layer_offset_unmodeled',
    result: {
      kind: 'indeterminate',
      certifiedSafeThrough: 17 / 18,
      stopTime: 17 / 18,
      unresolvedBracket: [17 / 18, 35 / 36],
      reason: 'hinge_layer_offset_unmodeled',
      stats,
    },
  }))

  assert.ok(detail)
  assert.equal(detail.reasonCode, 'hinge_layer_offset_unmodeled')
  assert.ok(detail.rows.some((row) =>
    row.kind === 'user'
    && row.value.includes('層ずらし')
    && row.value.includes('判定できません')))
})

test('unknown runner reasons are classified without exposing raw text', () => {
  const rawReason = 'secret_backend_payload:do-not-display'
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    requested: 80,
    applied: 0,
    status: 'indeterminate',
    reason: rawReason,
    result: null,
  }))

  assert.ok(detail)
  assert.equal(detail.resultKind, 'runner_failure')
  assert.equal(detail.reasonCode, 'unclassified')
  assert.equal(detail.bracket, null)
  assert.equal(detail.certifiedSafeThrough, null)
  assert.doesNotMatch(JSON.stringify(detail), /secret_backend_payload/u)
  assert.doesNotMatch(JSON.stringify(detail), /do-not-display/u)
})

test('inconsistent terminal contracts fail closed', () => {
  const malformedStates = [
    state({
      requested: 100,
      applied: 50,
      status: 'blocked',
      reason: 'wrong_reason',
      result: blockedResult(),
    }),
    state({
      requested: 100,
      applied: 51,
      status: 'blocked',
      reason: 'motion_blocked',
      result: blockedResult(),
    }),
    state({
      requested: 100,
      applied: 50,
      status: 'blocked',
      reason: 'motion_blocked',
      result: blockedResult({ unsafeBracket: [0.5, 0.5] }),
    }),
    state({
      requested: 100,
      applied: 50,
      status: 'blocked',
      reason: 'motion_blocked',
      result: blockedResult({ blockingSampleTime: 0.59 }),
    }),
    state({
      requested: 100,
      applied: 50,
      status: 'blocked',
      reason: 'motion_blocked',
      result: blockedResult({ blockingSampleTime: undefined }),
    }),
    state({
      requested: 100,
      applied: 50,
      status: 'blocked',
      reason: 'motion_blocked',
      result: blockedResult({ blockingSampleTime: Number.NaN }),
    }),
    state({
      requested: 100,
      applied: 50,
      status: 'blocked',
      reason: 'motion_blocked',
      result: blockedResult({
        stats: { ...stats, intervalTests: -1 },
      }),
    }),
    state({
      requested: 100,
      applied: 50,
      status: 'indeterminate',
      reason: 'work_limit',
      result: {
        kind: 'indeterminate',
        certifiedSafeThrough: 0.5,
        stopTime: 0.5,
        unresolvedBracket: [0.5, 0.6],
        reason: 'uncertified_interval',
        stats,
      },
    }),
  ]

  for (const malformed of malformedStates) {
    assert.equal(describeFoldPreviewContinuousMotionDetail(malformed), null)
  }
})

test('invalid blocker combinations degrade to generic blocked detail', () => {
  const invalidBlockers = [
    {
      firstFaceId: 'fixed-face',
      secondFaceId: 'moving-face',
      relation: 'non_adjacent',
      geometryClass: 'touching',
      hingeDecisionKind: 'outside_hinge_contact',
    },
    {
      firstFaceId: 'fixed-face',
      secondFaceId: 'moving-face',
      relation: 'hinge_adjacent',
      geometryClass: 'touching',
      hingeDecisionKind: 'outside_hinge_penetration',
    },
    {
      firstFaceId: 'fixed-face',
      secondFaceId: 'moving-face',
      relation: 'hinge_adjacent',
      geometryClass: 'indeterminate',
      hingeDecisionKind: 'outside_hinge_contact',
    },
  ]

  for (const blocker of invalidBlockers) {
    const detail = describeFoldPreviewContinuousMotionDetail(state({
      requested: 100,
      applied: 50,
      status: 'blocked',
      reason: 'motion_blocked',
      result: blockedResult({ blocker }),
    }), faceLabels)

    assert.ok(detail)
    assert.equal(detail.resultKind, 'blocked')
    assert.equal(detail.reasonCode, 'motion_blocked')
    assert.equal(detail.firstFaceNumber, null)
    assert.equal(detail.secondFaceNumber, null)
    assert.equal(detail.relation, null)
    assert.equal(detail.geometryClass, null)
    assert.equal(detail.hingeDecision, null)
    assert.doesNotMatch(JSON.stringify(detail), /fixed-face|moving-face/u)
  }
})

test('detail snapshots are deeply frozen and independent of later input mutation', () => {
  const mutableStats = { ...stats }
  const mutableBlocker = {
    firstFaceId: 'fixed-face',
    secondFaceId: 'moving-face',
    relation: 'non_adjacent',
    geometryClass: 'touching',
  }
  const mutableLabels = faceLabels.map((label) => ({ ...label }))
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    requested: 100,
    applied: 50,
    status: 'blocked',
    reason: 'motion_blocked',
    result: blockedResult({
      blocker: mutableBlocker,
      stats: mutableStats,
    }),
  }), mutableLabels)

  assert.ok(detail)
  const snapshot = JSON.stringify(detail)
  mutableStats.intervalTests = 999
  mutableBlocker.firstFaceId = 'changed-face'
  mutableBlocker.geometryClass = 'penetrating'
  mutableLabels[0].label = 'changed-label'

  assert.equal(JSON.stringify(detail), snapshot)
  assert.ok(Object.isFrozen(detail))
  assert.ok(Object.isFrozen(detail.path))
  assert.ok(Object.isFrozen(detail.certification))
  assert.ok(Object.isFrozen(detail.bracket))
  assert.ok(Object.isFrozen(detail.bracket?.progress))
  assert.ok(Object.isFrozen(detail.bracket?.anglesInPathOrder))
  assert.ok(Object.isFrozen(detail.rows))
  for (const row of detail.rows) assert.ok(Object.isFrozen(row))
})

test('request-bound blocking witnesses add detached unsafe-pose detail', () => {
  const sample = validBlockingSample()
  const detail = describeFoldPreviewContinuousMotionDetail(
    evidenceState(sample),
    faceLabels,
    validEvidenceContext(),
  )

  assert.ok(detail)
  assert.equal(detail.kind, 'blocked')
  assert.deepEqual(detail.blockingEvidence, {
    unsafeAnalysisDegrees: 60,
    firstTriangleNumber: 3,
    secondTriangleNumber: 5,
    positionCandidateCount: 4,
    normal: {
      x: 1,
      y: 0,
      z: 0,
      uniqueness: 'unique',
    },
    escapeDistance: 0.25,
    coverage: {
      eligiblePairCount: 1,
      attemptedPairCount: 1,
      capturedPairCount: 1,
      unavailablePairCount: 0,
      omittedByLimitCount: 0,
      authoritativePairScanComplete: true,
    },
    safety: {
      sampleTransformsAppliedToScene: false,
      scope: 'selected_triangle_prism_pair_only',
      autoApplicable: false,
    },
  })
  const rows = new Map(detail.rows.map((row) => [row.label, row.value]))
  assert.equal(rows.get('危険解析角度'), '60°')
  assert.equal(
    rows.get('局所三角形ペア'),
    '面 1（固定）の三角形 3 ↔ 面 2の三角形 5',
  )
  assert.equal(rows.get('位置候補数'), '4点')
  assert.equal(rows.get('局所分離方向'), '(1, 0, 0)・一意')
  assert.equal(rows.get('局所分離距離'), '0.25（3Dモデル座標）')
  assert.match(
    rows.get('解析行列の扱い') ?? '',
    /面行列は3D表示の更新に使用していません/u,
  )
  assert.match(rows.get('証拠の範囲') ?? '', /局所候補/u)
  assert.match(rows.get('自動適用可否') ?? '', /自動適用できません/u)
  assert.match(
    rows.get('証拠取得範囲') ?? '',
    /全ペア走査完了.*対象 1.*試行 1.*取得 1/u,
  )
  assert.match(detail.summaryText, /危険側/u)
  assert.match(detail.summaryText, /局所候補/u)
  assert.doesNotMatch(
    JSON.stringify(detail),
    /evidence-project|tree-context-key|source-pose-request-key|selected-edge/u,
  )
})

test('stale or internally inconsistent blocking witnesses fall back to basic blocked detail', () => {
  const changedSamples = [
    changeBlockingSample((sample) => {
      sample.version = 'future-or-hostile-version'
    }),
    changeBlockingSample((sample) => {
      sample.blockingSampleTime = 0.61
    }),
    changeBlockingSample((sample) => {
      sample.selectedAngleDegrees = 61
    }),
    changeBlockingSample((sample) => {
      sample.identity.revision = 8
    }),
    changeBlockingSample((sample) => {
      sample.identity.request.contextKey = 'stale-context'
    }),
    changeBlockingSample((sample) => {
      sample.angleVectors.target[1].angleDegrees = 46
    }),
    changeBlockingSample((sample) => {
      sample.angleVectors.sample[0].angleDegrees = 59
    }),
    changeBlockingSample((sample) => {
      sample.faceTransforms[1].faceId = 'wrong-face'
    }),
    changeBlockingSample((sample) => {
      sample.faceTransforms[0].elements[4] = Number.NaN
    }),
    changeBlockingSample((sample) => {
      sample.witnessCoverage.attemptedPairCount = 2
    }),
    changeBlockingSample((sample) => {
      sample.primaryWitnessIndex = 1
    }),
    changeBlockingSample((sample) => {
      sample.witnessSamples[0].geometryClass = 'touching'
    }),
    changeBlockingSample((sample) => {
      sample.witnessSamples[0].witness.normal.vector.x = 2
    }),
    changeBlockingSample((sample) => {
      sample.witnessSamples[0].witness.positionRegion.generators[0].x = 9
    }),
    changeBlockingSample((sample) => {
      sample.witnessSamples[0].witness.localSeparationHint.autoApplicable = true
    }),
    changeBlockingSample((sample) => {
      sample.witnessSamples[0].witness.toleratedGap = 2e-9
    }),
    changeBlockingSample((sample) => {
      sample.witnessSamples[0].witness.escapeDistance = 0
      sample.witnessSamples[0].witness.localSeparationHint.distance = 0
      sample.witnessSamples[0].witness.localSeparationHint.translation.x = 0
    }),
    changeBlockingSample((sample) => {
      sample.witnessCoverage.eligiblePairCount = 1_000_001
      sample.witnessCoverage.omittedByLimitCount = 1_000_000
    }),
  ]

  for (const [index, sample] of changedSamples.entries()) {
    const currentState = evidenceState(sample)
    const detail = describeFoldPreviewContinuousMotionDetail(
      currentState,
      faceLabels,
      validEvidenceContext(),
    )
    const basic = describeFoldPreviewContinuousMotionDetail(
      currentState,
      faceLabels,
    )
    assert.ok(detail, `case ${index}`)
    assert.deepEqual(detail, basic, `case ${index}`)
    assert.equal(detail.blockingEvidence, null, `case ${index}`)
    assert.equal(detail.geometryClass, 'penetrating', `case ${index}`)
    assert.ok(
      !detail.rows.some((row) => row.label === '危険解析角度'),
      `case ${index}`,
    )
  }

  const context = validEvidenceContext()
  const changedContexts: readonly FoldPreviewTreeBlockingSampleDetailContext[] = [
    { ...context, projectId: 'other-project' },
    { ...context, revision: 8 },
    { ...context, fixedFaceId: 'other-fixed-face' },
    { ...context, selectedHingeEdgeId: 'other-edge' },
    { ...context, contextKey: 'other-context' },
    { ...context, sourcePoseRequestKey: 'other-source-pose' },
    { ...context, generation: 10 },
    { ...context, requestSequence: 22 },
    { ...context, collisionThickness: 0.03 },
    { ...context, collisionThickness: 0 },
    {
      ...context,
      startAngles: context.startAngles.map((angle) => ({
        ...angle,
        angleDegrees: angle.edgeId === 'frozen-edge'
          ? 46
          : angle.angleDegrees,
      })),
    },
    { ...context, targetSelectedAngleDegrees: 99 },
  ]
  const currentState = evidenceState(validBlockingSample())
  const basic = describeFoldPreviewContinuousMotionDetail(
    currentState,
    faceLabels,
  )
  for (const [index, changedContext] of changedContexts.entries()) {
    const detail = describeFoldPreviewContinuousMotionDetail(
      currentState,
      faceLabels,
      changedContext,
    )
    assert.deepEqual(detail, basic, `context case ${index}`)
  }
})

test('hostile blocking-sample access and values preserve the authoritative block', () => {
  const throwingBlocker = {
    firstFaceId: 'fixed-face',
    secondFaceId: 'moving-face',
    relation: 'non_adjacent',
    geometryClass: 'penetrating',
  }
  Object.defineProperty(throwingBlocker, 'blockingSample', {
    enumerable: true,
    get() {
      throw new Error('secret-blocking-sample-getter')
    },
  })
  const throwingState = state({
    requested: 100,
    applied: 50,
    status: 'blocked',
    reason: 'motion_blocked',
    result: blockedResult({ blocker: throwingBlocker }),
  })
  const detail = describeFoldPreviewContinuousMotionDetail(
    throwingState,
    faceLabels,
    validEvidenceContext(),
  )
  assert.ok(detail)
  assert.equal(detail.blockingEvidence, null)
  assert.equal(detail.geometryClass, 'penetrating')
  assert.doesNotMatch(JSON.stringify(detail), /secret-blocking-sample-getter/u)

  const throwingContext = new Proxy(validEvidenceContext(), {
    get(target, property, receiver) {
      if (property === 'contextKey') {
        throw new Error('secret-context-getter')
      }
      return Reflect.get(target, property, receiver)
    },
  })
  const contextFailure = describeFoldPreviewContinuousMotionDetail(
    evidenceState(validBlockingSample()),
    faceLabels,
    throwingContext,
  )
  assert.ok(contextFailure)
  assert.equal(contextFailure.blockingEvidence, null)
  assert.equal(contextFailure.geometryClass, 'penetrating')
  assert.doesNotMatch(JSON.stringify(contextFailure), /secret-context-getter/u)

  const nonFiniteSample = changeBlockingSample((sample) => {
    sample.witnessSamples[0].witness.escapeDistance =
      Number.POSITIVE_INFINITY
  })
  const nonFinite = describeFoldPreviewContinuousMotionDetail(
    evidenceState(nonFiniteSample),
    faceLabels,
    validEvidenceContext(),
  )
  assert.ok(nonFinite)
  assert.equal(nonFinite.blockingEvidence, null)

  const overLimitSample = validBlockingSample()
  overLimitSample.witnessSamples = Array.from(
    { length: 17 },
    () => structuredClone(validBlockingSample().witnessSamples[0]),
  )
  const overLimit = describeFoldPreviewContinuousMotionDetail(
    evidenceState(overLimitSample),
    faceLabels,
    validEvidenceContext(),
  )
  assert.ok(overLimit)
  assert.equal(overLimit.blockingEvidence, null)
})

test('runner scalars and terminal brackets are snapshotted once', () => {
  const scalarReads = new Map<string, number>()
  const once = (name: string, value: unknown) => ({
    enumerable: true,
    get() {
      scalarReads.set(name, (scalarReads.get(name) ?? 0) + 1)
      return value
    },
  })
  const bracketReads = new Map<PropertyKey, number>()
  const bracket = new Proxy([0.5, 0.6], {
    get(target, property, receiver) {
      bracketReads.set(property, (bracketReads.get(property) ?? 0) + 1)
      return Reflect.get(target, property, receiver)
    },
  })
  const terminalState = {}
  Object.defineProperties(terminalState, {
    start: once('start', 0),
    applied: once('applied', 50),
    requested: once('requested', 100),
    status: once('status', 'blocked'),
    reason: once('reason', 'motion_blocked'),
    result: once('result', blockedResult({ unsafeBracket: bracket })),
  })

  const detail = describeFoldPreviewContinuousMotionDetail(
    terminalState as FoldPreviewContinuousMotionRunnerState<unknown>,
  )
  assert.ok(detail)
  for (const name of [
    'start',
    'applied',
    'requested',
    'status',
    'reason',
    'result',
  ]) {
    assert.equal(scalarReads.get(name), 1, name)
  }
  assert.equal(bracketReads.get('length'), 1)
  assert.equal(bracketReads.get('0'), 1)
  assert.equal(bracketReads.get('1'), 1)
})

test('blocking evidence output is deeply frozen and detached from its inputs', () => {
  const sample = validBlockingSample()
  const baseContext = validEvidenceContext()
  const context = {
    ...baseContext,
    startAngles: baseContext.startAngles.map((angle) => ({ ...angle })),
  }
  const detail = describeFoldPreviewContinuousMotionDetail(
    evidenceState(sample),
    faceLabels,
    context,
  )

  assert.ok(detail?.blockingEvidence)
  const snapshot = JSON.stringify(detail)
  sample.selectedAngleDegrees = 99
  sample.witnessSamples[0].witness.normal.vector.x = -1
  sample.witnessSamples[0].witness.escapeDistance = 10
  context.contextKey = 'mutated-context'
  context.startAngles[1].angleDegrees = 12
  assert.equal(JSON.stringify(detail), snapshot)

  const evidence = detail.blockingEvidence
  assert.ok(Object.isFrozen(evidence))
  assert.ok(Object.isFrozen(evidence.normal))
  assert.ok(Object.isFrozen(evidence.coverage))
  assert.ok(Object.isFrozen(evidence.safety))
  assert.ok(Object.isFrozen(detail.rows))
  for (const row of detail.rows) assert.ok(Object.isFrozen(row))
})

test('blocking evidence ignores hostile array iterators within fixed bounds', () => {
  const sample = validBlockingSample()
  const context = validEvidenceContext()
  const arrays: unknown[][] = [
    sample.angleVectors.start,
    sample.angleVectors.target,
    sample.angleVectors.sample,
    sample.faceTransforms,
    sample.faceTransforms[0].elements,
    sample.faceTransforms[1].elements,
    sample.witnessSamples,
    sample.witnessSamples[0].witness.firstSupport,
    sample.witnessSamples[0].witness.secondSupport,
    sample.witnessSamples[0].witness.positionRegion.generators,
    context.startAngles as unknown[],
  ]
  for (const array of arrays) {
    Object.defineProperty(array, Symbol.iterator, {
      configurable: true,
      value() {
        throw new Error('iterator-must-not-run')
      },
    })
  }

  const detail = describeFoldPreviewContinuousMotionDetail(
    evidenceState(sample),
    faceLabels,
    context,
  )
  assert.ok(detail?.blockingEvidence)
  assert.equal(detail.blockingEvidence.unsafeAnalysisDegrees, 60)
})

test('time-zero evidence describes unused sample matrices without denying the displayed pose', () => {
  const sample = validBlockingSample()
  sample.blockingSampleTime = 0
  sample.selectedAngleDegrees = 0
  sample.angleVectors.sample[0].angleDegrees = 0
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    requested: 100,
    applied: 0,
    start: 0,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      ...blockedResult(),
      certifiedSafeThrough: 0,
      stopTime: 0,
      unsafeBracket: [0, 0],
      blockingSampleTime: 0,
      blocker: {
        firstFaceId: 'fixed-face',
        secondFaceId: 'moving-face',
        relation: 'non_adjacent',
        geometryClass: 'penetrating',
        blockingSample: sample,
      },
    },
  }), faceLabels, validEvidenceContext())

  assert.ok(detail?.blockingEvidence)
  assert.equal(detail.blockingEvidence.unsafeAnalysisDegrees, 0)
  const rows = new Map(detail.rows.map((row) => [row.label, row.value]))
  assert.match(
    rows.get('解析行列の扱い') ?? '',
    /面行列は3D表示の更新に使用していません/u,
  )
  assert.doesNotMatch(
    rows.get('解析行列の扱い') ?? '',
    /危険姿勢は.*表示されていません/u,
  )
})

function blockedResult(overrides: Record<string, unknown> = {}) {
  return {
    kind: 'blocked',
    certifiedSafeThrough: 0.5,
    stopTime: 0.5,
    unsafeBracket: [0.5, 0.6],
    blockingSampleTime: 0.6,
    stats,
    ...overrides,
  }
}

function state(
  overrides: Record<string, unknown> = {},
): FoldPreviewContinuousMotionRunnerState<unknown> {
  return {
    requested: 52,
    applied: 0,
    start: 0,
    status: 'running',
    reason: null,
    result: null,
    ...overrides,
  } as FoldPreviewContinuousMotionRunnerState<unknown>
}

function evidenceState(
  blockingSample: ReturnType<typeof validBlockingSample>,
) {
  return state({
    requested: 100,
    applied: 50,
    status: 'blocked',
    reason: 'motion_blocked',
    result: blockedResult({
      blocker: {
        firstFaceId: 'fixed-face',
        secondFaceId: 'moving-face',
        relation: 'non_adjacent',
        geometryClass: 'penetrating',
        blockingSample,
      },
    }),
  })
}

function validEvidenceContext(): FoldPreviewTreeBlockingSampleDetailContext {
  const startAngles = [
    { edgeId: 'selected-edge', angleDegrees: 0 },
    { edgeId: 'frozen-edge', angleDegrees: 45 },
  ] as const
  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    {
      projectId: 'evidence-project',
      revision: 7,
      kind: 'fold_graph',
    },
    'fixed-face',
    0.02,
    startAngles,
  )
  assert.ok(sourcePoseRequestKey)
  return {
    projectId: 'evidence-project',
    revision: 7,
    fixedFaceId: 'fixed-face',
    selectedHingeEdgeId: 'selected-edge',
    contextKey: 'tree-context-key',
    sourcePoseRequestKey,
    generation: 9,
    requestSequence: 21,
    collisionThickness: 0.02,
    startAngles,
    targetSelectedAngleDegrees: 100,
  }
}

function validBlockingSample() {
  const context = validEvidenceContext()
  return {
    version: 'tree_single_hinge_blocking_sample_v1',
    sourcePose: 'blocking_evaluate_point_pose',
    blockingSampleTime: 0.6,
    selectedAngleDegrees: 60,
    collisionThickness: 0.02,
    identity: {
      projectId: 'evidence-project',
      revision: 7,
      revisionBinding: 'project_response_source_equal_v1',
      fixedFaceId: 'fixed-face',
      selectedHingeEdgeId: 'selected-edge',
      request: {
        contextKey: 'tree-context-key',
        sourcePoseRequestKey: context.sourcePoseRequestKey,
        generation: 9,
        requestSequence: 21,
      },
    },
    angleVectors: {
      start: [
        { edgeId: 'selected-edge', angleDegrees: 0 },
        { edgeId: 'frozen-edge', angleDegrees: 45 },
      ],
      target: [
        { edgeId: 'selected-edge', angleDegrees: 100 },
        { edgeId: 'frozen-edge', angleDegrees: 45 },
      ],
      sample: [
        { edgeId: 'selected-edge', angleDegrees: 60 },
        { edgeId: 'frozen-edge', angleDegrees: 45 },
      ],
    },
    faceTransforms: [
      {
        faceId: 'fixed-face',
        elements: identityMatrixElements(),
      },
      {
        faceId: 'moving-face',
        elements: [
          1, 0, 0, 0,
          0, 1, 0, 0,
          0, 0, 1, 0,
          0.5, 0, 0, 1,
        ],
      },
    ],
    witnessSamples: [
      {
        firstFaceId: 'fixed-face',
        secondFaceId: 'moving-face',
        relation: 'non_adjacent',
        firstTriangleIndex: 2,
        secondTriangleIndex: 4,
        geometryClass: 'penetrating',
        witness: {
          algorithm: 'triangle_prism_sat_witness_v1',
          geometryClass: 'penetrating',
          numericalMargin: 1e-9,
          normal: {
            vector: { x: 1, y: 0, z: 0 },
            convention: 'moves_second_away_from_first',
            uniqueness: 'unique',
          },
          escapeDistance: 0.25,
          toleratedGap: 0,
          firstSupport: [
            { x: 0, y: 0, z: 0 },
            { x: 0, y: 2, z: 0 },
          ],
          secondSupport: [
            { x: 2, y: 0, z: 0 },
            { x: 2, y: 2, z: 0 },
          ],
          positionRegion: {
            kind: 'support_midpoint_hull_v1',
            sourcePose: 'analyzed_input_pose',
            generators: [
              { x: 1, y: 0, z: 0 },
              { x: 1, y: 1, z: 0 },
              { x: 1, y: 1, z: 0 },
              { x: 1, y: 2, z: 0 },
            ],
          },
          localSeparationHint: {
            translation: { x: 0.25, y: 0, z: 0 },
            distance: 0.25,
            scope: 'selected_triangle_prism_pair_only',
            autoApplicable: false,
          },
        },
      },
    ],
    witnessCoverage: {
      scope:
        'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1',
      eligiblePairCount: 1,
      attemptedPairCount: 1,
      unavailablePairCount: 0,
      omittedByLimitCount: 0,
      authoritativePairScanComplete: true,
    },
    primaryWitnessIndex: 0,
  }
}

function changeBlockingSample(
  change: (sample: ReturnType<typeof validBlockingSample>) => void,
) {
  const sample = validBlockingSample()
  change(sample)
  return sample
}

function identityMatrixElements() {
  return [
    1, 0, 0, 0,
    0, 1, 0, 0,
    0, 0, 1, 0,
    0, 0, 0, 1,
  ]
}
