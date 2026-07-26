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
  provenDirect,
} from './geometricConstraintSemanticMusTestSupport.ts'

type CertifiedSemanticMus = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

test('keeps constructive and algebraic pair witnesses as separate exact counters', () => {
  const semanticMus = certified({
    current_assignment_witness_count: 0,
    single_constraint_constructive_witness_count: 0,
    pair_constraint_constructive_witness_count: 1,
    pair_constraint_algebraic_witness_count: 1,
  })
  const normalized = normalizeGeometricConstraintPreflightResponse(
    envelope(semanticMus, provenDirect()),
    BINDING,
  )
  assert.deepEqual(normalized?.semantic_mus, semanticMus)
  assert.equal(
    normalized?.semantic_mus?.status === 'certified'
      ? normalized.semantic_mus.pair_constraint_constructive_witness_count
      : null,
    1,
  )
  assert.equal(
    normalized?.semantic_mus?.status === 'certified'
      ? normalized.semantic_mus.pair_constraint_algebraic_witness_count
      : null,
    1,
  )
  assert.equal(Object.isFrozen(normalized?.semantic_mus), true)
})

test('pair witness counters reject missing, wrong-type, negative, noninteger, and inconsistent totals', () => {
  const missingConstructive = certified()
  delete missingConstructive.pair_constraint_constructive_witness_count
  const missingAlgebraic = certified()
  delete missingAlgebraic.pair_constraint_algebraic_witness_count
  const invalid = [
    missingConstructive,
    missingAlgebraic,
    { ...certified(), pair_constraint_constructive_witness_count: '0' },
    { ...certified(), pair_constraint_algebraic_witness_count: null },
    { ...certified(), pair_constraint_constructive_witness_count: -1 },
    { ...certified(), pair_constraint_algebraic_witness_count: -0 },
    { ...certified(), pair_constraint_constructive_witness_count: 0.5 },
    { ...certified(), pair_constraint_algebraic_witness_count: 1.5 },
    { ...certified(), pair_constraint_constructive_witness_count: 3 },
    { ...certified(), pair_constraint_algebraic_witness_count: Infinity },
    {
      ...certified(),
      single_constraint_constructive_witness_count: 0,
      pair_constraint_constructive_witness_count: 0,
      pair_constraint_algebraic_witness_count: 0,
    },
    {
      ...certified(),
      pair_constraint_constructive_witness_count: 1,
      pair_constraint_algebraic_witness_count: 1,
    },
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
})

test('presentation view model independently refuses an overcounted typed value', () => {
  const valid = certified({
    current_assignment_witness_count: 0,
    single_constraint_constructive_witness_count: 0,
    pair_constraint_constructive_witness_count: 1,
    pair_constraint_algebraic_witness_count: 1,
  }) as unknown as CertifiedSemanticMus
  const view =
    buildGeometricConstraintSemanticMusCertifiedViewModel(valid)
  assert.deepEqual(view && [
    view.pairConstraintConstructiveWitnessCount,
    view.pairConstraintAlgebraicWitnessCount,
  ], [1, 1])
  assert.equal(Object.isFrozen(view), true)
  assert.equal(Object.isFrozen(view?.constraintIds), true)

  const overcounted = {
    ...valid,
    current_assignment_witness_count: 2,
  }
  assert.equal(
    buildGeometricConstraintSemanticMusCertifiedViewModel(overcounted),
    null,
  )
})

test('presentation view model fails closed for hostile getters and proxies', () => {
  let getterCalls = 0
  const accessor = certified() as unknown as CertifiedSemanticMus
  Object.defineProperty(accessor, 'constraint_count', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private')
    },
  })
  const hostile = new Proxy(certified(), {
    get() {
      throw new Error('private')
    },
  }) as unknown as CertifiedSemanticMus

  for (const value of [accessor, hostile]) {
    assert.doesNotThrow(() => {
      assert.equal(
        buildGeometricConstraintSemanticMusCertifiedViewModel(value),
        null,
      )
    })
  }
  assert.equal(getterCalls, 1)
})
