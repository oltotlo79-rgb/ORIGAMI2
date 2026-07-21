import assert from 'node:assert/strict'
import test from 'node:test'
import { normalizeCurrentCyclePosePreviewResponseV1 } from '../src/lib/coreClient.ts'

const valid = {
  version: 1,
  transactionToken: '018f47a2-4b7a-7cc1-8abc-778899aabbcc',
  sourceRevision: 3,
  targetRevision: 4,
  closureLeafCount: 4,
  closureMaxDepth: 2,
  checkedHingeCount: 16,
  totalHingeCount: 16,
  continuousPathCertified: true,
  continuousLayerTransportModelId: 'native_continuous_layer_transport_certificate_v1',
  continuousLayerTransitionCount: 5,
  continuousLayerPairOrderCount: 1,
  continuousLayerTargetOrderSha256: 'ab'.repeat(32),
  sourceLayerOrder: [{
    lowerFace: '018f47a2-4b7a-7cc1-8abc-111111111111',
    upperFace: '018f47a2-4b7a-7cc1-8abc-222222222222',
  }],
  targetLayerOrder: [{
    lowerFace: '018f47a2-4b7a-7cc1-8abc-111111111111',
    upperFace: '018f47a2-4b7a-7cc1-8abc-222222222222',
  }],
  authorizesProjectMutation: false,
}

test('current-cycle proof summary accepts the bounded complete DTO', () => {
  assert.deepEqual(normalizeCurrentCyclePosePreviewResponseV1(valid, 3), valid)
})

test('current-cycle proof summary rejects tampering, bounds, and partial coverage', () => {
  const invalid = [
    { ...valid, injected: true },
    { ...valid, closureLeafCount: 65_537 },
    { ...valid, closureMaxDepth: 17 },
    { ...valid, checkedHingeCount: 15 },
    { ...valid, totalHingeCount: 129, checkedHingeCount: 129 },
    { ...valid, continuousLayerTargetOrderSha256: '00' },
    { ...valid, continuousLayerPairOrderCount: 2 },
    { ...valid, targetRevision: 5 },
  ]
  for (const value of invalid) {
    assert.throws(
      () => normalizeCurrentCyclePosePreviewResponseV1(value, 3),
      /invalid current-cycle preview response/,
    )
  }
})
