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
  formatInstructionDuration,
  instructionCaptureStatusText,
  instructionEditorErrorText,
  instructionPlaybackStatusText,
  instructionPoseMatchesApplied,
  instructionTimelineNoticeText,
  reduceInstructionPlayback,
  resolveInstructionPoseApplicationObservation,
  validateInstructionMetadata,
  type InstructionPlaybackState,
  type InstructionPlaybackStopReason,
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

test('validates authored cameras, arrows, and focus points', () => {
  const authored = step('step-1', CURRENT_FINGERPRINT, [])
  authored.visual = {
    camera: {
      position: { x: 4, y: 3, z: 5 },
      target: { x: 0, y: 0, z: 0 },
      up: { x: 0, y: 1, z: 0 },
    },
    arrows: [{
      start: { x: 0, y: 0, z: 0 },
      end: { x: 1, y: 0, z: 0 },
      label: 'fold',
    }],
    focus_points: [{
      position: { x: 0.5, y: 0, z: 0 },
      radius: 0.1,
      label: 'corner',
    }],
  }
  const presentation = createInstructionTimelinePresentation(
    { steps: [authored] },
    CURRENT_FINGERPRINT,
  )
  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  assert.deepEqual(presentation.steps[0]?.visual, authored.visual)

  for (const visual of [
    { ...authored.visual, camera: { ...authored.visual.camera!, target: { x: 4, y: 3, z: 5 } } },
    { ...authored.visual, arrows: [{ ...authored.visual.arrows[0]!, end: { x: 0, y: 0, z: 0 } }] },
    { ...authored.visual, focus_points: [{ ...authored.visual.focus_points[0]!, radius: 0 }] },
  ]) {
    assert.equal(createInstructionTimelinePresentation(
      { steps: [{ ...authored, visual }] },
      CURRENT_FINGERPRINT,
    ).kind, 'invalid')
  }
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

test('localizes playback states and every stop reason without changing authored titles', () => {
  const first = step('step-1', CURRENT_FINGERPRINT, [], 1_500)
  first.title = 'Crane wing'
  const presentation = createInstructionTimelinePresentation({
    steps: [first],
  }, CURRENT_FINGERPRINT)
  const plan = createInstructionPlaybackPlan('project', 0, presentation)
  assert.ok(plan)

  const applying = reduceInstructionPlayback(createInstructionPlaybackState(), {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  assert.equal(
    instructionPlaybackStatusText(applying),
    '手順 1「Crane wing」を表示しています',
  )
  assert.equal(
    instructionPlaybackStatusText(applying, 'en'),
    'Applying step 1, “Crane wing”',
  )

  const holding = reduceInstructionPlayback(applying, {
    kind: 'pose_applied',
    stepId: 'step-1',
    now: 10,
  })
  assert.equal(
    instructionPlaybackStatusText(holding, 'en'),
    'Showing step 1, “Crane wing”',
  )
  const complete = reduceInstructionPlayback(holding, {
    kind: 'tick',
    now: 1_510,
  })
  assert.equal(
    instructionPlaybackStatusText(complete, 'en'),
    'Finished playing all folding steps',
  )

  const englishStops: Readonly<Record<InstructionPlaybackStopReason, string>> = {
    stale_step: 'Playback stopped because the crease pattern changed for this step',
    project_changed: 'Playback stopped because the project changed',
    revision_changed: 'Playback stopped because the edited content changed',
    model_changed: 'Playback stopped because the 3D model changed',
    manual_pose: 'Playback stopped because the 3D pose was changed manually',
    benchmark: 'Playback stopped because a performance test started',
    file_operation: 'Playback stopped because a file operation started',
    apply_failed: 'Playback stopped because the 3D pose could not be applied',
    hidden: 'Playback stopped because the window became hidden',
    disposed: 'Playback stopped because the view was closed',
    canceled: 'Folding-step playback stopped',
  }
  for (const [reason, expected] of Object.entries(englishStops) as Array<
    [InstructionPlaybackStopReason, string]
  >) {
    const stopped: InstructionPlaybackState = {
      status: 'stopped',
      sequence: 2,
      reason,
      stepId: null,
    }
    assert.equal(instructionPlaybackStatusText(stopped, 'en'), expected)
    assert.ok(instructionPlaybackStatusText(stopped, 'ja').length > 0)
  }
})

test('localizes timeline notices, capture guidance, validation, and durations live', () => {
  assert.equal(
    instructionTimelineNoticeText({ kind: 'added', title: '鶴' }, 'en'),
    'Added “鶴”',
  )
  assert.equal(
    instructionTimelineNoticeText({ kind: 'pose_update_failed' }, 'ja'),
    '手順の姿勢を更新できませんでした',
  )
  assert.equal(
    instructionCaptureStatusText('pose_blocked', 'en'),
    'Records the displayed pose that stopped safely at a collision boundary.',
  )
  assert.match(
    instructionEditorErrorText('invalid_metadata', 'en'),
    /120 characters.*100–600000 ms/u,
  )
  assert.equal(
    instructionEditorErrorText('update_failed', 'en'),
    'Could not update the step details',
  )
  assert.equal(formatInstructionDuration(1_500), '1.5秒')
  assert.equal(formatInstructionDuration(1_500, 'en'), '1.5 seconds')
  assert.equal(formatInstructionDuration(90_000, 'en'), '1:30')
})

test('admits declarative-only steps but never treats them as a playable 3D pose', () => {
  const declarative = {
    ...step('declarative', OLD_FINGERPRINT, []),
    title: '中割り折りの説明',
    pose: {
      model: 'declarative_only_v1' as const,
      source_model_fingerprint: OLD_FINGERPRINT,
      fixed_face: null,
      hinge_angles: [],
    },
  }
  const presentation = createInstructionTimelinePresentation({
    steps: [declarative],
  }, CURRENT_FINGERPRINT)
  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  assert.equal(presentation.steps[0]?.declarativeOnly, true)
  assert.equal(presentation.steps[0]?.stale, false)
  assert.equal(
    createInstructionPlaybackPlan('project', 0, presentation),
    null,
  )
  assert.equal(
    instructionPoseMatchesApplied(declarative.pose, {
      projectId: 'project',
      revision: 0,
      fixedFaceId: null,
      hingeAngles: [],
      state: 'stable',
    }),
    false,
  )
  assert.match(
    instructionTimelineNoticeText({
      kind: 'declarative_playback_unsupported',
    }, 'en'),
    /cannot be played/u,
  )
})

test('mixed playback skips declarative steps without changing executable order or timeline ordinals', () => {
  const declarative = {
    ...step('declarative', OLD_FINGERPRINT, [], 100),
    pose: {
      model: 'declarative_only_v1' as const,
      source_model_fingerprint: OLD_FINGERPRINT,
      fixed_face: null,
      hinge_angles: [],
    },
  }
  const presentation = createInstructionTimelinePresentation({
    steps: [
      step('physical-1', CURRENT_FINGERPRINT, [], 100),
      declarative,
      step('physical-2', CURRENT_FINGERPRINT, [], 100),
    ],
  }, CURRENT_FINGERPRINT)
  const plan = createInstructionPlaybackPlan('project', 0, presentation)
  assert.ok(plan)
  assert.deepEqual(
    plan.steps.map(({ id, index }) => ({ id, index })),
    [
      { id: 'physical-1', index: 0 },
      { id: 'physical-2', index: 2 },
    ],
  )

  let state = reduceInstructionPlayback(createInstructionPlaybackState(), {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  assert.equal(state.status === 'applying' ? state.target.id : null, 'physical-1')
  state = reduceInstructionPlayback(state, {
    kind: 'pose_applied',
    stepId: 'physical-1',
    now: 0,
  })
  state = reduceInstructionPlayback(state, { kind: 'tick', now: 100 })
  assert.equal(state.status === 'applying' ? state.target.id : null, 'physical-2')
  assert.equal(
    instructionPlaybackStatusText(state, 'en'),
    'Applying step 3, “手順”',
  )

  const canceled = reduceInstructionPlayback(state, {
    kind: 'cancel',
    reason: 'canceled',
  })
  assert.equal(canceled.status === 'stopped' ? canceled.reason : null, 'canceled')
  assert.deepEqual(
    reduceInstructionPlayback(canceled, { kind: 'tick', now: 1_000 }),
    canceled,
  )
})

test('mixed playback stops before a stale physical step and rejects a forged declarative plan', () => {
  const declarative = {
    ...step('declarative', CURRENT_FINGERPRINT, [], 100),
    pose: {
      model: 'declarative_only_v1' as const,
      source_model_fingerprint: CURRENT_FINGERPRINT,
      fixed_face: null,
      hinge_angles: [],
    },
  }
  const presentation = createInstructionTimelinePresentation({
    steps: [
      step('physical-1', CURRENT_FINGERPRINT, [], 100),
      declarative,
      step('physical-stale', OLD_FINGERPRINT, [], 100),
    ],
  }, CURRENT_FINGERPRINT)
  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  const plan = createInstructionPlaybackPlan('project', 0, presentation)
  assert.ok(plan)

  let state = reduceInstructionPlayback(createInstructionPlaybackState(), {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  state = reduceInstructionPlayback(state, {
    kind: 'pose_applied',
    stepId: 'physical-1',
    now: 0,
  })
  state = reduceInstructionPlayback(state, { kind: 'tick', now: 100 })
  assert.deepEqual(state, {
    status: 'stopped',
    sequence: 1,
    reason: 'stale_step',
    stepId: 'physical-stale',
  })

  const forged = {
    projectId: 'project',
    revision: 0,
    modelFingerprint: CURRENT_FINGERPRINT,
    steps: [presentation.steps[1]!],
  }
  assert.equal(
    reduceInstructionPlayback(createInstructionPlaybackState(), {
      kind: 'start',
      plan: forged,
      startIndex: 0,
    }).status,
    'stopped',
  )
})

test('rejects declarative steps that smuggle a fixed face or hinge angle', () => {
  const base = {
    ...step('declarative', CURRENT_FINGERPRINT, []),
    pose: {
      model: 'declarative_only_v1' as const,
      source_model_fingerprint: CURRENT_FINGERPRINT,
      fixed_face: null,
      hinge_angles: [],
    },
  }
  for (const pose of [{
    ...base.pose,
    fixed_face: 'face-1',
  }, {
    ...base.pose,
    hinge_angles: [angle('hinge-1', 0)],
  }]) {
    assert.equal(
      createInstructionTimelinePresentation({
        steps: [{ ...base, pose }],
      }, CURRENT_FINGERPRINT).kind,
      'invalid',
    )
  }
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
    visual: {
      camera: null,
      arrows: [],
      focus_points: [],
    },
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
