import assert from 'node:assert/strict'
import test from 'node:test'

import {
  completeOwnedRequest,
  createOwnedRequestGate,
  ownedRequestActive,
  tryBeginOwnedRequest,
} from '../src/lib/ownedRequestGate.ts'

test('a second confirmation in the same commit cannot acquire or release the active request', () => {
  const gate = createOwnedRequestGate()
  const owner = tryBeginOwnedRequest(gate)
  assert.equal(owner, 1)
  assert.equal(ownedRequestActive(gate), true)

  assert.equal(tryBeginOwnedRequest(gate), null)
  assert.equal(completeOwnedRequest(gate, 2), false)
  assert.equal(ownedRequestActive(gate), true)

  assert.equal(completeOwnedRequest(gate, owner!), true)
  assert.equal(ownedRequestActive(gate), false)
  assert.equal(tryBeginOwnedRequest(gate), 2)
})

test('sequence wrap never produces zero and a completed owner cannot finish twice', () => {
  const gate = createOwnedRequestGate()
  gate.sequence = 0xffff_ffff
  const owner = tryBeginOwnedRequest(gate)
  assert.equal(owner, 1)
  assert.equal(completeOwnedRequest(gate, owner!), true)
  assert.equal(completeOwnedRequest(gate, owner!), false)
})
