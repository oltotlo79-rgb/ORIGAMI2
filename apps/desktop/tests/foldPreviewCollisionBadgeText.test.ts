import assert from 'node:assert/strict'
import test from 'node:test'

import { FOLD_PREVIEW_COLLISION_BADGE_TEXT } from '../src/lib/foldPreviewCollisionBadgeText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

test('fold preview collision badge catalog is closed, deeply frozen, and preserves its placeholder', () => {
  assert.deepEqual(Object.keys(FOLD_PREVIEW_COLLISION_BADGE_TEXT), [
    'warningAriaLabel',
    'informationAriaLabel',
    'visible',
  ])
  assert.equal(Object.isFrozen(FOLD_PREVIEW_COLLISION_BADGE_TEXT), true)
  for (const text of Object.values(FOLD_PREVIEW_COLLISION_BADGE_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.equal(
    formatLocalizedText(
      'ja',
      FOLD_PREVIEW_COLLISION_BADGE_TEXT.warningAriaLabel,
      { text: '貫通 1' },
    ),
    '安全上の警告。表示姿勢。貫通 1',
  )
  assert.equal(
    formatLocalizedText(
      'en',
      FOLD_PREVIEW_COLLISION_BADGE_TEXT.visible,
      { text: 'Contact 1' },
    ),
    'Current pose | Contact 1',
  )
})
