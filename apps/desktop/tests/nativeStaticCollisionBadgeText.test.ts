import assert from 'node:assert/strict'
import test from 'node:test'
import { formatLocalizedText } from '../src/lib/i18n.ts'
import { NATIVE_COLLISION_BADGE_TEXT } from '../src/lib/nativeStaticCollisionBadgeText.ts'

test('native collision badge catalog is closed, deeply frozen, and preserves description', () => {
  assert.deepEqual(Object.keys(NATIVE_COLLISION_BADGE_TEXT), [
    'ariaLabel', 'retryingAriaLabel', 'retryAriaLabel', 'retrying', 'retry',
  ])
  assert.equal(Object.isFrozen(NATIVE_COLLISION_BADGE_TEXT), true)
  for (const text of Object.values(NATIVE_COLLISION_BADGE_TEXT)) assert.equal(Object.isFrozen(text), true)
  assert.equal(formatLocalizedText('ja', NATIVE_COLLISION_BADGE_TEXT.ariaLabel, {
    description: '貫通 1',
  }), 'native厳密衝突判定。貫通 1')
})
