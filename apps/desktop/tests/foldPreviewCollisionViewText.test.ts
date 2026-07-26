import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  COLLISION_BADGE_TEXT,
  COLLISION_VIEW_TEXT,
} from '../src/lib/foldPreviewCollisionViewText.ts'

const VIEW_KEYS = [
  'pending',
  'pendingAccessible',
  'unavailable',
  'unavailableAccessible',
  'clearSeparateAccessible',
  'clearSeparate',
  'clearUnverifiedAccessible',
  'clearUnverified',
  'limitationSeparate',
  'limitationUnverified',
  'safetyReview',
  'detailedAccessible',
  'detailed',
] as const

const BADGE_KEYS = [
  'pending',
  'unavailable',
  'suffix',
  'penetrating',
  'holdWithContact',
  'contact',
  'sharedVertex',
  'flatStack',
  'corridor',
  'hingeContact',
  'clear',
  'noNarrowInteraction',
  'layerOffsetHold',
  'hingeDetail',
  'indeterminate',
  'hingeUnresolved',
] as const

const EXPECTED_PLACEHOLDERS = new Map<string, readonly string[]>([
  [
    'view.detailedAccessible',
    [
      'hingeLayerOffsetUnmodeled',
      'hingeModelAllowedContacts',
      'hingeModelCorridorOverlaps',
      'hingeModelFlatSurfaceStacks',
      'hingeOutsideContacts',
      'hingeOutsidePenetrations',
      'hingeUnresolvedInteractions',
      'indeterminateInteractions',
      'limitation',
      'narrowInteractions',
      'nonAdjacentContacts',
      'nonAdjacentPenetrations',
      'safetyReview',
      'topologyModelCount',
      'totalCandidates',
    ],
  ],
  [
    'view.detailed',
    [
      'contactCount',
      'hingeModelCount',
      'hingeUnresolvedInteractions',
      'indeterminateInteractions',
      'narrowInteractions',
      'penetrationCount',
      'topologyModelCount',
      'totalCandidates',
    ],
  ],
  ['badge.suffix', ['detail']],
  [
    'badge.penetrating',
    [
      'contactCount',
      'hingeOutsidePenetrations',
      'holdSuffix',
      'penetrationCount',
    ],
  ],
  ['badge.holdWithContact', ['contactCount', 'holdText']],
  ['badge.contact', ['contactCount', 'hingeOutsideContacts']],
  ['badge.sharedVertex', ['count']],
  ['badge.flatStack', ['count']],
  ['badge.corridor', ['contacts', 'overlaps']],
  ['badge.hingeContact', ['count']],
  ['badge.noNarrowInteraction', ['count']],
  ['badge.layerOffsetHold', ['count']],
  ['badge.hingeDetail', ['count']],
  ['badge.indeterminate', ['count', 'hingeDetail']],
  ['badge.hingeUnresolved', ['count']],
])

const PLACEHOLDER = /\{([A-Za-z][A-Za-z0-9_]*)\}/gu

function placeholders(value: string): string[] {
  return [...value.matchAll(PLACEHOLDER)]
    .map((match) => match[1]!)
    .sort()
}

function assertDeepFrozen(
  value: unknown,
  seen = new Set<object>(),
): void {
  if (!value || typeof value !== 'object' || seen.has(value)) return
  seen.add(value)
  assert.equal(Object.isFrozen(value), true)
  for (const nested of Object.values(value)) {
    assertDeepFrozen(nested, seen)
  }
}

test('collision view catalogs are closed, deeply frozen, and copy-exact', () => {
  assert.deepEqual(Object.keys(COLLISION_VIEW_TEXT), VIEW_KEYS)
  assert.deepEqual(Object.keys(COLLISION_BADGE_TEXT), BADGE_KEYS)
  assertDeepFrozen(COLLISION_VIEW_TEXT)
  assertDeepFrozen(COLLISION_BADGE_TEXT)

  assert.equal(
    COLLISION_BADGE_TEXT.pending,
    COLLISION_VIEW_TEXT.pending,
  )
  assert.equal(
    COLLISION_BADGE_TEXT.unavailable,
    COLLISION_VIEW_TEXT.unavailable,
  )

  assert.equal(
    createHash('sha256')
      .update(JSON.stringify({
        view: COLLISION_VIEW_TEXT,
        badge: COLLISION_BADGE_TEXT,
      }), 'utf8')
      .digest('hex'),
    '0388f7a6da9cbaf472a525c3a5ebebb298fe805d876a1a5fa53c53703e4f5aae',
  )
})

test('every locale pair has exact Japanese-English placeholder parity', () => {
  const catalogs = [
    ['view', COLLISION_VIEW_TEXT],
    ['badge', COLLISION_BADGE_TEXT],
  ] as const
  const actualPlaceholderKeys: string[] = []

  for (const [catalogName, catalog] of catalogs) {
    for (const [key, text] of Object.entries(catalog)) {
      assert.deepEqual(Object.keys(text), ['ja', 'en'])
      const ja = placeholders(text.ja)
      const en = placeholders(text.en)
      assert.deepEqual(en, ja, `${catalogName}.${key}`)
      assert.deepEqual(
        ja,
        EXPECTED_PLACEHOLDERS.get(`${catalogName}.${key}`) ?? [],
        `${catalogName}.${key}`,
      )
      if (ja.length > 0) actualPlaceholderKeys.push(`${catalogName}.${key}`)
    }
  }

  assert.deepEqual(
    actualPlaceholderKeys,
    [...EXPECTED_PLACEHOLDERS.keys()],
  )
})

test('collision view logic consumes presentation copy only from its catalog', async () => {
  const source = await readFile(
    new URL('../src/lib/foldPreviewCollisionView.ts', import.meta.url),
    'utf8',
  )

  assert.match(
    source,
    /from '\.\/foldPreviewCollisionViewText\.ts'/u,
  )
  assert.doesNotMatch(source, /\b(?:ja|en)\s*:/u)
  assert.doesNotMatch(source, /[\u3040-\u30ff\u3400-\u9fff]/u)
  for (const catalog of [COLLISION_VIEW_TEXT, COLLISION_BADGE_TEXT]) {
    for (const text of Object.values(catalog)) {
      assert.equal(source.includes(text.ja), false)
      assert.equal(source.includes(text.en), false)
    }
  }
})
