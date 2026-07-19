import assert from 'node:assert/strict'
import test from 'node:test'

import {
  describeFoldPreviewContinuousMotionDetail,
} from '../src/lib/foldPreviewContinuousMotionDetail.ts'
import type {
  FoldPreviewContinuousMotionRunnerState,
} from '../src/lib/foldPreviewContinuousMotionRunner.ts'
import {
  describeFoldPreviewContinuousMotion,
} from '../src/lib/foldPreviewContinuousMotionView.ts'

const stats = {
  intervalTests: 3,
  pointTests: 5,
  pointCacheHits: 1,
  maximumDepthReached: 2,
}

test('continuous motion status has complete English safety copy', () => {
  const clear = describeFoldPreviewContinuousMotion(state({
    requested: 90,
    applied: 90,
    status: 'clear',
    result: {
      kind: 'clear',
      certifiedSafeThrough: 1,
      stopTime: 1,
      stats,
    },
  }), 'en')
  assert.equal(
    clear.badgeText,
    'Middle surface · single path verified · displayed 90°',
  )
  assert.match(clear.accessibleText, /layer offsets are not included/u)
  assert.doesNotMatch(clear.accessibleText, /[ぁ-んァ-ヶ一-龠]/u)

  const blocked = describeFoldPreviewContinuousMotion(state({
    requested: 100,
    applied: 50,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0.5,
      stopTime: 0.5,
      unsafeBracket: [0.5, 0.6],
      blockingSampleTime: 0.6,
      stats,
    },
  }), 'en')
  assert.match(blocked.badgeText, /Stopped at verified path boundary/u)
  assert.match(blocked.accessibleText, /exact collision-onset angle is not known/u)
})

test('motion detail localizes rows and suppresses unknown raw reasons', () => {
  const detail = describeFoldPreviewContinuousMotionDetail(state({
    requested: 80,
    applied: 0,
    status: 'indeterminate',
    reason: 'native-secret-reason',
    result: null,
  }), [], null, 'en')
  assert.ok(detail)
  assert.equal(detail.title, 'Why the motion path could not start')
  assert.equal(detail.reasonCode, 'unclassified')
  assert.match(detail.summaryText, /Starting angle: 0°/u)
  assert.match(detail.summaryText, /Path safety could not be determined/u)
  assert.doesNotMatch(
    `${detail.title} ${detail.summaryText}`,
    /native-secret-reason/u,
  )
  assert.doesNotMatch(
    `${detail.title} ${detail.summaryText}`,
    /[ぁ-んァ-ヶ一-龠]/u,
  )
})

function state(
  overrides: Partial<FoldPreviewContinuousMotionRunnerState> = {},
): FoldPreviewContinuousMotionRunnerState {
  return {
    requested: 52,
    applied: 0,
    start: 0,
    status: 'running',
    reason: null,
    result: null,
    ...overrides,
  }
}
