import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createStackedFoldProofProgressModel,
  shouldRenderStackedFoldProofProgress,
  speculativeStackedFoldApplyIsCurrent,
} from '../src/lib/stackedFoldProofProgress.ts'
import type { StackedFoldReadResponse } from '../src/lib/stackedFoldRead.ts'
import {
  EMPTY_UNPROVEN_COUNTS,
  makeCertifiedStackedFoldResponse,
  makeSpeculativeSnapshot,
  makeSpeculativeStackedFoldResponse,
  SPECULATIVE_TOKEN,
} from './stackedFoldSpeculativeFixture.ts'

test('maps certified and speculative contracts without overclaiming proof', () => {
  const snapshot = makeSpeculativeSnapshot({
    applied: EMPTY_UNPROVEN_COUNTS,
    unappliedRedo: EMPTY_UNPROVEN_COUNTS,
  })
  const certified = createStackedFoldProofProgressModel({
    kind: 'ready',
    response: makeCertifiedStackedFoldResponse() as StackedFoldReadResponse,
  }, snapshot)
  assert.equal(certified.status, 'certified')
  assert.equal(certified.provenPairCount, 3)
  assert.equal(certified.totalPairCount, 3)
  assert.equal(certified.speculativeApplyAvailable, false)
  assert.equal(certified.proofFailure, null)

  const speculativeResponse =
    makeSpeculativeStackedFoldResponse() as StackedFoldReadResponse
  const speculative = createStackedFoldProofProgressModel({
    kind: 'ready',
    response: speculativeResponse,
  }, snapshot)
  assert.equal(speculative.status, 'evidence_insufficient')
  assert.equal(speculative.provenPairCount, 0)
  assert.equal(speculative.totalPairCount, 3)
  assert.equal(speculative.speculativeApplyAvailable, true)
  // Aggregate snapshot counts contain no entry binding or revert authority.
  assert.equal(speculative.proofFailure, null)
  assert.equal(
    speculativeStackedFoldApplyIsCurrent(
      speculativeResponse,
      snapshot,
      SPECULATIVE_TOKEN,
    ),
    true,
  )
  assert.equal(
    speculativeStackedFoldApplyIsCurrent(
      speculativeResponse,
      snapshot,
      null,
    ),
    false,
  )
})

test('stale, malformed, and unknown proof states all fail closed', () => {
  const snapshot = makeSpeculativeSnapshot()
  const response =
    makeSpeculativeStackedFoldResponse() as StackedFoldReadResponse
  const stale = createStackedFoldProofProgressModel({
    kind: 'ready',
    response,
  }, { ...snapshot, revision: snapshot.revision + 1 })
  assert.equal(stale.status, 'stale')
  assert.equal(stale.speculativeApplyAvailable, false)

  const malformed = makeSpeculativeStackedFoldResponse()
  malformed.transactionProposal.applyMode = 'future_certified'
  const unavailable = createStackedFoldProofProgressModel({
    kind: 'ready',
    response: malformed as StackedFoldReadResponse,
  }, snapshot)
  assert.equal(unavailable.status, 'evidence_insufficient')
  assert.equal(unavailable.speculativeApplyAvailable, false)
  assert.equal(unavailable.proofFailure, null)

  for (const [reason, expected] of [
    ['cycle_path_resource_limit', 'resource_limit'],
    ['cycle_path_cancelled', 'cancelled'],
    ['cycle_path_collision', 'blocked'],
    ['future_success', 'evidence_insufficient'],
  ] as const) {
    const failed = createStackedFoldProofProgressModel({
      kind: 'failed',
      reason,
    }, snapshot)
    assert.equal(failed.status, expected)
    assert.equal(failed.proofFailure, null)
  }
})

test('renders persisted warnings only from an exact safe aggregate summary', () => {
  const absent = createStackedFoldProofProgressModel(
    { kind: 'idle' },
    makeSpeculativeSnapshot(),
  )
  assert.equal(absent.unprovenHistory.kind, 'absent')
  assert.equal(shouldRenderStackedFoldProofProgress(absent), false)

  const known = createStackedFoldProofProgressModel(
    { kind: 'idle' },
    makeSpeculativeSnapshot({
      applied: {
        ...EMPTY_UNPROVEN_COUNTS,
        awaitingProof: 1,
      },
      unappliedRedo: EMPTY_UNPROVEN_COUNTS,
    }),
  )
  assert.equal(known.unprovenHistory.kind, 'known')
  assert.equal(shouldRenderStackedFoldProofProgress(known), true)

  const malformed = createStackedFoldProofProgressModel(
    { kind: 'idle' },
    makeSpeculativeSnapshot({
      applied: {
        ...EMPTY_UNPROVEN_COUNTS,
        awaitingProof: '1',
      },
      unappliedRedo: EMPTY_UNPROVEN_COUNTS,
    }),
  )
  assert.equal(malformed.unprovenHistory.kind, 'unavailable')
  assert.equal(shouldRenderStackedFoldProofProgress(malformed), true)
  assert.equal(malformed.proofFailure, null)
})

test('does not present the pre-Apply safety read as post-Apply proving', () => {
  const reading = createStackedFoldProofProgressModel(
    { kind: 'reading' },
    makeSpeculativeSnapshot(),
  )
  assert.equal(reading.status, null)
  assert.equal(reading.proofFailure, null)
  assert.equal(shouldRenderStackedFoldProofProgress(reading), false)
})
