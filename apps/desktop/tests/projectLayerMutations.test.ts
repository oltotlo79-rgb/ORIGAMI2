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
import {
  BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
  BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
} from '../src/lib/boundaryLengthAuthority.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from '../src/lib/deterministicTranscendentalModel.ts'

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

test('rejects a native layer mutation that alters only reference consensus', () => {
  const base = validBaseSnapshot()
  const response = validSnapshot()
  const baseConsensus = referenceConsensusV1()
  const responseConsensus = referenceConsensusV1()
  Object.assign(base.beginner_design_profile, {
    reference_consensus_v1: baseConsensus,
  })
  Object.assign(response.beginner_design_profile, {
    reference_consensus_v1: responseConsensus,
  })

  assert.ok(normalizeProjectLayerMutationSnapshot(response, base))

  responseConsensus.bindings[0]!.sha256[0] = 9
  assert.equal(normalizeProjectLayerMutationSnapshot(response, base), null)
})

test('requires the exact native unproven-history summary and preserves it', () => {
  const base = validBaseSnapshot()
  const source = validSnapshot()
  source.speculativeUnprovenFolds.applied.awaitingProof = 1
  assert.equal(normalizeProjectLayerMutationSnapshot(source, base), null)

  const missing = validSnapshot() as Record<string, unknown>
  delete missing.speculativeUnprovenFolds
  assert.equal(normalizeProjectLayerMutationSnapshot(missing, base), null)

  const unknown = validSnapshot()
  Object.assign(unknown.speculativeUnprovenFolds.applied, {
    futureCertified: 1,
  })
  assert.equal(normalizeProjectLayerMutationSnapshot(unknown, base), null)

  let getterCalls = 0
  const accessor = validSnapshot()
  Object.defineProperty(
    accessor.speculativeUnprovenFolds.applied,
    'awaitingProof',
    {
      enumerable: true,
      get() {
        getterCalls += 1
        throw new Error('private native detail')
      },
    },
  )
  assert.equal(normalizeProjectLayerMutationSnapshot(accessor, base), null)
  assert.equal(getterCalls, 0)

  const admitted = normalizeProjectLayerMutationSnapshot(
    validSnapshot(),
    base,
  )
  assert.deepEqual(
    admitted?.speculativeUnprovenFolds,
    base.speculativeUnprovenFolds,
  )
  assert.notEqual(
    admitted?.speculativeUnprovenFolds,
    base.speculativeUnprovenFolds,
  )
})

test('requires a refreshed, immutable-geometry boundary length authority', () => {
  const base = validBaseSnapshot()
  const missing = validSnapshot() as Record<string, unknown>
  delete missing.boundary_length_authority_v1
  assert.equal(normalizeProjectLayerMutationSnapshot(missing, base), null)

  const stale = validSnapshot()
  stale.boundary_length_authority_v1.revision = 7
  assert.equal(normalizeProjectLayerMutationSnapshot(stale, base), null)

  const changedStatus = validSnapshot()
  changedStatus.boundary_length_authority_v1.status = 'available'
  assert.equal(normalizeProjectLayerMutationSnapshot(changedStatus, base), null)

  const invalidBase = validBaseSnapshot() as Record<string, unknown>
  delete invalidBase.boundary_length_authority_v1
  assert.equal(
    normalizeProjectLayerMutationSnapshot(
      validSnapshot(),
      invalidBase as never,
    ),
    null,
  )

  const admitted = normalizeProjectLayerMutationSnapshot(
    validSnapshot(),
    base,
  )
  assert.ok(admitted)
  assert.notEqual(
    admitted.boundary_length_authority_v1,
    base.boundary_length_authority_v1,
  )
  assert.equal(
    (
      admitted.boundary_length_authority_v1 as
        ReturnType<typeof unavailableBoundaryLengthAuthority>
    ).revision,
    8,
  )
})

test('strictly admits the optional path-certificate DTO and rejects unknown or malformed fields', () => {
  const valid = validSnapshot()
  const validBase = validBaseSnapshot()
  const sourceReference = proofReference()
  validBase.instruction_timeline = proofTimeline(
    sourceReference,
  ) as typeof validBase.instruction_timeline
  const admitted = normalizeProjectLayerMutationSnapshot(valid, validBase)
  assert.ok(admitted)
  const admittedReference = admitted.instruction_timeline.steps[0]!
    .visual.path_certificate_reference_v1
  assert.ok(admittedReference)
  assert.ok(Object.isFrozen(admittedReference))
  assert.ok(Object.isFrozen(admittedReference.binding_sha256))
  sourceReference.binding_sha256[0] = 9
  assert.equal(admittedReference.binding_sha256[0], 1)

  for (const reference of [
    { ...proofReference(), future: true },
    { ...proofReference(), transition_count: 0 },
    { ...proofReference(), binding_sha256: [1] },
    { ...proofReference(), binding_sha256: [-1, ...Array(31).fill(1)] },
    { ...proofReference(), binding_sha256: Array(32) },
    { ...proofReference(), target_pose_sha256: Array(32).fill(2) },
  ]) {
    const invalid = validBaseSnapshot()
    invalid.instruction_timeline = proofTimeline(reference) as typeof invalid.instruction_timeline
    assert.equal(
      normalizeProjectLayerMutationSnapshot(validSnapshot(), invalid),
      null,
    )
  }
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
  assert.deepEqual(admitted.instruction_timeline, base.instruction_timeline)
  assert.notEqual(admitted.instruction_timeline, base.instruction_timeline)
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
    memo: '',
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
    instruction_timeline: { steps: [] },
    numeric_expressions: {},
    geometric_constraints: {},
    beginner_design_profile: {
      schema_version: 1,
      preset: 'balanced',
      shape_fidelity_weight: 35,
      foldability_weight: 35,
      step_count_weight: 15,
      paper_efficiency_weight: 15,
      generation_constraints: {
        schema_version: 1,
        maximum_steps: 60,
        detail_level: 'standard',
        target_category: null,
        target_parts: [],
        skeleton_segments: [],
        protrusions: [],
        bulge_targets: [],
        target_asset: null,
        allowed_techniques: ['valley_fold', 'mountain_fold'],
      },
    },
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
    element_metadata: {
      vertices: [],
      edges: [],
      faces: [],
    },
    annotations: {},
    underlays: {},
    fold_model_fingerprint: 'a'.repeat(64),
    reference_model_assets: [],
    boundary_length_authority_v1: unavailableBoundaryLengthAuthority(8),
    can_undo: true,
    can_redo: false,
    cutting_allowed: false,
    speculativeUnprovenFolds: {
      applied: {
        awaitingProof: 0,
        proofBlocked: 0,
        unknownEvidenceInsufficient: 0,
        unknownResourceLimit: 0,
        unknownCancelled: 0,
        unknownDeadlineReached: 0,
      },
      unappliedRedo: {
        awaitingProof: 0,
        proofBlocked: 0,
        unknownEvidenceInsufficient: 0,
        unknownResourceLimit: 0,
        unknownCancelled: 0,
        unknownDeadlineReached: 0,
      },
    },
  }
}

function proofReference() {
  return {
    version: 1,
    model_id: 'bounded_certified_pose_graph_path_reference_v1',
    binding_sha256: Array(32).fill(1),
    source_pose_sha256: Array(32).fill(2),
    target_pose_sha256: Array(32).fill(3),
    source_model_binding_sha256: Array(32).fill(4),
    transition_count: 1,
  }
}

function referenceConsensusV1() {
  return {
    schema_version: 1 as const,
    bindings: [
      {
        kind: 'image' as const,
        asset_id: '70000000-0000-4000-8000-000000000001',
        sha256: Array(32).fill(1),
        quality: 90,
      },
      {
        kind: 'reference_model' as const,
        asset_id: '80000000-0000-4000-8000-000000000001',
        sha256: Array(32).fill(2),
        quality: 80,
      },
    ],
    excluded_asset_id: '80000000-0000-4000-8000-000000000001',
  }
}

function proofTimeline(reference: Record<string, unknown> = proofReference()) {
  return {
    steps: [{
      visual: { path_certificate_reference_v1: reference },
    }],
  }
}

function validBaseSnapshot() {
  return {
    ...validSnapshot(),
    revision: 7,
    is_dirty: false,
    can_undo: false,
    boundary_length_authority_v1: unavailableBoundaryLengthAuthority(7),
  }
}

function unavailableBoundaryLengthAuthority(revision: number) {
  return {
    schema_version: BOUNDARY_LENGTH_AUTHORITY_SCHEMA_VERSION_V1,
    model_id: BOUNDARY_LENGTH_AUTHORITY_MODEL_ID_V1,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    project_instance_id: INSTANCE_ID,
    project_id: PROJECT_ID,
    revision,
    status: 'unavailable',
    entries: [],
  }
}
