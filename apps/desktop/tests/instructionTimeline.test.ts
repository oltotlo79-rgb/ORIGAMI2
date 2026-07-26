import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
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
  createInstructionInterpolatedStep,
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
  type InstructionCaptureStatus,
  type InstructionEditorError,
  type InstructionPlaybackState,
  type InstructionPlaybackStopReason,
  type InstructionTimelineNotice,
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

test('validates authored cameras, arrows, focus points, and hand guides', () => {
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
    hand_guides: [{
      kind: 'pinch',
      position: { x: 0.5, y: 0, z: 0 },
      direction: { x: 0, y: -1, z: 0 },
      label: 'pinch',
    }],
  }
  const presentation = createInstructionTimelinePresentation(
    { steps: [authored] },
    CURRENT_FINGERPRINT,
  )
  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  assert.deepEqual(presentation.steps[0]?.visual, authored.visual)
  const regrip = {
    ...authored,
    visual: {
      ...authored.visual,
      hand_guides: [{ ...authored.visual.hand_guides[0]!, kind: 'regrip' as const }],
    },
  }
  assert.equal(createInstructionTimelinePresentation(
    { steps: [regrip] },
    CURRENT_FINGERPRINT,
  ).kind, 'ready')

  for (const visual of [
    { ...authored.visual, camera: { ...authored.visual.camera!, target: { x: 4, y: 3, z: 5 } } },
    { ...authored.visual, arrows: [{ ...authored.visual.arrows[0]!, end: { x: 0, y: 0, z: 0 } }] },
    { ...authored.visual, focus_points: [{ ...authored.visual.focus_points[0]!, radius: 0 }] },
    { ...authored.visual, hand_guides: [{ ...authored.visual.hand_guides[0]!, kind: 'unknown' }] },
    { ...authored.visual, hand_guides: [{ ...authored.visual.hand_guides[0]!, direction: { x: 0, y: 0, z: 0 } }] },
  ]) {
    assert.equal(createInstructionTimelinePresentation(
      { steps: [{ ...authored, visual }] },
      CURRENT_FINGERPRINT,
    ).kind, 'invalid')
  }
})

test('interpolates every hinge together for smooth timeline animation', () => {
  const presentation = createInstructionTimelinePresentation({
    steps: [step('step-1', CURRENT_FINGERPRINT, [
      angle('hinge-1', 90),
      angle('hinge-2', 30),
    ])],
  }, CURRENT_FINGERPRINT)
  assert.equal(presentation.kind, 'ready')
  if (presentation.kind !== 'ready') return
  const interpolated = createInstructionInterpolatedStep(
    presentation.steps[0]!,
    appliedPose('stable', [
      { edgeId: 'hinge-1', angleDegrees: 10 },
      { edgeId: 'hinge-2', angleDegrees: 90 },
    ]),
    0.25,
  )
  assert.deepEqual(interpolated?.pose.hinge_angles, [
    { edge: 'hinge-1', angle_degrees: 30 },
    { edge: 'hinge-2', angle_degrees: 75 },
  ])
  assert.equal(createInstructionInterpolatedStep(
    presentation.steps[0]!,
    appliedPose('stable', [{ edgeId: 'hinge-1', angleDegrees: 10 }]),
    0.5,
  ), null)
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

test('goldens every playback state and stop reason without rewriting raw authored text', () => {
  const rawTitle = '  <b>{title}</b>  '
  const first = step('step-1', CURRENT_FINGERPRINT, [], 1_500)
  first.title = rawTitle
  const presentation = createInstructionTimelinePresentation({
    steps: [first],
  }, CURRENT_FINGERPRINT)
  const plan = createInstructionPlaybackPlan('project', 0, presentation)
  assert.ok(plan)

  const idle = createInstructionPlaybackState()
  const applying = reduceInstructionPlayback(idle, {
    kind: 'start',
    plan,
    startIndex: 0,
  })
  const holding = reduceInstructionPlayback(applying, {
    kind: 'pose_applied',
    stepId: 'step-1',
    now: 10,
  })
  const complete = reduceInstructionPlayback(holding, {
    kind: 'tick',
    now: 1_510,
  })
  const playbackGoldens = [
    {
      state: idle,
      ja: '再生停止中',
      en: 'Playback stopped',
    },
    {
      state: applying,
      ja: '手順 1「  <b>{title}</b>  」を表示しています',
      en: 'Applying step 1, “  <b>{title}</b>  ”',
    },
    {
      state: holding,
      ja: '手順 1「  <b>{title}</b>  」を表示中です',
      en: 'Showing step 1, “  <b>{title}</b>  ”',
    },
    {
      state: complete,
      ja: '折り手順の段階再生が完了しました',
      en: 'Finished playing all folding steps',
    },
  ] satisfies readonly Readonly<{
    state: InstructionPlaybackState
    ja: string
    en: string
  }>[]
  for (const golden of playbackGoldens) {
    assert.equal(instructionPlaybackStatusText(golden.state, 'ja'), golden.ja)
    assert.equal(instructionPlaybackStatusText(golden.state, 'en'), golden.en)
  }

  const stopGoldens = {
    stale_step: {
      ja: '展開図が変わった手順のため再生を停止しました',
      en: 'Playback stopped because the crease pattern changed for this step',
    },
    project_changed: {
      ja: 'プロジェクトが変わったため再生を停止しました',
      en: 'Playback stopped because the project changed',
    },
    revision_changed: {
      ja: '編集中の内容が変わったため再生を停止しました',
      en: 'Playback stopped because the edited content changed',
    },
    model_changed: {
      ja: '3Dモデルが変わったため再生を停止しました',
      en: 'Playback stopped because the 3D model changed',
    },
    manual_pose: {
      ja: '3D姿勢を手動変更したため再生を停止しました',
      en: 'Playback stopped because the 3D pose was changed manually',
    },
    benchmark: {
      ja: '性能テストを開始したため再生を停止しました',
      en: 'Playback stopped because a performance test started',
    },
    file_operation: {
      ja: 'ファイル操作を開始したため再生を停止しました',
      en: 'Playback stopped because a file operation started',
    },
    apply_failed: {
      ja: '3D姿勢を適用できなかったため再生を停止しました',
      en: 'Playback stopped because the 3D pose could not be applied',
    },
    hidden: {
      ja: '画面が非表示になったため再生を停止しました',
      en: 'Playback stopped because the window became hidden',
    },
    disposed: {
      ja: '画面を閉じたため再生を停止しました',
      en: 'Playback stopped because the view was closed',
    },
    canceled: {
      ja: '折り手順の再生を停止しました',
      en: 'Folding-step playback stopped',
    },
  } satisfies Readonly<Record<
    InstructionPlaybackStopReason,
    Readonly<{ ja: string; en: string }>
  >>
  const secretStepId = 'SECRET_STOP_STEP_ID'
  for (const [reason, golden] of Object.entries(stopGoldens) as Array<
    [InstructionPlaybackStopReason, { ja: string; en: string }]
  >) {
    const stopped: InstructionPlaybackState = {
      status: 'stopped',
      sequence: 2,
      reason,
      stepId: secretStepId,
    }
    const ja = instructionPlaybackStatusText(stopped, 'ja')
    const en = instructionPlaybackStatusText(stopped, 'en')
    assert.equal(ja, golden.ja)
    assert.equal(en, golden.en)
    assert.equal(ja.includes(secretStepId), false)
    assert.equal(en.includes(secretStepId), false)
  }
})

test('goldens every timeline notice and playback forwarding in both locales', () => {
  type NoticeKind = Exclude<InstructionTimelineNotice['kind'], 'playback'>
  type NoticeGolden = {
    readonly [Kind in NoticeKind]: Readonly<{
      notice: Extract<InstructionTimelineNotice, { kind: Kind }>
      ja: string
      en: string
    }>
  }
  const title = '  <b>{title}</b>  '
  const noticeGoldens = {
    add_failed: {
      notice: { kind: 'add_failed' },
      ja: '現在の3D姿勢を手順へ追加できませんでした',
      en: 'Could not add the current 3D pose as a step',
    },
    added: {
      notice: { kind: 'added', title },
      ja: '「  <b>{title}</b>  」を追加しました',
      en: 'Added “  <b>{title}</b>  ”',
    },
    updated: {
      notice: { kind: 'updated', title },
      ja: '「  <b>{title}</b>  」を更新しました',
      en: 'Updated “  <b>{title}</b>  ”',
    },
    update_failed: {
      notice: { kind: 'update_failed' },
      ja: '手順を更新できませんでした',
      en: 'Could not update the step',
    },
    pose_updated: {
      notice: { kind: 'pose_updated', title },
      ja: '「  <b>{title}</b>  」の姿勢を現在の3D表示で更新しました',
      en: 'Updated the pose for “  <b>{title}</b>  ” from the current 3D view',
    },
    pose_update_failed: {
      notice: { kind: 'pose_update_failed' },
      ja: '手順の姿勢を更新できませんでした',
      en: 'Could not update the step pose',
    },
    delete_failed: {
      notice: { kind: 'delete_failed' },
      ja: '手順を削除できませんでした',
      en: 'Could not delete the step',
    },
    deleted: {
      notice: { kind: 'deleted', title },
      ja: '「  <b>{title}</b>  」を削除しました',
      en: 'Deleted “  <b>{title}</b>  ”',
    },
    moved: {
      notice: { kind: 'moved' },
      ja: '手順の順番を変更しました',
      en: 'Changed the step order',
    },
    split: {
      notice: { kind: 'split' },
      ja: '手順を分割しました',
      en: 'Split the step',
    },
    merged: {
      notice: { kind: 'merged' },
      ja: '手順を次の手順と結合しました',
      en: 'Merged the step with the next step',
    },
    move_failed: {
      notice: { kind: 'move_failed' },
      ja: '手順を移動できませんでした',
      en: 'Could not move the step',
    },
    stale_pose: {
      notice: { kind: 'stale_pose' },
      ja: '展開図が変更された手順です。「現在の3D姿勢で更新」してから表示してください',
      en: 'The crease pattern changed for this step. Update it with the current 3D pose before showing it.',
    },
    pose_apply_failed: {
      notice: { kind: 'pose_apply_failed' },
      ja: 'この手順の姿勢は現在の3Dモデルへ適用できません',
      en: 'This step pose cannot be applied to the current 3D model',
    },
    pose_applying: {
      notice: { kind: 'pose_applying', title },
      ja: '「  <b>{title}</b>  」の保存姿勢を3Dへ適用しています',
      en: 'Applying the saved pose for “  <b>{title}</b>  ” to the 3D view',
    },
    model_required: {
      notice: { kind: 'model_required' },
      ja: '再生できる3Dモデルを準備してください',
      en: 'Prepare a 3D model that can be played',
    },
    no_steps: {
      notice: { kind: 'no_steps' },
      ja: '再生する手順がありません',
      en: 'There are no steps to play',
    },
    declarative_playback_unsupported: {
      notice: { kind: 'declarative_playback_unsupported' },
      ja: '説明専用ステップは3D姿勢を持たないため再生できません。内容は一覧で確認してください',
      en: 'Description-only steps have no 3D pose and cannot be played. Review them in the timeline list.',
    },
  } satisfies NoticeGolden

  assert.equal(Object.keys(noticeGoldens).length, 18)
  for (const golden of Object.values(noticeGoldens)) {
    assert.equal(instructionTimelineNoticeText(golden.notice, 'ja'), golden.ja)
    assert.equal(instructionTimelineNoticeText(golden.notice, 'en'), golden.en)
  }

  const playbackNotice: InstructionTimelineNotice = {
    kind: 'playback',
    state: createInstructionPlaybackState(),
  }
  assert.equal(
    instructionTimelineNoticeText(playbackNotice, 'ja'),
    instructionPlaybackStatusText(playbackNotice.state, 'ja'),
  )
  assert.equal(
    instructionTimelineNoticeText(playbackNotice, 'en'),
    instructionPlaybackStatusText(playbackNotice.state, 'en'),
  )
})

test('goldens every capture status and editor error in both locales', () => {
  const captureGoldens = {
    project_required: {
      ja: 'プロジェクトを読み込んでください。',
      en: 'Open a project first.',
    },
    pose_required: {
      ja: '現在のrevisionの3D表示を準備しています。',
      en: 'Preparing the 3D view for the current revision.',
    },
    pose_running: {
      ja: '3Dの動作が止まってから記録できます。',
      en: 'Wait for the 3D motion to stop before recording.',
    },
    pose_invalid: {
      ja: '現在の3D姿勢は手順として安全に読み取れません。',
      en: 'The current 3D pose cannot be read safely as a step.',
    },
    pose_blocked: {
      ja: '衝突境界で安全に停止している表示姿勢を記録します。',
      en: 'Records the displayed pose that stopped safely at a collision boundary.',
    },
    pose_indeterminate: {
      ja: '経路判定不能で停止した現在の表示姿勢だけを記録します。',
      en: 'Records only the current displayed pose that stopped because the path was indeterminate.',
    },
    pose_ready: {
      ja: '現在3Dに安全に表示されている姿勢を記録します。',
      en: 'Records the pose currently shown safely in 3D.',
    },
  } satisfies Readonly<Record<
    InstructionCaptureStatus,
    Readonly<{ ja: string; en: string }>
  >>
  for (const [status, golden] of Object.entries(captureGoldens) as Array<
    [InstructionCaptureStatus, { ja: string; en: string }]
  >) {
    assert.equal(instructionCaptureStatusText(status, 'ja'), golden.ja)
    assert.equal(instructionCaptureStatusText(status, 'en'), golden.en)
  }

  const editorGoldens = {
    invalid_metadata: {
      ja: 'タイトルは必須・改行なし120文字以内、表示時間は100〜600000msです。',
      en: 'The title is required, must be one line, and must be at most 120 characters. Display time must be 100–600000 ms.',
    },
    update_failed: {
      ja: '手順の説明を更新できませんでした',
      en: 'Could not update the step details',
    },
  } satisfies Readonly<Record<
    InstructionEditorError,
    Readonly<{ ja: string; en: string }>
  >>
  for (const [error, golden] of Object.entries(editorGoldens) as Array<
    [InstructionEditorError, { ja: string; en: string }]
  >) {
    assert.equal(instructionEditorErrorText(error, 'ja'), golden.ja)
    assert.equal(instructionEditorErrorText(error, 'en'), golden.en)
  }
})

test('goldens duration boundaries and non-finite inputs without normalizing them', () => {
  const durationGoldens = [
    { durationMs: Number.NaN, ja: 'NaN:NaN', en: 'NaN:NaN' },
    { durationMs: -1, ja: '0秒', en: '0 seconds' },
    { durationMs: Number.NEGATIVE_INFINITY, ja: '0秒', en: '0 seconds' },
    { durationMs: Number.POSITIVE_INFINITY, ja: 'Infinity:NaN', en: 'Infinity:NaN' },
    { durationMs: 0, ja: '0秒', en: '0 seconds' },
    { durationMs: 99, ja: '0.1秒', en: '0.1 seconds' },
    { durationMs: 100, ja: '0.1秒', en: '0.1 seconds' },
    { durationMs: 1_500, ja: '1.5秒', en: '1.5 seconds' },
    { durationMs: 59_949, ja: '59.9秒', en: '59.9 seconds' },
    { durationMs: 59_950, ja: '60秒', en: '60 seconds' },
    { durationMs: 59_999, ja: '60秒', en: '60 seconds' },
    { durationMs: 60_000, ja: '1:00', en: '1:00' },
    { durationMs: 90_000, ja: '1:30', en: '1:30' },
  ] as const
  for (const golden of durationGoldens) {
    assert.equal(formatInstructionDuration(golden.durationMs, 'ja'), golden.ja)
    assert.equal(formatInstructionDuration(golden.durationMs, 'en'), golden.en)
  }
})

test('preserves every forged presentation discriminant boundary', () => {
  const unknownPlayback = {
    status: 'forged_playback_status',
    sequence: 0,
  } as unknown as InstructionPlaybackState
  assert.equal(instructionPlaybackStatusText(unknownPlayback), undefined)

  const unknownStopReason = {
    status: 'stopped',
    sequence: 1,
    reason: 'SECRET_FORGED_STOP_REASON',
    stepId: 'SECRET_FORGED_STEP_ID',
  } as unknown as InstructionPlaybackState
  assert.equal(instructionPlaybackStatusText(unknownStopReason), undefined)

  const unknownNotice = {
    kind: 'SECRET_FORGED_NOTICE',
    title: 'SECRET_FORGED_TITLE',
  } as unknown as InstructionTimelineNotice
  assert.equal(instructionTimelineNoticeText(unknownNotice), undefined)

  assert.throws(
    () => instructionCaptureStatusText(
      'SECRET_FORGED_CAPTURE' as InstructionCaptureStatus,
    ),
    TypeError,
  )

  const unknownEditor = 'SECRET_FORGED_EDITOR' as InstructionEditorError
  assert.equal(
    instructionEditorErrorText(unknownEditor),
    'タイトルは必須・改行なし120文字以内、表示時間は100〜600000msです。',
  )
  assert.equal(
    instructionEditorErrorText(unknownEditor, 'en'),
    'The title is required, must be one line, and must be at most 120 characters. Display time must be 100–600000 ms.',
  )
})

test('falls back to Japanese for string, symbol, and throwing-proxy locales', () => {
  const rawTrapError = new Error('RAW_HOSTILE_LOCALE_TRAP')
  const throwingProxy = new Proxy(Object.create(null) as object, {
    get() {
      throw rawTrapError
    },
    getOwnPropertyDescriptor() {
      throw rawTrapError
    },
    getPrototypeOf() {
      throw rawTrapError
    },
    has() {
      throw rawTrapError
    },
    isExtensible() {
      throw rawTrapError
    },
    ownKeys() {
      throw rawTrapError
    },
  })
  const hostileLocales: readonly unknown[] = [
    'fr',
    Symbol('hostile-locale'),
    throwingProxy,
  ]
  for (const locale of hostileLocales) {
    assert.equal(
      instructionPlaybackStatusText(createInstructionPlaybackState(), locale as never),
      '再生停止中',
    )
    assert.equal(
      instructionTimelineNoticeText({
        kind: 'added',
        title: '  <b>{title}</b>  ',
      }, locale as never),
      '「  <b>{title}</b>  」を追加しました',
    )
    assert.equal(
      instructionCaptureStatusText('pose_ready', locale as never),
      '現在3Dに安全に表示されている姿勢を記録します。',
    )
    assert.equal(
      instructionEditorErrorText('invalid_metadata', locale as never),
      'タイトルは必須・改行なし120文字以内、表示時間は100〜600000msです。',
    )
    assert.equal(
      formatInstructionDuration(1_500, locale as never),
      '1.5秒',
    )
  }
})

test('duration formatting delegates its number locale to localized text', () => {
  const source = readFileSync(
    new URL('../src/lib/instructionTimeline.ts', import.meta.url),
    'utf8',
  )
  if (/INSTRUCTION_TIMELINE_PRESENTATION_TEXT\s+as\s+TEXT/u.test(source)) {
    assert.match(source, /TEXT\.duration\.numberLocale/u)
    assert.doesNotMatch(source, /\bDURATION_NUMBER_LOCALE\b/u)
  } else {
    assert.match(source, /\bDURATION_NUMBER_LOCALE\b/u)
  }
  assert.doesNotMatch(
    source,
    /locale\s*[!=]==?\s*['"](?:ja|en)['"]/u,
  )
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
      hand_guides: [],
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
