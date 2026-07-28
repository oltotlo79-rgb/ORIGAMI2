import assert from 'node:assert/strict'
import test from 'node:test'

import { normalizeStackedFoldReadResponse } from '../src/lib/stackedFoldRead.ts'
import {
  makeSpeculativeStackedFoldResponse,
  SPECULATIVE_EXPECTED_READ,
  SPECULATIVE_TOKEN,
} from './stackedFoldSpeculativeFixture.ts'

const normalize = (value: unknown) =>
  normalizeStackedFoldReadResponse(value, SPECULATIVE_EXPECTED_READ)

test('accepts only a self-consistent speculative-unproven apply contract', () => {
  const response = makeSpeculativeStackedFoldResponse()
  const admitted = normalize(response)
  assert.deepEqual(admitted, response)
  assert.notEqual(admitted, response)
  assert.equal(Object.isFrozen(admitted), true)
  assert.equal(Object.isFrozen(admitted?.transactionProposal), true)
})

test('speculativeApplyRejectsWhenBlockingSampleObserved', () => {
  const response = makeSpeculativeStackedFoldResponse()
  response.continuousPath.sampledNonblockingPoseCount = 1
  response.continuousPath.firstSampledBlockingAngleDegrees = 90
  assert.equal(normalize(response), null)
})

test('rejects speculative mode flags that could overclaim mutation authority', () => {
  const contradictions: readonly ((response: any) => void)[] = [
    (response) => { response.transactionProposal.readyForAtomicApply = true },
    (response) => { response.transactionProposal.authorizesProjectMutation = true },
    (response) => { response.transactionProposal.speculativeUnprovenAvailable = false },
    (response) => { response.transactionProposal.transactionToken = null },
    (response) => { response.transactionProposal.transactionToken = 'not-a-token' },
    (response) => { response.transactionProposal.failureClasses = [] },
    (response) => {
      response.transactionProposal.failureClasses = [
        'continuous_path_uncertified',
        'continuous_path_uncertified',
      ]
    },
    (response) => {
      response.transactionProposal.failureClasses = [
        'target_layer_order_unavailable',
        'continuous_path_uncertified',
      ]
    },
    (response) => { response.continuousPath.continuousClearanceCertified = true },
    (response) => {
      response.continuousPath.continuousCertificateModelId =
        'stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v2'
    },
    (response) => { response.continuousPath.sampledPoseCount = 0 },
    (response) => { response.continuousPath.sampledNonblockingPoseCount = 1 },
    (response) => { response.certifiedPathGraph = {} },
  ]

  for (const contradict of contradictions) {
    const response = makeSpeculativeStackedFoldResponse()
    contradict(response)
    assert.equal(normalize(response), null)
  }
})

test('rejects blocking endpoint evidence and uncertified target layer order', () => {
  const blocking = makeSpeculativeStackedFoldResponse()
  blocking.endpointCollision.allowedPairCount = 2
  blocking.endpointCollision.penetratingPairCount = 1
  blocking.endpointCollision.hasBlockingHold = true
  assert.equal(normalize(blocking), null)

  const layerUncertified = makeSpeculativeStackedFoldResponse()
  layerUncertified.flatEndpointLayerOrder.certified = false
  layerUncertified.flatEndpointLayerOrder.materialFaceCount = 0
  layerUncertified.flatEndpointLayerOrder.overlapCellCount = 0
  layerUncertified.transactionProposal.failureClasses = [
    'continuous_path_uncertified',
    'target_layer_order_unavailable',
  ]
  assert.equal(normalize(layerUncertified), null)
})

test('rejects mode/token cross-use, unknown fields, and legacy partial contracts', () => {
  const cases: any[] = []

  const noneWithToken = makeSpeculativeStackedFoldResponse()
  noneWithToken.transactionProposal.applyMode = 'none'
  cases.push(noneWithToken)

  const certifiedWithSpeculativeFlags = makeSpeculativeStackedFoldResponse()
  certifiedWithSpeculativeFlags.transactionProposal.applyMode = 'certified'
  cases.push(certifiedWithSpeculativeFlags)

  const unknownMode = makeSpeculativeStackedFoldResponse()
  unknownMode.transactionProposal.applyMode = 'future_mode'
  cases.push(unknownMode)

  const extraTransactionField = makeSpeculativeStackedFoldResponse()
  extraTransactionField.transactionProposal.untrusted = true
  cases.push(extraTransactionField)

  const extraTopField = makeSpeculativeStackedFoldResponse()
  extraTopField.rawProof = 'secret'
  cases.push(extraTopField)

  const legacyPartial = makeSpeculativeStackedFoldResponse()
  delete legacyPartial.transactionProposal.applyContractVersion
  delete legacyPartial.transactionProposal.applyMode
  delete legacyPartial.transactionProposal.speculativeUnprovenAvailable
  cases.push(legacyPartial)

  for (const response of cases) assert.equal(normalize(response), null)
})

test('keeps the canonical one-shot token only for the accepted contract', () => {
  const normalized = normalize(makeSpeculativeStackedFoldResponse())
  assert.equal(normalized?.transactionProposal.transactionToken, SPECULATIVE_TOKEN)

  const negativeZero = makeSpeculativeStackedFoldResponse()
  negativeZero.continuousPath.sampledPoseCount = -0
  negativeZero.continuousPath.sampledNonblockingPoseCount = -0
  assert.equal(normalize(negativeZero), null)
})

test('reads only detached own data and contains hostile accessors and Proxies', () => {
  let getterCalls = 0
  const accessor = makeSpeculativeStackedFoldResponse()
  Object.defineProperty(accessor.transactionProposal, 'applyMode', {
    enumerable: true,
    get() {
      getterCalls += 1
      throw new Error('private native detail')
    },
  })
  assert.equal(normalize(accessor), null)
  assert.equal(getterCalls, 0)

  let proxyGetCalls = 0
  const proxied = makeSpeculativeStackedFoldResponse()
  proxied.transactionProposal = new Proxy(proxied.transactionProposal, {
    get() {
      proxyGetCalls += 1
      throw new Error('private native detail')
    },
  })
  assert.notEqual(normalize(proxied), null)
  assert.equal(proxyGetCalls, 0)

  const revoked = makeSpeculativeStackedFoldResponse()
  const revocable = Proxy.revocable(revoked.transactionProposal, {})
  revoked.transactionProposal = revocable.proxy
  revocable.revoke()
  assert.equal(normalize(revoked), null)
})

test('rejects inherited, hidden, symbolic, sparse, and one-ULP wire drift', () => {
  assert.equal(
    normalize(Object.create(makeSpeculativeStackedFoldResponse())),
    null,
  )

  const hidden = makeSpeculativeStackedFoldResponse()
  Object.defineProperty(hidden.transactionProposal, 'privateField', {
    value: true,
  })
  assert.equal(normalize(hidden), null)

  const symbolic = makeSpeculativeStackedFoldResponse()
  symbolic.transactionProposal[Symbol('private')] = true
  assert.equal(normalize(symbolic), null)

  const sparse = makeSpeculativeStackedFoldResponse()
  sparse.transactionProposal.failureClasses = new Array(1)
  assert.equal(normalize(sparse), null)

  const oneUlp = makeSpeculativeStackedFoldResponse()
  oneUlp.continuousPath.sampledPoseCount = 2.0000000000000004
  assert.equal(normalize(oneUlp), null)
})
