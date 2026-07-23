import assert from 'node:assert/strict'
import test from 'node:test'
import {
  isEffectiveCutReadOnlyRequestV1,
  normalizeEffectiveCutReadOnlyResponseV1,
  type EffectiveCutReadOnlyRequestV1,
} from '../src/lib/coreClient.ts'

const request: EffectiveCutReadOnlyRequestV1 = {
  expectedProjectInstanceId: '018f47a2-4b7a-7cc1-8abc-112233445566',
  expectedProjectId: '018f47a2-4b7a-7cc1-8abc-665544332211',
  expectedRevision: 3,
  expectedFoldModelFingerprint: 'a'.repeat(64),
  requestedComponentKeys: [Array(32).fill(1)],
}

const response = {
  version: 1,
  projectInstanceId: request.expectedProjectInstanceId,
  projectId: request.expectedProjectId,
  revision: request.expectedRevision,
  foldModelFingerprint: request.expectedFoldModelFingerprint,
  effectiveSnapshotFingerprint: Array(32).fill(1),
  geometryModelId: 'effective_cut_collision_geometry_v1',
  geometryFingerprint: Array(32).fill(2),
  pairObservationModelId: 'effective_cut_source_flat_pair_observation_v1',
  pairObservationFingerprint: Array(32).fill(3),
  multiHingeGapModelId: 'effective_cut_multi_hinge_union_gap_diagnostic_v1',
  multiHingeGapFingerprint: Array(32).fill(4),
  sourceFlatPairCount: 3,
  separatedPairs: 1,
  touchingPairs: 1,
  sharedHingeCorridorObservedPairs: 0,
  sharedVertexCorridorObservedPairs: 0,
  penetratingPairs: 0,
  indeterminatePairs: 1,
  multiHingePairs: 1,
  multiHingeUnionCorridorUnprovedPairs: 1,
  authorizesProjectMutation: false,
  authorizesPersistence: false,
  authorizesSimulationAdmission: false,
  authorizesPairClassification: false,
  authorizesCollisionFreeClassification: false,
  authorizesPoseSolving: false,
  authorizesMaterialRemoval: false,
}

test('effective-cut parser accepts only bound aggregate read-only diagnostics', () => {
  assert.ok(normalizeEffectiveCutReadOnlyResponseV1(response, request))
  assert.equal(normalizeEffectiveCutReadOnlyResponseV1({ ...response, faceId: request.expectedProjectId }, request), null)
  assert.equal(normalizeEffectiveCutReadOnlyResponseV1({ ...response, authorizesPersistence: true }, request), null)
  assert.equal(normalizeEffectiveCutReadOnlyResponseV1({ ...response, sourceFlatPairCount: 4 }, request), null)
  assert.equal(normalizeEffectiveCutReadOnlyResponseV1({ ...response, revision: 4 }, request), null)
  assert.equal(normalizeEffectiveCutReadOnlyResponseV1({
    ...response,
    multiHingeUnionCorridorUnprovedPairs: 0,
  }, request), null)
  assert.equal(normalizeEffectiveCutReadOnlyResponseV1({
    ...response,
    sourceFlatPairCount: 50_001,
    separatedPairs: 50_001,
    touchingPairs: 0,
    indeterminatePairs: 0,
  }, request), null)
})

test('effective-cut request validator rejects malformed and ambiguous selections', () => {
  assert.equal(isEffectiveCutReadOnlyRequestV1(request), true)
  assert.equal(isEffectiveCutReadOnlyRequestV1(null), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({ ...request, extra: true }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({ ...request, expectedRevision: -1 }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({ ...request, expectedRevision: Number.NaN }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({
    ...request,
    expectedFoldModelFingerprint: 'A'.repeat(64),
  }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({ ...request, requestedComponentKeys: [] }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({
    ...request,
    requestedComponentKeys: [Array(31).fill(1)],
  }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({
    ...request,
    requestedComponentKeys: [Array(32).fill(1), Array(32).fill(1)],
  }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({
    ...request,
    requestedComponentKeys: [Array(32).fill(2), Array(32).fill(1)],
  }), false)
  assert.equal(isEffectiveCutReadOnlyRequestV1({
    ...request,
    requestedComponentKeys: [Array(32).fill(1), Array(32).fill(2)],
  }), true)
  assert.equal(normalizeEffectiveCutReadOnlyResponseV1(
    response,
    { ...request, expectedRevision: Number.NaN },
  ), null)
})
