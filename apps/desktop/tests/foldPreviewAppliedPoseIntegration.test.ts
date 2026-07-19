import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const previewSource = readFileSync(
  new URL('../src/components/FoldPreview.tsx', import.meta.url),
  'utf8',
)
const appSource = readFileSync(
  new URL('../src/App.tsx', import.meta.url),
  'utf8',
)
const timelineSource = readFileSync(
  new URL('../src/components/InstructionTimelinePanel.tsx', import.meta.url),
  'utf8',
)

test('FoldPreview publishes rendered endpoints instead of requested controls', () => {
  assert.match(
    previewSource,
    /angleDegrees:\s*currentSingleAppliedAngle/u,
  )
  assert.match(
    previewSource,
    /state:\s*appliedPoseState\(currentSingleMotionStatus\)/u,
  )
  assert.match(
    previewSource,
    /hingeAngles:\s*renderedTreePose\.appliedAngles/u,
  )
  assert.match(
    previewSource,
    /model\.kind === 'planar'[\s\S]*?fixedFaceId:\s*null[\s\S]*?hingeAngles:\s*\[\]/u,
  )
  assert.doesNotMatch(
    previewSource,
    /hingeAngles:\s*\[\{\s*edgeId:\s*model\.hinge\.edgeId,\s*angleDegrees:\s*safeAngle/u,
  )
})

test('App wires the detached observation into the real timeline editor', () => {
  assert.match(
    appSource,
    /onAppliedPoseChange=\{setAppliedFoldPose\}/u,
  )
  assert.match(
    appSource,
    /<InstructionTimelinePanel[\s\S]*?appliedPose=\{appliedFoldPose\}/u,
  )
  assert.match(
    timelineSource,
    /createInstructionPoseDraft\([\s\S]*?appliedPose/u,
  )
  assert.match(
    timelineSource,
    /instructionPoseMatchesApplied\(/u,
  )
})

test('playback discloses endpoint-only semantics and cancels unsafe lifecycles', () => {
  assert.match(
    timelineSource,
    /姿勢間の連続した折り経路の安全性は保証しません/u,
  )
  assert.match(
    timelineSource,
    /setTimeout\([\s\S]*?INSTRUCTION_APPLICATION_TIMEOUT_MS/u,
  )
  assert.match(
    timelineSource,
    /className="instruction-notice"[\s\S]*?\{noticeText\}/u,
  )
  assert.match(
    timelineSource,
    /className="visually-hidden" aria-live="polite"[\s\S]*?\{noticeText\}/u,
  )
  assert.match(
    timelineSource,
    /instructionTimelineNoticeText\(notice, locale\)/u,
  )
  for (const reason of [
    'project_changed',
    'revision_changed',
    'model_changed',
    'manual_pose',
    'benchmark',
    'file_operation',
    'hidden',
    'apply_failed',
    'disposed',
  ]) {
    assert.match(timelineSource, new RegExp(`['"]${reason}['"]`, 'u'))
  }
})
