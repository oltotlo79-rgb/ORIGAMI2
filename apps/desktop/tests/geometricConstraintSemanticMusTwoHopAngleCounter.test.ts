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
} from './geometricConstraintSemanticMusTestSupport.ts'

const CORE = Object.freeze(IDS.slice(0, 5))

type CertifiedSemanticMus = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

function semantic(overrides: Readonly<Record<string, unknown>> = {}) {
  return certified({
    constraint_ids: CORE,
    constraint_count: 5,
    deletion_witness_checks: 5,
    current_assignment_witness_count: 0,
    single_constraint_constructive_witness_count: 0,
    unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 5,
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
          kind: 'non_parallel_fixed_angle_in_parallel_component',
          vertex: IDS[5]!,
          first_edge: IDS[6]!,
          second_edge: IDS[7]!,
          parallel_constraint_count: 2,
        },
        constraint_ids: CORE,
      }],
      bounded_direct_mus: provenDirect(CORE),
    },
    semantic_mus: value,
  }
}

test('accepts and projects the exact unit-terminal two-hop angle counter', () => {
  const normalized = normalizeGeometricConstraintPreflightResponse(
    envelope(semantic()),
    BINDING,
  )
  const result = normalized?.semantic_mus
  assert.equal(
    result?.status === 'certified'
      ? result
        .unit_terminal_two_hop_parallel_angle_residual_only_witness_count
      : null,
    5,
  )
  const view = result?.status === 'certified'
    ? buildGeometricConstraintSemanticMusCertifiedViewModel(result)
    : null
  assert.equal(
    view?.unitTerminalTwoHopParallelAngleResidualOnlyWitnessCount,
    5,
  )
  assert.equal(view?.constraintCount, 5)
})

test('unit-terminal two-hop angle counter is required, bounded, and summed once', () => {
  const missing = semantic()
  delete missing
    .unit_terminal_two_hop_parallel_angle_residual_only_witness_count
  for (const invalid of [
    missing,
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: '5',
    }),
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: null,
    }),
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: -0,
    }),
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: -1,
    }),
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 1.5,
    }),
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 6,
    }),
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 4,
    }),
    semantic({
      unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 4,
      current_assignment_witness_count: 1,
    }),
    semantic({ future_unit_terminal_two_hop_angle_counter: 0 }),
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(envelope(invalid), BINDING),
      null,
    )
  }
})

test('hostile unit-terminal two-hop angle counter inputs execute zero traps', () => {
  let getterCalls = 0
  let proxyTrapCalls = 0
  const accessor = semantic()
  Object.defineProperty(
    accessor,
    'unit_terminal_two_hop_parallel_angle_residual_only_witness_count',
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

test('old semantic model IDs and missing direct bindings fail closed', () => {
  assert.equal(
    normalizeGeometricConstraintPreflightResponse(
      envelope(semantic({
        model_id: 'geometric_constraint_deterministic_binary64_semantic_mus_v3',
      })),
      BINDING,
    ),
    null,
  )
  const wrong = envelope(semantic())
  wrong.result.conflicts[0]!.constraint_ids = CORE.slice(0, 4)
  assert.equal(
    normalizeGeometricConstraintPreflightResponse(wrong, BINDING),
    null,
  )

  const wrongKind = envelope(semantic())
  Reflect.set(wrongKind.result.conflicts[0]!, 'conflict', {
    kind: 'perpendicular_orientations_in_parallel_component',
    horizontal_edge: IDS[6]!,
    vertical_edge: IDS[7]!,
    parallel_constraint_count: 2,
  })
  assert.equal(
    normalizeGeometricConstraintPreflightResponse(wrongKind, BINDING),
    null,
  )
})
