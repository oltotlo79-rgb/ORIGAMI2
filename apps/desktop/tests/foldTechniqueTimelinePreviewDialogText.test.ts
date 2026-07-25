import assert from 'node:assert/strict'
import test from 'node:test'

import {
  FOLD_TECHNIQUE_TIMELINE_PREVIEW_DIALOG_TEXT as TEXT,
  formatFoldTechniqueTimelinePreviewCount,
} from '../src/lib/foldTechniqueTimelinePreviewDialogText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
} from '../src/lib/i18n.ts'

test('timeline preview dialog catalog is closed, deeply frozen, and preserves placeholders', () => {
  assert.deepEqual(Object.keys(TEXT), [
    'eyebrow',
    'title',
    'close',
    'closeGlyph',
    'safety',
    'technique',
    'operations',
    'steps',
    'unsupported',
    'unsupportedNote',
    'previewList',
    'inertStep',
    'sourceKinds',
    'stale',
    'applying',
    'cancel',
    'confirm',
    'numberLocale',
  ])
  assert.deepEqual(Object.keys(TEXT.sourceKinds), [
    'technique',
    'parameter',
    'precondition',
    'operation',
  ])
  assert.equal(Object.isFrozen(TEXT), true)
  assert.equal(Object.isFrozen(TEXT.sourceKinds), true)
  for (const [key, value] of Object.entries(TEXT)) {
    if (key === 'sourceKinds') continue
    assert.equal(Object.isFrozen(value), true)
  }
  for (const value of Object.values(TEXT.sourceKinds)) {
    assert.equal(Object.isFrozen(value), true)
  }
  assert.equal(
    formatLocalizedText('en', TEXT.inertStep, { kind: 'Operation' }),
    'Description only · Operation',
  )
  assert.equal(selectLocalizedText('unsupported-locale', TEXT.title), TEXT.title.ja)
})

test('timeline preview count formatting owns fixed ja/en number locales', () => {
  assert.equal(
    formatFoldTechniqueTimelinePreviewCount(1_234_567, 'ja'),
    '1,234,567',
  )
  assert.equal(
    formatFoldTechniqueTimelinePreviewCount(1_234_567, 'en'),
    '1,234,567',
  )
  assert.equal(
    formatFoldTechniqueTimelinePreviewCount(1_234_567, 'unsupported-locale'),
    '1,234,567',
  )
})
