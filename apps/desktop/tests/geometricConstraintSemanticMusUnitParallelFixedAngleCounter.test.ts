import assert from 'node:assert/strict'
import test from 'node:test'

import {
  normalizeGeometricConstraintPreflightResponse,
  type GeometricConstraintSemanticMusV1,
} from '../src/lib/geometricConstraints.ts'
import {
  buildGeometricConstraintSemanticMusCertifiedViewModel,
} from '../src/lib/geometricConstraintSemanticMusViewModel.ts'
import {
  BINDING,
  certified,
  IDS,
  provenDirect,
  unknown,
} from './geometricConstraintSemanticMusTestSupport.ts'

const CORE = Object.freeze(IDS.slice(0, 3))

type CertifiedSemanticMus = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

function semantic(overrides: Readonly<Record<string, unknown>> = {}) {
  return certified({
    constraint_ids: CORE,
    constraint_count: 3,
    deletion_witness_checks: 3,
    current_assignment_witness_count: 0,
    single_constraint_constructive_witness_count: 0,
    unit_parallel_fixed_angle_residual_only_witness_count: 3,
    ...overrides,
  })
}

function envelope(value: unknown) {
  return {
    ...BINDING,
    result: {
      status: 'direct_conflict',
      conflicts: [{
        conflict: {
          kind: 'parallel_with_fixed_non_parallel_angle',
          first_edge: IDS[4]!,
          second_edge: IDS[5]!,
        },
        constraint_ids: CORE,
      }],
      bounded_direct_mus: provenDirect(CORE),
    },
    semantic_mus: value,
  }
}

test('accepts, freezes, and projects the exact unit parallel-fixed-angle counter', () => {
  const normalized = normalizeGeometricConstraintPreflightResponse(
    envelope(semantic()),
    BINDING,
  )
  const result = normalized?.semantic_mus
  assert.equal(
    result?.status === 'certified'
      ? result.unit_parallel_fixed_angle_residual_only_witness_count
      : null,
    3,
  )
  assert.ok(result && Object.isFrozen(result))
  const view = result?.status === 'certified'
    ? buildGeometricConstraintSemanticMusCertifiedViewModel(result)
    : null
  assert.equal(view?.unitParallelFixedAngleResidualOnlyWitnessCount, 3)
  assert.equal(view?.constraintCount, 3)
  assert.ok(view && Object.isFrozen(view))
})

test('unit parallel-fixed-angle counter is exact, required, and summed once', () => {
  const missing = semantic()
  delete missing.unit_parallel_fixed_angle_residual_only_witness_count
  for (const invalid of [
    missing,
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: '3',
    }),
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: null,
    }),
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: -0,
    }),
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: -1,
    }),
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: 1.5,
    }),
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: 4,
    }),
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: 2,
    }),
    semantic({
      current_assignment_witness_count: 1,
    }),
    semantic({
      unit_parallel_fixed_angle_residual_only_witness_count: 2,
      current_assignment_witness_count: 1,
    }),
    semantic({ future_unit_parallel_fixed_angle_counter: 0 }),
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(envelope(invalid), BINDING),
      null,
    )
  }
})

test('hostile unit parallel-fixed-angle inputs execute zero traps', () => {
  let getterCalls = 0
  let proxyTrapCalls = 0
  const accessor = semantic()
  Object.defineProperty(
    accessor,
    'unit_parallel_fixed_angle_residual_only_witness_count',
    {
      enumerable: true,
      get() {
        getterCalls += 1
        throw new Error('private')
      },
    },
  )
  Object.freeze(accessor)
  assert.doesNotThrow(() => {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        envelope(accessor),
        BINDING,
      ),
      null,
    )
  })

  const hostileProxy = new Proxy(semantic(), {
    get() {
      proxyTrapCalls += 1
      throw new Error('private')
    },
    ownKeys() {
      proxyTrapCalls += 1
      throw new Error('private')
    },
    getOwnPropertyDescriptor() {
      proxyTrapCalls += 1
      throw new Error('private')
    },
    getPrototypeOf() {
      proxyTrapCalls += 1
      throw new Error('private')
    },
  }) as unknown as CertifiedSemanticMus
  assert.doesNotThrow(() => {
    assert.equal(
      buildGeometricConstraintSemanticMusCertifiedViewModel(hostileProxy),
      null,
    )
  })
  assert.equal(getterCalls, 0)
  assert.equal(proxyTrapCalls, 0)
})

test('direct conflict requires the exact three-ID semantic core binding', () => {
  for (const constraintIds of [
    CORE.slice(0, 2),
    [...CORE, IDS[3]!],
  ]) {
    const wrong = envelope(semantic())
    wrong.result.conflicts[0]!.constraint_ids = constraintIds
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(wrong, BINDING),
      null,
    )
  }

  const wrongKind = envelope(semantic())
  Reflect.set(wrongKind.result.conflicts[0]!, 'conflict', {
    kind: 'parallel_with_perpendicular_orientations',
    horizontal_edge: IDS[4]!,
    vertical_edge: IDS[5]!,
  })
  assert.equal(
    normalizeGeometricConstraintPreflightResponse(wrongKind, BINDING),
    null,
  )
})

test('generic-angle four-ID direct causes remain wire-valid and semantic-unknown', () => {
  const genericCore = Object.freeze(IDS.slice(0, 4))
  const raw = {
    ...BINDING,
    result: {
      status: 'direct_conflict',
      conflicts: [{
        conflict: {
          kind: 'parallel_with_fixed_non_parallel_angle',
          first_edge: IDS[4]!,
          second_edge: IDS[5]!,
        },
        constraint_ids: genericCore,
      }],
      bounded_direct_mus: provenDirect(genericCore),
    },
    semantic_mus: unknown({
      direct_core_constraint_ids: genericCore,
      direct_oracle_calls: 7,
    }),
  }
  const normalized = normalizeGeometricConstraintPreflightResponse(raw, BINDING)
  assert.equal(normalized?.semantic_mus?.status, 'unknown')
  assert.deepEqual(
    normalized?.semantic_mus?.status === 'unknown'
      ? normalized.semantic_mus.direct_core_constraint_ids
      : null,
    genericCore,
  )
  assert.equal(
    normalizeGeometricConstraintPreflightResponse({
      ...raw,
      semantic_mus: certified({
        constraint_ids: genericCore,
        constraint_count: 4,
        deletion_witness_checks: 4,
        current_assignment_witness_count: 1,
        single_constraint_constructive_witness_count: 0,
        unit_parallel_fixed_angle_residual_only_witness_count: 3,
      }),
    }, BINDING),
    null,
  )
})
