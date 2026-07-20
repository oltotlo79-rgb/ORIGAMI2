import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export type MeshAnimationPreviewRequest = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
}>

export type MeshAnimationPreviewResponse = Readonly<{
  exportId: string
  projectInstanceId: string
  projectId: string
  revision: number
  sourceFingerprint: string
  frameCount: number
  vertexCount: number
  triangleCount: number
  durationSeconds: number
  byteCount: number
  mediaType: 'model/gltf-binary'
  fileExtension: 'glb'
}>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const isCount = (value: unknown): value is number =>
  Number.isSafeInteger(value) && (value as number) >= 0

const isPositiveCount = (value: unknown): value is number => isCount(value) && value > 0

export function isMeshAnimationPreviewRequest(
  value: unknown,
): value is MeshAnimationPreviewRequest {
  if (!isRecord(value)) return false
  const keys = Object.keys(value)
  return (
    keys.length === 3 &&
    keys.every((key) =>
      ['expectedProjectInstanceId', 'expectedProjectId', 'expectedRevision'].includes(key),
    ) &&
    isCanonicalNonNilUuid(value.expectedProjectInstanceId) &&
    isCanonicalNonNilUuid(value.expectedProjectId) &&
    isCount(value.expectedRevision)
  )
}

export function normalizeMeshAnimationPreviewResponse(
  value: unknown,
  expected: MeshAnimationPreviewRequest,
): MeshAnimationPreviewResponse | null {
  if (!isRecord(value)) return null
  if (
    !isCanonicalNonNilUuid(value.exportId) ||
    value.projectInstanceId !== expected.expectedProjectInstanceId ||
    value.projectId !== expected.expectedProjectId ||
    value.revision !== expected.expectedRevision ||
    typeof value.sourceFingerprint !== 'string' ||
    !/^[0-9a-f]{64}$/u.test(value.sourceFingerprint) ||
    !isPositiveCount(value.frameCount) ||
    value.frameCount < 2 ||
    value.frameCount > 256 ||
    !isPositiveCount(value.vertexCount) ||
    !isPositiveCount(value.triangleCount) ||
    typeof value.durationSeconds !== 'number' ||
    !Number.isFinite(value.durationSeconds) ||
    value.durationSeconds <= 0 ||
    !isPositiveCount(value.byteCount) ||
    value.byteCount > 64 * 1024 * 1024 ||
    value.mediaType !== 'model/gltf-binary' ||
    value.fileExtension !== 'glb'
  ) {
    return null
  }
  return value as MeshAnimationPreviewResponse
}
