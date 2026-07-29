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
  envelope,
  IDS,
  provenDirect,
} from './geometricConstraintSemanticMusTestSupport.ts'

type CertifiedSemanticMus = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

const mirrorOnly = () => certified({
  current_assignment_witness_count: 0,
  single_constraint_constructive_witness_count: 0,
  anchored_mirror_residual_only_witness_count: 2,
})

const UNIT_TWO_HOP_IDS = Object.freeze(IDS.slice(0, 5))

const unitTwoHopOnly = (
  overrides: Readonly<Record<string, unknown>> = {},
) => certified({
  constraint_ids: UNIT_TWO_HOP_IDS,
  constraint_count: 5,
  deletion_witness_checks: 5,
  current_assignment_witness_count: 0,
  single_constraint_constructive_witness_count: 0,
  unit_two_hop_parallel_residual_only_witness_count: 5,
  ...overrides,
})

function unitTwoHopEnvelope(semanticMus: unknown) {
  return {
    ...BINDING,
    result: {
      status: 'direct_conflict',
      conflicts: [{
        conflict: {
          kind: 'perpendicular_orientations_in_parallel_component',
          horizontal_edge: IDS[5]!,
          vertical_edge: IDS[6]!,
          parallel_constraint_count: 2,
        },
        constraint_ids: UNIT_TWO_HOP_IDS,
      }],
      bounded_direct_mus: provenDirect(UNIT_TWO_HOP_IDS),
    },
    semantic_mus: semanticMus,
  }
}

test('accepts, freezes, and projects the anchored-mirror residual-only counter', () => {
  const raw = mirrorOnly()
  const normalized = normalizeGeometricConstraintPreflightResponse(
    envelope(raw, provenDirect()),
    BINDING,
  )
  const semantic = normalized?.semantic_mus
  assert.equal(
    semantic?.status === 'certified'
      ? semantic.anchored_mirror_residual_only_witness_count
      : null,
    2,
  )
  assert.equal(Object.isFrozen(semantic), true)
  const view = semantic?.status === 'certified'
    ? buildGeometricConstraintSemanticMusCertifiedViewModel(semantic)
    : null
  assert.equal(view?.anchoredMirrorResidualOnlyWitnessCount, 2)
})

test('anchored-mirror counter is exact, required, and included once in the sum', () => {
  const missing = mirrorOnly()
  delete missing.anchored_mirror_residual_only_witness_count
  for (const invalid of [
    missing,
    {
      ...mirrorOnly(),
      anchored_mirror_residual_only_witness_count: '2',
    },
    {
      ...mirrorOnly(),
      anchored_mirror_residual_only_witness_count: null,
    },
    {
      ...mirrorOnly(),
      anchored_mirror_residual_only_witness_count: -0,
    },
    {
      ...mirrorOnly(),
      anchored_mirror_residual_only_witness_count: -1,
    },
    {
      ...mirrorOnly(),
      anchored_mirror_residual_only_witness_count: 1.5,
    },
    {
      ...mirrorOnly(),
      anchored_mirror_residual_only_witness_count: 3,
    },
    {
      ...mirrorOnly(),
      anchored_mirror_residual_only_witness_count: 1,
    },
    { ...mirrorOnly(), future_mirror_residual_count: 0 },
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        envelope(invalid, provenDirect()),
        BINDING,
      ),
      null,
    )
  }
})

test('hostile mirror-counter accessor and Proxy inputs execute zero traps', () => {
  let getterCalls = 0
  let proxyTrapCalls = 0
  const accessor = mirrorOnly()
  Object.defineProperty(
    accessor,
    'anchored_mirror_residual_only_witness_count',
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
        envelope(accessor, provenDirect()),
        BINDING,
      ),
      null,
    )
  })

  const hostileProxy = new Proxy(mirrorOnly(), {
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

test('accepts, freezes, and projects the five unit-terminal two-hop witnesses', () => {
  const raw = unitTwoHopOnly()
  const normalized = normalizeGeometricConstraintPreflightResponse(
    unitTwoHopEnvelope(raw),
    BINDING,
  )
  const semantic = normalized?.semantic_mus
  assert.equal(
    semantic?.status === 'certified'
      ? semantic.unit_two_hop_parallel_residual_only_witness_count
      : null,
    5,
  )
  assert.equal(
    semantic?.status === 'certified' ? semantic.constraint_ids.length : null,
    5,
  )
  assert.equal(Object.isFrozen(semantic), true)
  const view = semantic?.status === 'certified'
    ? buildGeometricConstraintSemanticMusCertifiedViewModel(semantic)
    : null
  assert.equal(view?.unitTwoHopParallelResidualOnlyWitnessCount, 5)
  assert.equal(view?.constraintIds.length, 5)
})

test('unit-terminal two-hop counter is exact, required, and summed exactly once', () => {
  const missing = unitTwoHopOnly()
  delete missing.unit_two_hop_parallel_residual_only_witness_count
  for (const invalid of [
    missing,
    unitTwoHopOnly({
      unit_two_hop_parallel_residual_only_witness_count: '5',
    }),
    unitTwoHopOnly({
      unit_two_hop_parallel_residual_only_witness_count: null,
    }),
    unitTwoHopOnly({
      unit_two_hop_parallel_residual_only_witness_count: -0,
    }),
    unitTwoHopOnly({
      unit_two_hop_parallel_residual_only_witness_count: -1,
    }),
    unitTwoHopOnly({
      unit_two_hop_parallel_residual_only_witness_count: 1.5,
    }),
    unitTwoHopOnly({
      unit_two_hop_parallel_residual_only_witness_count: 6,
    }),
    unitTwoHopOnly({
      unit_two_hop_parallel_residual_only_witness_count: 4,
    }),
    unitTwoHopOnly({ future_unit_two_hop_parallel_count: 0 }),
  ]) {
    assert.equal(
      normalizeGeometricConstraintPreflightResponse(
        unitTwoHopEnvelope(invalid),
        BINDING,
      ),
      null,
    )
  }
})

test('hostile unit-terminal two-hop counter inputs execute zero traps', () => {
  let getterCalls = 0
  let proxyTrapCalls = 0
  const accessor = unitTwoHopOnly()
  Object.defineProperty(
    accessor,
    'unit_two_hop_parallel_residual_only_witness_count',
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
        unitTwoHopEnvelope(accessor),
        BINDING,
      ),
      null,
    )
  })

  const hostileProxy = new Proxy(unitTwoHopOnly(), {
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
