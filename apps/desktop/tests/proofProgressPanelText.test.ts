import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { PROOF_PROGRESS_PANEL_TEXT as TEXT } from '../src/lib/proofProgressPanelText.ts'
import { PROOF_PROGRESS_STATES } from '../src/lib/proofProgressModel.ts'

const TOP_LEVEL_KEYS = [
  'ariaLabel',
  'title',
  'postApplyStarting',
  'postApplyUnavailable',
  'statusLabel',
  'status',
  'certifiedBadge',
  'unprovenBadge',
  'pairProgress',
  'pairProgressUnknownTotal',
  'appliedUnprovenWarning',
  'unappliedRedoNotice',
  'unprovenCounts',
  'appliedBreakdown',
  'redoBreakdown',
  'unprovenStatuses',
  'unprovenSummaryUnavailable',
  'speculativeApplyWarning',
  'speculativeApplyGroup',
  'speculativeConfirmation',
  'applySpeculative',
  'applyingSpeculative',
  'proofFailureTitle',
  'failureLocationLabel',
  'failureReasonLabel',
  'locations',
  'reasons',
  'subsequentEdits',
  'revertUnavailable',
  'destructiveConfirmation',
  'revertSteps',
  'revertRequested',
] as const

test('proof progress catalog is closed, typed, and deeply frozen', () => {
  assert.deepEqual(Object.keys(TEXT), TOP_LEVEL_KEYS)
  assert.deepEqual(Object.keys(TEXT.status), PROOF_PROGRESS_STATES)
  assert.deepEqual(Object.keys(TEXT.locations), [
    'applied_trimmed_base',
    'applied_retained_undo',
    'unapplied_redo',
  ])
  assert.deepEqual(Object.keys(TEXT.reasons), [
    'blocked',
    'evidence_insufficient',
    'resource_limit',
    'cancelled',
    'deadline',
  ])
  assert.equal(Object.isFrozen(TEXT), true)
  assert.equal(Object.isFrozen(TEXT.status), true)
  assert.equal(Object.isFrozen(TEXT.locations), true)
  assert.equal(Object.isFrozen(TEXT.reasons), true)
  visitLocalizedText(TEXT, (localized) => {
    assert.deepEqual(Object.keys(localized), ['ja', 'en'])
    assert.equal(Object.isFrozen(localized), true)
  })
})

test('proof progress placeholders are locale-equivalent', () => {
  visitLocalizedText(TEXT, (localized, path) => {
    assert.deepEqual(
      placeholders(localized.ja),
      placeholders(localized.en),
      path,
    )
  })
})

test('proof progress copy distinguishes proven, unproven, applied, and redo-only', () => {
  assert.notEqual(TEXT.certifiedBadge.ja, TEXT.unprovenBadge.ja)
  assert.notEqual(TEXT.certifiedBadge.en, TEXT.unprovenBadge.en)
  assert.match(TEXT.speculativeApplyWarning.ja, /未証明/u)
  assert.match(TEXT.speculativeApplyWarning.ja, /安全性の証明では/u)
  assert.match(TEXT.speculativeApplyWarning.en, /unproven/iu)
  assert.match(TEXT.speculativeApplyWarning.en, /not a safety certificate/iu)
  assert.doesNotMatch(TEXT.speculativeApplyWarning.ja, /自動/u)
  assert.doesNotMatch(TEXT.speculativeApplyWarning.en, /automatic/iu)
  assert.match(TEXT.appliedUnprovenWarning.ja, /現在の文書に適用/u)
  assert.match(TEXT.unappliedRedoNotice.ja, /現在は未適用/u)
  assert.match(TEXT.postApplyStarting.en, /post-Apply proof job/iu)
  assert.match(TEXT.postApplyUnavailable.ja, /未証明のまま/u)
  assert.match(TEXT.postApplyUnavailable.en, /remains unproven/iu)
  assert.doesNotMatch(
    `${TEXT.postApplyUnavailable.ja}${TEXT.postApplyUnavailable.en}`,
    /error|path|authority|geometry|座標|経路/iu,
  )
})

test('ProofProgressPanel keeps all display copy in the typed catalog', () => {
  const source = readFileSync(
    new URL('../src/components/ProofProgressPanel.tsx', import.meta.url),
    'utf8',
  )
  assert.match(source, /PROOF_PROGRESS_PANEL_TEXT as TEXT/u)
  assert.doesNotMatch(source, /\{\s*ja\s*:/u)
  assert.doesNotMatch(source, /locale === ['"]ja['"]/u)
  assert.doesNotMatch(source, /[\u3040-\u30ff\u3400-\u9fff]/u)
})

type Localized = Readonly<{ ja: string; en: string }>

function visitLocalizedText(
  value: unknown,
  visit: (localized: Localized, path: string) => void,
  path = 'TEXT',
) {
  assert.equal(typeof value, 'object', path)
  assert.notEqual(value, null, path)
  for (const [key, item] of Object.entries(value as Record<string, unknown>)) {
    const childPath = `${path}.${key}`
    if (
      typeof item === 'object'
      && item !== null
      && Object.keys(item).length === 2
      && typeof (item as Record<string, unknown>).ja === 'string'
      && typeof (item as Record<string, unknown>).en === 'string'
    ) {
      visit(item as Localized, childPath)
    } else {
      visitLocalizedText(item, visit, childPath)
    }
  }
}

function placeholders(value: string) {
  return [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/gu)]
    .map((match) => match[1])
    .sort()
}
