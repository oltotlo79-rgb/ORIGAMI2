import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  EFFECTIVE_CUT_DIAGNOSTIC_PANEL_TEXT as TEXT,
} from '../src/lib/effectiveCutDiagnosticPanelText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'ariaLabel',
  'title',
  'explanation',
  'loading',
  'reloadCandidates',
  'unavailable',
  'reload',
  'candidate',
  'faceCount',
  'removalClosure',
  'dependencies',
  'running',
  'diagnoseSelection',
  'cancel',
  'sourceFlatPairs',
  'indeterminate',
  'multiHingeCorridorUnproved',
] as const

test('effective-cut diagnostic catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(
    selectLocalizedText('ja', TEXT.title),
    '有効カット診断（読み取り専用）',
  )
  assert.equal(
    selectLocalizedText('en', TEXT.title),
    'Effective-cut diagnostic (read-only)',
  )
})

test('effective-cut diagnostic placeholders are locale-equivalent', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    candidate: { ja: ['index'], en: ['index'] },
    faceCount: { ja: ['count'], en: ['count'] },
    dependencies: { ja: ['count'], en: ['count'] },
  })
  assert.equal(
    formatLocalizedText('ja', TEXT.candidate, { index: 2 }),
    '候補 2',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.dependencies, { count: 3 }),
    ' (+3 dependencies)',
  )
})

test('effective-cut diagnostic component keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/EffectiveCutDiagnosticPanel.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(
    source,
    /EFFECTIVE_CUT_DIAGNOSTIC_PANEL_TEXT as TEXT/u,
  )
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\bja\b/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
})

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
