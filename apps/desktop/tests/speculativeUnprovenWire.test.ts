import assert from 'node:assert/strict'
import test from 'node:test'

import {
  normalizeProofFailureViewModelV1,
  unprovenHistorySummaryFromSnapshotV1,
} from '../src/lib/speculativeUnprovenWire.ts'

const valid = Object.freeze({
  location: 'applied_retained_undo',
  outcome: 'blocked',
  reason: null,
  subsequentEditCount: 2,
  undoStepsToRevert: 3,
})

test('proof failure parser emits only the coarse revert decision DTO', () => {
  assert.deepEqual(normalizeProofFailureViewModelV1(valid), {
    location: 'applied_retained_undo',
    reason: 'blocked',
    subsequentEditCount: 2,
    undoStepsToRevert: 3,
  })
  assert.deepEqual(normalizeProofFailureViewModelV1({
    ...valid,
    outcome: 'unknown',
    reason: 'deadline_reached',
  }), {
    location: 'applied_retained_undo',
    reason: 'deadline',
    subsequentEditCount: 2,
    undoStepsToRevert: 3,
  })
})

test('proof failure parser rejects unknown, extra, unsafe, and contradictory values', () => {
  const invalid = [
    { ...valid, rawPath: 'C:\\secret.ori2' },
    { ...valid, outcome: 'certified' },
    { ...valid, outcome: 'future_success' },
    { ...valid, location: 'internal_entry_17' },
    { ...valid, reason: 'collision_at_coordinate_3_4' },
    { ...valid, subsequentEditCount: Number.MAX_SAFE_INTEGER + 1 },
    { ...valid, subsequentEditCount: -1 },
    { ...valid, undoStepsToRevert: 2 },
    { ...valid, location: 'applied_trimmed_base', undoStepsToRevert: 3 },
    {
      ...valid,
      location: 'unapplied_redo',
      subsequentEditCount: 1,
      undoStepsToRevert: null,
    },
  ]
  for (const value of invalid) {
    assert.equal(normalizeProofFailureViewModelV1(value), null)
  }
  const accessor = { ...valid }
  Object.defineProperty(accessor, 'reason', {
    enumerable: true,
    get() {
      throw new Error('must not be read')
    },
  })
  assert.equal(normalizeProofFailureViewModelV1(accessor), null)

  assert.equal(
    normalizeProofFailureViewModelV1(Object.create(valid)),
    null,
  )
  const hidden = { ...valid }
  Object.defineProperty(hidden, 'privateField', { value: true })
  assert.equal(normalizeProofFailureViewModelV1(hidden), null)
  assert.equal(normalizeProofFailureViewModelV1({
    ...valid,
    [Symbol('private')]: true,
  }), null)

  let proxyGetCalls = 0
  const proxied = new Proxy({ ...valid }, {
    get() {
      proxyGetCalls += 1
      throw new Error('must not be read')
    },
  })
  assert.notEqual(normalizeProofFailureViewModelV1(proxied), null)
  assert.equal(proxyGetCalls, 0)
})

const emptyCounts = Object.freeze({
  awaitingProof: 0,
  proofBlocked: 0,
  unknownEvidenceInsufficient: 0,
  unknownResourceLimit: 0,
  unknownCancelled: 0,
  unknownDeadlineReached: 0,
})

test('snapshot coarse unproven summary uses exact safe checked counts', () => {
  assert.deepEqual(unprovenHistorySummaryFromSnapshotV1({
    speculativeUnprovenFolds: {
      applied: {
        ...emptyCounts,
        awaitingProof: 2,
        proofBlocked: 1,
        unknownResourceLimit: 3,
      },
      unappliedRedo: {
        ...emptyCounts,
        unknownCancelled: 4,
      },
    },
  }), {
    kind: 'known',
    applied: {
      awaitingProof: 2,
      proofBlocked: 1,
      unknownEvidenceInsufficient: 0,
      unknownResourceLimit: 3,
      unknownCancelled: 0,
      unknownDeadlineReached: 0,
    },
    unappliedRedo: {
      awaitingProof: 0,
      proofBlocked: 0,
      unknownEvidenceInsufficient: 0,
      unknownResourceLimit: 0,
      unknownCancelled: 4,
      unknownDeadlineReached: 0,
    },
    appliedTotal: 6,
    unappliedRedoTotal: 4,
  })
  assert.deepEqual(unprovenHistorySummaryFromSnapshotV1({}), {
    kind: 'absent',
  })
})

test('snapshot coarse summary fails unknown status and unsafe sums to unproven', () => {
  const invalidCounts = [
    { ...emptyCounts, futureCertified: 1 },
    { ...emptyCounts, awaitingProof: -1 },
    { ...emptyCounts, awaitingProof: -0 },
    { ...emptyCounts, awaitingProof: 0.5 },
    { ...emptyCounts, awaitingProof: 2.0000000000000004 },
    { ...emptyCounts, awaitingProof: Number.MAX_SAFE_INTEGER + 1 },
    {
      ...emptyCounts,
      awaitingProof: Number.MAX_SAFE_INTEGER,
      proofBlocked: 1,
    },
  ]
  for (const applied of invalidCounts) {
    assert.deepEqual(unprovenHistorySummaryFromSnapshotV1({
      speculativeUnprovenFolds: {
        applied,
        unappliedRedo: emptyCounts,
      },
    }), { kind: 'unavailable' })
  }
  assert.deepEqual(unprovenHistorySummaryFromSnapshotV1({
    speculativeUnprovenFolds: {
      applied: emptyCounts,
      unappliedRedo: emptyCounts,
      future: true,
    },
  }), { kind: 'unavailable' })
  const accessor = {}
  Object.defineProperty(accessor, 'speculativeUnprovenFolds', {
    enumerable: true,
    get() {
      throw new Error('must not be read')
    },
  })
  assert.deepEqual(unprovenHistorySummaryFromSnapshotV1(accessor), {
    kind: 'unavailable',
  })
})
