import { Vector3, type Matrix4 } from 'three'

export const MAX_FOLD_PREVIEW_COLLISION_FACES = 10_000
export const MAX_FOLD_PREVIEW_BROAD_PHASE_CANDIDATES = 100_000

const NUMERICAL_MARGIN_FACTOR = 64

export type FoldPreviewCollisionPoint = Readonly<{
  x: number
  z: number
}>

export type FoldPreviewCollisionFace = Readonly<{
  faceId: string
  polygon: readonly FoldPreviewCollisionPoint[]
  /** Transforms the common flat rest frame into the current folded pose. */
  transform: Matrix4
  /** Full paper thickness in preview world units. */
  thickness: number
}>

export type FoldPreviewCollisionAdjacency = Readonly<{
  edgeId: string
  firstFaceId: string
  secondFaceId: string
}>

export type FoldPreviewFaceBounds = Readonly<{
  faceId: string
  minX: number
  minY: number
  minZ: number
  maxX: number
  maxY: number
  maxZ: number
}>

export type FoldPreviewBroadPhaseCandidate = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  /** All hinge IDs shared by this face pair, in deterministic lexical order. */
  hingeEdgeIds: readonly string[]
  /** Conservative AABB overlap. Zero on any axis means contact within tolerance. */
  overlap: Readonly<{ x: number; y: number; z: number }>
  touching: boolean
}>

export type FoldPreviewBroadPhaseResult = Readonly<{
  /** Bounds are sorted by face ID and never expose the sweep ordering. */
  bounds: readonly FoldPreviewFaceBounds[]
  candidates: readonly FoldPreviewBroadPhaseCandidate[]
  /** Margin used to avoid dropping contacts solely through floating-point noise. */
  numericalMargin: number
}>

/**
 * Builds world-space face-prism AABBs and finds potentially contacting pairs.
 *
 * This is deliberately only a broad phase: a returned pair is not proof of a
 * collision. Hinge neighbours remain in the result because excluding the whole
 * pair could hide penetration away from the legal hinge contact.
 */
export function findFoldPreviewBroadPhaseCandidates(
  faces: readonly FoldPreviewCollisionFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[] = [],
): FoldPreviewBroadPhaseResult | null {
  if (
    !Array.isArray(faces)
    || !Array.isArray(adjacencies)
    || faces.length > MAX_FOLD_PREVIEW_COLLISION_FACES
  ) return null

  const bounds: FoldPreviewFaceBounds[] = []
  const faceIds = new Set<string>()
  let coordinateScale = 1
  for (const face of faces) {
    if (
      !face
      || !validId(face.faceId)
      || faceIds.has(face.faceId)
      || !Array.isArray(face.polygon)
      || face.polygon.length < 3
      || !Number.isFinite(face.thickness)
      || face.thickness < 0
      || !finiteAffineMatrix(face.transform)
    ) return null
    faceIds.add(face.faceId)

    const faceBounds = transformedPrismBounds(face)
    if (!faceBounds) return null
    bounds.push(faceBounds)
    coordinateScale = Math.max(
      coordinateScale,
      Math.abs(faceBounds.minX),
      Math.abs(faceBounds.minY),
      Math.abs(faceBounds.minZ),
      Math.abs(faceBounds.maxX),
      Math.abs(faceBounds.maxY),
      Math.abs(faceBounds.maxZ),
    )
  }
  if (!Number.isFinite(coordinateScale)) return null

  const adjacencyByPair = indexAdjacencies(adjacencies, faceIds)
  if (!adjacencyByPair) return null
  const numericalMargin = coordinateScale * Number.EPSILON * NUMERICAL_MARGIN_FACTOR
  if (!Number.isFinite(numericalMargin)) return null

  const sweepBounds = [...bounds].sort(compareSweepBounds)
  const candidates: FoldPreviewBroadPhaseCandidate[] = []
  for (let firstIndex = 0; firstIndex < sweepBounds.length; firstIndex += 1) {
    const first = sweepBounds[firstIndex]
    for (let secondIndex = firstIndex + 1; secondIndex < sweepBounds.length; secondIndex += 1) {
      const second = sweepBounds[secondIndex]
      if (second.minX - first.maxX > numericalMargin) break
      if (
        separated(first.minY, first.maxY, second.minY, second.maxY, numericalMargin)
        || separated(first.minZ, first.maxZ, second.minZ, second.maxZ, numericalMargin)
      ) continue

      const [firstFaceId, secondFaceId] = orderedPair(first.faceId, second.faceId)
      const hingeEdgeIds = adjacencyByPair.get(firstFaceId)?.get(secondFaceId) ?? []
      const overlap = {
        x: overlapLength(first.minX, first.maxX, second.minX, second.maxX),
        y: overlapLength(first.minY, first.maxY, second.minY, second.maxY),
        z: overlapLength(first.minZ, first.maxZ, second.minZ, second.maxZ),
      }
      candidates.push({
        firstFaceId,
        secondFaceId,
        relation: hingeEdgeIds.length > 0 ? 'hinge_adjacent' : 'non_adjacent',
        hingeEdgeIds,
        overlap,
        touching:
          overlap.x <= numericalMargin
          || overlap.y <= numericalMargin
          || overlap.z <= numericalMargin,
      })
      if (candidates.length > MAX_FOLD_PREVIEW_BROAD_PHASE_CANDIDATES) return null
    }
  }

  candidates.sort(compareCandidates)
  bounds.sort((first, second) => first.faceId.localeCompare(second.faceId))
  return { bounds, candidates, numericalMargin }
}

function transformedPrismBounds(
  face: FoldPreviewCollisionFace,
): FoldPreviewFaceBounds | null {
  const halfThickness = face.thickness / 2
  const bounds = {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    minZ: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
    maxZ: Number.NEGATIVE_INFINITY,
  }
  const transformed = new Vector3()
  for (const point of face.polygon) {
    if (!point || !Number.isFinite(point.x) || !Number.isFinite(point.z)) return null
    for (const y of halfThickness === 0 ? [0] : [-halfThickness, halfThickness]) {
      transformed.set(point.x, y, point.z).applyMatrix4(face.transform)
      if (![transformed.x, transformed.y, transformed.z].every(Number.isFinite)) return null
      bounds.minX = Math.min(bounds.minX, transformed.x)
      bounds.minY = Math.min(bounds.minY, transformed.y)
      bounds.minZ = Math.min(bounds.minZ, transformed.z)
      bounds.maxX = Math.max(bounds.maxX, transformed.x)
      bounds.maxY = Math.max(bounds.maxY, transformed.y)
      bounds.maxZ = Math.max(bounds.maxZ, transformed.z)
    }
  }
  return Object.values(bounds).every(Number.isFinite)
    ? { faceId: face.faceId, ...bounds }
    : null
}

function indexAdjacencies(
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  faceIds: ReadonlySet<string>,
) {
  const edgeIds = new Set<string>()
  const mutableIndex = new Map<string, Map<string, string[]>>()
  for (const adjacency of adjacencies) {
    if (
      !adjacency
      || !validId(adjacency.edgeId)
      || edgeIds.has(adjacency.edgeId)
      || !faceIds.has(adjacency.firstFaceId)
      || !faceIds.has(adjacency.secondFaceId)
      || adjacency.firstFaceId === adjacency.secondFaceId
    ) return null
    edgeIds.add(adjacency.edgeId)
    const [firstFaceId, secondFaceId] = orderedPair(
      adjacency.firstFaceId,
      adjacency.secondFaceId,
    )
    const bySecond = mutableIndex.get(firstFaceId) ?? new Map<string, string[]>()
    const edges = bySecond.get(secondFaceId) ?? []
    edges.push(adjacency.edgeId)
    bySecond.set(secondFaceId, edges)
    mutableIndex.set(firstFaceId, bySecond)
  }
  for (const bySecond of mutableIndex.values()) {
    for (const edges of bySecond.values()) edges.sort((first, second) => first.localeCompare(second))
  }
  return mutableIndex
}

function finiteAffineMatrix(transform: Matrix4) {
  if (
    !transform
    || !Array.isArray(transform.elements)
    || transform.elements.length !== 16
    || !transform.elements.every(Number.isFinite)
  ) return false
  const scale = Math.max(1, ...transform.elements.map(Math.abs))
  const tolerance = scale * Number.EPSILON * NUMERICAL_MARGIN_FACTOR
  return Math.abs(transform.elements[3]) <= tolerance
    && Math.abs(transform.elements[7]) <= tolerance
    && Math.abs(transform.elements[11]) <= tolerance
    && Math.abs(transform.elements[15] - 1) <= tolerance
}

function separated(
  firstMin: number,
  firstMax: number,
  secondMin: number,
  secondMax: number,
  margin: number,
) {
  return secondMin - firstMax > margin || firstMin - secondMax > margin
}

function overlapLength(
  firstMin: number,
  firstMax: number,
  secondMin: number,
  secondMax: number,
) {
  return Math.max(0, Math.min(firstMax, secondMax) - Math.max(firstMin, secondMin))
}

function compareSweepBounds(first: FoldPreviewFaceBounds, second: FoldPreviewFaceBounds) {
  return first.minX - second.minX
    || first.maxX - second.maxX
    || first.faceId.localeCompare(second.faceId)
}

function compareCandidates(
  first: FoldPreviewBroadPhaseCandidate,
  second: FoldPreviewBroadPhaseCandidate,
) {
  return first.firstFaceId.localeCompare(second.firstFaceId)
    || first.secondFaceId.localeCompare(second.secondFaceId)
}

function orderedPair(first: string, second: string): readonly [string, string] {
  return first.localeCompare(second) <= 0 ? [first, second] : [second, first]
}

function validId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}
