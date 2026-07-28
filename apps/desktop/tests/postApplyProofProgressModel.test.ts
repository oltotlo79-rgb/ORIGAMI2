import assert from 'node:assert/strict'
import test from 'node:test'

import type { PostApplyProofStatusV1 } from '../src/lib/postApplyProofSchedulerClient.ts'
import {
  createStackedFoldProofProgressModel,
} from '../src/lib/stackedFoldProofProgress.ts'
import {
  EMPTY_UNPROVEN_COUNTS,
  makeSpeculativeSnapshot,
} from './stackedFoldSpeculativeFixture.ts'

const projectInstanceId = '018f47a2-4b7a-7cc1-8abc-112233445566'
const projectId = '018f47a2-4b7a-7cc1-8abc-665544332211'
const jobToken = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'

function proofFailure(status: PostApplyProofStatusV1) {
  const reason = status === 'blocked'
    ? 'blocked'
    : status === 'unknown_evidence_insufficient'
      ? 'evidence_insufficient'
      : status === 'unknown_resource_limit'
        ? 'resource_limit'
        : status === 'unknown_cancelled'
          ? 'cancelled'
          : status === 'unknown_deadline_reached'
            ? 'deadline'
            : null
  return reason === null
    ? null
    : {
        location: 'applied_retained_undo' as const,
        reason,
        subsequentEditCount: 2,
        undoStepsToRevert: 3,
      }
}

test('all eight post-Apply statuses map to the truthful coarse panel states', () => {
  const expected = Object.freeze({
    proving: 'proving',
    certified: 'certified',
    blocked: 'blocked',
    unknown_evidence_insufficient: 'evidence_insufficient',
    unknown_resource_limit: 'resource_limit',
    unknown_cancelled: 'cancelled',
    unknown_deadline_reached: 'deadline',
    stale: 'stale',
  } satisfies Readonly<Record<PostApplyProofStatusV1, string>>)
  const snapshot = makeSpeculativeSnapshot({
    applied: { ...EMPTY_UNPROVEN_COUNTS, awaitingProof: 1 },
    unappliedRedo: EMPTY_UNPROVEN_COUNTS,
  })

  for (const [status, panelStatus] of Object.entries(expected) as
    readonly [PostApplyProofStatusV1, string][]) {
    const model = createStackedFoldProofProgressModel(
      { kind: 'idle' },
      snapshot,
      {
        kind: 'progress',
        progress: {
          version: 1,
          projectInstanceId,
          projectId,
          revision: 4,
          jobToken,
          status,
          provenPairCount: status === 'certified' ? 7 : 0,
          totalPairCount: 7,
          proofFailure: proofFailure(status),
        },
      },
    )
    assert.equal(model.status, panelStatus)
    assert.equal(model.provenPairCount, status === 'certified' ? 7 : 0)
    assert.equal(model.totalPairCount, 7)
    assert.equal(model.speculativeApplyAvailable, false)
    assert.equal(model.postApplyNotice, null)
    assert.equal(
      model.unprovenHistory.kind === 'known'
        ? model.unprovenHistory.applied.awaitingProof
        : null,
      1,
    )
    assert.deepEqual(model.proofFailure, proofFailure(status))
  }
})

test('starting and unavailable never invent a total or a proven count', () => {
  const snapshot = makeSpeculativeSnapshot({
    applied: EMPTY_UNPROVEN_COUNTS,
    unappliedRedo: EMPTY_UNPROVEN_COUNTS,
  })
  for (const [postApplyProof, status, notice] of [
    [{ kind: 'starting' }, 'proving', 'starting'],
    [{ kind: 'unavailable' }, 'evidence_insufficient', 'unavailable'],
  ] as const) {
    const model = createStackedFoldProofProgressModel(
      { kind: 'idle' },
      snapshot,
      postApplyProof,
    )
    assert.equal(model.status, status)
    assert.equal(model.postApplyNotice, notice)
    assert.equal(model.provenPairCount, 0)
    assert.equal(model.totalPairCount, null)
  }
})

test('a malformed history summary remains unavailable while proof progress renders', () => {
  const model = createStackedFoldProofProgressModel(
    { kind: 'idle' },
    makeSpeculativeSnapshot({
      applied: { ...EMPTY_UNPROVEN_COUNTS, awaitingProof: '1' },
      unappliedRedo: EMPTY_UNPROVEN_COUNTS,
    }),
    {
      kind: 'progress',
      progress: {
        version: 1,
        projectInstanceId,
        projectId,
        revision: 4,
        jobToken,
        status: 'proving',
        provenPairCount: 0,
        totalPairCount: 2,
        proofFailure: null,
      },
    },
  )
  assert.equal(model.status, 'proving')
  assert.deepEqual(model.unprovenHistory, { kind: 'unavailable' })

  const absent = createStackedFoldProofProgressModel(
    { kind: 'idle' },
    makeSpeculativeSnapshot(),
    {
      kind: 'progress',
      progress: {
        version: 1,
        projectInstanceId,
        projectId,
        revision: 4,
        jobToken,
        status: 'proving',
        provenPairCount: 0,
        totalPairCount: 2,
        proofFailure: null,
      },
    },
  )
  assert.deepEqual(absent.unprovenHistory, { kind: 'unavailable' })
})
