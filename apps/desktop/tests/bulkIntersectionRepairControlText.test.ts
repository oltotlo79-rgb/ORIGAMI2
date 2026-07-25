import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  BULK_INTERSECTION_REPAIR_CONTROL_TEXT as TEXT,
} from '../src/lib/bulkIntersectionRepairControlText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

const KEYS = ['repairing', 'repairAll', 'confirmation'] as const

test('bulk intersection repair catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.deepEqual(TEXT.repairing, {
    ja: '一括修復中…',
    en: 'Repairing…',
  })
})

test('bulk intersection repair placeholders are locale-equivalent', () => {
  for (const key of KEYS) {
    assert.deepEqual(
      placeholders(TEXT[key].ja),
      placeholders(TEXT[key].en),
      key,
    )
  }
  assert.deepEqual(placeholders(TEXT.repairing.ja), [])
  assert.deepEqual(placeholders(TEXT.repairAll.ja), ['count'])
  assert.deepEqual(placeholders(TEXT.confirmation.ja), ['count'])
  assert.equal(
    formatLocalizedText('ja', TEXT.repairAll, { count: 16 }),
    '交差を一括修復（16件）',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.confirmation, { count: 8 }),
    'Repair 8 unsplit intersections as one undoable edit?',
  )
})

test('bulk intersection repair component keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/BulkIntersectionRepairControl.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /BULK_INTERSECTION_REPAIR_CONTROL_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale\s*===/u)
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
