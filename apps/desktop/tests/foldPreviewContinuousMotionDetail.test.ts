import assert from 'node:assert/strict'
import test from 'node:test'

import {
  describeFoldPreviewContinuousMotionDetail,
  type FoldPreviewMotionFaceLabel,
} from '../src/lib/foldPreviewContinuousMotionDetail.ts'
import type {
  FoldPreviewContinuousMotionRunnerState,
} from '../src/lib/foldPreviewContinuousMotionRunner.ts'

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

function blockedResult(overrides: Record<string, unknown> = {}) {
  return {
    kind: 'blocked',
    certifiedSafeThrough: 0.5,
    stopTime: 0.5,
    unsafeBracket: [0.5, 0.6],
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
