import { invoke } from '@tauri-apps/api/core'

export type Fold3dFrameMetadata = Readonly<{
  index: number
  parent: number | null
  inherits: boolean
  vertexCount: number
}>

export type Fold3dFramesMetadata = Readonly<{
  token: string
  projectInstanceId: string
  projectId: string
  revision: number
  frameCount: number
  frames: readonly Fold3dFrameMetadata[]
  authorizesProjectImport: false
}>

export type Fold3dFrameSelection = Readonly<{
  token: string
  frameIndex: number
  vertexCount: number
  sourceSha256Hex: string
  previewImageDataUrl: string
  previewWidth: 512
  previewHeight: 384
  renderCoordinatesExposed: false
  authorizesProjectImport: false
  authorizesAppliedPose: false
  authorizesInstructionTimeline: false
}>

export type Fold3dPoseCompatibility = Readonly<{
  token: string
  frameIndex: number
  hingeCount: number
  sourceFingerprint: string
  authorizesProjectGeometryMutation: false
  requiresExplicitApply: true
}>

export type Fold3dTimelineCompatibility = Readonly<{
  token: string
  frameCount: number
  hingeCount: number
  durationMs: number
  sourceFingerprint: string
  geometryUnchanged: true
  requiresExplicitConfirmation: true
}>

const record = (value: unknown): Record<string, unknown> | null =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
const integer = (value: unknown) =>
  typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
const id = (value: unknown) =>
  typeof value === 'string' && /^[0-9a-f-]{36}$/i.test(value)
const exactKeys = (value: Record<string, unknown>, expected: readonly string[]) =>
  Object.keys(value).length === expected.length
  && expected.every((key) => Object.hasOwn(value, key))

export function normalizeFold3dFramesPicker(value: unknown): {
  canceled: boolean
  preview: Fold3dFramesMetadata | null
} | null {
  const root = record(value)
  if (!root || !exactKeys(root, ['canceled', 'preview'])
    || typeof root.canceled !== 'boolean') return null
  if (root.canceled) return root.preview === null
    ? { canceled: true, preview: null }
    : null
  const preview = record(root.preview)
  if (!preview || !exactKeys(preview, ['token', 'projectInstanceId', 'projectId',
    'revision', 'frameCount', 'frames', 'authorizesProjectImport'])
    || !id(preview.token) || !id(preview.projectInstanceId)
    || !id(preview.projectId) || !integer(preview.revision)
    || !integer(preview.frameCount) || !Array.isArray(preview.frames)
    || preview.frameCount !== preview.frames.length
    || preview.authorizesProjectImport !== false) return null
  const frames = preview.frames.map((candidate) => {
    const frame = record(candidate)
    if (!frame || !exactKeys(frame, ['index', 'parent', 'inherits', 'vertexCount'])
      || !integer(frame.index) || !integer(frame.vertexCount)
      || typeof frame.inherits !== 'boolean'
      || !(frame.parent === null || integer(frame.parent))) return null
    return frame as unknown as Fold3dFrameMetadata
  })
  if (frames.some((frame) => frame === null)) return null
  return { canceled: false, preview: {
    token: preview.token as string,
    projectInstanceId: preview.projectInstanceId as string,
    projectId: preview.projectId as string,
    revision: preview.revision as number,
    frameCount: preview.frameCount as number,
    frames: frames as Fold3dFrameMetadata[],
    authorizesProjectImport: false,
  }}
}

export function normalizeFold3dFrameSelection(value: unknown): Fold3dFrameSelection | null {
  const result = record(value)
  if (!result || !exactKeys(result, ['token', 'frameIndex', 'vertexCount',
    'sourceSha256Hex', 'previewImageDataUrl', 'previewWidth', 'previewHeight',
    'renderCoordinatesExposed', 'authorizesProjectImport', 'authorizesAppliedPose',
    'authorizesInstructionTimeline'])
    || !id(result.token) || !integer(result.frameIndex)
    || !integer(result.vertexCount)
    || typeof result.sourceSha256Hex !== 'string'
    || !/^[0-9a-f]{64}$/.test(result.sourceSha256Hex)
    || typeof result.previewImageDataUrl !== 'string'
    || !result.previewImageDataUrl.startsWith('data:image/png;base64,')
    || result.previewImageDataUrl.length > 700_000
    || result.previewWidth !== 512 || result.previewHeight !== 384
    || result.renderCoordinatesExposed !== false
    || result.authorizesProjectImport !== false
    || result.authorizesAppliedPose !== false
    || result.authorizesInstructionTimeline !== false) return null
  return result as unknown as Fold3dFrameSelection
}

export function normalizeFold3dPoseCompatibility(value: unknown): Fold3dPoseCompatibility | null {
  const result = record(value)
  if (!result || !exactKeys(result, ['token', 'frameIndex', 'hingeCount',
    'sourceFingerprint', 'authorizesProjectGeometryMutation', 'requiresExplicitApply'])
    || !id(result.token) || !integer(result.frameIndex) || !integer(result.hingeCount)
    || typeof result.sourceFingerprint !== 'string'
    || !/^[0-9a-f]{64}$/.test(result.sourceFingerprint)
    || result.authorizesProjectGeometryMutation !== false
    || result.requiresExplicitApply !== true) return null
  return result as unknown as Fold3dPoseCompatibility
}

export async function pickFold3dFrames() {
  const parsed = normalizeFold3dFramesPicker(await invoke<unknown>('preview_fold_3d_frames'))
  if (!parsed) throw new Error('invalid FOLD 3D frame picker response')
  return parsed
}

export async function selectFold3dFrame(preview: Fold3dFramesMetadata, frameIndex: number) {
  const parsed = normalizeFold3dFrameSelection(await invoke<unknown>('select_fold_3d_frame', {
    request: {
      token: preview.token,
      expectedProjectInstanceId: preview.projectInstanceId,
      expectedProjectId: preview.projectId,
      expectedRevision: preview.revision,
      frameIndex,
    },
  }))
  if (!parsed) throw new Error('invalid FOLD 3D frame selection response')
  return parsed
}

const request = (preview: Fold3dFramesMetadata, frameIndex: number) => ({
  token: preview.token,
  expectedProjectInstanceId: preview.projectInstanceId,
  expectedProjectId: preview.projectId,
  expectedRevision: preview.revision,
  frameIndex,
})

export async function prepareFold3dAppliedPose(
  preview: Fold3dFramesMetadata,
  frameIndex: number,
) {
  const parsed = normalizeFold3dPoseCompatibility(await invoke<unknown>(
    'prepare_fold_3d_applied_pose', { request: request(preview, frameIndex) },
  ))
  if (!parsed) throw new Error('invalid FOLD 3D pose compatibility response')
  return parsed
}

export async function applyFold3dAppliedPose(
  preview: Fold3dFramesMetadata,
  frameIndex: number,
) {
  const value = record(await invoke<unknown>('apply_fold_3d_applied_pose', {
    request: request(preview, frameIndex),
  }))
  const binding = value && record(value.binding)
  if (!value || !exactKeys(value, ['binding']) || !binding
    || !exactKeys(binding, ['projectInstanceId', 'projectId', 'revision', 'poseGeneration'])
    || binding.projectInstanceId !== preview.projectInstanceId
    || binding.projectId !== preview.projectId || binding.revision !== preview.revision
    || typeof binding.poseGeneration !== 'string' || !/^(0|[1-9][0-9]*)$/.test(binding.poseGeneration)) {
    throw new Error('invalid FOLD 3D pose apply response')
  }
  return binding as Readonly<{
    projectInstanceId: string
    projectId: string
    revision: number
    poseGeneration: string
  }>
}

export async function prepareFold3dInstructionTimeline(
  preview: Fold3dFramesMetadata,
  durationMs = 1_000,
) {
  const value = normalizeFold3dTimelineCompatibility(await invoke<unknown>(
    'prepare_fold_3d_instruction_timeline', {
    request: {
      token: preview.token,
      expectedProjectInstanceId: preview.projectInstanceId,
      expectedProjectId: preview.projectId,
      expectedRevision: preview.revision,
      durationMs,
      confirmed: false,
    },
    },
  ))
  if (!value || value.token !== preview.token || value.frameCount !== preview.frameCount
    || value.durationMs !== durationMs) {
    throw new Error('invalid FOLD 3D timeline compatibility response')
  }
  return value
}

export function normalizeFold3dTimelineCompatibility(
  input: unknown,
): Fold3dTimelineCompatibility | null {
  const value = record(input)
  if (!value || !exactKeys(value, ['token', 'frameCount', 'hingeCount', 'durationMs',
    'sourceFingerprint', 'geometryUnchanged', 'requiresExplicitConfirmation'])
    || !id(value.token) || !integer(value.frameCount) || (value.frameCount as number) === 0
    || (value.frameCount as number) > 256 || !integer(value.hingeCount)
    || !integer(value.durationMs) || (value.durationMs as number) < 100
    || (value.durationMs as number) > 600_000
    || typeof value.sourceFingerprint !== 'string'
    || !/^[0-9a-f]{64}$/.test(value.sourceFingerprint)
    || value.geometryUnchanged !== true || value.requiresExplicitConfirmation !== true) {
    return null
  }
  return value as unknown as Fold3dTimelineCompatibility
}

export async function applyFold3dInstructionTimeline(
  preview: Fold3dFramesMetadata,
  durationMs = 1_000,
) {
  return invoke<unknown>('apply_fold_3d_instruction_timeline', {
    request: {
      token: preview.token,
      expectedProjectInstanceId: preview.projectInstanceId,
      expectedProjectId: preview.projectId,
      expectedRevision: preview.revision,
      durationMs,
      confirmed: true,
    },
  })
}

export const cancelFold3dFrames = (token: string) =>
  invoke<void>('cancel_fold_3d_frames', { token })
