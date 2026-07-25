import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  GENERIC_TARGET_BINDING_LIST_TEXT as TEXT,
} from '../src/lib/genericTargetBindingListText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'ariaLabel',
  'bindingRow',
  'symmetryAsymmetric',
  'symmetryBilateral',
] as const

test('generic target binding list catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(
    selectLocalizedText('ja', TEXT.ariaLabel),
    '上限付き汎用対象binding寸法',
  )
  assert.equal(
    selectLocalizedText('en', TEXT.ariaLabel),
    'Bounded generic target binding dimensions',
  )
  assert.equal(
    selectLocalizedText('ja', TEXT.symmetryAsymmetric),
    '非対称単独',
  )
  assert.equal(
    selectLocalizedText('en', TEXT.symmetryBilateral),
    'bilateral',
  )
})

test('generic target binding row placeholders are locale-equivalent', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    bindingRow: {
      ja: ['id', 'symmetry', 'count', 'length', 'thickness'],
      en: ['id', 'symmetry', 'count', 'length', 'thickness'],
    },
  })
})

test('generic target binding rows stay byte-exact in both locales', () => {
  assert.equal(
    bindingRow('ja', 1, 'symmetryAsymmetric', 1, 100, 10),
    'binding 1・非対称単独・数 1・長さ 100・厚さ 10',
  )
  assert.equal(
    bindingRow('en', 2, 'symmetryBilateral', 4, 250, 25),
    'Binding 2 · bilateral · count 4 · length 250 · thickness 25',
  )
})

test('generic target binding list keeps all display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/GenericTargetBindingList.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /GENERIC_TARGET_BINDING_LIST_TEXT as TEXT/u)
  assert.match(source, /formatLocalizedText\(locale, TEXT\.bindingRow/u)
  assert.match(source, /selectLocalizedText\(locale, TEXT\.ariaLabel\)/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /Bounded generic target binding dimensions/u)
  assert.doesNotMatch(source, /asymmetric single/u)
  assert.doesNotMatch(source, /· count/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale === 'ja'/u)
  assert.doesNotMatch(source, /locale !== 'ja'/u)
})

function bindingRow(
  locale: 'ja' | 'en',
  id: number,
  symmetryKey: 'symmetryAsymmetric' | 'symmetryBilateral',
  count: number,
  length: number,
  thickness: number,
) {
  return formatLocalizedText(locale, TEXT.bindingRow, {
    id,
    symmetry: selectLocalizedText(locale, TEXT[symmetryKey]),
    count,
    length,
    thickness,
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
