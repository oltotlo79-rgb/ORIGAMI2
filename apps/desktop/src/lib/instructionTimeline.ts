import type {
  InstructionHingeAngle,
  InstructionPose,
  InstructionStep,
  InstructionTimeline,
} from './coreClient'
import type { FoldPreviewAppliedPoseSnapshot } from './foldPreviewAppliedPose'

export const INSTRUCTION_POSE_MODEL = 'absolute_hinge_angles_v1' as const
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
  pose: InstructionPose
  stale: boolean
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
    || presentation.steps.length === 0
  ) return null
  return Object.freeze({
    projectId,
    revision,
    modelFingerprint: presentation.currentFingerprint,
    steps: presentation.steps,
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
      holdUntil: event.now + state.target.durationMs,
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

export function instructionPoseMatchesApplied(
  pose: InstructionPose,
  applied: FoldPreviewAppliedPoseSnapshot | null,
): boolean {
  if (
    !applied
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
): string {
  switch (state.status) {
    case 'idle':
      return '再生停止中'
    case 'applying':
      return `手順 ${state.cursor + 1}「${state.target.title}」を表示しています`
    case 'holding':
      return `手順 ${state.cursor + 1}「${state.target.title}」を表示中です`
    case 'complete':
      return '折り手順の段階再生が完了しました'
    case 'stopped':
      return playbackStopText(state.reason)
  }
}

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
      'pose',
    ])
    || !validIdentity(value.id)
    || !validTitle(value.title)
    || !validText(value.description, MAX_INSTRUCTION_DESCRIPTION_CHARACTERS)
    || !validText(value.caution, MAX_INSTRUCTION_CAUTION_CHARACTERS)
    || !validDuration(value.duration_ms)
  ) return null
  const pose = parsePose(value.pose)
  if (!pose) return null
  return Object.freeze({
    index,
    id: value.id,
    title: value.title,
    description: value.description,
    caution: value.caution,
    durationMs: value.duration_ms,
    pose,
    stale: pose.source_model_fingerprint !== currentFingerprint,
  })
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
    || value.model !== INSTRUCTION_POSE_MODEL
    || !validFingerprint(value.source_model_fingerprint)
    || !(value.fixed_face === null || validIdentity(value.fixed_face))
    || !Array.isArray(value.hinge_angles)
    || value.hinge_angles.length > MAX_INSTRUCTION_HINGES_PER_STEP
  ) return null
  const hingeAngles = parseHingeAngles(value.hinge_angles)
  if (!hingeAngles) return null
  if (
    (value.fixed_face === null && hingeAngles.length !== 0)
    || (value.fixed_face !== null && hingeAngles.length === 0)
  ) return null
  return Object.freeze({
    model: INSTRUCTION_POSE_MODEL,
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

function playbackStopText(reason: InstructionPlaybackStopReason) {
  switch (reason) {
    case 'stale_step':
      return '展開図が変わった手順のため再生を停止しました'
    case 'project_changed':
      return 'プロジェクトが変わったため再生を停止しました'
    case 'revision_changed':
      return '編集中の内容が変わったため再生を停止しました'
    case 'model_changed':
      return '3Dモデルが変わったため再生を停止しました'
    case 'manual_pose':
      return '3D姿勢を手動変更したため再生を停止しました'
    case 'benchmark':
      return '性能テストを開始したため再生を停止しました'
    case 'file_operation':
      return 'ファイル操作を開始したため再生を停止しました'
    case 'apply_failed':
      return '3D姿勢を適用できなかったため再生を停止しました'
    case 'hidden':
      return '画面が非表示になったため再生を停止しました'
    case 'disposed':
      return '画面を閉じたため再生を停止しました'
    case 'canceled':
      return '折り手順の再生を停止しました'
  }
}

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

// Compile-time checks that the raw DTOs retain the exact persisted structure
// consumed by the validator above.
const _timelineShape: InstructionTimeline | null = null
const _stepShape: InstructionStep | null = null
void _timelineShape
void _stepShape
