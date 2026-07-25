import assert from 'node:assert/strict'
import test from 'node:test'
import { formatLocalizedText } from '../src/lib/i18n.ts'
import { PAPER_THICKNESS_TEXT } from '../src/lib/paperThicknessText.ts'

test('paper thickness catalog is closed, deeply frozen, and preserves its unit placeholder', () => {
  assert.deepEqual(Object.keys(PAPER_THICKNESS_TEXT), [
    'ariaLabel', 'title', 'description', 'increase', 'decrease', 'paperEdgeRatio',
  ])
  assert.equal(Object.isFrozen(PAPER_THICKNESS_TEXT), true)
  for (const text of Object.values(PAPER_THICKNESS_TEXT)) {
    assert.equal(Object.isFrozen(text), true)
  }
  assert.equal(
    formatLocalizedText('ja', PAPER_THICKNESS_TEXT.title, { unit: 'cm' }),
    '上下ボタンと矢印キーは物理量0.01 mm刻み。値はcmで直接入力できます',
  )
})
