import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  BEGINNER_GRID_PROGRESS_STATUS_TEXT as TEXT,
} from '../src/lib/beginnerGridProgressStatusText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

const KEYS = ['groupAriaLabel', 'cancel', 'progress'] as const

test('beginner grid progress catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(TEXT.groupAriaLabel.ja, '候補生成と局所改善の進捗')
  assert.equal(
    TEXT.groupAriaLabel.en,
    'Candidate generation and local refinement progress',
  )
})

test('beginner grid progress placeholders are locale-equivalent', () => {
  for (const key of KEYS) {
    assert.deepEqual(
      placeholders(TEXT[key].ja),
      placeholders(TEXT[key].en),
      key,
    )
  }
  assert.deepEqual(placeholders(TEXT.progress.ja), [
    'enumerated',
    'refined',
    'checked',
  ])
  const values = { enumerated: 27, refined: 24, checked: 3 }
  assert.equal(
    formatLocalizedText('ja', TEXT.progress, values),
    '列挙 27/27・局所改善 24/24・大域検証 3/3',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.progress, values),
    'Enumerated 27/27 · refined 24/24 · globally checked 3/3',
  )
})

test('beginner grid progress component keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/BeginnerGridProgressStatus.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /BEGINNER_GRID_PROGRESS_STATUS_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale\s*===/u)
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
