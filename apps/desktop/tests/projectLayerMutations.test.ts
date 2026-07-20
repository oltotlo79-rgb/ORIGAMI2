import assert from 'node:assert/strict'
import test from 'node:test'

import {
  admitProjectLayerMutationSnapshot,
  assignEdgeToProjectLayer,
  createProjectLayer,
  deleteProjectLayer,
  moveProjectLayer,
  normalizeProjectLayerMutationSnapshot,
  ProjectLayerMutationError,
  renameProjectLayer,
  updateProjectLayerPresentation,
} from '../src/lib/coreClient.ts'
import { DEFAULT_PROJECT_LAYER_ID } from '../src/lib/projectLayers.ts'

const INSTANCE_ID = '10000000-0000-4000-8000-000000000001'
const PROJECT_ID = '20000000-0000-4000-8000-000000000001'
const LAYER_ID = 'abcdef00-0000-4000-8000-000000000001'
const EDGE_ID = '40000000-0000-4000-8000-000000000001'

test('strictly admits and detaches one exact layer mutation snapshot', () => {
  const source = validSnapshot()
  const base = validBaseSnapshot()
  const admitted = admitProjectLayerMutationSnapshot(
    source,
    base,
    INSTANCE_ID,
    PROJECT_ID,
    7,
  )

  assert.equal(admitted.revision, 8)
  assert.equal(admitted.project_layers.layers[1]?.name, 'Details')
  assert.ok(Object.isFrozen(admitted))
  assert.ok(Object.isFrozen(admitted.project_layers))
  assert.ok(Object.isFrozen(admitted.project_layers.layers[1]))

  source.project_layers.layers[1]!.name = 'changed after admission'
  assert.equal(admitted.project_layers.layers[1]?.name, 'Details')
})

test('admits native mutation snapshots with independently omitted presentation defaults', () => {
  const source = validSnapshot()
  source.project_layers.layers[1] = {
    id: LAYER_ID,
    name: 'Details',
    content_kind: 'crease_pattern',
    locked: true,
  } as typeof source.project_layers.layers[number]

  const admitted = admitProjectLayerMutationSnapshot(
    source,
    validBaseSnapshot(),
    INSTANCE_ID,
    PROJECT_ID,
    7,
  )
  assert.deepEqual(admitted.project_layers.layers[1], {
    id: LAYER_ID,
    name: 'Details',
    content_kind: 'crease_pattern',
    visible: true,
    locked: true,
    opacity: 1,
  })
})

test('rejects malformed and hostile native layer mutation snapshots', () => {
  const base = validBaseSnapshot()
  let getterCalls = 0
  const accessor = validSnapshot()
  Object.defineProperty(accessor, 'project_layers', {
    enumerable: true,
    get() {
      getterCalls += 1
      return validSnapshot().project_layers
    },
  })

  for (const invalid of [
    null,
    { ...validSnapshot(), future: true },
    { ...validSnapshot(), fold_model_fingerprint: 'private-native-value' },
    {
      ...validSnapshot(),
      project_layers: {
        ...validSnapshot().project_layers,
        edge_assignments: [{
          edge: '50000000-0000-4000-8000-000000000001',
          layer: LAYER_ID,
        }],
      },
    },
    accessor,
  ]) {
    assert.equal(
      normalizeProjectLayerMutationSnapshot(invalid, base),
      null,
    )
  }
  assert.equal(getterCalls, 0)
})

test('distinguishes a malformed response from a stale project binding', () => {
  const base = validBaseSnapshot()
  assert.throws(
    () => admitProjectLayerMutationSnapshot(
      { private: 'native details' },
      base,
      INSTANCE_ID,
      PROJECT_ID,
      7,
    ),
    (error) => {
      assert.ok(error instanceof ProjectLayerMutationError)
      assert.equal(error.code, 'invalid_response')
      assert.doesNotMatch(error.message, /private|native details/u)
      assert.equal(error.cause, undefined)
      return true
    },
  )

  const foreignResponse = {
    ...validSnapshot(),
    project_instance_id: '50000000-0000-4000-8000-000000000001',
  }
  assert.throws(
    () => admitProjectLayerMutationSnapshot(
      foreignResponse,
      base,
      INSTANCE_ID,
      PROJECT_ID,
      7,
    ),
    (error) => (
      error instanceof ProjectLayerMutationError
      && error.code === 'stale_response'
    ),
  )
  assert.throws(
    () => admitProjectLayerMutationSnapshot(
      {
        ...validSnapshot(),
        revision: 9,
      },
      base,
      INSTANCE_ID,
      PROJECT_ID,
      7,
    ),
    (error) => (
      error instanceof ProjectLayerMutationError
      && error.code === 'stale_response'
    ),
  )
})

test('never adopts unverified nested response objects', () => {
  const base = validBaseSnapshot()
  const response = validSnapshot()
  let nestedGetterCalls = 0
  response.paper = {
    get thickness_mm() {
      nestedGetterCalls += 1
      throw new Error('private native paper value')
    },
  }
  response.instruction_timeline = {
    get steps() {
      nestedGetterCalls += 1
      throw new Error('private native timeline value')
    },
  }

  const admitted = admitProjectLayerMutationSnapshot(
    response,
    base,
    INSTANCE_ID,
    PROJECT_ID,
    7,
  )
  assert.equal(nestedGetterCalls, 0)
  assert.equal(admitted.paper, base.paper)
  assert.equal(admitted.crease_pattern, base.crease_pattern)
  assert.equal(admitted.instruction_timeline, base.instruction_timeline)
  assert.equal(admitted.numeric_expressions, base.numeric_expressions)
  assert.equal(admitted.geometric_constraints, base.geometric_constraints)
  assert.notEqual(admitted.project_layers, base.project_layers)
})

test('all six wrappers reject unsafe requests before native invocation', async () => {
  const base = validBaseSnapshot()
  const invalidRequests = [
    () => createProjectLayer(
      'not-a-project',
      7,
      INSTANCE_ID,
      base,
      'Details',
      'crease_pattern',
    ),
    () => createProjectLayer(
      PROJECT_ID,
      7,
      INSTANCE_ID,
      base,
      '   ',
      'crease_pattern',
    ),
    () => createProjectLayer(
      PROJECT_ID,
      7,
      INSTANCE_ID,
      base,
      'Details',
      'future' as 'crease_pattern',
    ),
    () => renameProjectLayer(
      PROJECT_ID,
      7,
      INSTANCE_ID,
      base,
      LAYER_ID.toUpperCase(),
      'Details',
    ),
    () => updateProjectLayerPresentation(
      PROJECT_ID,
      7,
      INSTANCE_ID,
      base,
      LAYER_ID,
      true,
      false,
      Number.NaN,
    ),
    () => moveProjectLayer(
      PROJECT_ID,
      7,
      INSTANCE_ID,
      base,
      LAYER_ID,
      -1,
    ),
    () => deleteProjectLayer(
      PROJECT_ID,
      7,
      INSTANCE_ID,
      base,
      '00000000-0000-0000-0000-000000000000',
    ),
    () => assignEdgeToProjectLayer(
      PROJECT_ID,
      Number.MAX_SAFE_INTEGER,
      INSTANCE_ID,
      base,
      EDGE_ID,
      LAYER_ID,
    ),
  ]

  for (const request of invalidRequests) {
    await assert.rejects(request(), (error) => (
      error instanceof ProjectLayerMutationError
      && error.code === 'invalid_request'
    ))
  }
})

function validSnapshot() {
  return {
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    name: 'Layer test',
    current_path: null,
    revision: 8,
    saved_revision: null,
    is_dirty: true,
    paper: {},
    crease_pattern: {
      vertices: [],
      edges: [{
        id: EDGE_ID,
        start: '50000000-0000-4000-8000-000000000001',
        end: '60000000-0000-4000-8000-000000000001',
        kind: 'mountain',
      }],
    },
    instruction_timeline: {},
    numeric_expressions: {},
    geometric_constraints: {},
    project_layers: {
      schema_version: 1,
      layers: [
        {
          id: DEFAULT_PROJECT_LAYER_ID,
          name: 'Crease Pattern',
          content_kind: 'crease_pattern' as const,
          visible: true,
          locked: false,
          opacity: 1,
        },
        {
          id: LAYER_ID,
          name: 'Details',
          content_kind: 'crease_pattern' as const,
          visible: true,
          locked: false,
          opacity: 1,
        },
      ],
      edge_assignments: [{
        edge: EDGE_ID,
        layer: LAYER_ID,
      }],
    },
    fold_model_fingerprint: 'a'.repeat(64),
    can_undo: true,
    can_redo: false,
    cutting_allowed: false,
  }
}

function validBaseSnapshot() {
  return {
    ...validSnapshot(),
    revision: 7,
    is_dirty: false,
    can_undo: false,
  }
}
