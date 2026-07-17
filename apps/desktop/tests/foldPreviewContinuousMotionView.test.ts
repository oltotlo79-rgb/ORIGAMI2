import assert from 'node:assert/strict'
import test from 'node:test'

import {
  describeFoldPreviewContinuousMotion,
} from '../src/lib/foldPreviewContinuousMotionView.ts'
import type {
  FoldPreviewContinuousMotionRunnerState,
} from '../src/lib/foldPreviewContinuousMotionRunner.ts'

const stats = {
  intervalTests: 1,
  pointTests: 2,
  pointCacheHits: 0,
  maximumDepthReached: 0,
}

test('missing, idle, and running motion remain explicitly non-terminal', () => {
  const preparing = describeFoldPreviewContinuousMotion(null)
  assert.equal(preparing.status, 'preparing')
  assert.match(preparing.badgeText, /準備中/u)
  assert.equal(preparing.terminalAnnouncement, null)

  const idle = describeFoldPreviewContinuousMotion(state({
    requested: null,
    status: 'idle',
  }))
  assert.equal(idle.status, 'idle')
  assert.match(idle.badgeText, /表示 0°/u)
  assert.equal(idle.terminalAnnouncement, null)

  const running = describeFoldPreviewContinuousMotion(state({
    requested: 80,
    applied: 42.1254,
    status: 'running',
  }))
  assert.equal(running.status, 'running')
  assert.equal(running.badgeText, '経路検証中・表示 42.125° / 指定 80°')
  assert.match(running.accessibleText, /判定完了までは経路確認済みとして扱いません/u)
  assert.equal(running.terminalAnnouncement, null)
})

test('clear, blocked, and indeterminate states report only the applied safe angle', () => {
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
  }))
  assert.equal(clear.status, 'clear')
  assert.equal(clear.badgeText, '中央面・単一経路確認済み・表示 90°')
  assert.match(clear.terminalAnnouncement ?? '', /指定角90度まで/u)

  const blocked = describeFoldPreviewContinuousMotion(state({
    requested: 100,
    applied: 58.375,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0.58375,
      stopTime: 0.58375,
      unsafeBracket: [0.58375, 0.6],
      stats,
    },
  }))
  assert.equal(
    blocked.badgeText,
    '経路確認済み境界で停止・表示 58.375° / 指定 100°',
  )
  assert.match(blocked.accessibleText, /最後に経路を確認できた58\.375度で停止/u)
  assert.match(blocked.accessibleText, /衝突開始角は確定していません/u)

  const indeterminate = describeFoldPreviewContinuousMotion(state({
    requested: 180,
    applied: 179.296875,
    status: 'indeterminate',
    reason: 'uncertified_interval',
    result: {
      kind: 'indeterminate',
      certifiedSafeThrough: 0.99609375,
      stopTime: 0.99609375,
      unresolvedBracket: [0.99609375, 1],
      reason: 'uncertified_interval',
      stats,
    },
  }))
  assert.equal(indeterminate.status, 'indeterminate')
  assert.match(indeterminate.badgeText, /経路を確認できず停止/u)
  assert.match(indeterminate.terminalAnnouncement ?? '', /安全を確認できない/u)
})

test('unverified and unsafe starts never claim a certified display angle', () => {
  const factoryFailure = describeFoldPreviewContinuousMotion(state({
    requested: 80,
    applied: 0,
    status: 'indeterminate',
    reason: 'job_factory_returned_null',
    result: null,
  }))
  assert.equal(factoryFailure.status, 'indeterminate')
  assert.match(factoryFailure.badgeText, /経路判定不能/u)
  assert.match(factoryFailure.accessibleText, /安全確認済みとして扱いません/u)
  assert.doesNotMatch(factoryFailure.accessibleText, /最後に経路を確認/u)

  const blockedAtStart = describeFoldPreviewContinuousMotion(state({
    requested: 90,
    applied: 0,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0,
      stopTime: 0,
      unsafeBracket: [0, 0],
      stats,
    },
  }))
  assert.equal(blockedAtStart.status, 'blocked')
  assert.match(blockedAtStart.badgeText, /開始姿勢で衝突/u)
  assert.match(blockedAtStart.accessibleText, /安全確認済みの姿勢として扱いません/u)
  assert.doesNotMatch(blockedAtStart.badgeText, /衝突手前/u)

  const blockedImmediately = describeFoldPreviewContinuousMotion(state({
    requested: 90,
    applied: 0,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0,
      stopTime: 0,
      unsafeBracket: [0, 0.01],
      stats,
    },
  }))
  assert.match(blockedImmediately.badgeText, /開始角からの範囲で衝突/u)
  assert.match(blockedImmediately.accessibleText, /開始姿勢の点判定は通過/u)
  assert.doesNotMatch(blockedImmediately.accessibleText, /開始姿勢で衝突を検出/u)

  const unknownAtStart = describeFoldPreviewContinuousMotion(state({
    requested: 90,
    applied: 0,
    status: 'indeterminate',
    reason: 'start_pose_indeterminate',
    result: {
      kind: 'indeterminate',
      certifiedSafeThrough: 0,
      stopTime: 0,
      unresolvedBracket: [0, 0],
      reason: 'start_pose_indeterminate',
      stats,
    },
  }))
  assert.match(unknownAtStart.badgeText, /開始姿勢を判定不能/u)
  assert.match(unknownAtStart.accessibleText, /安全確認済みとして扱いません/u)

  const unknownImmediately = describeFoldPreviewContinuousMotion(state({
    requested: 90,
    applied: 0,
    status: 'indeterminate',
    reason: 'uncertified_interval',
    result: {
      kind: 'indeterminate',
      certifiedSafeThrough: 0,
      stopTime: 0,
      unresolvedBracket: [0, 0.01],
      reason: 'uncertified_interval',
      stats,
    },
  }))
  assert.match(unknownImmediately.badgeText, /開始角からの範囲を判定不能/u)
  assert.match(unknownImmediately.accessibleText, /開始姿勢の点判定は通過/u)
})

test('malformed or disposed snapshots fail closed and all copy retains scope limits', () => {
  const malformed = describeFoldPreviewContinuousMotion(state({
    applied: Number.NaN,
  }))
  assert.equal(malformed.status, 'unavailable')
  assert.equal(malformed.badgeText, '経路判定不能')

  const malformedTerminal = describeFoldPreviewContinuousMotion(state({
    requested: 50,
    applied: 50,
    status: 'clear',
    result: null,
  }))
  assert.equal(malformedTerminal.status, 'unavailable')
  assert.doesNotMatch(malformedTerminal.accessibleText, /経路を確認しました/u)

  const mismatchedBoundary = describeFoldPreviewContinuousMotion(state({
    requested: 100,
    applied: 80,
    start: 0,
    status: 'blocked',
    reason: 'motion_blocked',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0.5,
      stopTime: 0.5,
      unsafeBracket: [0.5, 0.6],
      stats,
    },
  }))
  assert.equal(mismatchedBoundary.status, 'unavailable')
  assert.doesNotMatch(mismatchedBoundary.accessibleText, /最後に経路を確認/u)

  const mismatchedReason = describeFoldPreviewContinuousMotion(state({
    requested: 100,
    applied: 50,
    start: 0,
    status: 'blocked',
    reason: 'wrong_reason',
    result: {
      kind: 'blocked',
      certifiedSafeThrough: 0.5,
      stopTime: 0.5,
      unsafeBracket: [0.5, 0.6],
      stats,
    },
  }))
  assert.equal(mismatchedReason.status, 'unavailable')

  const nonzeroPointBracket = describeFoldPreviewContinuousMotion(state({
    requested: 100,
    applied: 50,
    start: 0,
    status: 'indeterminate',
    reason: 'uncertified_interval',
    result: {
      kind: 'indeterminate',
      certifiedSafeThrough: 0.5,
      stopTime: 0.5,
      unresolvedBracket: [0.5, 0.5],
      reason: 'uncertified_interval',
      stats,
    },
  }))
  assert.equal(nonzeroPointBracket.status, 'unavailable')

  const disposed = describeFoldPreviewContinuousMotion(state({
    requested: 50,
    applied: 20,
    status: 'disposed',
  }))
  assert.equal(disposed.status, 'unavailable')
  assert.match(disposed.badgeText, /表示 20°/u)

  for (const motion of [
    describeFoldPreviewContinuousMotion(null),
    describeFoldPreviewContinuousMotion(state()),
    disposed,
  ]) {
    assert.match(motion.accessibleText, /中央面基準/u)
    assert.match(motion.accessibleText, /折り癖と層ずれ/u)
  }
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
