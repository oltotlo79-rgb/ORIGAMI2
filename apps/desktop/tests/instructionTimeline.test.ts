import assert from 'node:assert/strict'
import test from 'node:test'

import type { InstructionTimeline } from '../src/lib/coreClient.ts'
import {
  DEFAULT_INSTRUCTION_DURATION_MS,
  INSTRUCTION_APPLICATION_TIMEOUT_MS,
  MAX_INSTRUCTION_HINGES_PER_STEP,
  MAX_INSTRUCTION_STEPS,
  MAX_INSTRUCTION_TOTAL_HINGES,
  createInstructionPlaybackPlan,
  createInstructionPlaybackState,
  createInstructionPoseDraft,
  createInstructionTimelinePresentation,
  instructionPoseMatchesApplied,
  reduceInstructionPlayback,
  resolveInstructionPoseApplicationObservation,
  validateInstructionMetadata,
} from '../src/lib/instructionTimeline.ts'
import type { FoldPreviewAppliedPoseSnapshot } from '../src/lib/foldPreviewAppliedPose.ts'

const CURRENT_FINGERPRINT = 'ab'.repeat(32)
const OLD_FINGERPRINT = 'cd'.repeat(32)

test('validates and detaches a current and a stale instruction step', () => {
  const timeline = {
    steps: [
      step('step-1', CURRENT_FINGERPRINT, [angle('hinge-1', 30)]),
      step('step-2', OLD_FINGERPRINT, [angle('hinge-1', 90)]),
    ],
  }
  const presentation = createInstructionTimelinePresentation(
    timeline,
    CURRENT_FINGERPRINT,
  )

  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  assert.equal(presentation.steps[0]?.stale, false)
  assert.equal(presentation.steps[1]?.stale, true)
  assert.equal(presentation.stepsById.get('step-2')?.index, 1)
  assert.equal(Object.isFrozen(presentation.stepsById), true)
  assert.equal('set' in presentation.stepsById, false)
  assert.equal(
    presentation.totalDurationMs,
    DEFAULT_INSTRUCTION_DURATION_MS * 2,
  )
  timeline.steps[0]!.title = 'mutated'
  assert.equal(presentation.steps[0]?.title, '手順')
})

test('fails closed for unknown fields, models, fingerprints, duplicates, and invalid values', () => {
  const valid = { steps: [step('step-1', CURRENT_FINGERPRINT, [angle('hinge-1', 30)])] }
  const invalid: unknown[] = [
    null,
    {},
    { ...valid, unknown: true },
    { steps: [{ ...valid.steps[0], unknown: true }] },
    { steps: [{ ...valid.steps[0], title: '' }] },
    { steps: [{ ...valid.steps[0], title: 'line\nbreak' }] },
    { steps: [{ ...valid.steps[0], title: 'tab\tbreak' }] },
    { steps: [{ ...valid.steps[0], title: 'next\u0085line' }] },
    { steps: [{ ...valid.steps[0], title: 'x'.repeat(121) }] },
    { steps: [{ ...valid.steps[0], description: 'x'.repeat(4_001) }] },
    { steps: [{ ...valid.steps[0], caution: 'x'.repeat(2_001) }] },
    { steps: [{ ...valid.steps[0], duration_ms: 99 }] },
    { steps: [{ ...valid.steps[0], duration_ms: 600_001 }] },
    { steps: [{ ...valid.steps[0], pose: { ...valid.steps[0]!.pose, model: 'future' } }] },
    { steps: [{ ...valid.steps[0], pose: {
      ...valid.steps[0]!.pose,
      source_model_fingerprint: CURRENT_FINGERPRINT.toUpperCase(),
    } }] },
    { steps: [{ ...valid.steps[0], pose: {
      ...valid.steps[0]!.pose,
      hinge_angles: [angle('hinge-1', 1), angle('hinge-1', 2)],
    } }] },
    { steps: [{ ...valid.steps[0], pose: {
      ...valid.steps[0]!.pose,
      hinge_angles: [angle('hinge-2', 1), angle('hinge-1', 2)],
    } }] },
    { steps: [{ ...valid.steps[0], pose: {
      ...valid.steps[0]!.pose,
      hinge_angles: [angle('hinge-1', -1)],
    } }] },
    { steps: [valid.steps[0], structuredClone(valid.steps[0])] },
  ]

  for (const value of invalid) {
    assert.equal(
      createInstructionTimelinePresentation(value, CURRENT_FINGERPRINT).kind,
      'invalid',
    )
  }
  assert.equal(
    createInstructionTimelinePresentation(valid, 'not-a-fingerprint').kind,
    'invalid',
  )
})

test('enforces inclusive step and hinge work limits in linear time', () => {
  const maximumStepTimeline = {
    steps: Array.from({ length: MAX_INSTRUCTION_STEPS }, (_, index) =>
      step(`step-${index}`, CURRENT_FINGERPRINT, [])),
  }
  assert.equal(
    createInstructionTimelinePresentation(
      maximumStepTimeline,
      CURRENT_FINGERPRINT,
    ).kind,
    'ready',
  )
  maximumStepTimeline.steps.push(step('too-many', CURRENT_FINGERPRINT, []))
  assert.equal(
    createInstructionTimelinePresentation(
      maximumStepTimeline,
      CURRENT_FINGERPRINT,
    ).kind,
    'invalid',
  )

  const maximumHinges = Array.from(
    { length: MAX_INSTRUCTION_HINGES_PER_STEP },
    (_, index) => angle(`hinge-${index.toString().padStart(5, '0')}`, index % 181),
  )
  assert.equal(
    createInstructionTimelinePresentation(
      { steps: [step('large', CURRENT_FINGERPRINT, maximumHinges)] },
      CURRENT_FINGERPRINT,
    ).kind,
    'ready',
  )
  maximumHinges.push(angle('too-many', 0))
  assert.equal(
    createInstructionTimelinePresentation(
      { steps: [step('large', CURRENT_FINGERPRINT, maximumHinges)] },
      CURRENT_FINGERPRINT,
    ).kind,
    'invalid',
  )

  const tenThousand = maximumHinges.slice(0, MAX_INSTRUCTION_HINGES_PER_STEP)
  const totalLimit = {
    steps: Array.from(
      { length: MAX_INSTRUCTION_TOTAL_HINGES / MAX_INSTRUCTION_HINGES_PER_STEP },
      (_, index) => step(
        `large-${index}`,
        CURRENT_FINGERPRINT,
        tenThousand.map((item) => ({
          ...item,
          edge: `${item.edge}-${index}`,
        })),
      ),
    ),
  }
  assert.equal(
    createInstructionTimelinePresentation(totalLimit, CURRENT_FINGERPRINT).kind,
    'ready',
  )
  totalLimit.steps.push(step('overflow', CURRENT_FINGERPRINT, [angle('extra', 0)]))
  assert.equal(
    createInstructionTimelinePresentation(totalLimit, CURRENT_FINGERPRINT).kind,
    'invalid',
  )
})

test('validates editable metadata and captures only an actually applied pose', () => {
  assert.deepEqual(validateInstructionMetadata({
    title: '  手順 1  ',
    description: '説明\n2行目',
    caution: '注意',
    durationMs: 1_500,
  }), {
    title: '手順 1',
    description: '説明\n2行目',
    caution: '注意',
    durationMs: 1_500,
  })
  assert.equal(validateInstructionMetadata({
    title: 'bad\u0000',
    description: '',
    caution: '',
    durationMs: 1_500,
  }), null)
  assert.equal(validateInstructionMetadata({
    title: 'bad\nline',
    description: '',
    caution: '',
    durationMs: 1_500,
  }), null)

  const applied = appliedPose('stable', [
    { edgeId: 'hinge-2', angleDegrees: -0 },
    { edgeId: 'hinge-1', angleDegrees: 35 },
  ])
  assert.deepEqual(createInstructionPoseDraft(applied, CURRENT_FINGERPRINT), {
    fixedFace: 'face-1',
    hingeAngles: [
      { edge: 'hinge-1', angle_degrees: 35 },
      { edge: 'hinge-2', angle_degrees: 0 },
    ],
  })
  assert.equal(
    createInstructionPoseDraft({ ...applied, state: 'running' }, CURRENT_FINGERPRINT),
    null,
  )
  assert.deepEqual(
    createInstructionPoseDraft({
      ...applied,
      fixedFaceId: null,
      hingeAngles: [],
    }, CURRENT_FINGERPRINT),
    { fixedFace: null, hingeAngles: [] },
  )
  assert.equal(
    createInstructionPoseDraft({
      ...applied,
      fixedFaceId: null,
    }, CURRENT_FINGERPRINT),
    null,
  )
  assert.equal(
    createInstructionPoseDraft({
      ...applied,
      hingeAngles: [],
    }, CURRENT_FINGERPRINT),
    null,
  )
})

test('matches complete hinge vectors independent of record order', () => {
  const pose = step('step', CURRENT_FINGERPRINT, [
    angle('hinge-1', 10),
    angle('hinge-2', 20),
  ]).pose
  assert.equal(instructionPoseMatchesApplied(
    pose,
    appliedPose('stable', [
      { edgeId: 'hinge-2', angleDegrees: 20 },
      { edgeId: 'hinge-1', angleDegrees: 10 },
    ]),
  ), true)
  assert.equal(instructionPoseMatchesApplied(
    pose,
    appliedPose('stable', [{ edgeId: 'hinge-1', angleDegrees: 10 }]),
  ), false)
  assert.equal(instructionPoseMatchesApplied(
    pose,
    appliedPose('stable', [
      { edgeId: 'hinge-1', angleDegrees: 10 },
      { edgeId: 'hinge-2', angleDegrees: 21 },
    ]),
  ), false)
})

test('application waits for a fresh observation and fails on a mismatched terminal endpoint', () => {
  assert.equal(INSTRUCTION_APPLICATION_TIMEOUT_MS, 30_000)
  const pose = step('step', CURRENT_FINGERPRINT, [
    angle('hinge-1', 90),
  ]).pose
  const beforeApply = appliedPose('stable', [
    { edgeId: 'hinge-1', angleDegrees: 10 },
  ])

  assert.equal(
    resolveInstructionPoseApplicationObservation(pose, beforeApply, beforeApply),
    'wait',
  )
  assert.equal(
    resolveInstructionPoseApplicationObservation(
      pose,
      beforeApply,
      { ...beforeApply, state: 'running' },
    ),
    'wait',
  )
  assert.equal(
    resolveInstructionPoseApplicationObservation(
      pose,
      beforeApply,
      {
        ...beforeApply,
        hingeAngles: beforeApply.hingeAngles.map((angle) => ({ ...angle })),
      },
    ),
    'wait',
  )
  for (const state of ['blocked', 'indeterminate'] as const) {
    assert.equal(
      resolveInstructionPoseApplicationObservation(
        pose,
        beforeApply,
        { ...beforeApply, state },
      ),
      'fail',
    )
  }
  assert.equal(
    resolveInstructionPoseApplicationObservation(
      pose,
      beforeApply,
      appliedPose('stable', [{ edgeId: 'hinge-1', angleDegrees: 11 }]),
    ),
    'fail',
  )
  assert.equal(
    resolveInstructionPoseApplicationObservation(
      pose,
      beforeApply,
      appliedPose('blocked', [{ edgeId: 'hinge-1', angleDegrees: 90 }]),
    ),
    'acknowledge',
  )
  assert.equal(
    resolveInstructionPoseApplicationObservation(pose, beforeApply, null),
    'wait',
  )
})

test('plays discrete endpoints through applying, acknowledgement, holding, and completion', () => {
  const presentation = createInstructionTimelinePresentation({
    steps: [
      step('step-1', CURRENT_FINGERPRINT, [angle('hinge', 30)], 100),
      step('step-2', CURRENT_FINGERPRINT, [angle('hinge', 60)], 200),
    ],
  }, CURRENT_FINGERPRINT)
  const plan = createInstructionPlaybackPlan('project', 7, presentation)
  assert.ok(plan)

  let state = reduceInstructionPlayback(createInstructionPlaybackState(), {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  assert.equal(state.status, 'applying')
  state = reduceInstructionPlayback(state, {
    kind: 'pose_applied',
    stepId: 'wrong',
    now: 10,
  })
  assert.equal(state.status, 'applying')
  state = reduceInstructionPlayback(state, {
    kind: 'pose_applied',
    stepId: 'step-1',
    now: 10,
  })
  assert.equal(state.status, 'holding')
  state = reduceInstructionPlayback(state, { kind: 'tick', now: 109 })
  assert.equal(state.status, 'holding')
  state = reduceInstructionPlayback(state, { kind: 'tick', now: 110 })
  assert.equal(state.status, 'applying')
  state = reduceInstructionPlayback(state, {
    kind: 'pose_applied',
    stepId: 'step-2',
    now: 120,
  })
  state = reduceInstructionPlayback(state, { kind: 'tick', now: 320 })
  assert.deepEqual(state, {
    status: 'complete',
    sequence: 1,
    lastStepId: 'step-2',
  })
})

test('stops at stale steps, failed application, and explicit invalidation', () => {
  const presentation = createInstructionTimelinePresentation({
    steps: [
      step('current', CURRENT_FINGERPRINT, [], 100),
      step('stale', OLD_FINGERPRINT, [], 100),
    ],
  }, CURRENT_FINGERPRINT)
  const plan = createInstructionPlaybackPlan('project', 0, presentation)
  assert.ok(plan)

  let state = reduceInstructionPlayback(createInstructionPlaybackState(), {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  state = reduceInstructionPlayback(state, {
    kind: 'pose_applied',
    stepId: 'current',
    now: 0,
  })
  state = reduceInstructionPlayback(state, { kind: 'tick', now: 100 })
  assert.equal(state.status, 'stopped')
  assert.equal(state.status === 'stopped' ? state.reason : null, 'stale_step')

  state = reduceInstructionPlayback(createInstructionPlaybackState(), {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  state = reduceInstructionPlayback(state, { kind: 'apply_failed' })
  assert.equal(state.status === 'stopped' ? state.reason : null, 'apply_failed')

  state = reduceInstructionPlayback(createInstructionPlaybackState(), {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  state = reduceInstructionPlayback(state, {
    kind: 'cancel',
    reason: 'manual_pose',
  })
  assert.equal(state.status === 'stopped' ? state.reason : null, 'manual_pose')
})

function step(
  id: string,
  fingerprint: string,
  hingeAngles: Array<{ edge: string; angle_degrees: number }>,
  durationMs = DEFAULT_INSTRUCTION_DURATION_MS,
) {
  return {
    id,
    title: '手順',
    description: '',
    caution: '',
    duration_ms: durationMs,
    pose: {
      model: 'absolute_hinge_angles_v1' as const,
      source_model_fingerprint: fingerprint,
      fixed_face: hingeAngles.length === 0 ? null : 'face-1',
      hinge_angles: hingeAngles,
    },
  }
}

function angle(edge: string, angleDegrees: number) {
  return { edge, angle_degrees: angleDegrees }
}

function appliedPose(
  state: FoldPreviewAppliedPoseSnapshot['state'],
  hingeAngles: FoldPreviewAppliedPoseSnapshot['hingeAngles'],
): FoldPreviewAppliedPoseSnapshot {
  return {
    projectId: 'project',
    revision: 7,
    fixedFaceId: 'face-1',
    hingeAngles,
    state,
  }
}

const _timelineTypeCheck: InstructionTimeline = { steps: [] }
void _timelineTypeCheck
