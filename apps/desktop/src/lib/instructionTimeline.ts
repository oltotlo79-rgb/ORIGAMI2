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
} from './i18n.ts'
import {
  INSTRUCTION_TIMELINE_PRESENTATION_TEXT as TEXT,
} from './instructionTimelinePresentationText.ts'

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
  | Readonly<{ kind: 'split' }>
  | Readonly<{ kind: 'merged' }>
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
      return selectLocalizedText(locale, TEXT.playback.idle)
    case 'applying':
      return formatLocalizedText(locale, TEXT.playback.applying, {
        step: state.target.index + 1,
        title: state.target.title,
      })
    case 'holding':
      return formatLocalizedText(locale, TEXT.playback.holding, {
        step: state.target.index + 1,
        title: state.target.title,
      })
    case 'complete':
      return selectLocalizedText(locale, TEXT.playback.complete)
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
      return selectLocalizedText(locale, TEXT.notices.add_failed)
    case 'added':
      return formatLocalizedText(locale, TEXT.notices.added, { title: notice.title })
    case 'updated':
      return formatLocalizedText(locale, TEXT.notices.updated, { title: notice.title })
    case 'update_failed':
      return selectLocalizedText(locale, TEXT.notices.update_failed)
    case 'pose_updated':
      return formatLocalizedText(locale, TEXT.notices.pose_updated, {
        title: notice.title,
      })
    case 'pose_update_failed':
      return selectLocalizedText(locale, TEXT.notices.pose_update_failed)
    case 'delete_failed':
      return selectLocalizedText(locale, TEXT.notices.delete_failed)
    case 'deleted':
      return formatLocalizedText(locale, TEXT.notices.deleted, { title: notice.title })
    case 'moved':
      return selectLocalizedText(locale, TEXT.notices.moved)
    case 'split':
      return selectLocalizedText(locale, TEXT.notices.split)
    case 'merged':
      return selectLocalizedText(locale, TEXT.notices.merged)
    case 'move_failed':
      return selectLocalizedText(locale, TEXT.notices.move_failed)
    case 'stale_pose':
      return selectLocalizedText(locale, TEXT.notices.stale_pose)
    case 'pose_apply_failed':
      return selectLocalizedText(locale, TEXT.notices.pose_apply_failed)
    case 'pose_applying':
      return formatLocalizedText(locale, TEXT.notices.pose_applying, {
        title: notice.title,
      })
    case 'model_required':
      return selectLocalizedText(locale, TEXT.notices.model_required)
    case 'no_steps':
      return selectLocalizedText(locale, TEXT.notices.no_steps)
    case 'declarative_playback_unsupported':
      return selectLocalizedText(
        locale,
        TEXT.notices.declarative_playback_unsupported,
      )
  }
}

export function instructionCaptureStatusText(
  status: InstructionCaptureStatus,
  locale: Locale = 'ja',
): string {
  return selectLocalizedText(locale, TEXT.capture[status])
}

export function instructionEditorErrorText(
  error: InstructionEditorError,
  locale: Locale = 'ja',
): string {
  if (error === 'update_failed') {
    return selectLocalizedText(locale, TEXT.editor.update_failed)
  }
  return formatLocalizedText(locale, TEXT.editor.invalid_metadata, {
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
      selectLocalizedText(locale, TEXT.duration.numberLocale),
      { maximumFractionDigits: 1 },
    )
    return formatLocalizedText(locale, TEXT.duration.seconds, { seconds: formatted })
  }
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = Math.floor(totalSeconds % 60)
  return `${minutes}:${String(seconds).padStart(2, '0')}`
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
    ...(Object.prototype.hasOwnProperty.call(value, 'cycle_layer_order_proof_v1')
      ? {
          cycle_layer_order_proof_v1:
            value.cycle_layer_order_proof_v1 as InstructionVisual['cycle_layer_order_proof_v1'],
        }
      : {}),
    ...(Object.prototype.hasOwnProperty.call(value, 'path_certificate_reference_v1')
      ? { path_certificate_reference_v1: pathCertificateReference }
      : {}),
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
      return selectLocalizedText(locale, TEXT.stopped.stale_step)
    case 'project_changed':
      return selectLocalizedText(locale, TEXT.stopped.project_changed)
    case 'revision_changed':
      return selectLocalizedText(locale, TEXT.stopped.revision_changed)
    case 'model_changed':
      return selectLocalizedText(locale, TEXT.stopped.model_changed)
    case 'manual_pose':
      return selectLocalizedText(locale, TEXT.stopped.manual_pose)
    case 'benchmark':
      return selectLocalizedText(locale, TEXT.stopped.benchmark)
    case 'file_operation':
      return selectLocalizedText(locale, TEXT.stopped.file_operation)
    case 'apply_failed':
      return selectLocalizedText(locale, TEXT.stopped.apply_failed)
    case 'hidden':
      return selectLocalizedText(locale, TEXT.stopped.hidden)
    case 'disposed':
      return selectLocalizedText(locale, TEXT.stopped.disposed)
    case 'canceled':
      return selectLocalizedText(locale, TEXT.stopped.canceled)
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

function hasRequiredAndOptionalKeys(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[],
) {
  const keys = Object.keys(value)
  return required.every((key) => Object.prototype.hasOwnProperty.call(value, key))
    && keys.every((key) => required.includes(key) || optional.includes(key))
}

// Compile-time checks that the raw DTOs retain the exact persisted structure
// consumed by the validator above.
const _timelineShape: InstructionTimeline | null = null
const _stepShape: InstructionStep | null = null
void _timelineShape
void _stepShape
