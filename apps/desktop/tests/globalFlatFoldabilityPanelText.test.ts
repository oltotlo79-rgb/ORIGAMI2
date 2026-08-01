import assert from 'node:assert/strict'
import test from 'node:test'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import { GLOBAL_FLAT_FOLDABILITY_PANEL_TEXT as TEXT } from '../src/lib/globalFlatFoldabilityPanelText.ts'

test('global flat-foldability panel catalog is exact and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), [
    'eyebrow',
    'title',
    'timeLimit',
    'seconds',
    'checking',
    'runAgain',
    'start',
    'cancelRequested',
    'cancel',
    'layerLoading',
    'layerEmpty',
    'layerUnavailable',
    'limitationsLabel',
    'limitationsTitle',
    'limitationsDetail',
  ])
  assert.equal(Object.isFrozen(TEXT), true)
  for (const entry of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(entry), ['ja', 'en'])
    assert.equal(Object.isFrozen(entry), true)
  }
  assert.equal(TEXT.title.ja, '全体平坦折り判定')
  assert.equal(TEXT.title.en, 'Global flat-foldability check')
  assert.equal(
    TEXT.limitationsDetail.en,
    'This check uses an ideal zero-thickness model. It does not guarantee foldability with paper thickness or layer offsets, ease of folding by hand, or a continuous collision-safe path to the flat state.',
  )
})

test('global flat-foldability time placeholder remains locale-equivalent', () => {
  assert.deepEqual(
    [...TEXT.seconds.ja.matchAll(/\{([^}]+)\}/gu)].map((match) => match[1]),
    ['seconds'],
  )
  assert.deepEqual(
    [...TEXT.seconds.en.matchAll(/\{([^}]+)\}/gu)].map((match) => match[1]),
    ['seconds'],
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.seconds, { seconds: 30 }),
    '30秒',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.seconds, { seconds: 30 }),
    '30 seconds',
  )
})
