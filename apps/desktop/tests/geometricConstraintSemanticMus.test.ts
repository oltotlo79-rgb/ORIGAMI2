import assert from 'node:assert/strict'
import test from 'node:test'

import {
  MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS,
  MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
  MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK,
  normalizeGeometricConstraintPreflightResponse,
} from '../src/lib/geometricConstraints.ts'
import {
  BINDING,
  certified,
  CORE,
  directResult,
  envelope,
  IDS,
  provenDirect,
  unknown,
  unknownDirect,
  uuid,
} from './geometricConstraintSemanticMusTestSupport.ts'

test('accepts legacy four-key envelopes without creating a semantic claim', () => {
  const legacy = {
    ...BINDING,
    result: directResult(provenDirect()),
  }
  const normalized = normalizeGeometricConstraintPreflightResponse(
    legacy,
    BINDING,
  )
  assert.deepEqual(normalized, legacy)
  assert.equal(Object.hasOwn(normalized ?? {}, 'semantic_mus'), false)

  const currentNonDirect = {
    ...BINDING,
    result: { status: 'no_direct_conflict' },
    semantic_mus: null,
  }
  assert.deepEqual(
    normalizeGeometricConstraintPreflightResponse(currentNonDirect, BINDING),
    currentNonDirect,
  )
  assert.equal(normalizeGeometricConstraintPreflightResponse({
    ...legacy,
    semantic_mus: null,
  }, BINDING), null)
  assert.equal(normalizeGeometricConstraintPreflightResponse({
    ...currentNonDirect,
    semantic_mus: certified(),
  }, BINDING), null)
  assert.equal(normalizeGeometricConstraintPreflightResponse({
    ...currentNonDirect,
    future: true,
  }, BINDING), null)
})

test('accepts and deeply freezes the exact certified semantic-MUS DTO', () => {
  const raw = envelope(certified(), provenDirect())
  const before = structuredClone(raw)
  const normalized = normalizeGeometricConstraintPreflightResponse(raw, BINDING)

  assert.deepEqual(normalized, raw)
  assert.deepEqual(raw, before)
  assertDeepFrozen(normalized)
  assert.equal(
    normalized?.semantic_mus?.status === 'certified'
      ? normalized.semantic_mus.single_constraint_constructive_witness_count
      : null,
    1,
  )
  assert.equal(MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS, 16)
  assert.equal(MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK, 20_000_000)
})

test('certified semantic-MUS parsing rejects malformed evidence and cross-check mismatches', () => {
  const missing = certified()
  delete missing.single_constraint_constructive_witness_count
  const tooManyIds = IDS.slice(0, 17)
  const invalid = [
    missing,
    { ...certified(), future: true },
    { ...certified(), status: 'future' },
    { ...certified(), model_id: 'future_model' },
    { ...certified(), constraint_ids: [] },
    { ...certified(), constraint_ids: [CORE[1], CORE[0]] },
    { ...certified(), constraint_ids: [CORE[0], CORE[0]] },
    { ...certified(), constraint_ids: [uuid(0xabc).toUpperCase(), CORE[1]] },
    { ...certified(), constraint_ids: tooManyIds, constraint_count: 17 },
    { ...certified(), constraint_count: 1 },
    { ...certified(), constraint_count: -0 },
    { ...certified(), direct_oracle_calls: 0 },
    {
      ...certified(),
      direct_oracle_calls: MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS + 1,
    },
    { ...certified(), direct_oracle_calls: 1.5 },
    { ...certified(), deletion_witness_checks: 1 },
    {
      ...certified(),
      deletion_witness_work:
        MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK + 1,
    },
    { ...certified(), deletion_witness_work: -0 },
    { ...certified(), current_assignment_witness_count: -1 },
    { ...certified(), axis_exactification_witness_count: 3 },
    { ...certified(), single_constraint_constructive_witness_count: -0 },
    {
      ...certified(),
      current_assignment_witness_count: 1,
      axis_exactification_witness_count: 1,
      single_constraint_constructive_witness_count: 1,
    },
    { ...certified(), authorizes_project_mutation: true },
    { ...certified(), replayable_across_runtimes: true },
  ]
  for (const semanticMus of invalid) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        envelope(semanticMus, provenDirect()),
        BINDING,
      ),
      null,
    )
  }

  for (const [semanticMus, bounded] of [
    [
      certified({ constraint_ids: [CORE[0]], constraint_count: 1 }),
      provenDirect(),
    ],
    [certified({ direct_oracle_calls: 8 }), provenDirect()],
    [
      certified(),
      {
        status: 'unknown',
        reason: 'oracle_incomplete',
        oracle_calls: 7,
        max_constraints: 16,
      },
    ],
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        envelope(semanticMus, bounded),
        BINDING,
      ),
      null,
    )
  }
  const unmatchedOuterWitness = envelope(certified(), provenDirect())
  unmatchedOuterWitness.result.conflicts = [{
    conflict: { kind: 'different_fixed_lengths', edge: uuid(201) },
    constraint_ids: [IDS[2], IDS[3]],
  }]
  assert.equal(
    normalizeGeometricConstraintPreflightResponse(
      unmatchedOuterWitness,
      BINDING,
    ),
    null,
  )
})

test('accepts only reachable Unknown semantic-MUS phase states', () => {
  const afterDirect = unknown({
    reason: 'deletion_witness_unavailable',
    direct_core_constraint_ids: CORE,
    direct_oracle_calls: 7,
    deletion_witness_checks: 1,
    certified_deletion_witnesses: 0,
    deletion_witness_work: 99,
  })
  const incompleteDirect = unknown({
    reason: 'direct_oracle_incomplete',
    direct_core_constraint_ids: [],
    direct_oracle_calls: 4,
  })
  const deletionLimit = unknown({
    reason: 'deletion_witness_limit_exceeded',
    direct_core_constraint_ids: CORE,
    direct_oracle_calls: 7,
  })
  const setupWorkLimit = unknown({
    reason: 'deletion_witness_work_limit_exceeded',
    direct_core_constraint_ids: CORE,
    direct_oracle_calls: 7,
  })
  const partialWorkLimit = unknown({
    reason: 'deletion_witness_work_limit_exceeded',
    direct_core_constraint_ids: CORE,
    direct_oracle_calls: 7,
    deletion_witness_checks: 1,
    certified_deletion_witnesses: 0,
    deletion_witness_work: 99,
  })
  const cancelledBeforeDirect = unknown({
    reason: 'cancelled',
    direct_core_constraint_ids: [],
    direct_oracle_calls: 0,
  })
  for (const [semanticMus, bounded] of [
    [afterDirect, provenDirect()],
    [incompleteDirect, unknownDirect('oracle_incomplete', 4)],
    [deletionLimit, provenDirect()],
    [setupWorkLimit, provenDirect()],
    [partialWorkLimit, provenDirect()],
    [cancelledBeforeDirect, unknownDirect('cancelled', 0)],
  ]) {
    const normalized = normalizeGeometricConstraintPreflightResponse(
      envelope(semanticMus, bounded),
      BINDING,
    )
    assert.deepEqual(normalized?.semantic_mus, semanticMus)
    assertDeepFrozen(normalized)
  }
})

test('Unknown semantic-MUS parsing rejects impossible counters, phases, and legacy mismatches', () => {
  const valid = unknown({
    reason: 'deletion_witness_unavailable',
    direct_core_constraint_ids: CORE,
    direct_oracle_calls: 7,
    deletion_witness_checks: 1,
    certified_deletion_witnesses: 0,
    deletion_witness_work: 99,
  })
  const invalid = [
    { ...valid, future: true },
    { ...valid, model_id: 'future_model' },
    { ...valid, reason: 'future' },
    { ...valid, direct_core_constraint_ids: [CORE[1], CORE[0]] },
    { ...valid, direct_core_constraint_ids: [CORE[0], CORE[0]] },
    {
      ...valid,
      direct_core_constraint_ids: [uuid(0xabc).toUpperCase(), CORE[1]],
    },
    { ...valid, direct_core_constraint_ids: IDS.slice(0, 17) },
    { ...valid, direct_oracle_calls: -0 },
    { ...valid, direct_oracle_calls: MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS + 1 },
    { ...valid, deletion_witness_checks: 3 },
    { ...valid, deletion_witness_checks: 1.5 },
    { ...valid, certified_deletion_witnesses: 2 },
    { ...valid, certified_deletion_witnesses: -0 },
    {
      ...valid,
      deletion_witness_work:
        MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK + 1,
    },
    { ...valid, deletion_witness_work: -0 },
    { ...valid, max_deletion_witness_checks: 15 },
    { ...valid, max_deletion_witness_work: 19_999_999 },
    { ...valid, authorizes_project_mutation: true },
    { ...valid, replayable_across_runtimes: true },
    unknown({
      reason: 'deletion_witness_unavailable',
      direct_core_constraint_ids: [],
      direct_oracle_calls: 4,
    }),
    unknown({
      reason: 'direct_oracle_incomplete',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
    }),
    unknown({
      reason: 'deletion_witness_limit_exceeded',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
      deletion_witness_checks: 1,
    }),
    unknown({
      reason: 'deletion_witness_limit_exceeded',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
      deletion_witness_work: 1,
    }),
    unknown({
      reason: 'deletion_witness_unavailable',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
      deletion_witness_checks: 1,
      certified_deletion_witnesses: 1,
      deletion_witness_work: 99,
    }),
    unknown({
      reason: 'deletion_witness_unavailable',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
      deletion_witness_checks: 1,
      certified_deletion_witnesses: 0,
      deletion_witness_work: 0,
    }),
    unknown({
      reason: 'deletion_witness_work_limit_exceeded',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
      deletion_witness_work: 1,
    }),
    unknown({
      reason: 'deletion_witness_work_limit_exceeded',
      direct_core_constraint_ids: CORE,
      direct_oracle_calls: 7,
      deletion_witness_checks: 1,
      certified_deletion_witnesses: 1,
      deletion_witness_work: 99,
    }),
  ]
  for (const semanticMus of invalid) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        envelope(semanticMus, provenDirect()),
        BINDING,
      ),
      null,
    )
  }

  for (const [semanticMus, bounded] of [
    [valid, provenDirect([CORE[0]!], 7)],
    [valid, provenDirect(CORE, 8)],
    [
      unknown({
        reason: 'direct_oracle_incomplete',
        direct_core_constraint_ids: [],
        direct_oracle_calls: 4,
      }),
      unknownDirect('cancelled', 4),
    ],
    [
      unknown({
        reason: 'cancelled',
        direct_core_constraint_ids: [],
        direct_oracle_calls: 0,
      }),
      unknownDirect('constraint_limit_exceeded', 0),
    ],
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        envelope(semanticMus, bounded),
        BINDING,
      ),
      null,
    )
  }
})

test('semantic-MUS nested accessors and hostile proxies fail closed without reads', () => {
  let getterCalls = 0
  const accessor = certified()
  Object.defineProperty(accessor, 'constraint_ids', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private')
    },
  })
  const hostile = new Proxy({}, {
    ownKeys() {
      throw new Error('private')
    },
  })

  for (const semanticMus of [accessor, hostile]) {
    assert.doesNotThrow(() => {
      assert.equal(
        normalizeGeometricConstraintPreflightResponse(
          envelope(semanticMus, provenDirect()),
          BINDING,
        ),
        null,
      )
    })
  }
  assert.equal(getterCalls, 0)
})


function assertDeepFrozen(value: unknown, seen = new Set<object>()): void {
  if (value === null || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) assertDeepFrozen(child, seen)
}
