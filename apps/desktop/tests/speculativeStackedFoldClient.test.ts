import assert from 'node:assert/strict'
import test from 'node:test'

import {
  normalizeSpeculativeStackedFoldApplyRequestV1,
} from '../src/lib/speculativeStackedFoldClient.ts'

const token = '018f47a2-4b7a-7cc1-8abc-778899aabbcc'

test('speculativeApplyRequiresExplicitConfirmation', () => {
  assert.deepEqual(normalizeSpeculativeStackedFoldApplyRequestV1({
    transactionToken: token,
    explicitConfirmation: true,
  }), {
    transactionToken: token,
    explicitConfirmation: true,
  })
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1({
    transactionToken: token,
    explicitConfirmation: false,
  }), null)
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1({
    transactionToken: token,
  }), null)
})

test('speculative Apply request rejects extra fields, invalid tokens, and accessors', () => {
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1({
    transactionToken: token,
    explicitConfirmation: true,
    certified: true,
  }), null)
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1({
    transactionToken: 'not-a-token',
    explicitConfirmation: true,
  }), null)
  const accessor = {
    transactionToken: token,
    explicitConfirmation: true,
  }
  Object.defineProperty(accessor, 'transactionToken', {
    enumerable: true,
    get() {
      throw new Error('must not be read')
    },
  })
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1(accessor), null)

  const inherited = Object.create({
    transactionToken: token,
    explicitConfirmation: true,
  })
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1(inherited), null)

  const hidden = {
    transactionToken: token,
    explicitConfirmation: true,
  }
  Object.defineProperty(hidden, 'privateField', { value: true })
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1(hidden), null)

  const symbolic = {
    transactionToken: token,
    explicitConfirmation: true,
    [Symbol('private')]: true,
  }
  assert.equal(normalizeSpeculativeStackedFoldApplyRequestV1(symbolic), null)

  let proxyGetCalls = 0
  const proxied = new Proxy({
    transactionToken: token,
    explicitConfirmation: true,
  }, {
    get() {
      proxyGetCalls += 1
      throw new Error('must not be read')
    },
  })
  assert.notEqual(normalizeSpeculativeStackedFoldApplyRequestV1(proxied), null)
  assert.equal(proxyGetCalls, 0)
})
