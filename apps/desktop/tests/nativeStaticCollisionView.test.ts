import assert from 'node:assert/strict'
import test from 'node:test'
import {
  presentNativeStaticCollision,
  selectBoundNativeStaticCollisionView,
  type CurrentStaticCollisionDiagnostic,
} from '../src/lib/nativeStaticCollisionView.ts'

const certified: CurrentStaticCollisionDiagnostic = {
  status: 'certified_nonblocking',
  reason: null,
  expectedUnorderedFacePairs: 0,
  provenPenetratingPairs: 0,
  firstProvenPenetratingPair: null,
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
