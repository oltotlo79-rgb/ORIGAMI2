import assert from 'node:assert/strict'
import test from 'node:test'

import { INSTRUCTION_EXPORT_COPY } from '../src/lib/instructionExportDialogText.ts'

test('instruction export dialog catalog is locale-complete and deeply frozen', () => {
  assert.deepEqual(Object.keys(INSTRUCTION_EXPORT_COPY), ['ja', 'en'])
  assertLocaleShape(INSTRUCTION_EXPORT_COPY.ja, INSTRUCTION_EXPORT_COPY.en)
  assertDeeplyFrozen(INSTRUCTION_EXPORT_COPY)
})

test('instruction export catalog preserves reviewed format and warning copy', () => {
  assert.equal(INSTRUCTION_EXPORT_COPY.ja.eyebrow, '折り図の書き出し')
  assert.equal(INSTRUCTION_EXPORT_COPY.en.title, 'Review format and output')
  assert.equal(
    INSTRUCTION_EXPORT_COPY.en.optionDetails.pdf,
    'Combine fixed-isometric diagrams with authored camera and hand/regrip guide details into a multi-page PDF',
  )
  assert.deepEqual(INSTRUCTION_EXPORT_COPY.ja.metadata, {
    format: '形式',
    specification: '出力仕様',
    profile: '出力プロファイル',
    projection: '投影プロファイル',
    suggestedName: '保存名候補',
    size: 'サイズ',
    steps: '折り手順',
    pages: 'ページ',
    cautions: '注意事項',
    revision: '固定元',
  })
  assert.equal(INSTRUCTION_EXPORT_COPY.en.stop, 'Stop generation')
})

function assertLocaleShape(
  left: Readonly<Record<string, unknown>>,
  right: Readonly<Record<string, unknown>>,
) {
  assert.deepEqual(Object.keys(left), Object.keys(right))
  for (const key of Object.keys(left)) {
    const leftValue = left[key]
    const rightValue = right[key]
    assert.equal(typeof leftValue, typeof rightValue, key)
    if (
      typeof leftValue === 'object'
      && leftValue !== null
      && typeof rightValue === 'object'
      && rightValue !== null
    ) {
      assertLocaleShape(
        leftValue as Readonly<Record<string, unknown>>,
        rightValue as Readonly<Record<string, unknown>>,
      )
    }
  }
}

function assertDeeplyFrozen(value: unknown) {
  if (typeof value !== 'object' || value === null) return
  assert.equal(Object.isFrozen(value), true)
  for (const child of Object.values(value)) {
    assertDeeplyFrozen(child)
  }
}
