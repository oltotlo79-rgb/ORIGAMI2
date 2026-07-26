import { invoke } from '@tauri-apps/api/core'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

export const CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1 =
  'native_stacked_fold_non_flat_planar_order_v1'
export const CURRENT_NON_FLAT_LAYER_ORDER_TREE_POSE_MODEL_ID_V1 =
  'tree_absolute_hinge_angles_v1'
export const CURRENT_NON_FLAT_LAYER_ORDER_GRAPH_POSE_MODEL_ID_V1 =
  'closed_graph_absolute_hinge_angles_v1'

/** The only two admissible pose model IDs, one per issuer kind. */
export type CurrentNonFlatLayerOrderPoseModelIdV1 =
  | typeof CURRENT_NON_FLAT_LAYER_ORDER_TREE_POSE_MODEL_ID_V1
  | typeof CURRENT_NON_FLAT_LAYER_ORDER_GRAPH_POSE_MODEL_ID_V1

export const MAX_NON_FLAT_VIEW_FACES_V1 = 512
export const MAX_NON_FLAT_VIEW_HINGES_V1 = 4_096
export const MAX_NON_FLAT_VIEW_CELLS_V1 = 4_096
export const MAX_NON_FLAT_VIEW_POLYGON_POINTS_V1 = 4_096
export const MAX_NON_FLAT_VIEW_WORLD_POINTS_V1 = 100_000
export const MAX_NON_FLAT_VIEW_EXACT_POINTS_V1 = 100_000
export const MAX_NON_FLAT_VIEW_EXACT_MAGNITUDE_BYTES_V1 = 8 * 1024 * 1024

const SHA256_HEX = /^[0-9a-f]{64}$/u
const CANONICAL_GENERATION = /^[1-9][0-9]*$/u
const EVEN_LOWER_HEX = /^(?:[0-9a-f]{2})*$/u
const MAX_U64 = 18_446_744_073_709_551_615n

/** The only admissible dropped-axis and plane-axis pairs. */
const AXIS_PLANES: Readonly<Record<string, readonly [string, string]>> =
  Object.freeze({
    x: Object.freeze(['y', 'z'] as const),
    y: Object.freeze(['x', 'z'] as const),
    z: Object.freeze(['x', 'y'] as const),
  })

export type ExactRationalV1 = Readonly<{
  sign: 'negative' | 'zero' | 'positive'
  numeratorMagnitudeHex: string
  denominatorMagnitudeHex: string
}>

export type ExactPointV1 = Readonly<{
  u: ExactRationalV1
  v: ExactRationalV1
}>

export type ExactAffineV1 = Readonly<{
  m00: ExactRationalV1
  m01: ExactRationalV1
  m10: ExactRationalV1
  m11: ExactRationalV1
  tx: ExactRationalV1
  ty: ExactRationalV1
}>

export type NonFlatFaceProjectionV1 = Readonly<{
  droppedWorldAxis: string
  planeAxes: readonly [string, string]
  sourceToPlaneProjectionExact: ExactAffineV1
}>

export type NonFlatViewFaceV1 = Readonly<{
  faceId: string
  faceKeySha256: string
  worldOuterBoundaryXyzMm: readonly (readonly [number, number, number])[]
  projection: NonFlatFaceProjectionV1
}>

export type NonFlatCellProjectionV1 = Readonly<{
  droppedWorldAxis: string
  planeAxes: readonly [string, string]
  roundedBoundaryUvMm: readonly (readonly [number, number])[]
  exactBoundaryUv: readonly ExactPointV1[]
}>

export type NonFlatViewCellV1 = Readonly<{
  cellKeySha256: string
  exactBoundarySha256: string
  lowerFaceId: string
  upperFaceId: string
  projection: NonFlatCellProjectionV1
}>

export type NonFlatViewHingeAngleV1 = Readonly<{
  edgeId: string
  angleDegrees: number
}>

export type NonFlatViewPoseV1 = Readonly<{
  modelId: CurrentNonFlatLayerOrderPoseModelIdV1
  generation: string
  fixedFaceId: string
  hingeAngles: readonly NonFlatViewHingeAngleV1[]
}>

export type NonFlatViewWorkV1 = Readonly<{
  testedFacePairs: number
  materialFaceCount: number
  sourceOverlapCellsAuthenticated: number
  overlapCellCount: number
  facePairOrderCount: number
  worldBoundaryPointCount: number
  exactBoundaryPointCount: number
}>

export type CurrentNonFlatLayerOrderViewV1 = Readonly<{
  version: 1
  modelId: typeof CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprintSha256: string
  pose: NonFlatViewPoseV1
  faces: readonly NonFlatViewFaceV1[]
  cells: readonly NonFlatViewCellV1[]
  work: NonFlatViewWorkV1
  readOnly: true
  authorizesProjectMutation: false
}>

export type CurrentNonFlatLayerOrderViewErrorKindV1 =
  | 'stale_authority'
  | 'invalid_evidence'
  | 'resource_limit'
  | 'internal_failure'

/**
 * Reads an own data property without running accessors.
 *
 * Getters, setters, inherited values, and reflection traps that throw all fail
 * closed as `undefined`, so a hostile payload can never execute code here.
 */
function ownData(record: object, key: string): unknown {
  let descriptor: PropertyDescriptor | undefined
  try {
    descriptor = Object.getOwnPropertyDescriptor(record, key)
  } catch {
    return undefined
  }
  if (!descriptor || !('value' in descriptor)) return undefined
  return descriptor.value
}

/** Captures all own descriptors in one fail-closed reflection operation. */
function ownDescriptorSnapshot(value: object): PropertyDescriptorMap | null {
  try {
    return Object.getOwnPropertyDescriptors(value)
  } catch {
    return null
  }
}

/** Detaches an object whose own enumerable keys are exactly `keys`. */
function exactRecord(
  value: unknown,
  keys: readonly string[],
): Record<string, unknown> | null {
  if (typeof value !== 'object' || value === null) return null
  let isArray: boolean
  try {
    isArray = Array.isArray(value)
  } catch {
    return null
  }
  if (isArray) return null
  const descriptors = ownDescriptorSnapshot(value)
  if (!descriptors) return null
  const ownKeys = Reflect.ownKeys(descriptors)
  if (ownKeys.some((key) => typeof key === 'symbol')) return null
  const names = ownKeys as string[]
  if (names.length !== keys.length) return null
  const expected = [...keys].sort()
  const actual = names.slice().sort()
  for (let index = 0; index < expected.length; index += 1) {
    if (actual[index] !== expected[index]) return null
  }
  const record: Record<string, unknown> = {}
  for (const key of keys) {
    const descriptor = descriptors[key]
    if (!descriptor || !('value' in descriptor)) return null
    record[key] = descriptor.value
  }
  return record
}

/** Detaches a dense array with no extra own properties. */
function denseArray(value: unknown, maximum: number): unknown[] | null {
  let isArray: boolean
  try {
    isArray = Array.isArray(value)
  } catch {
    return null
  }
  if (!isArray || typeof value !== 'object' || value === null) return null
  const descriptors = ownDescriptorSnapshot(value)
  if (!descriptors) return null
  const ownKeys = Reflect.ownKeys(descriptors)
  if (ownKeys.some((key) => typeof key === 'symbol')) return null
  const lengthDescriptor = descriptors.length
  if (!lengthDescriptor || !('value' in lengthDescriptor)) return null
  const length = lengthDescriptor.value
  if (
    typeof length !== 'number'
    || !Number.isSafeInteger(length)
    || length < 0
    || length > maximum
    || ownKeys.length !== length + 1
  ) return null
  const items: unknown[] = []
  for (let index = 0; index < length; index += 1) {
    const descriptor = descriptors[String(index)]
    if (!descriptor || !('value' in descriptor)) return null
    items.push(descriptor.value)
  }
  return items
}

function finiteNumber(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  if (Object.is(value, -0)) return null
  return value
}

function safeCount(value: unknown, maximum: number): number | null {
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) return null
  if (Object.is(value, -0) || value < 0 || value > maximum) return null
  return value
}

function sha256Hex(value: unknown): string | null {
  return typeof value === 'string' && SHA256_HEX.test(value) ? value : null
}

function exactRational(
  value: unknown,
  budget: { bytes: number },
): ExactRationalV1 | null {
  const record = exactRecord(value, [
    'sign',
    'numeratorMagnitudeHex',
    'denominatorMagnitudeHex',
  ])
  if (!record) return null
  const sign = record.sign
  const numerator = record.numeratorMagnitudeHex
  const denominator = record.denominatorMagnitudeHex
  if (
    (sign !== 'negative' && sign !== 'zero' && sign !== 'positive')
    || typeof numerator !== 'string'
    || typeof denominator !== 'string'
    || !EVEN_LOWER_HEX.test(numerator)
    || !EVEN_LOWER_HEX.test(denominator)
    || denominator.length === 0
    || denominator.startsWith('00')
    || /^0+$/u.test(denominator)
    || (sign === 'zero') !== (numerator.length === 0)
    || (sign === 'zero' && denominator !== '01')
    || (numerator.length > 0 && numerator.startsWith('00'))
  ) return null
  budget.bytes += numerator.length / 2 + denominator.length / 2
  if (
    !Number.isSafeInteger(budget.bytes)
    || budget.bytes > MAX_NON_FLAT_VIEW_EXACT_MAGNITUDE_BYTES_V1
  ) return null
  return Object.freeze({
    sign,
    numeratorMagnitudeHex: numerator,
    denominatorMagnitudeHex: denominator,
  })
}

function exactAffine(
  value: unknown,
  budget: { bytes: number },
): ExactAffineV1 | null {
  const record = exactRecord(value, ['m00', 'm01', 'm10', 'm11', 'tx', 'ty'])
  if (!record) return null
  const m00 = exactRational(record.m00, budget)
  const m01 = exactRational(record.m01, budget)
  const m10 = exactRational(record.m10, budget)
  const m11 = exactRational(record.m11, budget)
  const tx = exactRational(record.tx, budget)
  const ty = exactRational(record.ty, budget)
  if (!m00 || !m01 || !m10 || !m11 || !tx || !ty) return null
  return Object.freeze({ m00, m01, m10, m11, tx, ty })
}

function planeAxes(
  droppedWorldAxis: unknown,
  value: unknown,
): readonly [string, string] | null {
  if (typeof droppedWorldAxis !== 'string') return null
  const expected = AXIS_PLANES[droppedWorldAxis]
  if (!expected) return null
  const axes = denseArray(value, 2)
  if (!axes || axes.length !== 2) return null
  if (axes[0] !== expected[0] || axes[1] !== expected[1]) return null
  return Object.freeze([expected[0], expected[1]] as [string, string])
}

function worldPolygon(
  value: unknown,
): readonly (readonly [number, number, number])[] | null {
  const points = denseArray(value, MAX_NON_FLAT_VIEW_POLYGON_POINTS_V1)
  if (!points || points.length < 3) return null
  const detached: (readonly [number, number, number])[] = []
  for (const point of points) {
    const coordinates = denseArray(point, 3)
    if (!coordinates || coordinates.length !== 3) return null
    const x = finiteNumber(coordinates[0])
    const y = finiteNumber(coordinates[1])
    const z = finiteNumber(coordinates[2])
    if (x === null || y === null || z === null) return null
    detached.push(Object.freeze([x, y, z] as [number, number, number]))
  }
  return Object.freeze(detached)
}

function face(
  value: unknown,
  budget: { bytes: number },
): NonFlatViewFaceV1 | null {
  const record = exactRecord(value, [
    'faceId',
    'faceKeySha256',
    'worldOuterBoundaryXyzMm',
    'projection',
  ])
  if (!record) return null
  const faceId = record.faceId
  const faceKeySha256 = sha256Hex(record.faceKeySha256)
  const boundary = worldPolygon(record.worldOuterBoundaryXyzMm)
  const projection = exactRecord(record.projection, [
    'droppedWorldAxis',
    'planeAxes',
    'sourceToPlaneProjectionExact',
  ])
  if (
    !isCanonicalNonNilUuid(faceId)
    || faceKeySha256 === null
    || !boundary
    || !projection
  ) return null
  const axes = planeAxes(projection.droppedWorldAxis, projection.planeAxes)
  const affine = exactAffine(projection.sourceToPlaneProjectionExact, budget)
  if (!axes || !affine) return null
  return Object.freeze({
    faceId,
    faceKeySha256,
    worldOuterBoundaryXyzMm: boundary,
    projection: Object.freeze({
      droppedWorldAxis: projection.droppedWorldAxis as string,
      planeAxes: axes,
      sourceToPlaneProjectionExact: affine,
    }),
  })
}

function cell(
  value: unknown,
  faces: ReadonlyMap<string, NonFlatViewFaceV1>,
  budget: { bytes: number },
): NonFlatViewCellV1 | null {
  const record = exactRecord(value, [
    'cellKeySha256',
    'exactBoundarySha256',
    'lowerFaceId',
    'upperFaceId',
    'projection',
  ])
  if (!record) return null
  const cellKeySha256 = sha256Hex(record.cellKeySha256)
  const exactBoundarySha256 = sha256Hex(record.exactBoundarySha256)
  const lowerFaceId = record.lowerFaceId
  const upperFaceId = record.upperFaceId
  if (
    cellKeySha256 === null
    || exactBoundarySha256 === null
    || !isCanonicalNonNilUuid(lowerFaceId)
    || !isCanonicalNonNilUuid(upperFaceId)
    || lowerFaceId === upperFaceId
  ) return null
  const lower = faces.get(lowerFaceId)
  const upper = faces.get(upperFaceId)
  if (!lower || !upper) return null
  const projection = exactRecord(record.projection, [
    'droppedWorldAxis',
    'planeAxes',
    'roundedBoundaryUvMm',
    'exactBoundaryUv',
  ])
  if (!projection) return null
  const axes = planeAxes(projection.droppedWorldAxis, projection.planeAxes)
  if (
    !axes
    || projection.droppedWorldAxis !== lower.projection.droppedWorldAxis
    || projection.droppedWorldAxis !== upper.projection.droppedWorldAxis
  ) return null
  const rounded = denseArray(
    projection.roundedBoundaryUvMm,
    MAX_NON_FLAT_VIEW_POLYGON_POINTS_V1,
  )
  const exactPoints = denseArray(
    projection.exactBoundaryUv,
    MAX_NON_FLAT_VIEW_POLYGON_POINTS_V1,
  )
  if (
    !rounded
    || !exactPoints
    || rounded.length !== exactPoints.length
    || rounded.length < 3
  ) return null
  const detachedRounded: (readonly [number, number])[] = []
  for (const point of rounded) {
    const coordinates = denseArray(point, 2)
    if (!coordinates || coordinates.length !== 2) return null
    const u = finiteNumber(coordinates[0])
    const v = finiteNumber(coordinates[1])
    if (u === null || v === null) return null
    detachedRounded.push(Object.freeze([u, v] as [number, number]))
  }
  const detachedExact: ExactPointV1[] = []
  for (const point of exactPoints) {
    const pair = exactRecord(point, ['u', 'v'])
    if (!pair) return null
    const u = exactRational(pair.u, budget)
    const v = exactRational(pair.v, budget)
    if (!u || !v) return null
    detachedExact.push(Object.freeze({ u, v }))
  }
  return Object.freeze({
    cellKeySha256,
    exactBoundarySha256,
    lowerFaceId,
    upperFaceId,
    projection: Object.freeze({
      droppedWorldAxis: projection.droppedWorldAxis as string,
      planeAxes: axes,
      roundedBoundaryUvMm: Object.freeze(detachedRounded),
      exactBoundaryUv: Object.freeze(detachedExact),
    }),
  })
}

function pose(value: unknown): NonFlatViewPoseV1 | null {
  const record = exactRecord(value, [
    'modelId',
    'generation',
    'fixedFaceId',
    'hingeAngles',
  ])
  if (!record) return null
  const generation = record.generation
  const modelId = record.modelId
  if (
    (modelId !== CURRENT_NON_FLAT_LAYER_ORDER_TREE_POSE_MODEL_ID_V1
      && modelId !== CURRENT_NON_FLAT_LAYER_ORDER_GRAPH_POSE_MODEL_ID_V1)
    || typeof generation !== 'string'
    || !CANONICAL_GENERATION.test(generation)
    || !isCanonicalNonNilUuid(record.fixedFaceId)
  ) return null
  let parsed: bigint
  try {
    parsed = BigInt(generation)
  } catch {
    return null
  }
  if (parsed < 1n || parsed > MAX_U64) return null
  const angles = denseArray(record.hingeAngles, MAX_NON_FLAT_VIEW_HINGES_V1)
  if (!angles || angles.length === 0) return null
  const detached: NonFlatViewHingeAngleV1[] = []
  let previous: string | null = null
  let nonFlat = false
  for (const angle of angles) {
    const entry = exactRecord(angle, ['edgeId', 'angleDegrees'])
    if (!entry) return null
    const edgeId = entry.edgeId
    const angleDegrees = finiteNumber(entry.angleDegrees)
    if (
      !isCanonicalNonNilUuid(edgeId)
      || angleDegrees === null
      || angleDegrees < 0
      || angleDegrees > 180
      || (previous !== null && previous >= edgeId)
    ) return null
    if (angleDegrees !== 0 && angleDegrees !== 180) nonFlat = true
    previous = edgeId
    detached.push(Object.freeze({ edgeId, angleDegrees }))
  }
  if (!nonFlat) return null
  return Object.freeze({
    modelId,
    generation,
    fixedFaceId: record.fixedFaceId as string,
    hingeAngles: Object.freeze(detached),
  })
}

function work(value: unknown): NonFlatViewWorkV1 | null {
  const record = exactRecord(value, [
    'testedFacePairs',
    'materialFaceCount',
    'sourceOverlapCellsAuthenticated',
    'overlapCellCount',
    'facePairOrderCount',
    'worldBoundaryPointCount',
    'exactBoundaryPointCount',
  ])
  if (!record) return null
  const testedFacePairs = safeCount(record.testedFacePairs, Number.MAX_SAFE_INTEGER)
  const materialFaceCount = safeCount(record.materialFaceCount, MAX_NON_FLAT_VIEW_FACES_V1)
  const sourceOverlapCellsAuthenticated = safeCount(
    record.sourceOverlapCellsAuthenticated,
    Number.MAX_SAFE_INTEGER,
  )
  const overlapCellCount = safeCount(record.overlapCellCount, MAX_NON_FLAT_VIEW_CELLS_V1)
  const facePairOrderCount = safeCount(record.facePairOrderCount, MAX_NON_FLAT_VIEW_CELLS_V1)
  const worldBoundaryPointCount = safeCount(
    record.worldBoundaryPointCount,
    MAX_NON_FLAT_VIEW_WORLD_POINTS_V1,
  )
  const exactBoundaryPointCount = safeCount(
    record.exactBoundaryPointCount,
    MAX_NON_FLAT_VIEW_EXACT_POINTS_V1,
  )
  if (
    testedFacePairs === null
    || materialFaceCount === null
    || sourceOverlapCellsAuthenticated === null
    || overlapCellCount === null
    || facePairOrderCount === null
    || worldBoundaryPointCount === null
    || exactBoundaryPointCount === null
  ) return null
  return Object.freeze({
    testedFacePairs,
    materialFaceCount,
    sourceOverlapCellsAuthenticated,
    overlapCellCount,
    facePairOrderCount,
    worldBoundaryPointCount,
    exactBoundaryPointCount,
  })
}

/**
 * Detaches an untrusted viewer response into the exact V1 contract.
 *
 * Every failure is `null`; a partially trusted value is never returned. The
 * result is a fresh, deeply frozen copy, so later mutation of the input cannot
 * change it.
 */
export function normalizeCurrentNonFlatLayerOrderViewV1(
  value: unknown,
): CurrentNonFlatLayerOrderViewV1 | null {
  const root = exactRecord(value, [
    'version',
    'modelId',
    'projectInstanceId',
    'projectId',
    'revision',
    'foldModelFingerprintSha256',
    'pose',
    'faces',
    'cells',
    'work',
    'readOnly',
    'authorizesProjectMutation',
  ])
  if (!root) return null
  const revision = safeCount(root.revision, Number.MAX_SAFE_INTEGER)
  const fingerprint = sha256Hex(root.foldModelFingerprintSha256)
  if (
    root.version !== 1
    || root.modelId !== CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1
    || !isCanonicalNonNilUuid(root.projectInstanceId)
    || !isCanonicalNonNilUuid(root.projectId)
    || revision === null
    || fingerprint === null
    || root.readOnly !== true
    || root.authorizesProjectMutation !== false
  ) return null
  const parsedPose = pose(root.pose)
  const parsedWork = work(root.work)
  if (!parsedPose || !parsedWork) return null
  const budget = { bytes: 0 }
  const rawFaces = denseArray(root.faces, MAX_NON_FLAT_VIEW_FACES_V1)
  if (!rawFaces || rawFaces.length === 0) return null
  const detachedFaces: NonFlatViewFaceV1[] = []
  const byId = new Map<string, NonFlatViewFaceV1>()
  const faceKeys = new Set<string>()
  let previousFaceId: string | null = null
  let worldPoints = 0
  for (const raw of rawFaces) {
    const parsed = face(raw, budget)
    if (
      !parsed
      || (previousFaceId !== null && previousFaceId >= parsed.faceId)
      || faceKeys.has(parsed.faceKeySha256)
    ) return null
    faceKeys.add(parsed.faceKeySha256)
    worldPoints += parsed.worldOuterBoundaryXyzMm.length
    if (
      !Number.isSafeInteger(worldPoints)
      || worldPoints > MAX_NON_FLAT_VIEW_WORLD_POINTS_V1
    ) return null
    previousFaceId = parsed.faceId
    byId.set(parsed.faceId, parsed)
    detachedFaces.push(parsed)
  }
  const rawCells = denseArray(root.cells, MAX_NON_FLAT_VIEW_CELLS_V1)
  if (!rawCells) return null
  const detachedCells: NonFlatViewCellV1[] = []
  let previousCellKey: string | null = null
  let exactPoints = 0
  for (const raw of rawCells) {
    const parsed = cell(raw, byId, budget)
    if (
      !parsed
      || (previousCellKey !== null && previousCellKey >= parsed.cellKeySha256)
    ) return null
    exactPoints += parsed.projection.exactBoundaryUv.length
    if (
      !Number.isSafeInteger(exactPoints)
      || exactPoints > MAX_NON_FLAT_VIEW_EXACT_POINTS_V1
    ) return null
    previousCellKey = parsed.cellKeySha256
    detachedCells.push(parsed)
  }
  if (
    parsedWork.materialFaceCount !== detachedFaces.length
    || parsedWork.overlapCellCount !== detachedCells.length
    || parsedWork.facePairOrderCount !== detachedCells.length
    || parsedWork.worldBoundaryPointCount !== worldPoints
    || parsedWork.exactBoundaryPointCount !== exactPoints
  ) return null
  return Object.freeze({
    version: 1,
    modelId: CURRENT_NON_FLAT_LAYER_ORDER_VIEW_MODEL_ID_V1,
    projectInstanceId: root.projectInstanceId as string,
    projectId: root.projectId as string,
    revision,
    foldModelFingerprintSha256: fingerprint,
    pose: parsedPose,
    faces: Object.freeze(detachedFaces),
    cells: Object.freeze(detachedCells),
    work: parsedWork,
    readOnly: true,
    authorizesProjectMutation: false,
  })
}

export class CurrentNonFlatLayerOrderViewError extends Error {
  readonly kind: CurrentNonFlatLayerOrderViewErrorKindV1

  constructor(kind: CurrentNonFlatLayerOrderViewErrorKindV1) {
    super(kind)
    this.name = 'CurrentNonFlatLayerOrderViewError'
    this.kind = kind
  }
}

/** Maps a native payload to a closed frontend kind without interpolating data. */
function errorKind(value: unknown): CurrentNonFlatLayerOrderViewErrorKindV1 {
  if (typeof value === 'object' && value !== null) {
    const category = ownData(value, 'category')
    if (
      category === 'stale_authority'
      || category === 'invalid_evidence'
      || category === 'resource_limit'
      || category === 'internal_failure'
    ) return category
  }
  return 'internal_failure'
}

export type CurrentNonFlatLayerOrderViewRequestSource = Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprintSha256: string
  appliedPose: Readonly<{
    state: string
    projectId: string
    revision: number
    fixedFaceId: string | null
    hingeAngles: readonly Readonly<{ edgeId: string; angleDegrees: number }>[]
  }>
}>

type DetachedCurrentNonFlatLayerOrderViewRequestSource = Readonly<{
  projectInstanceId: string
  projectId: string
  revision: number
  foldModelFingerprintSha256: string
  fixedFaceId: string
  hingeAngles: readonly Readonly<{ edgeId: string; angleDegrees: number }>[]
}>

/**
 * Detaches and validates the complete invoke authority without reading a
 * getter, array index accessor, or Proxy `get` trap.
 */
function detachCurrentNonFlatLayerOrderViewRequestSource(
  value: unknown,
): DetachedCurrentNonFlatLayerOrderViewRequestSource | null {
  const source = exactRecord(value, [
    'projectInstanceId',
    'projectId',
    'revision',
    'foldModelFingerprintSha256',
    'appliedPose',
  ])
  if (!source) return null
  const pose = exactRecord(source.appliedPose, [
    'projectId',
    'revision',
    'fixedFaceId',
    'hingeAngles',
    'state',
  ])
  if (!pose || pose.state !== 'stable') return null

  const revision = safeCount(source.revision, Number.MAX_SAFE_INTEGER)
  const poseRevision = safeCount(pose.revision, Number.MAX_SAFE_INTEGER)
  const fingerprint = sha256Hex(source.foldModelFingerprintSha256)
  if (
    !isCanonicalNonNilUuid(source.projectInstanceId)
    || !isCanonicalNonNilUuid(source.projectId)
    || !isCanonicalNonNilUuid(pose.projectId)
    || !isCanonicalNonNilUuid(pose.fixedFaceId)
    || revision === null
    || poseRevision === null
    || pose.projectId !== source.projectId
    || poseRevision !== revision
    || fingerprint === null
  ) return null

  const rawAngles = denseArray(pose.hingeAngles, MAX_NON_FLAT_VIEW_HINGES_V1)
  if (!rawAngles || rawAngles.length === 0) return null
  const hingeAngles: { edgeId: string; angleDegrees: number }[] = []
  let hasNonFlatAngle = false
  for (const rawAngle of rawAngles) {
    const angle = exactRecord(rawAngle, ['edgeId', 'angleDegrees'])
    if (!angle || !isCanonicalNonNilUuid(angle.edgeId)) return null
    const angleDegrees = finiteNumber(angle.angleDegrees)
    if (angleDegrees === null || angleDegrees < 0 || angleDegrees > 180) {
      return null
    }
    if (angleDegrees !== 0 && angleDegrees !== 180) hasNonFlatAngle = true
    hingeAngles.push({ edgeId: angle.edgeId, angleDegrees })
  }
  if (!hasNonFlatAngle) return null
  hingeAngles.sort((left, right) => {
    if (left.edgeId < right.edgeId) return -1
    if (left.edgeId > right.edgeId) return 1
    return 0
  })
  if (
    hingeAngles.some((angle, index) =>
      index > 0 && hingeAngles[index - 1]?.edgeId === angle.edgeId)
  ) return null

  return Object.freeze({
    projectInstanceId: source.projectInstanceId,
    projectId: source.projectId,
    revision,
    foldModelFingerprintSha256: fingerprint,
    fixedFaceId: pose.fixedFaceId,
    hingeAngles: Object.freeze(hingeAngles.map((angle) => Object.freeze(angle))),
  })
}

/**
 * Invokes the read-only viewer for the applied non-flat layer order.
 *
 * `null` means the project genuinely owns no non-flat evidence. The request is
 * only sent when the applied pose is stable and bound to the same project and
 * revision as the snapshot.
 */
export async function getCurrentNonFlatLayerOrderViewV1(
  source: CurrentNonFlatLayerOrderViewRequestSource,
): Promise<CurrentNonFlatLayerOrderViewV1 | null> {
  const detached = detachCurrentNonFlatLayerOrderViewRequestSource(source)
  if (!detached) return null
  const hingeAngles = detached.hingeAngles
  const request = {
    version: 1,
    expectedProjectInstanceId: detached.projectInstanceId,
    expectedProjectId: detached.projectId,
    expectedRevision: detached.revision,
    expectedFoldModelFingerprintSha256: detached.foldModelFingerprintSha256,
    expectedAppliedPose: {
      fixedFaceId: detached.fixedFaceId,
      hingeAngles,
    },
  }
  let raw: unknown
  try {
    raw = await invoke('get_current_non_flat_layer_order_view_v1', { request })
  } catch (error) {
    throw new CurrentNonFlatLayerOrderViewError(errorKind(error))
  }
  if (raw === null) return null
  const parsed = normalizeCurrentNonFlatLayerOrderViewV1(raw)
  if (
    !parsed
    || parsed.projectInstanceId !== request.expectedProjectInstanceId
    || parsed.projectId !== request.expectedProjectId
    || parsed.revision !== request.expectedRevision
    || parsed.foldModelFingerprintSha256
      !== request.expectedFoldModelFingerprintSha256
    || parsed.pose.fixedFaceId !== request.expectedAppliedPose.fixedFaceId
    || parsed.pose.hingeAngles.length !== hingeAngles.length
    || parsed.pose.hingeAngles.some((angle, index) => {
      const requested = hingeAngles[index]
      return requested === undefined
        || angle.edgeId !== requested.edgeId
        || !Object.is(angle.angleDegrees, requested.angleDegrees)
    })
  ) throw new CurrentNonFlatLayerOrderViewError('invalid_evidence')
  return parsed
}
