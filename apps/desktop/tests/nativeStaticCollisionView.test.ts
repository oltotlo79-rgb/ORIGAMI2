import assert from 'node:assert/strict'
import test from 'node:test'
import {
  presentNativeStaticCollision,
  presentNativeStaticCollisionPairDiagnostics,
  selectBoundNativeStaticCollisionView,
  type CurrentStaticCollisionDiagnostic,
  type CurrentStaticCollisionPairDiagnostic,
} from '../src/lib/nativeStaticCollisionView.ts'

const certified: CurrentStaticCollisionDiagnostic = {
  status: 'certified_nonblocking',
  reason: null,
  expectedUnorderedFacePairs: 0,
  provenPenetratingPairs: 0,
  firstProvenPenetratingPair: null,
  pairClassificationCounts: {
    separated: 0,
    touching: 0,
    allowed: 0,
    penetrating: 0,
    indeterminate: 0,
    candidateExcluded: 0,
  },
  pairDiagnostics: [],
}

test('only an affirmative native certificate receives the clear presentation', () => {
  const view = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: certified,
  })

  assert.equal(view.dataStatus, 'certified_nonblocking')
  assert.equal(view.badgeClass, 'is-certified')
  assert.equal(view.requiresSafetyReview, false)
  assert.match(view.badgeText, /ゼロ厚み面貫通・重なりなし/)
  assert.match(view.accessibleText, /証明/)
})

test('pair presentation exposes every classification and makes holds as prominent as penetration', () => {
  const pairs: readonly CurrentStaticCollisionPairDiagnostic[] = [
    pairDiagnostic(2, 'separated'),
    pairDiagnostic(3, 'touching'),
    pairDiagnostic(4, 'allowed'),
    pairDiagnostic(5, 'penetrating'),
    pairDiagnostic(6, 'indeterminate'),
  ]
  const details = presentNativeStaticCollisionPairDiagnostics({
    status: 'blocking',
    reason: 'proven_zero_thickness_penetration',
    expectedUnorderedFacePairs: 5,
    provenPenetratingPairs: 1,
    firstProvenPenetratingPair: {
      firstFaceId: pairs[3]!.firstFaceId,
      secondFaceId: pairs[3]!.secondFaceId,
    },
    pairClassificationCounts: {
      separated: 1,
      touching: 1,
      allowed: 1,
      penetrating: 1,
      indeterminate: 1,
      candidateExcluded: 0,
    },
    pairDiagnostics: pairs,
  }, 'en')

  assert.ok(details)
  assert.equal(details.totalPairCount, 5)
  assert.equal(details.displayedPairCount, 5)
  assert.equal(details.omittedPairCount, 0)
  assert.equal(details.omittedText, null)
  assert.match(details.countsText, /penetrating 1 \/ indeterminate 1/u)
  assert.deepEqual(
    details.pairs.map((pair) => pair.disposition),
    ['penetrating', 'indeterminate', 'separated', 'touching', 'allowed'],
    'blocking rows are canonical within their priority group and remain visible',
  )
  assert.equal(details.pairs[0]?.risk, 'blocking')
  assert.equal(details.pairs[1]?.risk, 'blocking')
  assert.equal(details.pairs[0]?.rowClass, 'is-penetrating')
  assert.equal(details.pairs[1]?.rowClass, 'is-indeterminate')
  assert.match(details.pairs[0]?.text ?? '', /dual-gate transversal proof/u)
  assert.match(
    details.pairs[1]?.text ?? '',
    /shared-hinge solid classification/u,
  )
})

test('pair presentation caps DOM rows, prioritizes blocking pairs, and states omissions', () => {
  const pairs = Array.from(
    { length: 205 },
    (_, index): CurrentStaticCollisionPairDiagnostic => {
      const disposition = index >= 203 ? 'indeterminate' : 'separated'
      return pairDiagnostic(index + 2, disposition)
    },
  )
  const details = presentNativeStaticCollisionPairDiagnostics({
    status: 'blocking',
    reason: 'evidence_unavailable',
    expectedUnorderedFacePairs: 205,
    provenPenetratingPairs: null,
    firstProvenPenetratingPair: null,
    pairClassificationCounts: {
      separated: 203,
      touching: 0,
      allowed: 0,
      penetrating: 0,
      indeterminate: 2,
      candidateExcluded: 0,
    },
    pairDiagnostics: pairs,
  }, 'en')

  assert.ok(details)
  assert.equal(details.totalPairCount, 205)
  assert.equal(details.displayedPairCount, 200)
  assert.equal(details.omittedPairCount, 5)
  assert.equal(details.pairs.length, 200)
  assert.deepEqual(
    details.pairs.slice(0, 2).map((pair) => pair.disposition),
    ['indeterminate', 'indeterminate'],
  )
  assert.match(details.omittedText ?? '', /Showing 200 of 205 pairs; 5 omitted/u)
  assert.match(details.accessibleCountsText, /same prominence as penetration/u)
})

test('a result for an older pose is hidden synchronously before effects run', () => {
  const bound = {
    requestKey: 'old-pose',
    view: { kind: 'ready', diagnostic: certified } as const,
  }

  assert.deepEqual(
    selectBoundNativeStaticCollisionView(false, 'new-pose', bound),
    { kind: 'checking' },
  )
  assert.deepEqual(
    selectBoundNativeStaticCollisionView(true, 'old-pose', bound),
    { kind: 'waiting' },
  )
  assert.equal(
    selectBoundNativeStaticCollisionView(false, 'old-pose', bound),
    bound.view,
  )
  assert.deepEqual(
    selectBoundNativeStaticCollisionView(false, null, bound),
    { kind: 'idle' },
  )
})

test('a proven zero-thickness penetration or overlap is blocking and publishes the count', () => {
  const view = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      status: 'blocking',
      reason: 'proven_zero_thickness_penetration',
      expectedUnorderedFacePairs: 3,
      provenPenetratingPairs: 1,
      firstProvenPenetratingPair: {
        firstFaceId: 'face-a',
        secondFaceId: 'face-b',
      },
    },
  })

  assert.equal(view.dataStatus, 'penetrating')
  assert.equal(view.badgeClass, 'is-blocked')
  assert.equal(view.requiresSafetyReview, true)
  assert.match(view.badgeText, /ゼロ厚み面貫通・重なり 1・安全認定不可/)
  assert.match(view.accessibleText, /正の面積を持つ重なり/)
})

test('a proven positive-thickness material penetration has a distinct presentation', () => {
  const view = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      status: 'blocking',
      reason: 'proven_positive_thickness_penetration',
      expectedUnorderedFacePairs: 3,
      provenPenetratingPairs: 1,
      firstProvenPenetratingPair: {
        firstFaceId: '00000000-0000-4000-8000-000000000001',
        secondFaceId: '00000000-0000-4000-8000-000000000002',
      },
    },
  })

  assert.equal(view.dataStatus, 'penetrating')
  assert.equal(view.badgeClass, 'is-blocked')
  assert.equal(view.requiresSafetyReview, true)
  assert.equal(
    view.badgeText,
    '厳密判定｜紙厚を含む材料貫通 1・安全認定不可',
  )
  assert.equal(
    view.accessibleText,
    '現在の表示姿勢で紙厚を含む材料の貫通1件を厳密証明したため、安全認定を遮断しました。',
  )
})

test('a malformed positive-thickness reason fails closed instead of inventing a count', () => {
  const view = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      status: 'blocking',
      reason: 'proven_positive_thickness_penetration',
      expectedUnorderedFacePairs: 3,
      provenPenetratingPairs: 0,
      firstProvenPenetratingPair: null,
    },
  })

  assert.equal(view.dataStatus, 'unavailable')
  assert.equal(view.badgeClass, 'is-unavailable')
  assert.equal(view.requiresSafetyReview, true)
  assert.doesNotMatch(view.badgeText, /材料貫通/)
})

for (const reason of [
  'evidence_unavailable',
  'resource_limit_exceeded',
  'inconsistent_state',
] as const) {
  test(`${reason} remains an equally prominent safety hold`, () => {
    const view = presentNativeStaticCollision({
      kind: 'ready',
      diagnostic: {
        status: 'blocking',
        reason,
        expectedUnorderedFacePairs: reason === 'evidence_unavailable' ? 3 : null,
        provenPenetratingPairs: null,
        firstProvenPenetratingPair: null,
      },
    })

    assert.equal(view.dataStatus, 'indeterminate')
    assert.equal(view.badgeClass, 'is-indeterminate')
    assert.equal(view.requiresSafetyReview, true)
    assert.match(view.badgeText, /交差の可能性・判定保留/)
    assert.match(view.accessibleText, /安全確認済みとして扱わない/)
  })
}

test('checking, failed, and contradictory DTOs never look certified', () => {
  const checking = presentNativeStaticCollision({ kind: 'checking' })
  const waiting = presentNativeStaticCollision({ kind: 'waiting' })
  const failed = presentNativeStaticCollision({ kind: 'failed' })
  const contradictory = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      ...certified,
      status: 'unavailable',
      reason: null,
    },
  })
  const incompleteCertificate = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      ...certified,
      expectedUnorderedFacePairs: null,
    },
  })

  for (const view of [
    waiting,
    checking,
    failed,
    contradictory,
    incompleteCertificate,
  ]) {
    assert.notEqual(view.dataStatus, 'certified_nonblocking')
    assert.equal(view.requiresSafetyReview, true)
  }
})

test('English presents every exact-check outcome without changing wire status', () => {
  assert.deepEqual(
    presentNativeStaticCollision({ kind: 'idle' }, 'en'),
    {
      dataStatus: 'idle',
      badgeClass: 'is-idle',
      badgeText: 'Exact check | Waiting for pose',
      accessibleText:
        'The exact collision check is waiting for a stable displayed pose.',
      requiresSafetyReview: false,
    },
  )
  assert.match(
    presentNativeStaticCollision({ kind: 'waiting' }, 'en').accessibleText,
    /Do not treat this pose as safety-verified/u,
  )
  assert.equal(
    presentNativeStaticCollision({ kind: 'checking' }, 'en').badgeText,
    'Exact check | Checking',
  )
  assert.equal(
    presentNativeStaticCollision({ kind: 'failed' }, 'en').badgeText,
    'Exact check | Failed · safety review required',
  )

  const clear = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: certified,
  }, 'en')
  assert.equal(clear.dataStatus, 'certified_nonblocking')
  assert.match(clear.badgeText, /No zero-thickness surface penetration or overlap/u)

  const zeroThickness = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      status: 'blocking',
      reason: 'proven_zero_thickness_penetration',
      expectedUnorderedFacePairs: 3,
      provenPenetratingPairs: 2,
      firstProvenPenetratingPair: null,
    },
  }, 'en')
  assert.equal(zeroThickness.dataStatus, 'penetrating')
  assert.match(zeroThickness.badgeText, /penetration or overlap 2/u)

  const positiveThickness = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      status: 'blocking',
      reason: 'proven_positive_thickness_penetration',
      expectedUnorderedFacePairs: 3,
      provenPenetratingPairs: 1,
      firstProvenPenetratingPair: {
        firstFaceId: '00000000-0000-4000-8000-000000000001',
        secondFaceId: '00000000-0000-4000-8000-000000000002',
      },
    },
  }, 'en')
  assert.equal(
    positiveThickness.badgeText,
    'Exact check | Material penetration including paper thickness 1 · safety certification denied',
  )
  assert.match(
    positiveThickness.accessibleText,
    /material penetrations including paper thickness/u,
  )

  const reasonLabels = new Map([
    ['evidence_unavailable', 'Insufficient evidence'],
    ['resource_limit_exceeded', 'Resource limit'],
    ['inconsistent_state', 'Inconsistent state'],
  ] as const)
  for (const [reason, label] of reasonLabels) {
    const view = presentNativeStaticCollision({
      kind: 'ready',
      diagnostic: {
        status: 'blocking',
        reason,
        expectedUnorderedFacePairs: reason === 'evidence_unavailable' ? 3 : null,
        provenPenetratingPairs: null,
        firstProvenPenetratingPair: null,
      },
    }, 'en')
    assert.equal(view.dataStatus, 'indeterminate')
    assert.match(view.badgeText, new RegExp(label, 'u'))
    assert.match(view.accessibleText, /Do not treat this pose as safety-verified/u)
  }

  const unavailable = presentNativeStaticCollision({
    kind: 'ready',
    diagnostic: {
      status: 'unavailable',
      reason: 'pose_authority_unavailable',
      expectedUnorderedFacePairs: null,
      provenPenetratingPairs: null,
      firstProvenPenetratingPair: null,
    },
  }, 'en')
  assert.equal(unavailable.dataStatus, 'unavailable')
  assert.equal(
    unavailable.badgeText,
    'Exact check | Unavailable · safety review required',
  )
})

function pairDiagnostic(
  secondFaceNumber: number,
  disposition: CurrentStaticCollisionPairDiagnostic['disposition'],
): CurrentStaticCollisionPairDiagnostic {
  const common = {
    firstFaceId: '00000000-0000-4000-8000-000000000001',
    secondFaceId:
      `00000000-0000-4000-8000-${String(secondFaceNumber).padStart(12, '0')}`,
    strictTransversalDualGateProven: false,
    wholeFaceOverlapProven: false,
    sharedHingeBoundaryContactProven: false,
    sharedHingeSolidClassified: false,
  } as const
  if (disposition === 'separated') {
    return {
      ...common,
      topology: 'no_shared_feature',
      evidence: 'separated',
      policyDecision: 'separated',
      disposition,
    }
  }
  if (disposition === 'touching') {
    return {
      ...common,
      topology: 'no_shared_feature',
      evidence: 'point_contact',
      policyDecision: 'touching',
      disposition,
    }
  }
  if (disposition === 'allowed') {
    return {
      ...common,
      topology: 'shared_vertex',
      evidence: 'shared_feature_contact',
      policyDecision: 'allowed_shared_vertex_contact',
      disposition,
    }
  }
  if (disposition === 'penetrating') {
    return {
      ...common,
      topology: 'no_shared_feature',
      evidence: 'transversal_crossing',
      policyDecision: 'penetrating',
      disposition,
      strictTransversalDualGateProven: true,
    }
  }
  return {
    ...common,
    topology: 'shared_hinge_edge',
    evidence: 'indeterminate',
    policyDecision: 'indeterminate',
    disposition,
    sharedHingeSolidClassified: true,
  }
}
