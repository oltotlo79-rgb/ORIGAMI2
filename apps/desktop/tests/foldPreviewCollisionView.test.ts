import assert from 'node:assert/strict'
import test from 'node:test'

import {
  collisionBadgeClass,
  collisionBadgeText,
  collisionDataStatus,
  collisionPoseKey,
  collisionSummariesEqual,
  describeCollisionSummary,
  type CollisionSummary,
} from '../src/lib/foldPreviewCollisionView.ts'

type ReadyCollisionSummary = Extract<CollisionSummary, { kind: 'ready' }>

test('allowed hinge-model interactions remain informational instead of collisions', () => {
  const allowed = ready({
    totalCandidates: 2,
    hingeAdjacentCandidates: 2,
    narrowInteractions: 2,
    hingeInteractions: 2,
    hingeModelAllowedContacts: 1,
    hingeModelCorridorOverlaps: 1,
  })

  assert.equal(collisionDataStatus(allowed), 'hinge-model')
  assert.equal(collisionBadgeClass(allowed), 'has-hinge-candidates')
  assert.equal(
    collisionBadgeText(allowed),
    '許容折り目領域内重なり 1・境界接触 1',
  )
  assert.match(describeCollisionSummary(allowed), /ヒンジモデル許容 2/)
  assert.match(
    describeCollisionSummary(allowed, true),
    /モデルで許容した折り目境界接触1件、折り目領域内重なり1件/,
  )

  const boundaryOnly = ready({
    totalCandidates: 1,
    hingeAdjacentCandidates: 1,
    narrowInteractions: 1,
    hingeInteractions: 1,
    hingeModelAllowedContacts: 1,
  })
  assert.equal(collisionBadgeText(boundaryOnly), 'ヒンジ境界接触 1・他衝突 0')
})

test('outside-hinge penetrations and contacts use blocking collision states', () => {
  const contact = ready({
    totalCandidates: 1,
    hingeAdjacentCandidates: 1,
    narrowInteractions: 1,
    hingeInteractions: 1,
    hingeOutsideContacts: 1,
  })
  assert.equal(collisionDataStatus(contact), 'contact')
  assert.equal(collisionBadgeClass(contact), 'has-contact')
  assert.equal(collisionBadgeText(contact), '接触 1（ヒンジ外 1）・貫通 0')

  const penetration = ready({
    totalCandidates: 2,
    hingeAdjacentCandidates: 2,
    narrowInteractions: 2,
    hingeInteractions: 2,
    hingeOutsidePenetrations: 1,
    hingeOutsideContacts: 1,
  })
  assert.equal(collisionDataStatus(penetration), 'penetrating')
  assert.equal(collisionBadgeClass(penetration), 'has-penetrations')
  assert.equal(collisionBadgeText(penetration), '貫通 1（ヒンジ外 1）・接触 1')
})

test('penetration outranks indeterminate, which outranks contact', () => {
  const contact = ready({
    totalCandidates: 1,
    nonAdjacentContacts: 1,
    narrowInteractions: 1,
  })
  const indeterminate = {
    ...contact,
    indeterminateInteractions: 1,
  }
  const penetration = {
    ...indeterminate,
    nonAdjacentPenetrations: 1,
  }

  assert.equal(collisionDataStatus(contact), 'contact')
  assert.equal(collisionBadgeClass(contact), 'has-contact')
  assert.equal(collisionDataStatus(indeterminate), 'indeterminate')
  assert.equal(collisionBadgeClass(indeterminate), 'has-indeterminate')
  assert.equal(collisionDataStatus(penetration), 'penetrating')
  assert.equal(collisionBadgeClass(penetration), 'has-penetrations')
})

test('summary equality observes every hinge-policy presentation field', () => {
  const baseline = ready()
  assert.equal(collisionSummariesEqual(baseline, { ...baseline }), true)
  assert.equal(collisionSummariesEqual(null, baseline), false)

  const fields = [
    'hingeModelAllowedContacts',
    'hingeModelCorridorOverlaps',
    'hingeOutsidePenetrations',
    'hingeOutsideContacts',
    'hingeUnresolvedInteractions',
  ] as const
  for (const field of fields) {
    assert.equal(
      collisionSummariesEqual(baseline, { ...baseline, [field]: baseline[field] + 1 }),
      false,
      field,
    )
  }

  const unavailable: CollisionSummary = { kind: 'unavailable', requestKey: 'pose' }
  assert.equal(collisionSummariesEqual(unavailable, { ...unavailable }), true)
  assert.equal(collisionSummariesEqual(
    unavailable,
    { kind: 'unavailable', requestKey: 'next-pose' },
  ), false)
})

test('pose keys preserve null physical thickness and canonicalize hinge-angle order', () => {
  const model = {
    projectId: 'project',
    revision: 7,
    kind: 'fold_graph',
  } as const
  const reverseOrder = collisionPoseKey(model, 'root', null, 52, [
    { edgeId: 'hinge-b', angleDegrees: 80 },
    { edgeId: 'hinge-a', angleDegrees: 25 },
  ])
  const forwardOrder = collisionPoseKey(model, 'root', null, 52, [
    { edgeId: 'hinge-a', angleDegrees: 25 },
    { edgeId: 'hinge-b', angleDegrees: 80 },
  ])

  assert.equal(reverseOrder, forwardOrder)
  assert.notEqual(forwardOrder, collisionPoseKey(model, 'root', 0, 52, [
    { edgeId: 'hinge-a', angleDegrees: 25 },
    { edgeId: 'hinge-b', angleDegrees: 80 },
  ]))
  assert.notEqual(forwardOrder, collisionPoseKey(model, 'root', null, 53, [
    { edgeId: 'hinge-a', angleDegrees: 25 },
    { edgeId: 'hinge-b', angleDegrees: 80 },
  ]))
  assert.equal(JSON.parse(forwardOrder)[4], null)
  assert.equal(collisionPoseKey(null, null, null, 0, undefined), '')
})

test('pending, unavailable, clear, and detailed descriptions retain safety wording', () => {
  assert.equal(describeCollisionSummary(null), '衝突判定中')
  assert.equal(describeCollisionSummary(null, true), '現在姿勢の衝突候補を判定中')
  const unavailable: CollisionSummary = { kind: 'unavailable', requestKey: 'pose' }
  assert.equal(describeCollisionSummary(unavailable), '衝突判定不能')
  assert.equal(
    describeCollisionSummary(unavailable, true),
    '現在姿勢の衝突判定は利用できません',
  )
  assert.match(describeCollisionSummary(ready(), true), /連続運動中の衝突は未検証/)

  const detailed = ready({
    totalCandidates: 5,
    hingeAdjacentCandidates: 4,
    narrowInteractions: 5,
    nonAdjacentPenetrations: 1,
    nonAdjacentContacts: 1,
    hingeInteractions: 4,
    hingeModelAllowedContacts: 1,
    hingeModelCorridorOverlaps: 1,
    hingeOutsidePenetrations: 1,
    hingeOutsideContacts: 1,
    hingeUnresolvedInteractions: 1,
    indeterminateInteractions: 1,
  })
  const accessible = describeCollisionSummary(detailed, true)
  assert.match(accessible, /中央面基準の共有ヒンジモデル外貫通1件/)
  assert.match(accessible, /共有ヒンジモデル外接触1件/)
  assert.match(accessible, /現在姿勢に対する中央面基準の近似判定/)
  assert.match(accessible, /実際の折り癖、層ずれ、連続運動中の衝突は未検証/)
})

function ready(overrides: Partial<ReadyCollisionSummary> = {}): ReadyCollisionSummary {
  return {
    kind: 'ready',
    requestKey: 'pose',
    totalCandidates: 0,
    nonAdjacentCandidates: 0,
    hingeAdjacentCandidates: 0,
    narrowInteractions: 0,
    nonAdjacentPenetrations: 0,
    nonAdjacentContacts: 0,
    hingeInteractions: 0,
    hingeModelAllowedContacts: 0,
    hingeModelCorridorOverlaps: 0,
    hingeOutsidePenetrations: 0,
    hingeOutsideContacts: 0,
    hingeUnresolvedInteractions: 0,
    indeterminateInteractions: 0,
    ...overrides,
  }
}
