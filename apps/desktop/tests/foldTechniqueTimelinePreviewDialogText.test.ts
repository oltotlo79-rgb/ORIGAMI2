import assert from 'node:assert/strict'
import test from 'node:test'

import { FOLD_TECHNIQUE_TIMELINE_PREVIEW_DIALOG_TEXT as TEXT } from '../src/lib/foldTechniqueTimelinePreviewDialogText.ts'
import { formatLocalizedText } from '../src/lib/i18n.ts'

test('timeline preview dialog catalog is closed, deeply frozen, and preserves placeholders', () => {
  assert.deepEqual(Object.keys(TEXT), [
    'eyebrow',
    'title',
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
})
