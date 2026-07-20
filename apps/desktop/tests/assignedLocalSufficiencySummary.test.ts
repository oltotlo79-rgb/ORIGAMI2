import assert from 'node:assert/strict'
import test from 'node:test'
import { normalizeAssignedLocalSufficiencySummaryResponseV1 } from '../src/lib/coreClient.ts'

const request = {
  expectedProjectInstanceId: '018f47a2-4b7a-7cc1-8abc-112233445566',
  expectedProjectId: '018f47a2-4b7a-7cc1-8abc-665544332211',
  expectedRevision: 7,
  expectedFoldModelFingerprint: 'a'.repeat(64),
}
const vertices = [
  { status: 'necessary_failed', vertex: request.expectedProjectInstanceId },
  {
    status: 'sufficient_proven',
    vertex: request.expectedProjectId,
    model_id: 'assigned_single_vertex_unique_blb_crimp_v1',
    reduction_steps: 2,
  },
] as const
const response = {
  version: 1,
  projectInstanceId: request.expectedProjectInstanceId,
  projectId: request.expectedProjectId,
  revision: request.expectedRevision,
  foldModelFingerprint: request.expectedFoldModelFingerprint,
  vertices,
  totalReductionSteps: 2,
  authorizesProjectMutation: false,
}

test('summary keeps necessary failure separate from sufficient proof', () => {
  assert.deepEqual(normalizeAssignedLocalSufficiencySummaryResponseV1(response, request), response)
  assert.equal(normalizeAssignedLocalSufficiencySummaryResponseV1({
    ...response,
    vertices: [{ ...vertices[0], status: 'sufficient_proven' }],
  }, request), null)
})

test('summary fails closed on stale binding, duplicate vertices and work mismatch', () => {
  assert.equal(normalizeAssignedLocalSufficiencySummaryResponseV1({
    ...response,
    revision: 8,
  }, request), null)
  assert.equal(normalizeAssignedLocalSufficiencySummaryResponseV1({
    ...response,
    vertices: [vertices[0], vertices[0]],
    totalReductionSteps: 0,
  }, request), null)
  assert.equal(normalizeAssignedLocalSufficiencySummaryResponseV1({
    ...response,
    totalReductionSteps: 3,
  }, request), null)
  assert.equal(normalizeAssignedLocalSufficiencySummaryResponseV1({
    ...response,
    authorizesProjectMutation: true,
  }, request), null)
})
