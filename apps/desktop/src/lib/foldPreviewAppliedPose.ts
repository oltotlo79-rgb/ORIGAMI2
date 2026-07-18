import type { FoldPreviewHingeAngle } from './foldPreviewKinematics'

export type FoldPreviewAppliedPoseState =
  | 'stable'
  | 'running'
  | 'blocked'
  | 'indeterminate'

export type FoldPreviewAppliedPoseSnapshot = Readonly<{
  projectId: string
  revision: number
  fixedFaceId: string | null
  hingeAngles: readonly FoldPreviewHingeAngle[]
  state: FoldPreviewAppliedPoseState
}>

const MAX_APPLIED_POSE_HINGES = 100_000

/**
 * Detaches the observable pose from Three.js objects, motion runners, and
 * private request authority before it leaves FoldPreview.
 */
export function createFoldPreviewAppliedPoseSnapshot(value: {
  projectId: unknown
  revision: unknown
  fixedFaceId: unknown
  hingeAngles: unknown
  state: unknown
}): FoldPreviewAppliedPoseSnapshot | null {
  if (
    !validIdentity(value.projectId)
    || !validRevision(value.revision)
    || !(value.fixedFaceId === null || validIdentity(value.fixedFaceId))
    || !validState(value.state)
    || !Array.isArray(value.hingeAngles)
    || value.hingeAngles.length > MAX_APPLIED_POSE_HINGES
  ) return null
  const edgeIds = new Set<string>()
  const hingeAngles: FoldPreviewHingeAngle[] = []
  for (const item of value.hingeAngles) {
    if (
      !isRecord(item)
      || !validIdentity(item.edgeId)
      || !validAngle(item.angleDegrees)
      || edgeIds.has(item.edgeId)
    ) return null
    edgeIds.add(item.edgeId)
    hingeAngles.push(Object.freeze({
      edgeId: item.edgeId,
      angleDegrees: normalizeZero(item.angleDegrees),
    }))
  }
  return Object.freeze({
    projectId: value.projectId,
    revision: value.revision,
    fixedFaceId: value.fixedFaceId,
    hingeAngles: Object.freeze(hingeAngles),
    state: value.state,
  })
}

function validState(value: unknown): value is FoldPreviewAppliedPoseState {
  return value === 'stable'
    || value === 'running'
    || value === 'blocked'
    || value === 'indeterminate'
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validIdentity(value: unknown): value is string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 512) return false
  for (const character of value) {
    const code = character.codePointAt(0)
    if (
      code === undefined
      || code <= 0x1f
      || (code >= 0x7f && code <= 0x9f)
    ) return false
  }
  return true
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function normalizeZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}
