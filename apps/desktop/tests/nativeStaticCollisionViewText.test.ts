import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { selectLocalizedText } from '../src/lib/i18n.ts'
import {
  NATIVE_STATIC_COLLISION_PAIR_DISPOSITION_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_POLICY_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_TOPOLOGY_TEXT,
  NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT,
  NATIVE_STATIC_COLLISION_VIEW_TEXT,
} from '../src/lib/nativeStaticCollisionViewText.ts'

const catalogs = [
  NATIVE_STATIC_COLLISION_VIEW_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_DISPOSITION_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_TOPOLOGY_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_POLICY_TEXT,
  NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT,
] as const

test('native static collision presentation catalogs are closed and deeply frozen', () => {
  assert.equal(Object.keys(NATIVE_STATIC_COLLISION_VIEW_TEXT).length, 34)
  assert.equal(
    Object.keys(NATIVE_STATIC_COLLISION_PAIR_DISPOSITION_TEXT).length,
    5,
  )
  assert.equal(
    Object.keys(NATIVE_STATIC_COLLISION_PAIR_TOPOLOGY_TEXT).length,
    3,
  )
  assert.equal(
    Object.keys(NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT).length,
    11,
  )
  assert.equal(
    Object.keys(NATIVE_STATIC_COLLISION_PAIR_POLICY_TEXT).length,
    6,
  )
  assert.equal(
    Object.keys(NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT).length,
    4,
  )

  for (const catalog of catalogs) {
    assert.equal(Object.isFrozen(catalog), true)
    for (const text of Object.values(catalog)) {
      assert.deepEqual(Object.keys(text), ['ja', 'en'])
      assert.equal(Object.isFrozen(text), true)
      assert.equal(typeof text.ja, 'string')
      assert.equal(typeof text.en, 'string')
      assert.deepEqual(placeholders(text.ja), placeholders(text.en))
    }
  }
})

test('native static collision catalogs preserve exact text and Japanese fallback', () => {
  assert.deepEqual(NATIVE_STATIC_COLLISION_VIEW_TEXT.pairCounts, {
    ja: '面ペア {total}件: 分離 {separated} / 接触 {touching} / 許容 {allowed} / 貫通 {penetrating} / 判定保留 {indeterminate}',
    en: 'Face pairs {total}: separated {separated} / touching {touching} / allowed {allowed} / penetrating {penetrating} / indeterminate {indeterminate}',
  })
  assert.deepEqual(
    NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT.shared_feature_flat_stack,
    {
      ja: '共有要素の平坦積層（層順認証時のみ許容）',
      en: 'shared-feature flat stack (allowed only with certified layer order)',
    },
  )
  assert.equal(
    selectLocalizedText(
      'unsupported',
      NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT.strictTransversalDualGate,
    ),
    '横断交差の二重証明',
  )
})

test('native static collision view delegates every locale choice to the catalog', () => {
  const source = readFileSync(
    new URL('../src/lib/nativeStaticCollisionView.ts', import.meta.url),
    'utf8',
  )
  assert.doesNotMatch(source, /locale\s*[!=]==?\s*['"](?:ja|en)['"]/u)
  assert.doesNotMatch(source, /['"](?:ja|en)['"]\s*\?/u)
  assert.match(source, /NATIVE_STATIC_COLLISION_VIEW_TEXT/u)
  assert.match(source, /NATIVE_STATIC_COLLISION_PAIR_DISPOSITION_TEXT/u)
  assert.match(source, /NATIVE_STATIC_COLLISION_PAIR_TOPOLOGY_TEXT/u)
  assert.match(source, /NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT/u)
  assert.match(source, /NATIVE_STATIC_COLLISION_PAIR_POLICY_TEXT/u)
  assert.match(source, /NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT/u)
})

function placeholders(value: string): readonly string[] {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1]!)
    .sort()
}
