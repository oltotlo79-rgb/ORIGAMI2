import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  BEGINNER_SHAPE_CANVAS_PREVIEW_TEXT as TEXT,
} from '../src/lib/beginnerShapeCanvasPreviewText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

const KEYS = [
  'heading',
  'outlineToPreview',
  'bodyOption',
  'bindingOption',
  'canvasAriaLabel',
  'help',
  'missingOutline',
] as const

test('beginner shape canvas preview catalog is closed and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), KEYS)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const key of KEYS) {
    assert.deepEqual(Object.keys(TEXT[key]), ['ja', 'en'], key)
    assert.equal(Object.isFrozen(TEXT[key]), true, key)
  }
  assert.deepEqual(
    Object.fromEntries(KEYS.map((key) => [
      key,
      {
        ja: selectLocalizedText('ja', TEXT[key]),
        en: selectLocalizedText('en', TEXT[key]),
      },
    ])),
    {
      heading: {
        ja: '目標形状2Dプレビュー',
        en: '2D target-shape preview',
      },
      outlineToPreview: {
        ja: '表示する輪郭',
        en: 'Outline to preview',
      },
      bodyOption: { ja: '胴体', en: 'Body' },
      bindingOption: {
        ja: 'binding {bindingId}',
        en: 'Binding {bindingId}',
      },
      canvasAriaLabel: {
        ja: '{selectionLabel}の輪郭プレビュー',
        en: '{selectionLabel} outline preview',
      },
      help: {
        ja: 'control pointをpointerで移動できます。矢印キーは0.1 mm、Shift+矢印は1 mm移動します。',
        en: 'Move a control point with the pointer. Arrow keys move 0.1 mm; Shift+Arrow moves 1 mm.',
      },
      missingOutline: {
        ja: 'このbindingには局所輪郭がありません。',
        en: 'This binding has no local outline.',
      },
    },
  )
})

test('beginner shape canvas preview placeholders are locale-equivalent', () => {
  assert.deepEqual(
    Object.fromEntries(KEYS.map((key) => [
      key,
      {
        ja: placeholders(TEXT[key].ja),
        en: placeholders(TEXT[key].en),
      },
    ])),
    {
      heading: { ja: [], en: [] },
      outlineToPreview: { ja: [], en: [] },
      bodyOption: { ja: [], en: [] },
      bindingOption: { ja: ['bindingId'], en: ['bindingId'] },
      canvasAriaLabel: {
        ja: ['selectionLabel'],
        en: ['selectionLabel'],
      },
      help: { ja: [], en: [] },
      missingOutline: { ja: [], en: [] },
    },
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.bindingOption, { bindingId: 7 }),
    'binding 7',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.bindingOption, { bindingId: 7 }),
    'Binding 7',
  )
  assert.equal(
    formatLocalizedText('ja', TEXT.canvasAriaLabel, {
      selectionLabel: 'binding 7',
    }),
    'binding 7の輪郭プレビュー',
  )
  assert.equal(
    formatLocalizedText('en', TEXT.canvasAriaLabel, {
      selectionLabel: 'Binding 7',
    }),
    'Binding 7 outline preview',
  )
})

test('beginner shape canvas preview keeps display copy in the catalog', () => {
  const source = readFileSync(
    new URL(
      '../src/components/BeginnerShapeCanvasPreview.tsx',
      import.meta.url,
    ),
    'utf8',
  )
  assert.match(source, /BEGINNER_SHAPE_CANVAS_PREVIEW_TEXT as TEXT/u)
  assert.doesNotMatch(source, /[ぁ-んァ-ン一-龯]/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale\s*===/u)
  assert.doesNotMatch(source, /locale\s*!==/u)
})

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
}
