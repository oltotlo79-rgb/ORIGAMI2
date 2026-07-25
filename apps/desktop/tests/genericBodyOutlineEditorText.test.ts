import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  GENERIC_BODY_OUTLINE_EDITOR_TEXT as TEXT,
} from '../src/lib/genericBodyOutlineEditorText.ts'

const KEYS = [
  'legend',
  'outlineMode',
  'outlineModeAria',
  'symmetricOption',
  'generalOption',
  'outlinePoints',
  'outlinePointsAria',
  'applyOutline',
  'clearOutline',
  'invalidSymmetricOutline',
  'invalidGeneralOutline',
] as const

test('generic body outline editor catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(TEXT.legend.ja, '左右対称の胴体輪郭')
  assert.equal(TEXT.legend.en, 'Symmetric body outline')
  assert.equal(TEXT.outlinePointsAria.ja, '胴体輪郭点')
  assert.equal(TEXT.outlinePointsAria.en, 'Body outline points')
})

test('generic body outline editor placeholders are locale-equivalent', () => {
  for (const key of KEYS) {
    assert.deepEqual(
      placeholders(TEXT[key].ja),
      placeholders(TEXT[key].en),
      key,
    )
  }
  assert.deepEqual(
    Object.fromEntries(KEYS.map((key) => [key, placeholders(TEXT[key].ja)])),
    Object.fromEntries(KEYS.map((key) => [key, []])),
  )
})

test('generic body outline editor keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/GenericBodyOutlineEditor.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /GENERIC_BODY_OUTLINE_EDITOR_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ヶ一-龠]/u)
  assert.doesNotMatch(source, /locale\s*===/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
