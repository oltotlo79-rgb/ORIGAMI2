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

const zeroClosureOnly = () => certified({
  current_assignment_witness_count: 0,
  single_constraint_constructive_witness_count: 0,
  zero_length_closure_constructive_witness_count: 2,
})

test('accepts, freezes, and projects the distinct zero-length-closure counter', () => {
  const raw = zeroClosureOnly()
  const normalized = normalizeGeometricConstraintPreflightResponse(
    envelope(raw, provenDirect()),
    BINDING,
  )
  const semantic = normalized?.semantic_mus
  assert.equal(
    semantic?.status === 'certified'
      ? semantic.zero_length_closure_constructive_witness_count
      : null,
    2,
  )
  assert.equal(Object.isFrozen(semantic), true)
  const view = semantic?.status === 'certified'
    ? buildGeometricConstraintSemanticMusCertifiedViewModel(semantic)
    : null
  assert.equal(view?.zeroLengthClosureConstructiveWitnessCount, 2)
})

test('zero-length-closure counter is exact, required, and included once in the sum', () => {
  const missing = zeroClosureOnly()
  delete missing.zero_length_closure_constructive_witness_count
  for (const invalid of [
    missing,
    { ...zeroClosureOnly(), zero_length_closure_constructive_witness_count: '2' },
    { ...zeroClosureOnly(), zero_length_closure_constructive_witness_count: null },
    { ...zeroClosureOnly(), zero_length_closure_constructive_witness_count: -0 },
    { ...zeroClosureOnly(), zero_length_closure_constructive_witness_count: -1 },
    { ...zeroClosureOnly(), zero_length_closure_constructive_witness_count: 1.5 },
    { ...zeroClosureOnly(), zero_length_closure_constructive_witness_count: 3 },
    { ...zeroClosureOnly(), zero_length_closure_constructive_witness_count: 1 },
    { ...zeroClosureOnly(), future_zero_closure_count: 0 },
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

test('hostile accessor and Proxy inputs execute zero traps at wire and display boundaries', () => {
  let getterCalls = 0
  let proxyTrapCalls = 0
  const accessor = zeroClosureOnly()
  Object.defineProperty(
    accessor,
    'zero_length_closure_constructive_witness_count',
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

  const hostileProxy = new Proxy(zeroClosureOnly(), {
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
