import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import type { Locale } from '../src/lib/i18n.ts'
import {
  createLocalFlatFoldabilityPresentation,
  localFlatFoldabilityConditionLabel,
  localFlatFoldabilityReasonLabel,
} from '../src/lib/localFlatFoldabilityPresentation.ts'
import {
  formatLocalFlatFoldabilityReason,
  formatLocalFlatFoldabilitySummary,
  LOCAL_FLAT_FOLDABILITY_PRESENTATION_TEXT as TEXT,
  selectLocalFlatFoldabilityPresentationText,
} from '../src/lib/localFlatFoldabilityPresentationText.ts'

test('local flat-foldability presentation catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), ['ja', 'en'])
  assert.deepEqual(Object.keys(TEXT.ja), Object.keys(TEXT.en))
  assertMatchingCatalog(TEXT.ja, TEXT.en)
  assertDeeplyFrozen(TEXT)
})

test('local flat-foldability formatters preserve every placeholder and number representation', () => {
  assert.equal(
    formatLocalFlatFoldabilityReason('fold_degree_limit', 1_234, 'ja'),
    '折り線次数が厳密計算上限（1234）を超えたため判定不能です',
  )
  assert.equal(
    formatLocalFlatFoldabilityReason('fold_degree_limit', 1_234, 'en'),
    'Indeterminate because the fold degree exceeds the exact limit (1234).',
  )
  assert.equal(formatLocalFlatFoldabilityReason(null, 8, 'ja'), '')
  assert.equal(
    formatLocalFlatFoldabilitySummary(
      'indeterminate',
      {
        satisfied: 1,
        violated: 2,
        notApplicable: 3,
        indeterminate: 4,
      },
      'en',
    ),
    'At least one vertex has indeterminate local necessary conditions (satisfied 1, violated 2, not applicable 3, indeterminate 4).',
  )
})

test('unknown presentation locales fail closed to the Japanese catalog', () => {
  const invalidLocale = 'fr'
  const runtimeLocale = invalidLocale as Locale

  assert.strictEqual(
    selectLocalFlatFoldabilityPresentationText(invalidLocale),
    TEXT.ja,
  )
  assert.equal(
    localFlatFoldabilityConditionLabel('not_applicable', runtimeLocale),
    '対象外',
  )
  assert.equal(
    localFlatFoldabilityReasonLabel(
      'fold_degree_limit',
      8,
      runtimeLocale,
    ),
    '折り線次数が厳密計算上限（8）を超えたため判定不能です',
  )
  assert.equal(
    createLocalFlatFoldabilityPresentation(
      null,
      [],
      runtimeLocale,
    ).summaryText,
    '局所平坦折り条件の結果を確認できませんでした。成立とは扱いません。',
  )
})

test('presentation parsing delegates all display copy to the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/lib/localFlatFoldabilityPresentation.ts',
      import.meta.url,
    ),
    'utf8',
  )

  assert.match(
    source,
    /selectLocalFlatFoldabilityPresentationText\(locale\)/u,
  )
  assert.match(source, /formatLocalFlatFoldabilityReason\(/u)
  assert.match(source, /formatLocalFlatFoldabilitySummary\(/u)
  assert.doesNotMatch(source, /\blocale\s*(?:===|!==)/u)
  assert.doesNotMatch(source, /\bformatLocalizedText\(/u)
  assert.doesNotMatch(source, /\bfunction localized\(/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
})

function assertMatchingCatalog(ja: unknown, en: unknown, path = '') {
  assert.equal(typeof ja, typeof en, path)
  if (
    ja === null
    || en === null
    || typeof ja !== 'object'
    || typeof en !== 'object'
  ) {
    assert.equal(typeof ja, 'string', path)
    assert.deepEqual(
      placeholders(ja as string),
      placeholders(en as string),
      path,
    )
    return
  }

  assert.deepEqual(Object.keys(ja), Object.keys(en), path)
  for (const key of Object.keys(ja)) {
    assertMatchingCatalog(
      (ja as Record<string, unknown>)[key],
      (en as Record<string, unknown>)[key],
      path.length === 0 ? key : `${path}.${key}`,
    )
  }
}

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) {
    assertDeeplyFrozen(child)
  }
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
    .sort()
}
