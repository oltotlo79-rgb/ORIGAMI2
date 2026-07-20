import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export type StackedFoldFixedSide = 'left' | 'right'
export type StackedFoldRotationDirection = 'positive' | 'negative'

export type StackedFoldReadRequest = Readonly<{
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
  first: readonly [number, number, number]
  second: readonly [number, number, number]
  fixedSide: StackedFoldFixedSide
  rotationDirection: StackedFoldRotationDirection
  requestedAngleDegrees: number
}>

export type StackedFoldReadResponse = Readonly<{
  guardModelId: string
  proposalModelId: string
  materialMapModelId: string
  binding: Readonly<{
    projectInstanceId: string
    projectId: string
    sourceRevision: number
    poseGeneration: number
    layerOrderGeneration: number
  }>
  support: 'no_hinge_single_face' | 'bit_exact_flat_endpoint_tree'
  crossedCells: readonly unknown[]
  targetFaces: readonly string[]
  materialSegments: readonly unknown[]
  topologyProof: Readonly<{ targetFingerprintSha256: string }>
  endpointCollision: Readonly<{ hasBlockingHold: boolean }>
  work: Readonly<{ scannedCells: number }>
  authorizesProjectMutation: false
  authorizesApplyStackedFold: false
  flatEndpointLayerOrder: Readonly<{
    applicable: boolean
    certified: boolean
    materialFaceCount: number
    overlapCellCount: number
  }>
}>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const isCount = (value: unknown): value is number =>
  Number.isSafeInteger(value) && (value as number) >= 0

const isFinitePoint = (value: unknown): value is [number, number, number] =>
  Array.isArray(value) &&
  value.length === 3 &&
  value.every((coordinate) => typeof coordinate === 'number' && Number.isFinite(coordinate))

const isLowerSha256 = (value: unknown): value is string =>
  typeof value === 'string' && /^[0-9a-f]{64}$/u.test(value)

export function isStackedFoldReadRequest(value: unknown): value is StackedFoldReadRequest {
  if (!isRecord(value)) return false
  const first = value.first
  const second = value.second
  return (
    isCanonicalNonNilUuid(value.expectedProjectInstanceId) &&
    isCanonicalNonNilUuid(value.expectedProjectId) &&
    isCount(value.expectedRevision) &&
    isFinitePoint(first) &&
    isFinitePoint(second) &&
    first.some((coordinate, index) => coordinate !== second[index]) &&
    (value.fixedSide === 'left' || value.fixedSide === 'right') &&
    (value.rotationDirection === 'positive' || value.rotationDirection === 'negative') &&
    typeof value.requestedAngleDegrees === 'number' &&
    Number.isFinite(value.requestedAngleDegrees) &&
    value.requestedAngleDegrees > 0 &&
    value.requestedAngleDegrees <= 180
  )
}

export function normalizeStackedFoldReadResponse(
  value: unknown,
  expected: Pick<
    StackedFoldReadRequest,
    'expectedProjectInstanceId' | 'expectedProjectId' | 'expectedRevision'
  >,
): StackedFoldReadResponse | null {
  if (
    !isRecord(value) ||
    !isRecord(value.binding) ||
    !isRecord(value.topologyProof) ||
    !isRecord(value.endpointCollision) ||
    !isRecord(value.work) ||
    !isRecord(value.flatEndpointLayerOrder)
  )
    return null
  const binding = value.binding
  const layerOrder = value.flatEndpointLayerOrder
  if (
    typeof value.guardModelId !== 'string' ||
    value.guardModelId.length === 0 ||
    typeof value.proposalModelId !== 'string' ||
    value.proposalModelId.length === 0 ||
    typeof value.materialMapModelId !== 'string' ||
    value.materialMapModelId.length === 0 ||
    binding.projectInstanceId !== expected.expectedProjectInstanceId ||
    binding.projectId !== expected.expectedProjectId ||
    binding.sourceRevision !== expected.expectedRevision ||
    !isCount(binding.poseGeneration) ||
    !isCount(binding.layerOrderGeneration) ||
    (value.support !== 'no_hinge_single_face' &&
      value.support !== 'bit_exact_flat_endpoint_tree') ||
    !Array.isArray(value.crossedCells) ||
    !Array.isArray(value.targetFaces) ||
    !value.targetFaces.every(isCanonicalNonNilUuid) ||
    !Array.isArray(value.materialSegments) ||
    !isLowerSha256(value.topologyProof.targetFingerprintSha256) ||
    typeof value.endpointCollision.hasBlockingHold !== 'boolean' ||
    !isCount(value.work.scannedCells) ||
    value.authorizesProjectMutation !== false ||
    value.authorizesApplyStackedFold !== false ||
    typeof layerOrder.applicable !== 'boolean' ||
    typeof layerOrder.certified !== 'boolean' ||
    !isCount(layerOrder.materialFaceCount) ||
    !isCount(layerOrder.overlapCellCount) ||
    (!layerOrder.applicable &&
      (layerOrder.certified ||
        layerOrder.materialFaceCount !== 0 ||
        layerOrder.overlapCellCount !== 0)) ||
    (!layerOrder.certified &&
      (layerOrder.materialFaceCount !== 0 || layerOrder.overlapCellCount !== 0))
  ) {
    return null
  }
  return value as StackedFoldReadResponse
}
