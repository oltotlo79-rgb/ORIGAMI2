import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  RECOGNITION_CONTOUR_COPY_ACTION_TEXT as TEXT,
} from '../src/lib/recognitionContourCopyActionText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

const KEYS = ['summary', 'confirmation', 'copy'] as const

test('recognition contour copy catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.equal(
    TEXT.confirmation.ja,
    '認識候補の輪郭を編集欄へコピーしますか？保存するまでprojectは変更されません。',
  )
  assert.equal(
    TEXT.confirmation.en,
    'Copy the proposed contours into the editor? The project stays unchanged until saved.',
  )
})

test('recognition contour copy placeholders are locale-equivalent', () => {
  for (const key of KEYS) {
    assert.deepEqual(
      placeholders(TEXT[key].ja),
      placeholders(TEXT[key].en),
      key,
    )
  }
  assert.deepEqual(placeholders(TEXT.summary.ja), [
    'bodyPointCount',
    'localContourCount',
  ])
  assert.equal(
    formatLocalizedText('ja', TEXT.summary, {
      bodyPointCount: 6,
      localContourCount: 1,
    }),
    '編集可能な胴体輪郭 6 点・局所輪郭 1 件',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.summary, {
      bodyPointCount: 4,
      localContourCount: 2,
    }),
    'Editable body contour: 4 points; local contours: 2',
  )
})

test('recognition contour copy component keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/RecognitionContourCopyAction.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /RECOGNITION_CONTOUR_COPY_ACTION_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale\s*===/u)
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
