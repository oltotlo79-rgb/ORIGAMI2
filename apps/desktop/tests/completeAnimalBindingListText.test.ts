import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  COMPLETE_ANIMAL_BINDING_LIST_TEXT as TEXT,
} from '../src/lib/completeAnimalBindingListText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'ariaLabel',
  'fourPartCount',
  'fivePartCount',
  'bindingRow',
] as const

test('complete-animal binding list catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.deepEqual(TEXT.fourPartCount, { ja: '四', en: 'Four' })
  assert.deepEqual(TEXT.fivePartCount, { ja: '五', en: 'Five' })
})

test('complete-animal binding placeholders are locale-equivalent and exact', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    ariaLabel: {
      ja: ['partCount'],
      en: ['partCount'],
    },
    bindingRow: {
      ja: ['id', 'count', 'length', 'thickness'],
      en: ['id', 'count', 'length', 'thickness'],
    },
  })

  assert.equal(
    formatLocalizedText('ja', TEXT.ariaLabel, {
      partCount: selectLocalizedText('ja', TEXT.fourPartCount),
    }),
    '完全動物の四部位binding寸法',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.ariaLabel, {
      partCount: selectLocalizedText('en', TEXT.fivePartCount),
    }),
    'Five complete-animal binding dimensions',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.bindingRow, {
      id: 4,
      count: 4,
      length: 400,
      thickness: 40,
    }),
    'binding 4・数 4・長さ 400・厚さ 40',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.bindingRow, {
      id: 5,
      count: 2,
      length: 500,
      thickness: 50,
    }),
    'Binding 5 · count 2 · length 500 · thickness 50',
  )
})

test('complete-animal binding list keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/CompleteAnimalBindingList.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /COMPLETE_ANIMAL_BINDING_LIST_TEXT as TEXT/u)
  assert.match(source, /formatLocalizedText\(locale, TEXT\.ariaLabel/u)
  assert.match(source, /formatLocalizedText\(locale, TEXT\.bindingRow/u)
  assert.match(source, /selectLocalizedText\(/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale\s*===/u)
  assert.doesNotMatch(source, /locale\s*!==/u)
})

function placeholderMap(value: typeof TEXT) {
  const result: Record<string, Record<'ja' | 'en', string[]>> = {}
  for (const key of KEYS) {
    const ja = placeholders(value[key].ja)
    const en = placeholders(value[key].en)
    assert.deepEqual(ja, en, key)
    if (ja.length > 0) result[key] = { ja, en }
  }
  return result
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
