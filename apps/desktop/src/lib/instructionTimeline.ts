import type {
  InstructionHingeAngle,
  InstructionPose,
  InstructionStep,
  InstructionTimeline,
  InstructionVisual,
} from './coreClient'
import type { FoldPreviewAppliedPoseSnapshot } from './foldPreviewAppliedPose'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

export const INSTRUCTION_POSE_MODEL = 'absolute_hinge_angles_v1' as const
export const DECLARATIVE_INSTRUCTION_POSE_MODEL = 'declarative_only_v1' as const
export const MAX_INSTRUCTION_STEPS = 512
export const MAX_INSTRUCTION_HINGES_PER_STEP = 10_000
export const MAX_INSTRUCTION_TOTAL_HINGES = 100_000
export const MAX_INSTRUCTION_TITLE_CHARACTERS = 120
export const MAX_INSTRUCTION_DESCRIPTION_CHARACTERS = 4_000
export const MAX_INSTRUCTION_CAUTION_CHARACTERS = 2_000
export const MIN_INSTRUCTION_DURATION_MS = 100
export const MAX_INSTRUCTION_DURATION_MS = 600_000
export const DEFAULT_INSTRUCTION_DURATION_MS = 1_500
export const INSTRUCTION_APPLICATION_TIMEOUT_MS = 30_000

const LOWER_HEX_SHA256 = /^[0-9a-f]{64}$/u

export type InstructionStepPresentation = Readonly<{
  index: number
  id: string
  title: string
  description: string
  caution: string
  durationMs: number
  visual: InstructionVisual
  pose: InstructionPose
  stale: boolean
  declarativeOnly: boolean
}>

export type InstructionTimelinePresentation =
  | Readonly<{
      kind: 'invalid'
      reason: 'invalid_fingerprint' | 'invalid_timeline'
    }>
  | Readonly<{
      kind: 'ready'
      currentFingerprint: string
      steps: readonly InstructionStepPresentation[]
      stepsById: Readonly<{
        get(id: string): InstructionStepPresentation | undefined
        has(id: string): boolean
      }>
      totalDurationMs: number
    }>

export type InstructionMetadataDraft = Readonly<{
  title: string
  description: string
  caution: string
  durationMs: number
}>

export type InstructionPoseDraft = Readonly<{
  fixedFace: string | null
  hingeAngles: readonly InstructionHingeAngle[]
}>

export type InstructionPlaybackStopReason =
  | 'stale_step'
  | 'project_changed'
  | 'revision_changed'
  | 'model_changed'
  | 'manual_pose'
  | 'benchmark'
  | 'file_operation'
  | 'apply_failed'
  | 'hidden'
  | 'disposed'
  | 'canceled'

export type InstructionPlaybackPlan = Readonly<{
  projectId: string
  revision: number
  modelFingerprint: string
  steps: readonly InstructionStepPresentation[]
}>

type InstructionPlaybackBase = Readonly<{
  sequence: number
}>

export type InstructionPlaybackState =
  | (InstructionPlaybackBase & Readonly<{ status: 'idle' }>)
  | (InstructionPlaybackBase & Readonly<{
      status: 'applying'
      plan: InstructionPlaybackPlan
      cursor: number
      target: InstructionStepPresentation
    }>)
  | (InstructionPlaybackBase & Readonly<{
      status: 'holding'
      plan: InstructionPlaybackPlan
      cursor: number
      target: InstructionStepPresentation
      holdUntil: number
    }>)
  | (InstructionPlaybackBase & Readonly<{
      status: 'complete'
      lastStepId: string
    }>)
  | (InstructionPlaybackBase & Readonly<{
      status: 'stopped'
      reason: InstructionPlaybackStopReason
      stepId: string | null
    }>)

export type InstructionPlaybackEvent =
  | Readonly<{
      kind: 'start'
      plan: InstructionPlaybackPlan
      startIndex: number
    }>
  | Readonly<{
      kind: 'pose_applied'
      stepId: string
      now: number
      animated?: boolean
    }>
  | Readonly<{
      kind: 'tick'
      now: number
    }>
  | Readonly<{
      kind: 'cancel'
      reason: InstructionPlaybackStopReason
    }>
  | Readonly<{
      kind: 'apply_failed'
    }>
  | Readonly<{
      kind: 'reset'
    }>

export type InstructionTimelineNotice =
  | Readonly<{ kind: 'playback'; state: InstructionPlaybackState }>
  | Readonly<{ kind: 'add_failed' }>
  | Readonly<{ kind: 'added'; title: string }>
  | Readonly<{ kind: 'updated'; title: string }>
  | Readonly<{ kind: 'update_failed' }>
  | Readonly<{ kind: 'pose_updated'; title: string }>
  | Readonly<{ kind: 'pose_update_failed' }>
  | Readonly<{ kind: 'delete_failed' }>
  | Readonly<{ kind: 'deleted'; title: string }>
  | Readonly<{ kind: 'moved' }>
  | Readonly<{ kind: 'move_failed' }>
  | Readonly<{ kind: 'stale_pose' }>
  | Readonly<{ kind: 'pose_apply_failed' }>
  | Readonly<{ kind: 'pose_applying'; title: string }>
  | Readonly<{ kind: 'model_required' }>
  | Readonly<{ kind: 'no_steps' }>
  | Readonly<{ kind: 'declarative_playback_unsupported' }>

export type InstructionCaptureStatus =
  | 'project_required'
  | 'pose_required'
  | 'pose_running'
  | 'pose_invalid'
  | 'pose_blocked'
  | 'pose_indeterminate'
  | 'pose_ready'

export type InstructionEditorError =
  | 'invalid_metadata'
  | 'update_failed'

export function createInstructionTimelinePresentation(
  value: unknown,
  currentFingerprint: unknown,
): InstructionTimelinePresentation {
  if (!validFingerprint(currentFingerprint)) {
    return Object.freeze({ kind: 'invalid', reason: 'invalid_fingerprint' })
  }
  try {
    const timeline = parseTimeline(value, currentFingerprint)
    if (!timeline) {
      return Object.freeze({ kind: 'invalid', reason: 'invalid_timeline' })
    }
    return timeline
  } catch {
    return Object.freeze({ kind: 'invalid', reason: 'invalid_timeline' })
  }
}

export function validateInstructionMetadata(
  value: InstructionMetadataDraft,
): InstructionMetadataDraft | null {
  if (
    !validTitle(value.title)
    || !validText(value.description, MAX_INSTRUCTION_DESCRIPTION_CHARACTERS)
    || !validText(value.caution, MAX_INSTRUCTION_CAUTION_CHARACTERS)
    || !validDuration(value.durationMs)
  ) return null
  return Object.freeze({
    title: value.title.trim(),
    description: value.description,
    caution: value.caution,
    durationMs: value.durationMs,
  })
}

export function createInstructionPoseDraft(
  applied: FoldPreviewAppliedPoseSnapshot | null,
  currentFingerprint: string,
): InstructionPoseDraft | null {
  if (
    !applied
    || applied.state === 'running'
    || !validFingerprint(currentFingerprint)
    || applied.hingeAngles.length > MAX_INSTRUCTION_HINGES_PER_STEP
  ) return null
  const hingeAngles = parseHingeAngles(applied.hingeAngles.map((angle) => ({
    edge: angle.edgeId,
    angle_degrees: angle.angleDegrees,
  })).sort((left, right) => compareCanonicalIdentity(left.edge, right.edge)))
  if (!hingeAngles) return null
  if (
    (applied.fixedFaceId === null && hingeAngles.length !== 0)
    || (applied.fixedFaceId !== null && hingeAngles.length === 0)
  ) return null
  return Object.freeze({
    fixedFace: applied.fixedFaceId,
    hingeAngles,
  })
}

export function createInstructionPlaybackPlan(
  projectId: string,
  revision: number,
  presentation: InstructionTimelinePresentation,
): InstructionPlaybackPlan | null {
  if (
    presentation.kind !== 'ready'
    || !validIdentity(projectId)
    || !validRevision(revision)
  ) return null
  const executableSteps = presentation.steps.filter(
    (step) => !step.declarativeOnly,
  )
  if (executableSteps.length === 0) return null
  return Object.freeze({
    projectId,
    revision,
    modelFingerprint: presentation.currentFingerprint,
    steps: Object.freeze(executableSteps),
  })
}

export function createInstructionPlaybackState(): InstructionPlaybackState {
  return Object.freeze({ status: 'idle', sequence: 0 })
}

export function reduceInstructionPlayback(
  state: InstructionPlaybackState,
  event: InstructionPlaybackEvent,
): InstructionPlaybackState {
  if (event.kind === 'reset') {
    return Object.freeze({ status: 'idle', sequence: state.sequence + 1 })
  }
  if (event.kind === 'cancel') {
    if (state.status === 'idle') return state
    return stopped(state, event.reason)
  }
  if (event.kind === 'apply_failed') {
    return state.status === 'applying'
      ? stopped(state, 'apply_failed')
      : state
  }
  if (event.kind === 'start') {
    const target = event.plan.steps[event.startIndex]
    if (
      !validPlaybackPlan(event.plan)
      || !Number.isSafeInteger(event.startIndex)
      || event.startIndex < 0
      || !target
    ) return stopped(state, 'apply_failed')
    if (target.stale) {
      return Object.freeze({
        status: 'stopped',
        sequence: state.sequence + 1,
        reason: 'stale_step',
        stepId: target.id,
      })
    }
    return Object.freeze({
      status: 'applying',
      sequence: state.sequence + 1,
      plan: event.plan,
      cursor: event.startIndex,
      target,
    })
  }
  if (event.kind === 'pose_applied') {
    if (
      state.status !== 'applying'
      || event.stepId !== state.target.id
      || !validClock(event.now)
    ) return state
    return Object.freeze({
      status: 'holding',
      sequence: state.sequence,
      plan: state.plan,
      cursor: state.cursor,
      target: state.target,
      holdUntil: event.now + (event.animated ? 0 : state.target.durationMs),
    })
  }
  if (state.status !== 'holding' || !validClock(event.now)) return state
  if (event.now < state.holdUntil) return state
  const nextCursor = state.cursor + 1
  const target = state.plan.steps[nextCursor]
  if (!target) {
    return Object.freeze({
      status: 'complete',
      sequence: state.sequence,
      lastStepId: state.target.id,
    })
  }
  if (target.stale) {
    return Object.freeze({
      status: 'stopped',
      sequence: state.sequence,
      reason: 'stale_step',
      stepId: target.id,
    })
  }
  return Object.freeze({
    status: 'applying',
    sequence: state.sequence,
    plan: state.plan,
    cursor: nextCursor,
    target,
  })
}

export function createInstructionInterpolatedStep(
  target: InstructionStepPresentation,
  start: FoldPreviewAppliedPoseSnapshot | null,
  progress: number,
): InstructionStepPresentation | null {
  if (
    target.declarativeOnly
    || target.stale
    || !start
    || start.state === 'running'
    || start.fixedFaceId !== target.pose.fixed_face
    || !Number.isFinite(progress)
    || progress < 0
    || progress > 1
    || start.hingeAngles.length !== target.pose.hinge_angles.length
  ) return null
  const startByEdge = new Map<string, number>()
  for (const angle of start.hingeAngles) {
    if (
      startByEdge.has(angle.edgeId)
      || !Number.isFinite(angle.angleDegrees)
    ) return null
    startByEdge.set(angle.edgeId, angle.angleDegrees)
  }
  const hingeAngles = target.pose.hinge_angles.map((angle) => {
    const startAngle = startByEdge.get(angle.edge)
    if (startAngle === undefined) return null
    return Object.freeze({
      edge: angle.edge,
      angle_degrees: normalizeZero(
        startAngle + (angle.angle_degrees - startAngle) * progress,
      ),
    })
  })
  if (hingeAngles.some((angle) => angle === null)) return null
  return Object.freeze({
    ...target,
    pose: Object.freeze({
      ...target.pose,
      hinge_angles: Object.freeze(hingeAngles) as readonly InstructionHingeAngle[],
    }),
  })
}

export function instructionPoseMatchesApplied(
  pose: InstructionPose,
  applied: FoldPreviewAppliedPoseSnapshot | null,
): boolean {
  if (
    pose.model !== INSTRUCTION_POSE_MODEL
    || !applied
    || applied.state === 'running'
    || pose.fixed_face !== applied.fixedFaceId
    || pose.hinge_angles.length !== applied.hingeAngles.length
  ) return false
  const actualByEdge = new Map<string, number>()
  for (const angle of applied.hingeAngles) {
    if (actualByEdge.has(angle.edgeId)) return false
    actualByEdge.set(angle.edgeId, normalizeZero(angle.angleDegrees))
  }
  return pose.hinge_angles.every(({ edge, angle_degrees }) =>
    Object.is(
      normalizeZero(angle_degrees),
      actualByEdge.get(edge),
  ))
}

export function resolveInstructionPoseApplicationObservation(
  pose: InstructionPose,
  observationAtApply: FoldPreviewAppliedPoseSnapshot | null,
  currentObservation: FoldPreviewAppliedPoseSnapshot | null,
): 'acknowledge' | 'wait' | 'fail' {
  if (instructionPoseMatchesApplied(pose, currentObservation)) return 'acknowledge'
  if (
    currentObservation === null
    || currentObservation === observationAtApply
    || currentObservation.state === 'running'
  ) return 'wait'
  if (
    currentObservation.state === 'blocked'
    || currentObservation.state === 'indeterminate'
  ) return 'fail'
  // Tree-pose application is committed on the next animation frame. During
  // that hand-off FoldPreview can legitimately publish a freshly detached
  // `stable` object for the still-rendered pre-apply endpoint. Compare the
  // endpoint values instead of treating object identity as an acknowledgement.
  if (appliedPoseEndpointsMatch(observationAtApply, currentObservation)) {
    return 'wait'
  }
  return 'fail'
}

function appliedPoseEndpointsMatch(
  left: FoldPreviewAppliedPoseSnapshot | null,
  right: FoldPreviewAppliedPoseSnapshot,
) {
  if (
    left === null
    || left.projectId !== right.projectId
    || left.revision !== right.revision
    || left.fixedFaceId !== right.fixedFaceId
    || left.hingeAngles.length !== right.hingeAngles.length
  ) return false
  const rightByEdge = new Map<string, number>()
  for (const angle of right.hingeAngles) {
    if (rightByEdge.has(angle.edgeId)) return false
    rightByEdge.set(angle.edgeId, normalizeZero(angle.angleDegrees))
  }
  return left.hingeAngles.every(({ edgeId, angleDegrees }) =>
    Object.is(
      normalizeZero(angleDegrees),
      rightByEdge.get(edgeId),
    ))
}

export function instructionPlaybackStatusText(
  state: InstructionPlaybackState,
  locale: Locale = 'ja',
): string {
  switch (state.status) {
    case 'idle':
      return selectLocalizedText(locale, PLAYBACK_IDLE_TEXT)
    case 'applying':
      return formatLocalizedText(locale, PLAYBACK_APPLYING_TEXT, {
        step: state.target.index + 1,
        title: state.target.title,
      })
    case 'holding':
      return formatLocalizedText(locale, PLAYBACK_HOLDING_TEXT, {
        step: state.target.index + 1,
        title: state.target.title,
      })
    case 'complete':
      return selectLocalizedText(locale, PLAYBACK_COMPLETE_TEXT)
    case 'stopped':
      return playbackStopText(state.reason, locale)
  }
}

export function instructionTimelineNoticeText(
  notice: InstructionTimelineNotice,
  locale: Locale = 'ja',
): string {
  switch (notice.kind) {
    case 'playback':
      return instructionPlaybackStatusText(notice.state, locale)
    case 'add_failed':
      return selectLocalizedText(locale, NOTICE_ADD_FAILED)
    case 'added':
      return formatLocalizedText(locale, NOTICE_ADDED, { title: notice.title })
    case 'updated':
      return formatLocalizedText(locale, NOTICE_UPDATED, { title: notice.title })
    case 'update_failed':
      return selectLocalizedText(locale, NOTICE_UPDATE_FAILED)
    case 'pose_updated':
      return formatLocalizedText(locale, NOTICE_POSE_UPDATED, {
        title: notice.title,
      })
    case 'pose_update_failed':
      return selectLocalizedText(locale, NOTICE_POSE_UPDATE_FAILED)
    case 'delete_failed':
      return selectLocalizedText(locale, NOTICE_DELETE_FAILED)
    case 'deleted':
      return formatLocalizedText(locale, NOTICE_DELETED, { title: notice.title })
    case 'moved':
      return selectLocalizedText(locale, NOTICE_MOVED)
    case 'move_failed':
      return selectLocalizedText(locale, NOTICE_MOVE_FAILED)
    case 'stale_pose':
      return selectLocalizedText(locale, NOTICE_STALE_POSE)
    case 'pose_apply_failed':
      return selectLocalizedText(locale, NOTICE_POSE_APPLY_FAILED)
    case 'pose_applying':
      return formatLocalizedText(locale, NOTICE_POSE_APPLYING, {
        title: notice.title,
      })
    case 'model_required':
      return selectLocalizedText(locale, NOTICE_MODEL_REQUIRED)
    case 'no_steps':
      return selectLocalizedText(locale, NOTICE_NO_STEPS)
    case 'declarative_playback_unsupported':
      return selectLocalizedText(
        locale,
        NOTICE_DECLARATIVE_PLAYBACK_UNSUPPORTED,
      )
  }
}

export function instructionCaptureStatusText(
  status: InstructionCaptureStatus,
  locale: Locale = 'ja',
): string {
  return selectLocalizedText(locale, CAPTURE_STATUS_TEXT[status])
}

export function instructionEditorErrorText(
  error: InstructionEditorError,
  locale: Locale = 'ja',
): string {
  if (error === 'update_failed') {
    return selectLocalizedText(locale, EDITOR_UPDATE_FAILED)
  }
  return formatLocalizedText(locale, EDITOR_INVALID_METADATA, {
    titleMaximum: MAX_INSTRUCTION_TITLE_CHARACTERS,
    durationMinimum: MIN_INSTRUCTION_DURATION_MS,
    durationMaximum: MAX_INSTRUCTION_DURATION_MS,
  })
}

export function formatInstructionDuration(
  durationMs: number,
  locale: Locale = 'ja',
): string {
  const totalSeconds = Math.max(0, durationMs) / 1_000
  if (totalSeconds < 60) {
    const formatted = totalSeconds.toLocaleString(
      locale === 'en' ? 'en-US' : 'ja-JP',
      { maximumFractionDigits: 1 },
    )
    return formatLocalizedText(locale, DURATION_SECONDS, { seconds: formatted })
  }
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = Math.floor(totalSeconds % 60)
  return `${minutes}:${String(seconds).padStart(2, '0')}`
}

const PLAYBACK_IDLE_TEXT = localized('再生停止中', 'Playback stopped')
const PLAYBACK_APPLYING_TEXT = localized(
  '手順 {step}「{title}」を表示しています',
  'Applying step {step}, “{title}”',
)
const PLAYBACK_HOLDING_TEXT = localized(
  '手順 {step}「{title}」を表示中です',
  'Showing step {step}, “{title}”',
)
const PLAYBACK_COMPLETE_TEXT = localized(
  '折り手順の段階再生が完了しました',
  'Finished playing all folding steps',
)
const NOTICE_ADD_FAILED = localized(
  '現在の3D姿勢を手順へ追加できませんでした',
  'Could not add the current 3D pose as a step',
)
const NOTICE_ADDED = localized(
  '「{title}」を追加しました',
  'Added “{title}”',
)
const NOTICE_UPDATED = localized(
  '「{title}」を更新しました',
  'Updated “{title}”',
)
const NOTICE_UPDATE_FAILED = localized(
  '手順を更新できませんでした',
  'Could not update the step',
)
const NOTICE_POSE_UPDATED = localized(
  '「{title}」の姿勢を現在の3D表示で更新しました',
  'Updated the pose for “{title}” from the current 3D view',
)
const NOTICE_POSE_UPDATE_FAILED = localized(
  '手順の姿勢を更新できませんでした',
  'Could not update the step pose',
)
const NOTICE_DELETE_FAILED = localized(
  '手順を削除できませんでした',
  'Could not delete the step',
)
const NOTICE_DELETED = localized(
  '「{title}」を削除しました',
  'Deleted “{title}”',
)
const NOTICE_MOVED = localized(
  '手順の順番を変更しました',
  'Changed the step order',
)
const NOTICE_MOVE_FAILED = localized(
  '手順を移動できませんでした',
  'Could not move the step',
)
const NOTICE_STALE_POSE = localized(
  '展開図が変更された手順です。「現在の3D姿勢で更新」してから表示してください',
  'The crease pattern changed for this step. Update it with the current 3D pose before showing it.',
)
const NOTICE_POSE_APPLY_FAILED = localized(
  'この手順の姿勢は現在の3Dモデルへ適用できません',
  'This step pose cannot be applied to the current 3D model',
)
const NOTICE_POSE_APPLYING = localized(
  '「{title}」の保存姿勢を3Dへ適用しています',
  'Applying the saved pose for “{title}” to the 3D view',
)
const NOTICE_MODEL_REQUIRED = localized(
  '再生できる3Dモデルを準備してください',
  'Prepare a 3D model that can be played',
)
const NOTICE_NO_STEPS = localized(
  '再生する手順がありません',
  'There are no steps to play',
)
const NOTICE_DECLARATIVE_PLAYBACK_UNSUPPORTED = localized(
  '説明専用ステップは3D姿勢を持たないため再生できません。内容は一覧で確認してください',
  'Description-only steps have no 3D pose and cannot be played. Review them in the timeline list.',
)
const EDITOR_INVALID_METADATA = localized(
  'タイトルは必須・改行なし{titleMaximum}文字以内、表示時間は{durationMinimum}〜{durationMaximum}msです。',
  'The title is required, must be one line, and must be at most {titleMaximum} characters. Display time must be {durationMinimum}–{durationMaximum} ms.',
)
const EDITOR_UPDATE_FAILED = localized(
  '手順の説明を更新できませんでした',
  'Could not update the step details',
)
const CAPTURE_STATUS_TEXT = Object.freeze({
  project_required: localized(
    'プロジェクトを読み込んでください。',
    'Open a project first.',
  ),
  pose_required: localized(
    '現在のrevisionの3D表示を準備しています。',
    'Preparing the 3D view for the current revision.',
  ),
  pose_running: localized(
    '3Dの動作が止まってから記録できます。',
    'Wait for the 3D motion to stop before recording.',
  ),
  pose_invalid: localized(
    '現在の3D姿勢は手順として安全に読み取れません。',
    'The current 3D pose cannot be read safely as a step.',
  ),
  pose_blocked: localized(
    '衝突境界で安全に停止している表示姿勢を記録します。',
    'Records the displayed pose that stopped safely at a collision boundary.',
  ),
  pose_indeterminate: localized(
    '経路判定不能で停止した現在の表示姿勢だけを記録します。',
    'Records only the current displayed pose that stopped because the path was indeterminate.',
  ),
  pose_ready: localized(
    '現在3Dに安全に表示されている姿勢を記録します。',
    'Records the pose currently shown safely in 3D.',
  ),
}) satisfies Readonly<Record<InstructionCaptureStatus, LocalizedText>>
const DURATION_SECONDS = localized('{seconds}秒', '{seconds} seconds')

function parseTimeline(
  value: unknown,
  currentFingerprint: string,
): Extract<InstructionTimelinePresentation, { kind: 'ready' }> | null {
  if (!isRecord(value) || !hasExactKeys(value, ['steps']) || !Array.isArray(value.steps)) {
    return null
  }
  if (value.steps.length > MAX_INSTRUCTION_STEPS) return null
  const stepIds = new Set<string>()
  const steps: InstructionStepPresentation[] = []
  let totalHinges = 0
  let totalDurationMs = 0
  for (let index = 0; index < value.steps.length; index += 1) {
    const parsed = parseStep(value.steps[index], index, currentFingerprint)
    if (!parsed || stepIds.has(parsed.id)) return null
    stepIds.add(parsed.id)
    totalHinges += parsed.pose.hinge_angles.length
    if (totalHinges > MAX_INSTRUCTION_TOTAL_HINGES) return null
    totalDurationMs += parsed.durationMs
    steps.push(parsed)
  }
  const stepIndex = new Map(steps.map((step) => [step.id, step]))
  const stepsById = Object.freeze({
    get: (id: string) => stepIndex.get(id),
    has: (id: string) => stepIndex.has(id),
  })
  return Object.freeze({
    kind: 'ready',
    currentFingerprint,
    steps: Object.freeze(steps),
    stepsById,
    totalDurationMs,
  })
}

function parseStep(
  value: unknown,
  index: number,
  currentFingerprint: string,
): InstructionStepPresentation | null {
  if (
    !isRecord(value)
    || !hasExactKeys(value, [
      'id',
      'title',
      'description',
      'caution',
      'duration_ms',
      'visual',
      'pose',
    ])
    || !validIdentity(value.id)
    || !validTitle(value.title)
    || !validText(value.description, MAX_INSTRUCTION_DESCRIPTION_CHARACTERS)
    || !validText(value.caution, MAX_INSTRUCTION_CAUTION_CHARACTERS)
    || !validDuration(value.duration_ms)
  ) return null
  const pose = parsePose(value.pose)
  const visual = parseInstructionVisual(value.visual)
  if (!pose || !visual) return null
  return Object.freeze({
    index,
    id: value.id,
    title: value.title,
    description: value.description,
    caution: value.caution,
    durationMs: value.duration_ms,
    visual,
    pose,
    stale: pose.model === INSTRUCTION_POSE_MODEL
      && pose.source_model_fingerprint !== currentFingerprint,
    declarativeOnly: pose.model === DECLARATIVE_INSTRUCTION_POSE_MODEL,
  })
}

export function parseInstructionVisual(value: unknown): InstructionVisual | null {
  if (
    !isRecord(value)
    || !hasRequiredAndOptionalKeys(
      value,
      ['camera', 'arrows', 'focus_points', 'hand_guides'],
      ['cycle_layer_order_proof_v1', 'path_certificate_reference_v1'],
    )
    || !(value.camera === null || isCamera(value.camera))
    || !Array.isArray(value.arrows)
    || !Array.isArray(value.focus_points)
    || !Array.isArray(value.hand_guides)
    || value.arrows.length + value.focus_points.length + value.hand_guides.length > 64
  ) return null
  const pathCertificateReference = parsePathCertificateReference(
    value.path_certificate_reference_v1,
  )
  if (pathCertificateReference === false) return null
  const arrows = value.arrows.map((arrow) => {
    if (
      !isRecord(arrow)
      || !hasExactKeys(arrow, ['start', 'end', 'label'])
      || !isPoint3(arrow.start)
      || !isPoint3(arrow.end)
      || samePoint3(arrow.start, arrow.end)
      || !validMarkerLabel(arrow.label)
    ) return null
    return Object.freeze({ start: arrow.start, end: arrow.end, label: arrow.label })
  })
  const focusPoints = value.focus_points.map((focus) => {
    if (
      !isRecord(focus)
      || !hasExactKeys(focus, ['position', 'radius', 'label'])
      || !isPoint3(focus.position)
      || typeof focus.radius !== 'number'
      || !Number.isFinite(focus.radius)
      || focus.radius <= 0
      || !validMarkerLabel(focus.label)
    ) return null
    return Object.freeze({
      position: focus.position,
      radius: focus.radius,
      label: focus.label,
    })
  })
  const handGuides = value.hand_guides.map((guide) => {
    if (
      !isRecord(guide)
      || !hasExactKeys(guide, ['kind', 'position', 'direction', 'label'])
      || !['pinch', 'hold', 'push', 'regrip'].includes(String(guide.kind))
      || !isPoint3(guide.position)
      || !isPoint3(guide.direction)
      || samePoint3(guide.direction, { x: 0, y: 0, z: 0 })
      || !validMarkerLabel(guide.label)
    ) return null
    return Object.freeze({
      kind: guide.kind as 'pinch' | 'hold' | 'push' | 'regrip',
      position: guide.position,
      direction: guide.direction,
      label: guide.label,
    })
  })
  if (
    arrows.some((arrow) => arrow === null)
    || focusPoints.some((focus) => focus === null)
    || handGuides.some((guide) => guide === null)
  ) {
    return null
  }
  return Object.freeze({
    camera: value.camera,
    arrows: Object.freeze(arrows) as InstructionVisual['arrows'],
    focus_points: Object.freeze(focusPoints) as InstructionVisual['focus_points'],
    hand_guides: Object.freeze(handGuides) as InstructionVisual['hand_guides'],
    cycle_layer_order_proof_v1:
      value.cycle_layer_order_proof_v1 as InstructionVisual['cycle_layer_order_proof_v1'],
    path_certificate_reference_v1: pathCertificateReference,
  })
}

function parsePathCertificateReference(
  value: unknown,
): InstructionVisual['path_certificate_reference_v1'] | false {
  if (value === undefined || value === null) return value
  if (!isRecord(value) || !hasExactKeys(value, [
    'version',
    'model_id',
    'binding_sha256',
    'source_pose_sha256',
    'target_pose_sha256',
    'source_model_binding_sha256',
    'transition_count',
  ])) return false
  const byteArray = (candidate: unknown) => Array.isArray(candidate)
    && candidate.length === 32
    && candidate.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
  if (
    value.version !== 1
    || value.model_id !== 'bounded_certified_pose_graph_path_reference_v1'
    || !byteArray(value.binding_sha256)
    || !byteArray(value.source_pose_sha256)
    || !byteArray(value.target_pose_sha256)
    || !byteArray(value.source_model_binding_sha256)
    || !(value.binding_sha256 as number[]).some((byte) => byte !== 0)
    || !(value.source_model_binding_sha256 as number[]).some((byte) => byte !== 0)
    || JSON.stringify(value.source_pose_sha256) === JSON.stringify(value.target_pose_sha256)
    || !Number.isSafeInteger(value.transition_count)
    || Number(value.transition_count) < 1
    || Number(value.transition_count) > 64
  ) return false
  return Object.freeze({
    version: 1,
    model_id: 'bounded_certified_pose_graph_path_reference_v1',
    binding_sha256: Object.freeze([...(value.binding_sha256 as number[])]),
    source_pose_sha256: Object.freeze([...(value.source_pose_sha256 as number[])]),
    target_pose_sha256: Object.freeze([...(value.target_pose_sha256 as number[])]),
    source_model_binding_sha256:
      Object.freeze([...(value.source_model_binding_sha256 as number[])]),
    transition_count: Number(value.transition_count),
  })
}

function isCamera(value: unknown): value is NonNullable<InstructionVisual['camera']> {
  return isRecord(value)
    && hasExactKeys(value, ['position', 'target', 'up'])
    && isPoint3(value.position)
    && isPoint3(value.target)
    && isPoint3(value.up)
    && !samePoint3(value.position, value.target)
    && !samePoint3(value.up, { x: 0, y: 0, z: 0 })
}

function isPoint3(value: unknown): value is { x: number; y: number; z: number } {
  return isRecord(value)
    && hasExactKeys(value, ['x', 'y', 'z'])
    && typeof value.x === 'number'
    && typeof value.y === 'number'
    && typeof value.z === 'number'
    && Number.isFinite(value.x)
    && Number.isFinite(value.y)
    && Number.isFinite(value.z)
}

function samePoint3(
  left: { x: number; y: number; z: number },
  right: { x: number; y: number; z: number },
) {
  return left.x === right.x && left.y === right.y && left.z === right.z
}

function validMarkerLabel(value: unknown): value is string {
  return typeof value === 'string'
    && [...value].length <= 120
    && ![...value].some((character) => /\p{Cc}/u.test(character))
}

function parsePose(value: unknown): InstructionPose | null {
  if (
    !isRecord(value)
    || !hasExactKeys(value, [
      'model',
      'source_model_fingerprint',
      'fixed_face',
      'hinge_angles',
    ])
    || (
      value.model !== INSTRUCTION_POSE_MODEL
      && value.model !== DECLARATIVE_INSTRUCTION_POSE_MODEL
    )
    || !validFingerprint(value.source_model_fingerprint)
    || !(value.fixed_face === null || validIdentity(value.fixed_face))
    || !Array.isArray(value.hinge_angles)
    || value.hinge_angles.length > MAX_INSTRUCTION_HINGES_PER_STEP
  ) return null
  const hingeAngles = parseHingeAngles(value.hinge_angles)
  if (!hingeAngles) return null
  if (
    value.model === DECLARATIVE_INSTRUCTION_POSE_MODEL
    && (value.fixed_face !== null || hingeAngles.length !== 0)
  ) return null
  if (
    value.model === INSTRUCTION_POSE_MODEL
    && (
      (value.fixed_face === null && hingeAngles.length !== 0)
      || (value.fixed_face !== null && hingeAngles.length === 0)
    )
  ) return null
  return Object.freeze({
    model: value.model,
    source_model_fingerprint: value.source_model_fingerprint,
    fixed_face: value.fixed_face,
    hinge_angles: hingeAngles,
  })
}

function parseHingeAngles(value: readonly unknown[]): readonly InstructionHingeAngle[] | null {
  const edgeIds = new Set<string>()
  const result: InstructionHingeAngle[] = []
  let previousEdgeId: string | null = null
  for (const item of value) {
    if (
      !isRecord(item)
      || !hasExactKeys(item, ['edge', 'angle_degrees'])
      || !validIdentity(item.edge)
      || !validAngle(item.angle_degrees)
      || edgeIds.has(item.edge)
      || (
        previousEdgeId !== null
        && compareCanonicalIdentity(previousEdgeId, item.edge) >= 0
      )
    ) return null
    edgeIds.add(item.edge)
    previousEdgeId = item.edge
    result.push(Object.freeze({
      edge: item.edge,
      angle_degrees: normalizeZero(item.angle_degrees),
    }))
  }
  return Object.freeze(result)
}

function validPlaybackPlan(plan: InstructionPlaybackPlan) {
  return validIdentity(plan.projectId)
    && validRevision(plan.revision)
    && validFingerprint(plan.modelFingerprint)
    && plan.steps.length > 0
    && plan.steps.length <= MAX_INSTRUCTION_STEPS
    && plan.steps.every((step) =>
      !step.declarativeOnly && step.pose.model === INSTRUCTION_POSE_MODEL)
}

function stopped(
  state: InstructionPlaybackState,
  reason: InstructionPlaybackStopReason,
): InstructionPlaybackState {
  const stepId = state.status === 'applying' || state.status === 'holding'
    ? state.target.id
    : null
  return Object.freeze({
    status: 'stopped',
    sequence: state.sequence + 1,
    reason,
    stepId,
  })
}

function playbackStopText(
  reason: InstructionPlaybackStopReason,
  locale: Locale,
) {
  switch (reason) {
    case 'stale_step':
      return selectLocalizedText(locale, PLAYBACK_STOP_STALE_STEP)
    case 'project_changed':
      return selectLocalizedText(locale, PLAYBACK_STOP_PROJECT_CHANGED)
    case 'revision_changed':
      return selectLocalizedText(locale, PLAYBACK_STOP_REVISION_CHANGED)
    case 'model_changed':
      return selectLocalizedText(locale, PLAYBACK_STOP_MODEL_CHANGED)
    case 'manual_pose':
      return selectLocalizedText(locale, PLAYBACK_STOP_MANUAL_POSE)
    case 'benchmark':
      return selectLocalizedText(locale, PLAYBACK_STOP_BENCHMARK)
    case 'file_operation':
      return selectLocalizedText(locale, PLAYBACK_STOP_FILE_OPERATION)
    case 'apply_failed':
      return selectLocalizedText(locale, PLAYBACK_STOP_APPLY_FAILED)
    case 'hidden':
      return selectLocalizedText(locale, PLAYBACK_STOP_HIDDEN)
    case 'disposed':
      return selectLocalizedText(locale, PLAYBACK_STOP_DISPOSED)
    case 'canceled':
      return selectLocalizedText(locale, PLAYBACK_STOP_CANCELED)
  }
}

const PLAYBACK_STOP_STALE_STEP = localized(
  '展開図が変わった手順のため再生を停止しました',
  'Playback stopped because the crease pattern changed for this step',
)
const PLAYBACK_STOP_PROJECT_CHANGED = localized(
  'プロジェクトが変わったため再生を停止しました',
  'Playback stopped because the project changed',
)
const PLAYBACK_STOP_REVISION_CHANGED = localized(
  '編集中の内容が変わったため再生を停止しました',
  'Playback stopped because the edited content changed',
)
const PLAYBACK_STOP_MODEL_CHANGED = localized(
  '3Dモデルが変わったため再生を停止しました',
  'Playback stopped because the 3D model changed',
)
const PLAYBACK_STOP_MANUAL_POSE = localized(
  '3D姿勢を手動変更したため再生を停止しました',
  'Playback stopped because the 3D pose was changed manually',
)
const PLAYBACK_STOP_BENCHMARK = localized(
  '性能テストを開始したため再生を停止しました',
  'Playback stopped because a performance test started',
)
const PLAYBACK_STOP_FILE_OPERATION = localized(
  'ファイル操作を開始したため再生を停止しました',
  'Playback stopped because a file operation started',
)
const PLAYBACK_STOP_APPLY_FAILED = localized(
  '3D姿勢を適用できなかったため再生を停止しました',
  'Playback stopped because the 3D pose could not be applied',
)
const PLAYBACK_STOP_HIDDEN = localized(
  '画面が非表示になったため再生を停止しました',
  'Playback stopped because the window became hidden',
)
const PLAYBACK_STOP_DISPOSED = localized(
  '画面を閉じたため再生を停止しました',
  'Playback stopped because the view was closed',
)
const PLAYBACK_STOP_CANCELED = localized(
  '折り手順の再生を停止しました',
  'Folding-step playback stopped',
)

function validTitle(value: unknown): value is string {
  return typeof value === 'string'
    && value.trim().length > 0
    && characterCount(value) <= MAX_INSTRUCTION_TITLE_CHARACTERS
    && validTitleControls(value)
}

function validText(value: unknown, maximumCharacters: number): value is string {
  return typeof value === 'string'
    && characterCount(value) <= maximumCharacters
    && validTextControls(value)
}

function validTextControls(value: string) {
  for (const character of value) {
    const code = character.codePointAt(0)
    if (code === undefined) return false
    if (isControlCodePoint(code) && character !== '\n' && character !== '\t') {
      return false
    }
  }
  return true
}

function validTitleControls(value: string) {
  for (const character of value) {
    const code = character.codePointAt(0)
    if (code === undefined || isControlCodePoint(code)) return false
  }
  return true
}

function characterCount(value: string) {
  return [...value].length
}

function validDuration(value: unknown): value is number {
  return Number.isSafeInteger(value)
    && (value as number) >= MIN_INSTRUCTION_DURATION_MS
    && (value as number) <= MAX_INSTRUCTION_DURATION_MS
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validFingerprint(value: unknown): value is string {
  return typeof value === 'string' && LOWER_HEX_SHA256.test(value)
}

function validIdentity(value: unknown): value is string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 512) return false
  for (const character of value) {
    const code = character.codePointAt(0)
    if (code === undefined || isControlCodePoint(code)) return false
  }
  return true
}

function validRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validClock(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function compareCanonicalIdentity(left: string, right: string) {
  return left < right ? -1 : left > right ? 1 : 0
}

function isControlCodePoint(codePoint: number) {
  return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]) {
  const keys = Object.keys(value)
  return keys.length === expected.length
    && expected.every((key) => Object.prototype.hasOwnProperty.call(value, key))
}

function hasRequiredAndOptionalKeys(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
) {
  const keys = Object.keys(value)
  return required.every((key) => Object.prototype.hasOwnProperty.call(value, key))
    && keys.every((key) => required.includes(key) || optional.includes(key))
}

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

// Compile-time checks that the raw DTOs retain the exact persisted structure
// consumed by the validator above.
const _timelineShape: InstructionTimeline | null = null
const _stepShape: InstructionStep | null = null
void _timelineShape
void _stepShape
