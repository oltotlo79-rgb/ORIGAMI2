import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  PROTRUSION_DIMENSION_EDITOR_TEXT as TEXT,
} from '../src/lib/protrusionDimensionEditorText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'bindingSummary',
  'symmetryNone',
  'symmetryBilateral',
  'symmetryRadial',
  'partKind',
  'symmetry',
  'count',
  'rootWidth',
  'tipWidth',
  'length',
  'bilateralSpacing',
  'thickness',
  'mountVertical',
  'mountForeAft',
  'directionHorizontal',
  'directionVertical',
  'curvature',
  'motionMinimum',
  'motionMaximum',
  'joint',
  'side',
  'priority',
  'rootWidthLabel',
  'tipWidthLabel',
  'lengthLabel',
  'bilateralSpacingLabel',
  'thicknessLabel',
  'mountVerticalLabel',
  'mountForeAftLabel',
  'curvatureLabel',
  'motionMinimumLabel',
  'motionMaximumLabel',
  'ariaBinding',
  'ariaBindingMillimetres',
  'ariaBindingDegrees',
  'jointFixed',
  'jointHinge',
  'jointBall',
  'sideFront',
  'sideBack',
  'sideEither',
  'remove',
  'moveUp',
  'moveDown',
] as const

test('protrusion dimension catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(selectLocalizedText('ja', TEXT.curvatureLabel), '曲率 (度)')
  assert.equal(
    selectLocalizedText('en', TEXT.curvatureLabel),
    'Curvature (degrees)',
  )
  assert.equal(
    selectLocalizedText('ja', TEXT.rootWidthLabel),
    '根元幅 (mm、任意)',
  )
  assert.equal(
    selectLocalizedText('en', TEXT.rootWidthLabel),
    'Root width (mm, optional)',
  )
  assert.equal(selectLocalizedText('ja', TEXT.sideEither), 'どちらでも可')
  assert.equal(selectLocalizedText('en', TEXT.sideEither), 'Either')
  assert.equal(selectLocalizedText('ja', TEXT.symmetryRadial), '放射対称')
  assert.equal(selectLocalizedText('en', TEXT.symmetryRadial), 'Radial')
  assert.equal(selectLocalizedText('ja', TEXT.count), '個数')
  assert.equal(selectLocalizedText('en', TEXT.count), 'Count')
})

test('protrusion dimension placeholders are locale-equivalent', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    bindingSummary: {
      ja: ['id', 'symmetry', 'count'],
      en: ['id', 'symmetry', 'count'],
    },
    ariaBinding: { ja: ['name', 'id'], en: ['name', 'id'] },
    ariaBindingMillimetres: { ja: ['name', 'id'], en: ['name', 'id'] },
    ariaBindingDegrees: { ja: ['name', 'id'], en: ['name', 'id'] },
  })
})

test('protrusion dimension summary and ARIA names stay byte-exact', () => {
  assert.equal(
    formatLocalizedText('ja', TEXT.bindingSummary, {
      id: 1,
      symmetry: selectLocalizedText('ja', TEXT.symmetryBilateral),
      count: 2,
    }),
    'binding 1・左右対称・数 2',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.bindingSummary, {
      id: 1,
      symmetry: selectLocalizedText('en', TEXT.symmetryNone),
      count: 1,
    }),
    'Binding 1 · Asymmetric single · count 1',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.bindingSummary, {
      id: 3,
      symmetry: selectLocalizedText('en', TEXT.symmetryRadial),
      count: 3,
    }),
    'Binding 3 · Radial · count 3',
  )
  assert.equal(bindingName('en', TEXT.ariaBinding, TEXT.symmetry, 1),
    'Symmetry binding 1')
  assert.equal(bindingName('en', TEXT.ariaBinding, TEXT.partKind, 1),
    'Part kind binding 1')
  assert.equal(bindingName('en', TEXT.ariaBinding, TEXT.count, 1),
    'Count binding 1')
  assert.equal(
    bindingName('en', TEXT.ariaBindingMillimetres, TEXT.length, 1),
    'Length binding 1 (mm)',
  )
  assert.equal(
    bindingName('en', TEXT.ariaBindingMillimetres, TEXT.bilateralSpacing, 1),
    'Bilateral spacing binding 1 (mm)',
  )
  assert.equal(
    bindingName('ja', TEXT.ariaBindingMillimetres, TEXT.rootWidth, 1),
    '根元幅 binding 1 (mm)',
  )
  assert.equal(
    bindingName('ja', TEXT.ariaBindingMillimetres, TEXT.tipWidth, 1),
    '先端幅 binding 1 (mm)',
  )
  assert.equal(
    bindingName('en', TEXT.ariaBindingDegrees, TEXT.motionMinimum, 1),
    'Motion minimum binding 1 (degrees)',
  )
  assert.equal(
    bindingName('ja', TEXT.ariaBindingDegrees, TEXT.curvature, 1),
    '曲率 binding 1 (度)',
  )
})

test('protrusion dimension editor keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/ProtrusionDimensionEditor.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /PROTRUSION_DIMENSION_EDITOR_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale === 'ja'/u)
  assert.doesNotMatch(source, /locale !== 'ja'/u)
})

function bindingName(
  locale: 'ja' | 'en',
  template: Readonly<Record<'ja' | 'en', string>>,
  name: Readonly<Record<'ja' | 'en', string>>,
  id: number,
) {
  return formatLocalizedText(locale, template, {
    name: selectLocalizedText(locale, name),
    id,
  })
}

function placeholderMap(
  value: Readonly<Record<string, Readonly<Record<'ja' | 'en', string>>>>,
) {
  return Object.fromEntries(
    Object.entries(value).flatMap(([key, localized]) => {
      const ja = placeholders(localized.ja)
      const en = placeholders(localized.en)
      return ja.length === 0 && en.length === 0
        ? []
        : [[key, { ja, en }]]
    }),
  )
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
