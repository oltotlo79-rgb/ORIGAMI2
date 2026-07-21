import assert from 'node:assert/strict'
import test from 'node:test'

import {
  ProjectFileClientError,
  createProjectFileClient,
  normalizeProjectFileResponse,
} from '../src/lib/projectFileClient.ts'
const INSTANCE_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_ID = '20000000-0000-4000-8000-000000000002'

test('normal project operations expose no path and strictly admit a pathless snapshot', async () => {
  const calls: string[] = []
  const client = createProjectFileClient(async command => {
    calls.push(command)
    return { canceled: false, project: validProjectSnapshot() }
  })

  const response = await client.run('save_as')
  assert.equal(response.canceled, false)
  assert.deepEqual(calls, ['save_project_as'])
  assert.equal(response.project.current_path, null)
  assert.doesNotMatch(JSON.stringify(response), /file_name|directory|secret\.ori2/ui)
  assert.ok(Object.isFrozen(response))
})

function validProjectSnapshot() {
  return {
    project_instance_id: INSTANCE_ID, project_id: PROJECT_ID,
    name: 'Project', memo: '', current_path: null, revision: 4, saved_revision: 4,
    is_dirty: false,
    paper: {
      boundary_vertices: [], thickness_mm: 0.1, length_display_unit: 'mm',
      cutting_allowed: false,
      front: { color: { red: 255, green: 255, blue: 255, alpha: 255 }, texture_asset: null },
      back: { color: { red: 248, green: 248, blue: 245, alpha: 255 }, texture_asset: null },
    },
    crease_pattern: { vertices: [], edges: [] },
    instruction_timeline: { steps: [] }, numeric_expressions: {},
    geometric_constraints: { schema_version: 1, constraints: [] },
    beginner_design_profile: {
      schema_version: 1, preset: 'balanced', shape_fidelity_weight: 35,
      foldability_weight: 35, step_count_weight: 15, paper_efficiency_weight: 15,
      generation_constraints: {
        schema_version: 1, maximum_steps: 60, detail_level: 'standard',
        target_category: null, target_parts: [], skeleton_segments: [], protrusions: [],
        bulge_targets: [], target_asset: null, allowed_techniques: ['valley_fold', 'mountain_fold'],
      },
    },
    project_layers: {
      schema_version: 1,
      layers: [{
        id: '00000000-0000-4000-8000-000000000001', name: 'Crease Pattern',
        content_kind: 'crease_pattern', visible: true, locked: false, opacity: 1,
      }],
      edge_assignments: [],
    },
    element_metadata: { vertices: [], edges: [], faces: [] },
    annotations: { schema_version: 1, annotations: [] },
    underlays: { schema_version: 1, underlays: [] },
    fold_model_fingerprint: 'a'.repeat(64), can_undo: false, can_redo: false,
    cutting_allowed: false,
  }
}

test('unknown fields, malformed snapshots, and path-bearing envelopes fail closed', () => {
  const snapshot = validProjectSnapshot()
  for (const value of [
    { canceled: false, project: snapshot, path: 'secret.ori2' },
    { canceled: false, project: { ...snapshot, revision: -1 } },
    { canceled: 'false', project: snapshot },
    { canceled: false, project: snapshot, command: 'run-script' },
  ]) {
    assert.throws(
      () => normalizeProjectFileResponse(value),
      (error: unknown) => error instanceof ProjectFileClientError
        && error.code === 'invalid_response',
    )
  }
})

test('concurrent operations are rejected and a failed operation releases the session lock', async () => {
  let release!: () => void
  const held = new Promise<void>(resolve => { release = resolve })
  let invocation = 0
  const client = createProjectFileClient(async () => {
    invocation += 1
    if (invocation === 1) {
      await held
      throw new Error('disk')
    }
    return { canceled: true, project: validProjectSnapshot() }
  })

  const first = client.run('open')
  await assert.rejects(
    client.run('save'),
    (error: unknown) => error instanceof ProjectFileClientError && error.code === 'busy',
  )
  release()
  await assert.rejects(first, /disk/u)
  assert.equal((await client.run('save')).canceled, true)
})
