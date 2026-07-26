import assert from 'node:assert/strict'
import test from 'node:test'

import type { ProjectSnapshot } from '../src/lib/coreClient.ts'
import type { FoldPreviewAppliedPoseSnapshot } from '../src/lib/foldPreviewAppliedPose.ts'
import { planInstructionAutoRecord } from '../src/lib/instructionAutoRecord.ts'
import { INSTRUCTION_AUTO_RECORD_TEXT } from '../src/lib/instructionAutoRecordText.ts'

const snapshot = {
  project_id: 'project',
  revision: 7,
  fold_model_fingerprint: 'ab'.repeat(32),
  instruction_timeline: { steps: [] },
} as ProjectSnapshot

const pose: FoldPreviewAppliedPoseSnapshot = {
  projectId: 'project',
  revision: 7,
  fixedFaceId: 'face',
  hingeAngles: [{ edgeId: 'hinge', angleDegrees: 90 }],
  state: 'stable',
}

test('plans exactly one editable step for a completed manual 3D edit', () => {
  const plan = planInstructionAutoRecord({
    enabled: true,
    sequence: 4,
    lastRecordedSequence: 3,
    snapshot,
    appliedPose: pose,
    locale: 'en',
  })
  assert.ok(plan)
  assert.equal(plan.sequence, 4)
  assert.equal(plan.title, 'Auto-recorded step 1')
  assert.deepEqual(plan.pose, {
    fixedFace: 'face',
    hingeAngles: [{ edge: 'hinge', angle_degrees: 90 }],
  })
})

test('auto-recorded titles use a frozen bilingual catalog with Japanese fallback', () => {
  assert.equal(Object.isFrozen(INSTRUCTION_AUTO_RECORD_TEXT), true)
  assert.deepEqual(INSTRUCTION_AUTO_RECORD_TEXT.stepTitle, {
    ja: '自動記録 手順 {step}',
    en: 'Auto-recorded step {step}',
  })
  assert.equal(Object.isFrozen(INSTRUCTION_AUTO_RECORD_TEXT.stepTitle), true)

  const plan = planInstructionAutoRecord({
    enabled: true,
    sequence: 4,
    lastRecordedSequence: 3,
    snapshot,
    appliedPose: pose,
    locale: 'unsupported' as never,
  })
  assert.equal(plan?.title, '自動記録 手順 1')
})

test('does not record enablement, playback, running, or stale poses', () => {
  for (const input of [
    { enabled: false, sequence: 4, lastRecordedSequence: 3, appliedPose: pose },
    { enabled: true, sequence: 3, lastRecordedSequence: 3, appliedPose: pose },
    {
      enabled: true,
      sequence: 4,
      lastRecordedSequence: 3,
      appliedPose: { ...pose, state: 'running' as const },
    },
    {
      enabled: true,
      sequence: 4,
      lastRecordedSequence: 3,
      appliedPose: { ...pose, revision: 6 },
    },
  ]) {
    assert.equal(planInstructionAutoRecord({
      ...input,
      snapshot,
      locale: 'ja',
    }), null)
  }
})
