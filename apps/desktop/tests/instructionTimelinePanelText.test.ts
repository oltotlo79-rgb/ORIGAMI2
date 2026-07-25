import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { formatLocalizedText } from '../src/lib/i18n.ts'
import { INSTRUCTION_TIMELINE_PANEL_TEXT as TEXT } from '../src/lib/instructionTimelinePanelText.ts'

const panelSource = readFileSync(
  new URL('../src/components/InstructionTimelinePanel.tsx', import.meta.url),
  'utf8',
)

test('instruction timeline panel catalog is closed and deeply frozen', () => {
  assert.equal(Object.keys(TEXT).length, 66)
  assert.equal(Object.isFrozen(TEXT), true)
  for (const entry of Object.values(TEXT)) {
    assert.deepEqual(Object.keys(entry), ['ja', 'en'])
    assert.equal(Object.isFrozen(entry), true)
  }
  assert.equal(TEXT.heading.ja, '折り手順')
  assert.equal(TEXT.heading.en, 'Folding instructions')
  assert.equal(TEXT.onionPreparing.en, 'Preparing ghost.')
  assert.equal(
    TEXT.visualHelp.ja,
    'camera、arrows、focus_pointsに加え、hand_guidesへpinch/hold/push/regripとposition/direction/labelを指定します。',
  )
})

test('instruction timeline panel has no inline locale branches for display text', () => {
  assert.doesNotMatch(panelSource, /\blocale\s*(?:===|!==)/u)
  for (const key of [
    'certificateExportBlockedTitle',
    'animationExportAction',
    'reorderHelp',
    'completedFormThumbnailSuffix',
    'pathCertificateHeading',
    'pathCertificateReview',
    'certificateFingerprintLabel',
    'verifiedTransitionsLabel',
    'sourcePoseLabel',
    'targetPoseLabel',
    'sourceModelBindingLabel',
    'pathCertificateIdentityHelp',
    'pathCertificateEndpointMismatch',
    'pathCertificateDescriptionMismatch',
    'pathCertificateTextOnly',
    'splitAction',
    'mergeWithNextAction',
  ]) {
    assert.match(panelSource, new RegExp(`TEXT\\.${key}\\b`, 'u'))
  }
  assert.equal(TEXT.completedFormThumbnailSuffix.ja, ' · 完成形サムネイル')
  assert.equal(TEXT.completedFormThumbnailSuffix.en, ' · Completed-form thumbnail')
  assert.equal(TEXT.pathCertificateHeading.ja, '構造化経路証明')
  assert.equal(TEXT.pathCertificateHeading.en, 'Structured path certificate')
})

test('instruction timeline placeholders are locale-equivalent and preserve output', () => {
  const placeholders = Object.fromEntries(
    Object.entries(TEXT).flatMap(([key, entry]) => {
      const ja = [...entry.ja.matchAll(/\{([^}]+)\}/gu)].map((match) => match[1])
      const en = [...entry.en.matchAll(/\{([^}]+)\}/gu)].map((match) => match[1])
      return ja.length === 0 && en.length === 0 ? [] : [[key, { ja, en }]]
    }),
  )
  assert.deepEqual(placeholders, {
    defaultStepTitle: { ja: ['step'], en: ['step'] },
    deleteConfirmation: { ja: ['title'], en: ['title'] },
    stepCount: { ja: ['count'], en: ['count'] },
    stepCountOne: { ja: ['count'], en: ['count'] },
    totalDuration: { ja: ['duration'], en: ['duration'] },
    emptyTimeline: { ja: ['captureStatus'], en: ['captureStatus'] },
  })
  assert.equal(
    formatLocalizedText('en', TEXT.deleteConfirmation, { title: 'Fold the wing' }),
    'Delete “Fold the wing”?',
  )
})
