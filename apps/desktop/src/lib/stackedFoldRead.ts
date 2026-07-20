import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export type StackedFoldFixedSide = 'left' | 'right'
export type StackedFoldRotationDirection = 'positive' | 'negative'

export const STACKED_FOLD_READ_GUARD_MODEL_ID_V1 = 'native_flat_stacked_fold_read_guard_v1'
export const STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1 =
  'native_linear_stacked_fold_read_proposal_v1'
export const STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1 =
  'native_flat_stacked_fold_material_map_v1'
export const STACKED_FOLD_PATH_CERTIFICATE_MODEL_IDS = Object.freeze([
  'stacked_fold_single_hinge_zero_thickness_continuous_certificate_v1',
  'stacked_fold_single_hinge_positive_thickness_continuous_certificate_v1',
  'stacked_fold_collinear_tree_zero_thickness_continuous_certificate_v1',
  'stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1',
  'stacked_fold_two_hinge_interval_zero_thickness_continuous_certificate_v1',
  'stacked_fold_tree_interval_zero_thickness_continuous_certificate_v1',
  'stacked_fold_cycle_interval_zero_thickness_continuous_certificate_v1',
] as const)

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
    boundaryWorld: readonly (readonly [number, number, number])[]
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
  continuousPath: Readonly<{
    modelId: string
    continuousCertificateModelId: string | null
    sampledPoseCount: number
    sampledNonblockingPoseCount: number
    intervalLeafCount: number
    intervalPairWork: number
    intervalCandidateLimit: number
    closureRequired: boolean
    closureLeafCount: number
    closurePairWork: number
    firstClosureFailureAngleDegrees: number | null
    firstSampledBlockingAngleDegrees: number | null
    requestedAngleDegrees: number
    continuousClearanceCertified: boolean
    safeStopAngleDegrees: number
    authorizesProjectMutation: boolean
    paperThicknessMm: number
  }>
  transactionProposal: Readonly<{
    transactionToken: string | null
    sourceProjectId: string
    sourceRevision: number
    targetRevision: number
    sourceFingerprintSha256: string
    targetFingerprintSha256: string
    addedVertexCount: number
    addedEdgeCount: number
    mountainCreaseCount: number
    valleyCreaseCount: number
    timelineStepCount: number
    timelineCompleteHingeAngleCount: number
    requestedAngleDegrees: number
    readyForAtomicApply: boolean
    failureClasses: readonly (
      | 'continuous_path_uncertified'
      | 'target_layer_order_unavailable'
    )[]
    authorizesProjectMutation: boolean
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

const hasExactKeys = (value: Record<string, unknown>, fields: readonly string[]): boolean => {
  const keys = Object.keys(value)
  return keys.length === fields.length && keys.every((key) => fields.includes(key))
}

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
    | 'expectedProjectInstanceId'
    | 'expectedProjectId'
    | 'expectedRevision'
    | 'requestedAngleDegrees'
  >,
): StackedFoldReadResponse | null {
  if (
    !isRecord(value) ||
    !isRecord(value.binding) ||
    !isRecord(value.topologyProof) ||
    !isRecord(value.endpointCollision) ||
    !isRecord(value.continuousPath) ||
    !isRecord(value.transactionProposal) ||
    !isRecord(value.work) ||
    !isRecord(value.flatEndpointLayerOrder)
  )
    return null
  const binding = value.binding
  const topologyProof = value.topologyProof
  const endpointCollision = value.endpointCollision
  const continuousPath = value.continuousPath
  const transaction = value.transactionProposal
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
    !hasExactKeys(value, [
      'guardModelId',
      'proposalModelId',
      'materialMapModelId',
      'binding',
      'support',
      'crossedCells',
      'targetFaces',
      'materialSegments',
      'topologyProof',
      'endpointCollision',
      'continuousPath',
      'flatEndpointLayerOrder',
      'transactionProposal',
      'work',
      'authorizesProjectMutation',
      'authorizesApplyStackedFold',
    ]) ||
    !hasExactKeys(binding, [
      'projectInstanceId',
      'projectId',
      'sourceRevision',
      'poseGeneration',
      'layerOrderGeneration',
    ]) ||
    !hasExactKeys(topologyProof, [
      'targetFingerprintSha256',
      'targetVertexCount',
      'targetEdgeCount',
      'targetBoundaryVertexCount',
      'lineageRecordCount',
      'sourceEdgeSubdivisionCount',
      'expectedCreaseSubdivisionCount',
      'targetMaterialFaceCount',
      'targetHingeCount',
    ]) ||
    !hasExactKeys(endpointCollision, [
      ...endpointCountFields,
      'hasBlockingHold',
    ]) ||
    !hasExactKeys(continuousPath, [
      'modelId',
      'continuousCertificateModelId',
      'sampledPoseCount',
      'sampledNonblockingPoseCount',
      'intervalLeafCount',
      'intervalPairWork',
      'intervalCandidateLimit',
      'closureRequired',
      'closureLeafCount',
      'closurePairWork',
      'firstClosureFailureAngleDegrees',
      'firstSampledBlockingAngleDegrees',
      'requestedAngleDegrees',
      'continuousClearanceCertified',
      'safeStopAngleDegrees',
      'authorizesProjectMutation',
      'paperThicknessMm',
    ]) ||
    !hasExactKeys(transaction, [
      'transactionToken',
      'sourceProjectId',
      'sourceRevision',
      'targetRevision',
      'sourceFingerprintSha256',
      'targetFingerprintSha256',
      'addedVertexCount',
      'addedEdgeCount',
      'mountainCreaseCount',
      'valleyCreaseCount',
      'timelineStepCount',
      'timelineCompleteHingeAngleCount',
      'requestedAngleDegrees',
      'readyForAtomicApply',
      'failureClasses',
      'authorizesProjectMutation',
    ]) ||
    !hasExactKeys(work, [
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
    !hasExactKeys(layerOrder, [
      'applicable',
      'certified',
      'materialFaceCount',
      'overlapCellCount',
    ]) ||
    value.guardModelId !== STACKED_FOLD_READ_GUARD_MODEL_ID_V1 ||
    value.proposalModelId !== STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1 ||
    value.materialMapModelId !== STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1 ||
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
        hasExactKeys(cell, ['cellKeySha256', 'bottomToTopFaces', 'boundaryWorld']) &&
        isLowerSha256(cell.cellKeySha256) &&
        Array.isArray(cell.bottomToTopFaces) &&
        cell.bottomToTopFaces.length > 0 &&
        cell.bottomToTopFaces.every(isCanonicalNonNilUuid) &&
        Array.isArray(cell.boundaryWorld) &&
        cell.boundaryWorld.length >= 3 &&
        cell.boundaryWorld.length <= 4096 &&
        cell.boundaryWorld.every(isFinitePoint),
    ) ||
    !Array.isArray(value.targetFaces) ||
    value.targetFaces.length === 0 ||
    !value.targetFaces.every(isCanonicalNonNilUuid) ||
    !Array.isArray(value.materialSegments) ||
    value.materialSegments.length !== value.targetFaces.length ||
    !value.materialSegments.every(
      (segment) =>
        isRecord(segment) &&
        hasExactKeys(segment, ['faceId', 'start', 'end', 'fixedSide', 'assignment']) &&
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
    typeof continuousPath.modelId !== 'string' ||
    (continuousPath.continuousCertificateModelId !== null &&
      !STACKED_FOLD_PATH_CERTIFICATE_MODEL_IDS.some(
        (modelId) => modelId === continuousPath.continuousCertificateModelId,
      )) ||
    !isCount(continuousPath.sampledPoseCount) ||
    !isCount(continuousPath.sampledNonblockingPoseCount) ||
    !isCount(continuousPath.intervalLeafCount) ||
    !isCount(continuousPath.intervalPairWork) ||
    !isCount(continuousPath.intervalCandidateLimit) ||
    typeof continuousPath.closureRequired !== 'boolean' ||
    !isCount(continuousPath.closureLeafCount) ||
    !isCount(continuousPath.closurePairWork) ||
    (continuousPath.firstClosureFailureAngleDegrees !== null &&
      (typeof continuousPath.firstClosureFailureAngleDegrees !== 'number' ||
        !Number.isFinite(continuousPath.firstClosureFailureAngleDegrees))) ||
    continuousPath.sampledNonblockingPoseCount > continuousPath.sampledPoseCount ||
    (continuousPath.firstSampledBlockingAngleDegrees !== null &&
      (typeof continuousPath.firstSampledBlockingAngleDegrees !== 'number' ||
        !Number.isFinite(continuousPath.firstSampledBlockingAngleDegrees))) ||
    typeof continuousPath.requestedAngleDegrees !== 'number' ||
    !Number.isFinite(continuousPath.requestedAngleDegrees) ||
    continuousPath.requestedAngleDegrees !== expected.requestedAngleDegrees ||
    typeof continuousPath.safeStopAngleDegrees !== 'number' ||
    !Number.isFinite(continuousPath.safeStopAngleDegrees) ||
    typeof continuousPath.continuousClearanceCertified !== 'boolean' ||
    typeof continuousPath.authorizesProjectMutation !== 'boolean' ||
    typeof continuousPath.paperThicknessMm !== 'number' ||
    !Number.isFinite(continuousPath.paperThicknessMm) ||
    continuousPath.paperThicknessMm < 0 ||
    (transaction.transactionToken !== null &&
      !isCanonicalNonNilUuid(transaction.transactionToken)) ||
    transaction.sourceProjectId !== expected.expectedProjectId ||
    transaction.sourceRevision !== expected.expectedRevision ||
    !isCount(transaction.targetRevision) ||
    transaction.targetRevision !== transaction.sourceRevision + 1 ||
    !isLowerSha256(transaction.sourceFingerprintSha256) ||
    !isLowerSha256(transaction.targetFingerprintSha256) ||
    transaction.targetFingerprintSha256 !== topologyProof.targetFingerprintSha256 ||
    !allCounts(transaction, [
      'addedVertexCount',
      'addedEdgeCount',
      'mountainCreaseCount',
      'valleyCreaseCount',
      'timelineStepCount',
      'timelineCompleteHingeAngleCount',
    ]) ||
    typeof transaction.requestedAngleDegrees !== 'number' ||
    !Number.isFinite(transaction.requestedAngleDegrees) ||
    transaction.requestedAngleDegrees !== expected.requestedAngleDegrees ||
    typeof transaction.readyForAtomicApply !== 'boolean' ||
    !Array.isArray(transaction.failureClasses) ||
    !transaction.failureClasses.every((failure) =>
      failure === 'continuous_path_uncertified' ||
      failure === 'target_layer_order_unavailable') ||
    typeof transaction.authorizesProjectMutation !== 'boolean' ||
    transaction.readyForAtomicApply !==
      (transaction.transactionToken !== null &&
        transaction.failureClasses.length === 0 &&
        transaction.authorizesProjectMutation) ||
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
