import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  COMPLETE_INSECT_BINDING_LIST_TEXT as TEXT,
} from '../src/lib/completeInsectBindingListText.ts'
import { formatLocalizedText, selectLocalizedText } from '../src/lib/i18n.ts'

const KEYS = [
  'listAriaLabel',
  'wingPair',
  'antennaPair',
  'legPair1',
  'legPair2',
  'legPair3',
  'bindingRow',
] as const

test('complete insect binding catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.deepEqual(TEXT.wingPair, { ja: '翼の組', en: 'Wing pair' })
  assert.deepEqual(TEXT.antennaPair, { ja: '触角の組', en: 'Antenna pair' })
  assert.deepEqual(TEXT.legPair3, { ja: '脚の組3', en: 'Leg pair 3' })
})

test('complete insect binding placeholders are locale-equivalent', () => {
  for (const key of KEYS) {
    assert.deepEqual(
      placeholders(TEXT[key].ja),
      placeholders(TEXT[key].en),
      key,
    )
  }
  assert.deepEqual(placeholders(TEXT.bindingRow.ja), [
    'label',
    'bindingId',
    'length',
    'thickness',
  ])
  const values = {
    label: selectLocalizedText('en', TEXT.wingPair),
    bindingId: 1,
    length: 100,
    thickness: 10,
  }
  assert.equal(
    formatLocalizedText('en', TEXT.bindingRow, values),
    'Wing pair · binding 1 · length 100 · thickness 10',
  )
})

test('complete insect binding list keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/CompleteInsectBindingList.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /COMPLETE_INSECT_BINDING_LIST_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale\s*===/u)
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
