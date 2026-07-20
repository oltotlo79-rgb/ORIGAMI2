import assert from 'node:assert/strict'
import { test } from 'node:test'
import { normalizeAssignedLocalSufficiencyResponseV1 } from '../src/lib/coreClient.ts'

const instance = '018f47a2-4b7a-7cc1-8abc-112233445566'
const project = '018f47a2-4b7a-7cc1-8abc-665544332211'
const vertex = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'
const first = '018f47a2-4b7a-7cc1-8abc-111111111111'
const second = '018f47a2-4b7a-7cc1-8abc-222222222222'
const request = {
  expectedProjectInstanceId: instance,
  expectedProjectId: project,
  expectedRevision: 7,
  vertex,
} as const

test('assigned local sufficiency admits only bound proof witnesses', () => {
  const response = {
    version: 1,
    projectInstanceId: instance,
    projectId: project,
    revision: 7,
    result: {
      status: 'proven',
      model_id: 'assigned_single_vertex_unique_blb_crimp_v1',
      vertex,
      reduction_steps: 1,
      reductions: [{ first_crease: first, second_crease: second }],
    },
    authorizesProjectMutation: false,
  } as const
  assert.deepEqual(normalizeAssignedLocalSufficiencyResponseV1(response, request), response)
  assert.equal(normalizeAssignedLocalSufficiencyResponseV1({ ...response, revision: 8 }, request), null)
  assert.equal(normalizeAssignedLocalSufficiencyResponseV1({
    ...response,
    result: { ...response.result, reduction_steps: 0 },
  }, request), null)
  assert.equal(normalizeAssignedLocalSufficiencyResponseV1({
    ...response,
    result: {
      ...response.result,
      reductions: [{ first_crease: first, second_crease: first }],
    },
  }, request), null)
  assert.equal(normalizeAssignedLocalSufficiencyResponseV1({
    ...response,
    authorizesProjectMutation: true,
  }, request), null)
})

test('assigned local sufficiency keeps indeterminate reasons closed', () => {
  const response = {
    version: 1,
    projectInstanceId: instance,
    projectId: project,
    revision: 7,
    result: { status: 'indeterminate', vertex, reason: 'resource_limit' },
    authorizesProjectMutation: false,
  } as const
  assert.deepEqual(normalizeAssignedLocalSufficiencyResponseV1(response, request), response)
  assert.equal(normalizeAssignedLocalSufficiencyResponseV1({
    ...response,
    result: { ...response.result, reason: 'probably_foldable' },
  }, request), null)
})
