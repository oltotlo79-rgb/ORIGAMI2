import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createProofScopePresentation,
  LOCAL_SUFFICIENCY_CERTIFICATE_MODEL,
  PROOF_SCOPE_DIAGNOSTICS_SCHEMA,
  PROOF_SCOPE_VISIBLE_VERTEX_LIMIT,
} from '../src/lib/proofScopePresentation.ts'
import { GLOBAL_FLAT_FOLDABILITY_MODEL_ID } from '../src/lib/globalFlatFoldability.ts'
import type { AssignedLocalSufficiencySummaryResponseV1 } from '../src/lib/coreClient.ts'

const counts = {
  face_count: 3,
  overlap_cell_count: 1,
  constraint_count: 2,
  search_node_count: 4,
}
const summary = {
  model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  elapsed_ms: 10,
  counts,
}
const local: AssignedLocalSufficiencySummaryResponseV1 = {
  version: 1,
  projectInstanceId: 'instance',
  projectId: 'project',
  revision: 2,
  foldModelFingerprint: 'a'.repeat(64),
  vertices: [
    { vertex: 'v1', status: 'necessary_failed' },
    {
      vertex: 'v2',
      status: 'sufficient_proven',
      model_id: LOCAL_SUFFICIENCY_CERTIFICATE_MODEL,
      reduction_steps: 1,
    },
    { vertex: 'v3', status: 'indeterminate', reason: 'cancelled' },
  ],
  totalReductionSteps: 1,
  authorizesProjectMutation: false,
}

test('global and local proof scopes remain separate and deterministic', () => {
  const possible = createProofScopePresentation({
    state: 'completed',
    result: {
      verdict: 'possible',
      summary,
      layer_order: {
        model_id: 'facewise_layer_order_v1',
        layer_count: 3,
        max_ply: 2,
        reference_face_number: 1,
        layer_view_available: true,
      },
    },
  }, local)
  assert.equal(possible.diagnostics.schema, PROOF_SCOPE_DIAGNOSTICS_SCHEMA)
  assert.equal(possible.diagnostics.global.status, 'possible')
  assert.equal(possible.diagnostics.local.necessaryFailed, 1)
  assert.equal(possible.diagnostics.local.sufficientProven, 1)
  assert.equal(possible.diagnostics.local.indeterminate, 1)
  assert.match(possible.diagnosticsJson, /"readOnly": true/u)
  assert.doesNotMatch(possible.diagnosticsJson, /timestamp|projectId|fingerprint|"v[123]"/iu)
  assert.equal(
    possible.diagnosticsJson,
    createProofScopePresentation({
      state: 'completed',
      result: {
        verdict: 'possible',
        summary,
        layer_order: {
          model_id: 'facewise_layer_order_v1',
          layer_count: 3,
          max_ply: 2,
          reference_face_number: 1,
          layer_view_available: true,
        },
      },
    }, structuredClone(local)).diagnosticsJson,
  )
  assert.ok(Object.isFrozen(possible.diagnostics))
  assert.ok(Object.isFrozen(possible.diagnostics.global.unproven))
})

test('possible impossible and unknown never derive from local sufficiency', () => {
  const jobs = [
    [null, 'not_checked'],
    [{
      state: 'running',
      cancel_requested: false,
      progress: {
        model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
        phase: 'searching',
        completed_work: 1,
        total_work: null,
        elapsed_ms: 1,
        counts,
      },
    }, 'in_progress'],
    [{
      state: 'completed',
      result: { verdict: 'unknown', summary, reason: 'proof_not_completed' },
    }, 'unknown'],
  ] as const
  for (const [job, expected] of jobs) {
    assert.equal(
      createProofScopePresentation(job, local).diagnostics.global.status,
      expected,
    )
  }
  assert.equal(
    createProofScopePresentation(null, {
      ...local,
      vertices: local.vertices.filter((item) => item.status === 'sufficient_proven'),
    }).diagnostics.global.status,
    'not_checked',
  )
})

test('hostile global input fails closed and visible related vertices are bounded', () => {
  const vertices = Array.from({ length: PROOF_SCOPE_VISIBLE_VERTEX_LIMIT + 5 }, (_, index) => ({
    vertex: `vertex-${index}`,
    status: 'necessary_failed' as const,
  }))
  const presentation = createProofScopePresentation(
    { state: 'completed', raw_error: 'C:\\Users\\alice\\private.ori' },
    { ...local, vertices, totalReductionSteps: 0 },
  )
  assert.equal(presentation.diagnostics.global.status, 'unavailable')
  assert.equal(presentation.selectableVertices.length, PROOF_SCOPE_VISIBLE_VERTEX_LIMIT)
  assert.equal(presentation.hiddenVertexCount, 5)
  assert.doesNotMatch(presentation.diagnosticsJson, /alice|private|vertex-/u)
})
