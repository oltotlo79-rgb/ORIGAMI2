import assert from 'node:assert/strict'
import test from 'node:test'

import { CREASE_EXPORT_COPY } from '../src/lib/creaseExportDialogText.ts'

test('crease export dialog catalog is locale-complete and deeply frozen', () => {
  assert.deepEqual(Object.keys(CREASE_EXPORT_COPY), ['ja', 'en'])
  assertLocaleShape(CREASE_EXPORT_COPY.ja, CREASE_EXPORT_COPY.en)
  assertDeeplyFrozen(CREASE_EXPORT_COPY)
})

test('crease export dialog catalog preserves reviewed byte-sensitive copy', () => {
  assert.equal(CREASE_EXPORT_COPY.ja.eyebrow, '展開図の書き出し')
  assert.equal(CREASE_EXPORT_COPY.en.title, 'Review format and information loss')
  assert.equal(
    CREASE_EXPORT_COPY.ja.description,
    '現在の編集リビジョンから展開図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。',
  )
  assert.equal(
    CREASE_EXPORT_COPY.en.generating,
    ' data is being validated and generated…',
  )
  assert.deepEqual(CREASE_EXPORT_COPY.ja.assignmentLabels, {
    boundary: '外周',
    mountain: '山折り',
    valley: '谷折り',
    auxiliary: '補助線',
    cut: '切断線',
  })
  assert.deepEqual(CREASE_EXPORT_COPY.en.formatSummaries, {
    fold: 'FOLD 1.2 · 2D creasePattern · coordinates in mm',
    svg: 'Static line SVG · 1 SVG unit = 1 mm',
    pdf: 'Full-size 1:1 vector · drawing bounds + 10 mm margins',
    dxf: 'AC1021 text form · UTF-8 · mm · 5 semantic layers',
  })
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
