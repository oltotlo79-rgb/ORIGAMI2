import assert from 'node:assert/strict'
import test from 'node:test'

import {
  DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET,
  GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
  GLOBAL_FLAT_FOLDABILITY_LIMITS,
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  GLOBAL_FLAT_FOLDABILITY_PROOF_FACE_LIMIT,
  GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS,
  isGlobalFlatFoldabilityTimePreset,
  normalizeGlobalFlatFoldabilityTimePreset,
  parseGlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityErrorCategory,
  type GlobalFlatFoldabilityUnknownReason,
} from '../src/lib/globalFlatFoldability.ts'

const COUNTS = {
  face_count: 12,
  overlap_cell_count: 34,
  constraint_count: 56,
  search_node_count: 78,
}

const SUMMARY = {
  model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  elapsed_ms: 1_250,
  counts: COUNTS,
}

const PROGRESS = {
  model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  phase: 'searching',
  completed_work: 78,
  total_work: null,
  elapsed_ms: 1_000,
  counts: COUNTS,
}

test('time presets are the closed 5/30/120-second set with a 30-second default', () => {
  assert.deepEqual([...GLOBAL_FLAT_FOLDABILITY_TIME_PRESETS], [5, 30, 120])
  assert.equal(DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET, 30)
  for (const preset of [5, 30, 120]) {
    assert.equal(isGlobalFlatFoldabilityTimePreset(preset), true)
    assert.equal(normalizeGlobalFlatFoldabilityTimePreset(preset), preset)
  }
  for (const rejected of [0, 1, 29, 300, '30', null, {}, Number.NaN]) {
    assert.equal(isGlobalFlatFoldabilityTimePreset(rejected), false)
    assert.equal(
      normalizeGlobalFlatFoldabilityTimePreset(rejected),
      DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET,
    )
  }
})

test('queued and running jobs retain only bounded progress data', () => {
  const queued = {
    state: 'queued',
    cancel_requested: false,
    progress: { ...PROGRESS, phase: 'capturing' },
  }
  const running = {
    state: 'running',
    cancel_requested: true,
    progress: PROGRESS,
  }

  const parsedQueued = parseGlobalFlatFoldabilityJobDto(queued)
  const parsedRunning = parseGlobalFlatFoldabilityJobDto(running)
  assert.deepEqual(parsedQueued, queued)
  assert.deepEqual(parsedRunning, running)
  assert.ok(parsedQueued && Object.isFrozen(parsedQueued))
  assert.ok(parsedRunning && Object.isFrozen(parsedRunning.progress.counts))
  assert.notEqual(parsedRunning?.progress, PROGRESS)
  assert.equal(
    parseGlobalFlatFoldabilityJobDto({
      ...queued,
      progress: { ...PROGRESS, phase: 'searching' },
    }),
    null,
    'queued jobs must remain in the capturing phase',
  )
})

test('completed possible, impossible and every unknown reason stay distinct', () => {
  const possible = {
    state: 'completed',
    result: {
      verdict: 'possible',
      summary: SUMMARY,
      layer_order: {
        model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
        layer_count: 12,
        max_ply: 4,
        reference_face_number: 1,
        layer_view_available: true,
      },
    },
  }
  const impossible = {
    state: 'completed',
    result: {
      verdict: 'impossible',
      summary: SUMMARY,
      proof: {
        category: 'layer_constraints_contradictory',
        face_numbers: [2, 9],
      },
    },
  }
  const unknownReasons: readonly GlobalFlatFoldabilityUnknownReason[] = [
    'unsupported_topology',
    'non_convex_face',
    'time_limit_reached',
    'work_limit_reached',
    'exact_number_limit_reached',
    'overlap_arrangement_limit_reached',
    'constraint_limit_reached',
    'proof_not_completed',
    'local_conditions_indeterminate',
  ]

  assert.deepEqual(parseGlobalFlatFoldabilityJobDto(possible), possible)
  assert.deepEqual(parseGlobalFlatFoldabilityJobDto(impossible), impossible)
  for (const reason of unknownReasons) {
    const unknown = {
      state: 'completed',
      result: { verdict: 'unknown', summary: SUMMARY, reason },
    }
    assert.deepEqual(parseGlobalFlatFoldabilityJobDto(unknown), unknown)
  }
})

test('cancelled, failed and stale are separate job states rather than verdicts', () => {
  assert.deepEqual(
    parseGlobalFlatFoldabilityJobDto({ state: 'cancelled', summary: SUMMARY }),
    { state: 'cancelled', summary: SUMMARY },
  )
  assert.deepEqual(
    parseGlobalFlatFoldabilityJobDto({ state: 'stale', summary: SUMMARY }),
    { state: 'stale', summary: SUMMARY },
  )
  const categories: readonly GlobalFlatFoldabilityErrorCategory[] = [
    'invalid_request',
    'snapshot_unavailable',
    'worker_unavailable',
    'result_unavailable',
    'internal_failure',
  ]
  for (const error_category of categories) {
    const failed = { state: 'failed', summary: SUMMARY, error_category }
    assert.deepEqual(parseGlobalFlatFoldabilityJobDto(failed), failed)
  }

  assert.equal(
    parseGlobalFlatFoldabilityJobDto({
      state: 'completed',
      result: {
        verdict: 'cancelled',
        summary: SUMMARY,
      },
    }),
    null,
  )
  assert.equal(
    parseGlobalFlatFoldabilityJobDto({
      state: 'completed',
      result: {
        verdict: 'possible',
        summary: SUMMARY,
        layer_order: {
          model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
          layer_count: 11,
          max_ply: 2,
          reference_face_number: 1,
          layer_view_available: true,
        },
      },
    }),
    null,
  )
})

test('all visible work counts accept the exact design limit and reject limit + 1', () => {
  const exactCounts = {
    face_count: GLOBAL_FLAT_FOLDABILITY_LIMITS.materialFaceCount,
    overlap_cell_count: GLOBAL_FLAT_FOLDABILITY_LIMITS.overlapCellCount,
    constraint_count: GLOBAL_FLAT_FOLDABILITY_LIMITS.constraintCount,
    search_node_count: GLOBAL_FLAT_FOLDABILITY_LIMITS.searchNodeCount,
  }
  const exact = {
    state: 'completed',
    result: {
      verdict: 'unknown',
      summary: { ...SUMMARY, counts: exactCounts },
      reason: 'work_limit_reached',
    },
  }
  assert.ok(parseGlobalFlatFoldabilityJobDto(exact))

  const bounds = [
    ['face_count', GLOBAL_FLAT_FOLDABILITY_LIMITS.materialFaceCount],
    ['overlap_cell_count', GLOBAL_FLAT_FOLDABILITY_LIMITS.overlapCellCount],
    ['constraint_count', GLOBAL_FLAT_FOLDABILITY_LIMITS.constraintCount],
    ['search_node_count', GLOBAL_FLAT_FOLDABILITY_LIMITS.searchNodeCount],
  ] as const
  for (const [key, maximum] of bounds) {
    assert.equal(
      parseGlobalFlatFoldabilityJobDto({
        ...exact,
        result: {
          ...exact.result,
          summary: {
            ...exact.result.summary,
            counts: { ...exactCounts, [key]: maximum + 1 },
          },
        },
      }),
      null,
      `${key} must reject its design limit + 1`,
    )
  }
})

test('progress and proof lists are internally consistent and bounded', () => {
  assert.equal(
    parseGlobalFlatFoldabilityJobDto({
      state: 'running',
      cancel_requested: false,
      progress: { ...PROGRESS, completed_work: 4, total_work: 3 },
    }),
    null,
  )
  assert.equal(
    parseGlobalFlatFoldabilityJobDto({
      state: 'completed',
      result: {
        verdict: 'possible',
        summary: SUMMARY,
        layer_order: {
          model_id: GLOBAL_FLAT_FOLDABILITY_LAYER_ORDER_MODEL_ID,
          layer_count: 13,
          max_ply: 2,
          reference_face_number: 1,
          layer_view_available: true,
        },
      },
    }),
    null,
  )
  assert.equal(
    parseGlobalFlatFoldabilityJobDto(impossibleWithFaces([])),
    null,
  )
  assert.equal(
    parseGlobalFlatFoldabilityJobDto(impossibleWithFaces([1, 1])),
    null,
  )
  assert.equal(
    parseGlobalFlatFoldabilityJobDto(impossibleWithFaces([2, 1])),
    null,
  )
  assert.equal(
    parseGlobalFlatFoldabilityJobDto(impossibleWithFaces([13])),
    null,
  )
  assert.equal(
    parseGlobalFlatFoldabilityJobDto(
      impossibleWithFaces(
        Array.from(
          { length: GLOBAL_FLAT_FOLDABILITY_PROOF_FACE_LIMIT + 1 },
          (_, index) => (index % COUNTS.face_count) + 1,
        ),
      ),
    ),
    null,
  )
})

test('the parser fails closed on unknown categories, extra private fields and coordinates', () => {
  const validRunning = {
    state: 'running',
    cancel_requested: false,
    progress: PROGRESS,
  }
  const privatePath = 'C:\\Users\\alice\\秘密の作品.ori'
  const rejected = [
    { ...validRunning, raw_error: privatePath },
    { ...validRunning, job_id: 'opaque-but-owned-by-controller' },
    { ...validRunning, internal_id: 'search-stack-12' },
    {
      ...validRunning,
      progress: { ...PROGRESS, coordinates: [[12.3, 45.6]] },
    },
    {
      ...validRunning,
      progress: {
        ...PROGRESS,
        counts: { ...COUNTS, raw_face_ids: ['private-face-id'] },
      },
    },
    {
      ...validRunning,
      progress: { ...PROGRESS, model_id: 'future_model' },
    },
    {
      state: 'completed',
      result: {
        verdict: 'unknown',
        summary: SUMMARY,
        reason: 'timeout_with_backend_detail',
      },
    },
    {
      state: 'failed',
      summary: SUMMARY,
      error_category: 'panic',
    },
    { state: 'future_state', summary: SUMMARY },
  ]
  for (const value of rejected) {
    assert.equal(parseGlobalFlatFoldabilityJobDto(value), null)
  }
})

test('hostile accessors, proxies, symbols and non-plain objects do not escape validation', () => {
  let getterCalled = false
  const accessor = Object.create(null) as Record<string, unknown>
  Object.defineProperty(accessor, 'state', {
    enumerable: true,
    get() {
      getterCalled = true
      throw new Error('C:\\Users\\alice\\private.ori')
    },
  })
  assert.equal(parseGlobalFlatFoldabilityJobDto(accessor), null)
  assert.equal(getterCalled, false)

  const proxy = new Proxy({}, {
    ownKeys() {
      throw new Error('private search stack')
    },
  })
  assert.equal(parseGlobalFlatFoldabilityJobDto(proxy), null)

  const symbolValue = {
    state: 'cancelled',
    summary: SUMMARY,
    [Symbol('raw-error')]: 'private',
  }
  assert.equal(parseGlobalFlatFoldabilityJobDto(symbolValue), null)
  assert.equal(parseGlobalFlatFoldabilityJobDto(new Date()), null)
})

function impossibleWithFaces(face_numbers: readonly number[]) {
  return {
    state: 'completed',
    result: {
      verdict: 'impossible',
      summary: SUMMARY,
      proof: {
        category: 'local_conditions_violated',
        face_numbers,
      },
    },
  }
}
