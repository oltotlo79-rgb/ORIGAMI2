import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const collision = readFileSync('../../crates/ori-collision/src/static_collision.rs', 'utf8')
const view = readFileSync('src/lib/nativeStaticCollisionView.ts', 'utf8')
const native = readFileSync('src/lib/nativeStaticCollisionNative.ts', 'utf8')

test('an authenticated zero-thickness shared-hinge full fold stays non-penetrating without layer order', () => {
  assert.match(collision, /shared_hinge_flat_stack_proven/u)
  assert.match(collision, /IntersectionEvidenceV2::SharedFeatureFlatStack/u)
  assert.match(collision, /TopologyContactDecision::RequiresHingeModel/u)
  assert.match(collision, /shared_hinge_flat_stack_proven \{[\s\S]*?StaticCollisionPairDisposition::Indeterminate/u)
})

test('the strict frontend accepts and explains the bounded flat-stack evidence', () => {
  assert.match(native, /evidence === 'shared_feature_flat_stack'/u)
  assert.match(view, /shared_feature_flat_stack: '共有要素の平坦積層'/u)
  assert.match(view, /shared_feature_flat_stack: 'shared-feature flat stack'/u)
  assert.match(view, /requires_hinge_model: 'ヒンジモデル必須'/u)
  assert.match(view, /requires_hinge_model: 'hinge model required'/u)
})
