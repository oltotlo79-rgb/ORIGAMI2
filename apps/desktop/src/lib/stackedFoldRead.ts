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
  crossedCells: readonly Readonly<{
    cellKeySha256: string
    bottomToTopFaces: readonly string[]
  }>[]
  targetFaces: readonly string[]
  materialSegments: readonly Readonly<{
    faceId: string
    start: readonly [number, number]
    end: readonly [number, number]
    fixedSide: StackedFoldFixedSide
    assignment: 'mountain' | 'valley'
  }>[]
  topologyProof: Readonly<{
    targetFingerprintSha256: string
    targetVertexCount: number
    targetEdgeCount: number
    targetBoundaryVertexCount: number
    lineageRecordCount: number
    sourceEdgeSubdivisionCount: number
    expectedCreaseSubdivisionCount: number
    targetMaterialFaceCount: number
    targetHingeCount: number
  }>
  endpointCollision: Readonly<{
    expectedPairCount: number
    separatedPairCount: number
    touchingPairCount: number
    allowedPairCount: number
    penetratingPairCount: number
    indeterminatePairCount: number
    hasBlockingHold: boolean
  }>
  work: Readonly<{
    scannedCells: number
    totalBoundaryVertices: number
    totalLayerRecords: number
    orientationTests: number
    exactArithmeticOperations: number
    maximumExactIntegerBits: number
    totalExactIntegerBits: number
    retainedCells: number
    retainedTargetFaces: number
  }>
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

const isFinitePoint2 = (value: unknown): value is [number, number] =>
  Array.isArray(value) &&
  value.length === 2 &&
  value.every((coordinate) => typeof coordinate === 'number' && Number.isFinite(coordinate))

const allCounts = (value: Record<string, unknown>, fields: readonly string[]): boolean =>
  fields.every((field) => isCount(value[field]))

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
  const topologyProof = value.topologyProof
  const endpointCollision = value.endpointCollision
  const work = value.work
  const layerOrder = value.flatEndpointLayerOrder
  const endpointCountFields = [
    'expectedPairCount',
    'separatedPairCount',
    'touchingPairCount',
    'allowedPairCount',
    'penetratingPairCount',
    'indeterminatePairCount',
  ] as const
  const endpointCountsValid = allCounts(endpointCollision, endpointCountFields)
  const endpointPairSum = endpointCountsValid
    ? endpointCountFields
        .slice(1)
        .reduce((sum, field) => sum + Number(endpointCollision[field]), 0)
    : -1
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
    !value.crossedCells.every(
      (cell) =>
        isRecord(cell) &&
        isLowerSha256(cell.cellKeySha256) &&
        Array.isArray(cell.bottomToTopFaces) &&
        cell.bottomToTopFaces.length > 0 &&
        cell.bottomToTopFaces.every(isCanonicalNonNilUuid),
    ) ||
    !Array.isArray(value.targetFaces) ||
    value.targetFaces.length === 0 ||
    !value.targetFaces.every(isCanonicalNonNilUuid) ||
    !Array.isArray(value.materialSegments) ||
    value.materialSegments.length !== value.targetFaces.length ||
    !value.materialSegments.every(
      (segment) =>
        isRecord(segment) &&
        isCanonicalNonNilUuid(segment.faceId) &&
        isFinitePoint2(segment.start) &&
        isFinitePoint2(segment.end) &&
        (segment.start[0] !== segment.end[0] || segment.start[1] !== segment.end[1]) &&
        (segment.fixedSide === 'left' || segment.fixedSide === 'right') &&
        (segment.assignment === 'mountain' || segment.assignment === 'valley'),
    ) ||
    !isLowerSha256(topologyProof.targetFingerprintSha256) ||
    !allCounts(topologyProof, [
      'targetVertexCount',
      'targetEdgeCount',
      'targetBoundaryVertexCount',
      'lineageRecordCount',
      'sourceEdgeSubdivisionCount',
      'expectedCreaseSubdivisionCount',
      'targetMaterialFaceCount',
      'targetHingeCount',
    ]) ||
    !endpointCountsValid ||
    endpointPairSum !== endpointCollision.expectedPairCount ||
    endpointCollision.hasBlockingHold !==
      (Number(endpointCollision.penetratingPairCount) > 0 ||
        Number(endpointCollision.indeterminatePairCount) > 0) ||
    !allCounts(work, [
      'scannedCells',
      'totalBoundaryVertices',
      'totalLayerRecords',
      'orientationTests',
      'exactArithmeticOperations',
      'maximumExactIntegerBits',
      'totalExactIntegerBits',
      'retainedCells',
      'retainedTargetFaces',
    ]) ||
    work.retainedCells !== value.crossedCells.length ||
    work.retainedTargetFaces !== value.targetFaces.length ||
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
