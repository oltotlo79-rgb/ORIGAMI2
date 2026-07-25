import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  RECENT_PROJECTS_CONTROL_TEXT as TEXT,
} from '../src/lib/recentProjectsControlText.ts'

const KEYS = [
  'title',
  'empty',
  'listUnavailable',
  'invalidated',
  'openFailed',
] as const

test('recent projects control catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }

  assert.equal(TEXT.title.ja, '最近使った作品')
  assert.equal(TEXT.title.en, 'Recent projects')
  assert.equal(
    TEXT.invalidated.ja,
    '作品が移動または置換されたため一覧から削除しました。',
  )
  assert.equal(
    TEXT.invalidated.en,
    'The project moved or was replaced and was removed.',
  )
})

test('recent projects control placeholders are locale-equivalent', () => {
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

test('recent projects control keeps display copy in the typed catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/RecentProjectsControl.tsx',
      import.meta.url,
    ),
    'utf8',
  )

  assert.match(source, /RECENT_PROJECTS_CONTROL_TEXT as TEXT/u)
  assert.match(source, /selectLocalizedText\(locale, TEXT\[key\]\)/u)
  assert.doesNotMatch(source, /locale\s*===/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /[\u3040-\u30ff\u3400-\u9fff]/u)
  assert.doesNotMatch(
    source,
    /Recent projects|No recent projects|project moved|could not be opened safely/u,
  )
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
