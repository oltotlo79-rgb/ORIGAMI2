import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { selectLocalizedText } from '../src/lib/i18n.ts'
import { LENGTH_UNIT_PRESENTATION_TEXT } from '../src/lib/lengthUnitText.ts'

test('length unit presentation catalog is closed and deeply frozen', () => {
  assert.deepEqual(
    Object.keys(LENGTH_UNIT_PRESENTATION_TEXT),
    ['numberLocale', 'paperEdgeRatio', 'unavailable'],
  )
  assert.equal(Object.isFrozen(LENGTH_UNIT_PRESENTATION_TEXT), true)
  for (const text of Object.values(LENGTH_UNIT_PRESENTATION_TEXT)) {
    assert.deepEqual(Object.keys(text), ['ja', 'en'])
    assert.equal(Object.isFrozen(text), true)
  }
})

test('length unit presentation preserves exact copy and Japanese fallback', () => {
  assert.deepEqual(LENGTH_UNIT_PRESENTATION_TEXT.paperEdgeRatio, {
    ja: '紙辺比',
    en: 'paper-edge ratio',
  })
  assert.deepEqual(LENGTH_UNIT_PRESENTATION_TEXT.unavailable, {
    ja: '計測不可',
    en: 'Unavailable',
  })
  assert.equal(
    selectLocalizedText(
      'unsupported',
      LENGTH_UNIT_PRESENTATION_TEXT.numberLocale,
    ),
    'ja-JP',
  )
})

test('length unit formatting delegates locale choice to the catalog', () => {
  const source = readFileSync(
    new URL('../src/lib/lengthUnit.ts', import.meta.url),
    'utf8',
  )
  assert.doesNotMatch(source, /locale\s*[!=]==?\s*['"](?:ja|en)['"]/u)
  assert.doesNotMatch(source, /['"](?:ja|en)['"]\s*\?/u)
  assert.match(source, /LENGTH_UNIT_PRESENTATION_TEXT/u)
  assert.match(source, /selectLocalizedText/u)
})
