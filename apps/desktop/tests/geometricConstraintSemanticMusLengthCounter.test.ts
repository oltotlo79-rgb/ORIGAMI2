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
  CORE,
  certified,
  envelope,
  provenDirect,
} from './geometricConstraintSemanticMusTestSupport.ts'

type CertifiedSemanticMus = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

const lengthOnly = () => certified({
  current_assignment_witness_count: 0,
  single_constraint_constructive_witness_count: 0,
  length_constraint_constructive_witness_count: 2,
  zero_length_closure_constructive_witness_count: 0,
})

test('accepts and freezes the exact bounded length-only witness counter', () => {
  const semanticMus = lengthOnly()
  const normalized = normalizeGeometricConstraintPreflightResponse(
    envelope(semanticMus, provenDirect()),
    BINDING,
  )
  assert.deepEqual(normalized?.semantic_mus, semanticMus)
  assert.equal(
    normalized?.semantic_mus?.status === 'certified'
      ? normalized.semantic_mus.length_constraint_constructive_witness_count
      : null,
    2,
  )
  assert.equal(Object.isFrozen(normalized?.semantic_mus), true)
})

test('length counter rejects missing, extra, wrong numeric classes, and sum mismatches', () => {
  const missing = lengthOnly()
  delete missing.length_constraint_constructive_witness_count
  const invalid = [
    missing,
    { ...lengthOnly(), future_length_witness_count: 0 },
    { ...lengthOnly(), length_constraint_constructive_witness_count: '2' },
    { ...lengthOnly(), length_constraint_constructive_witness_count: null },
    { ...lengthOnly(), length_constraint_constructive_witness_count: -0 },
    { ...lengthOnly(), length_constraint_constructive_witness_count: -1 },
    { ...lengthOnly(), length_constraint_constructive_witness_count: 1.5 },
    { ...lengthOnly(), length_constraint_constructive_witness_count: 3 },
    {
      ...lengthOnly(),
      length_constraint_constructive_witness_count:
        Number.MAX_SAFE_INTEGER + 1,
    },
    { ...lengthOnly(), length_constraint_constructive_witness_count: 1 },
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

test('wire parsing rejects hostile length accessors and proxies without executing them', () => {
  let getterCalls = 0
  const accessor = lengthOnly()
  Object.defineProperty(accessor, 'length_constraint_constructive_witness_count', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private')
    },
  })
  Object.freeze(accessor)
  const hostile = new Proxy(lengthOnly(), {
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

test('display projection catches hostile length getters and count overstatement', () => {
  let getterCalls = 0
  let proxyTrapCalls = 0
  const accessor = lengthOnly() as unknown as CertifiedSemanticMus
  Object.defineProperty(accessor, 'length_constraint_constructive_witness_count', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private')
    },
  })
  const hostileProxy = new Proxy(lengthOnly(), {
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
  const hostileIds = lengthOnly()
  let idGetterCalls = 0
  const ids = [...CORE]
  Object.defineProperty(ids, '0', {
    enumerable: true,
    get() {
      idGetterCalls += 1
      throw new Error('private')
    },
  })
  Object.freeze(ids)
  hostileIds.constraint_ids = ids
  Object.freeze(hostileIds)
  const overcounted = Object.freeze({
    ...lengthOnly(),
    constraint_ids: Object.freeze([...CORE]),
    length_constraint_constructive_witness_count: 3,
  }) as unknown as CertifiedSemanticMus
  for (const value of [
    accessor,
    hostileProxy,
    hostileIds as unknown as CertifiedSemanticMus,
    overcounted,
  ]) {
    assert.doesNotThrow(() => {
      assert.equal(
        buildGeometricConstraintSemanticMusCertifiedViewModel(value),
        null,
      )
    })
  }
  assert.equal(getterCalls, 0)
  assert.equal(proxyTrapCalls, 0)
  assert.equal(idGetterCalls, 0)
})
