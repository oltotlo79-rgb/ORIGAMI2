import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  formatGlobalFlatFoldabilityActiveLive,
  formatGlobalFlatFoldabilityElapsedMilliseconds,
  formatGlobalFlatFoldabilityExhaustiveFaces,
  formatGlobalFlatFoldabilityFaceList,
  formatGlobalFlatFoldabilityItemCount,
  formatGlobalFlatFoldabilityLayerCount,
  formatGlobalFlatFoldabilityMaximumOverlap,
  formatGlobalFlatFoldabilityProgressWork,
  formatGlobalFlatFoldabilityUnknownLive,
  GLOBAL_FLAT_FOLDABILITY_PRESENTATION_TEXT as TEXT,
  selectGlobalFlatFoldabilityPresentationText,
} from '../src/lib/globalFlatFoldabilityPresentationText.ts'

test('global flat-foldability presentation catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), ['ja', 'en'])
  assert.deepEqual(Object.keys(TEXT.ja), Object.keys(TEXT.en))
  assertDeeplyFrozen(TEXT)

  for (const key of Object.keys(TEXT.ja)) {
    const typedKey = key as keyof typeof TEXT.ja
    const ja = TEXT.ja[typedKey]
    const en = TEXT.en[typedKey]
    if (typeof ja === 'string' && typeof en === 'string') {
      assert.deepEqual(placeholders(ja), placeholders(en), key)
      continue
    }
    assert.equal(typeof ja, 'object', key)
    assert.equal(typeof en, 'object', key)
    assert.deepEqual(Object.keys(ja), Object.keys(en), key)
    for (const nestedKey of Object.keys(ja)) {
      const typedNestedKey = nestedKey as keyof typeof ja
      assert.deepEqual(
        placeholders(ja[typedNestedKey]),
        placeholders(en[typedNestedKey]),
        `${key}.${nestedKey}`,
      )
    }
  }
})

test('unknown presentation locales fail closed to Japanese copy and formats', () => {
  const invalidLocale = 'fr'
  assert.strictEqual(
    selectGlobalFlatFoldabilityPresentationText(invalidLocale),
    TEXT.ja,
  )
  assert.equal(
    formatGlobalFlatFoldabilityProgressWork(12_340, null, invalidLocale),
    '12,340件完了（総数は計算中）',
  )
  assert.equal(
    formatGlobalFlatFoldabilityFaceList([2, 9], invalidLocale),
    '面 2、面 9',
  )
})

test('presentation formatters preserve locale, separators, and boundary values', () => {
  assert.equal(
    formatGlobalFlatFoldabilityActiveLive(
      '判定中',
      '重なり領域を構築しています',
      'ja',
    ),
    '判定中。重なり領域を構築しています。',
  )
  assert.equal(
    formatGlobalFlatFoldabilityActiveLive(
      'Checking',
      'Building overlap regions',
      'en',
    ),
    'Checking. Building overlap regions.',
  )
  assert.equal(
    formatGlobalFlatFoldabilityProgressWork(250, 1_000, 'ja'),
    '250 / 1,000件完了',
  )
  assert.equal(
    formatGlobalFlatFoldabilityProgressWork(250, 1_000, 'en'),
    '250 / 1,000 completed',
  )
  assert.equal(formatGlobalFlatFoldabilityItemCount(1_234, 'ja'), '1,234件')
  assert.equal(formatGlobalFlatFoldabilityItemCount(1_234, 'en'), '1,234')
  assert.equal(formatGlobalFlatFoldabilityLayerCount(1_234, 'ja'), '1,234層')
  assert.equal(
    formatGlobalFlatFoldabilityLayerCount(1_234, 'en'),
    '1,234 layers',
  )
  assert.equal(formatGlobalFlatFoldabilityMaximumOverlap(14, 'ja'), '14 ply')
  assert.equal(formatGlobalFlatFoldabilityMaximumOverlap(14, 'en'), '14 ply')
  assert.equal(
    formatGlobalFlatFoldabilityFaceList([2, 9], 'ja'),
    '面 2、面 9',
  )
  assert.equal(
    formatGlobalFlatFoldabilityFaceList([2, 9], 'en'),
    'Face 2, Face 9',
  )
  assert.equal(
    formatGlobalFlatFoldabilityExhaustiveFaces([1, 2], 1_234, 'ja'),
    '全体：面 1、面 2（ほか1,232面）',
  )
  assert.equal(
    formatGlobalFlatFoldabilityExhaustiveFaces([1, 2], 2, 'en'),
    'All: Face 1, Face 2',
  )
  assert.equal(formatGlobalFlatFoldabilityElapsedMilliseconds(999, 'en'), '999 ms')
  assert.equal(formatGlobalFlatFoldabilityElapsedMilliseconds(1_000, 'en'), '1 s')
  assert.equal(formatGlobalFlatFoldabilityElapsedMilliseconds(59_999, 'en'), '60 s')
  assert.equal(formatGlobalFlatFoldabilityElapsedMilliseconds(60_000, 'en'), '1 min')
  assert.equal(
    formatGlobalFlatFoldabilityElapsedMilliseconds(65_400, 'ja'),
    '1分5秒',
  )
  assert.equal(
    formatGlobalFlatFoldabilityUnknownLive('時間制限です。', 'ja'),
    '全体平坦折り判定の結果は、不明です。時間制限です。',
  )
})

test('presentation module delegates display copy and locale formatting', () => {
  const source = readFileSync(
    new URL(
      '../src/lib/globalFlatFoldabilityPresentation.ts',
      import.meta.url,
    ),
    'utf8',
  )

  assert.match(
    source,
    /selectGlobalFlatFoldabilityPresentationText\(locale\)/u,
  )
  assert.match(source, /formatGlobalFlatFoldabilityProgressWork\(/u)
  assert.match(source, /formatGlobalFlatFoldabilityElapsedMilliseconds\(/u)
  assert.match(source, /formatGlobalFlatFoldabilityExhaustiveFaces\(/u)
  assert.doesNotMatch(source, /\blocale\s*(?:===|!==)/u)
  assert.doesNotMatch(source, /\.toLocaleString\(/u)
  assert.doesNotMatch(source, /\bformatLocalizedText\(/u)
  assert.doesNotMatch(source, /\bfunction localized\(/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
})

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
