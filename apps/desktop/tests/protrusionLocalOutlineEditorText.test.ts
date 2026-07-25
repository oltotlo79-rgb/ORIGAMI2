import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  PROTRUSION_LOCAL_OUTLINE_EDITOR_TEXT as TEXT,
} from '../src/lib/protrusionLocalOutlineEditorText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

const KEYS = [
  'legend',
  'outlinePoints',
  'outlinePointsAria',
  'applyOutline',
  'clearOutline',
  'invalidOutline',
] as const

test('protrusion local outline catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(TEXT.legend.ja, '局所輪郭（任意）')
  assert.equal(TEXT.legend.en, 'Local outline (optional)')
  assert.equal(TEXT.invalidOutline.ja,
    '3〜8点の有界な輪郭を入力してください。左右対称bindingでは鏡像点が必要です。')
  assert.equal(TEXT.invalidOutline.en,
    'Enter 3 to 8 bounded points. Bilateral bindings require mirrored points.')
})

test('protrusion local outline placeholders are locale-equivalent', () => {
  for (const key of KEYS) {
    assert.deepEqual(
      placeholders(TEXT[key].ja),
      placeholders(TEXT[key].en),
      key,
    )
  }
  assert.deepEqual(
    Object.fromEntries(KEYS.map((key) => [key, placeholders(TEXT[key].ja)])),
    {
      legend: [],
      outlinePoints: [],
      outlinePointsAria: ['bindingId'],
      applyOutline: [],
      clearOutline: [],
      invalidOutline: [],
    },
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.outlinePointsAria, { bindingId: 7 }),
    '局所輪郭点 binding 7',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.outlinePointsAria, { bindingId: 7 }),
    'Local outline points binding 7',
  )
})

test('protrusion local outline editor keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/ProtrusionLocalOutlineEditor.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /PROTRUSION_LOCAL_OUTLINE_EDITOR_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale\s*===/u)
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
