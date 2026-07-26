import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  CURRENT_NON_FLAT_LAYER_ORDER_VIEWER_TEXT as TEXT,
} from '../src/lib/currentNonFlatLayerOrderViewerText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'title',
  'readOnlyBadge',
  'noMutationAuthority',
  'loading',
  'absent',
  'failedStaleAuthority',
  'failedInvalidEvidence',
  'failedResourceLimit',
  'failedInternalFailure',
  'reload',
  'worldPaneHeading',
  'worldPaneAria',
  'projectionPaneHeading',
  'projectionPaneAria',
  'axisX',
  'axisY',
  'axisZ',
  'axisU',
  'axisV',
  'droppedAxisX',
  'droppedAxisY',
  'droppedAxisZ',
  'lowerFace',
  'upperFace',
  'selectedFace',
  'selectedCell',
  'selectFace',
  'selectCell',
  'exactBoundaryDigest',
  'faceCount',
  'cellCount',
  'zeroCellWarning',
  'distinctCoordinateSystems',
] as const

test('non-flat viewer catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(selectLocalizedText('ja', TEXT.readOnlyBadge), '読み取り専用')
  assert.equal(selectLocalizedText('en', TEXT.readOnlyBadge), 'Read-only')
  assert.equal(
    selectLocalizedText('en', TEXT.zeroCellWarning),
    'There are no overlap cells to show. This is not a proof that nothing collides.',
  )
})

test('non-flat viewer placeholders are locale-equivalent', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    selectFace: { ja: ['label'], en: ['label'] },
    selectCell: { ja: ['label'], en: ['label'] },
    faceCount: { ja: ['count'], en: ['count'] },
    cellCount: { ja: ['count'], en: ['count'] },
  })
  assert.equal(
    formatLocalizedText('ja', TEXT.faceCount, { count: 2 }),
    '面 2 件',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.cellCount, { count: 3 }),
    '3 overlap cells',
  )
})

test('the two panes never claim one coordinate system', () => {
  for (const locale of ['ja', 'en'] as const) {
    const world = selectLocalizedText(locale, TEXT.worldPaneAria)
    const projection = selectLocalizedText(locale, TEXT.projectionPaneAria)
    assert.notEqual(world, projection)
    assert.match(world, /World XYZ|world XYZ/u)
    assert.match(projection, /Projection UV|UV/u)
    assert.match(selectLocalizedText(locale, TEXT.distinctCoordinateSystems), /UV/u)
  }
})

test('the non-flat viewer keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/CurrentNonFlatLayerOrderViewer.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /CURRENT_NON_FLAT_LAYER_ORDER_VIEWER_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale === 'ja'/u)
  // The viewer must not expose any project-mutating control.
  for (const forbidden of [/onApplied/u, /refreshSnapshot/u, /applyStackedFold/u]) {
    assert.doesNotMatch(source, forbidden)
  }
})

function placeholderMap(
  value: Readonly<Record<string, Readonly<Record<'ja' | 'en', string>>>>,
) {
  return Object.fromEntries(
    Object.entries(value).flatMap(([key, localized]) => {
      const ja = placeholders(localized.ja)
      const en = placeholders(localized.en)
      return ja.length === 0 && en.length === 0 ? [] : [[key, { ja, en }]]
    }),
  )
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)].map((match) => match[1])
}
