import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  PROOF_SCOPE_SUMMARY_TEXT as TEXT,
} from '../src/lib/proofScopeSummaryText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'proofCoverage',
  'separateProofs',
  'global',
  'globalCertificate',
  'targetScope',
  'targetScopeValue',
  'localSummary',
  'localCertificate',
  'globalNotChecked',
  'globalInProgress',
  'globalPossible',
  'globalImpossible',
  'globalUnknown',
  'globalUnavailable',
  'localNecessaryFailed',
  'localSufficientProven',
  'localIndeterminate',
  'localUnavailable',
  'localCounts',
  'relatedVertices',
  'vertexLabel',
  'hiddenVertices',
  'diagnosticsSummary',
  'diagnosticsJson',
] as const

test('proof scope catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(selectLocalizedText('ja', TEXT.proofCoverage), '証明範囲')
  assert.equal(selectLocalizedText('en', TEXT.proofCoverage), 'Proof coverage')
  assert.equal(
    selectLocalizedText('ja', TEXT.separateProofs),
    '全体判定・局所必要条件・局所十分性は、互いに別の証明です。',
  )
  assert.equal(
    selectLocalizedText('en', TEXT.separateProofs),
    'The global result, local necessary conditions, and local sufficiency are separate proofs.',
  )
  assert.equal(
    selectLocalizedText('en', TEXT.diagnosticsJson),
    'Proof coverage diagnostics JSON',
  )
})

test('proof scope keeps global and local status vocabularies distinct', () => {
  assert.deepEqual(
    ['globalNotChecked', 'globalInProgress', 'globalPossible',
      'globalImpossible', 'globalUnknown', 'globalUnavailable']
      .map((key) => selectLocalizedText('en', TEXT[key as keyof typeof TEXT])),
    ['Not checked', 'In progress', 'Possible', 'Impossible', 'Unknown',
      'Unavailable'],
  )
  assert.deepEqual(
    ['localNecessaryFailed', 'localSufficientProven', 'localIndeterminate']
      .map((key) => selectLocalizedText('ja', TEXT[key as keyof typeof TEXT])),
    ['必要条件不成立', '十分性証明', '判定不能'],
  )
  assert.equal(selectLocalizedText('ja', TEXT.localUnavailable), '未取得')
  assert.notEqual(
    selectLocalizedText('ja', TEXT.localUnavailable),
    selectLocalizedText('ja', TEXT.globalUnavailable),
  )
})

test('proof scope placeholders are locale-equivalent', () => {
  assert.deepEqual(placeholderMap(TEXT), {
    localCounts: {
      ja: ['necessaryFailed', 'sufficientProven', 'indeterminate'],
      en: ['necessaryFailed', 'sufficientProven', 'indeterminate'],
    },
    vertexLabel: { ja: ['index'], en: ['index'] },
    hiddenVertices: { ja: ['count'], en: ['count'] },
  })
  assert.equal(
    formatLocalizedText('en', TEXT.localCounts, {
      necessaryFailed: 1,
      sufficientProven: 1,
      indeterminate: 1,
    }),
    'Necessary failed 1; sufficiency proven 1; indeterminate 1',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.localCounts, {
      necessaryFailed: 1,
      sufficientProven: 2,
      indeterminate: 3,
    }),
    '必要条件不成立 1・十分性証明 2・判定不能 3',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.vertexLabel, { index: 2 }),
    'Vertex 2',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.vertexLabel, { index: 2 }),
    '頂点 2',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.hiddenVertices, { count: 4 }),
    '4 more vertices',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.hiddenVertices, { count: 4 }),
    'ほか 4 頂点',
  )
})

test('proof scope summary keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL('../src/components/ProofScopeSummary.tsx', import.meta.url),
    'utf8',
  )
  assert.match(source, /PROOF_SCOPE_SUMMARY_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale === 'ja'/u)
  assert.doesNotMatch(source, /locale !== 'ja'/u)
  assert.match(source, /labels\[status as keyof typeof labels\] \?\? labels\.unavailable/u)
  assert.match(source, /return TEXT\.localIndeterminate/u)
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
