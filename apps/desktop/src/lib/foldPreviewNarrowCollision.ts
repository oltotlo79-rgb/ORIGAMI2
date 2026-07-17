import { Vector3, type Matrix4 } from 'three'
import {
  findFoldPreviewPoseBroadPhaseCandidates,
  type FoldPreviewCollisionAdjacency,
  type FoldPreviewCollisionPoseFace,
} from './foldPreviewCollision.ts'
import { triangulateFoldPreviewPolygon } from './foldPreviewGeometry.ts'

export const MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS = 1_000_000

const SAT_MARGIN_FACTOR = 4
const PARALLEL_AXIS_TOLERANCE = Number.EPSILON * 128
const RIGID_TRANSFORM_TOLERANCE = 1e-10

export type FoldPreviewNarrowPhaseInteraction = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  hingeEdgeIds: readonly string[]
  geometryClass: 'touching' | 'penetrating' | 'indeterminate'
}>

export type FoldPreviewNarrowPhaseResult = Readonly<{
  broadPhaseCandidates: number
  interactions: readonly FoldPreviewNarrowPhaseInteraction[]
  trianglePairTests: number
  satTests: number
  numericalMargin: number
}>

type TrianglePrism = Readonly<{
  vertices: readonly Vector3[]
  faceAxes: readonly Vector3[]
  edgeDirections: readonly Vector3[]
  bounds: Readonly<{
    minX: number
    minY: number
    minZ: number
    maxX: number
    maxY: number
    maxZ: number
  }>
}>

type PrismIntersection = 'separated' | 'touching' | 'penetrating' | 'indeterminate'

/**
 * Refines conservative face AABBs with SAT tests between triangulated paper
 * prisms. The output is still geometric, not an origami legality decision:
 * shared-hinge interactions remain explicitly tagged for the contact-policy
 * layer instead of being silently accepted or rejected here. This evaluates
 * one immutable pose only; continuous collision detection is a later stage.
 */
export function findFoldPreviewNarrowPhaseInteractions(
  faces: readonly FoldPreviewCollisionPoseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
): FoldPreviewNarrowPhaseResult | null {
  const broadPhase = findFoldPreviewPoseBroadPhaseCandidates(
    faces,
    faceTransforms,
    thickness,
    adjacencies,
  )
  if (!broadPhase) return null
  for (const face of faces) {
    const transform = faceTransforms.get(face.id)
    if (!transform || !rigidTransform(transform)) return null
  }

  const facesById = new Map(faces.map((face) => [face.id, face]))
  if (facesById.size !== faces.length) return null
  const numericalMargin = broadPhase.numericalMargin * SAT_MARGIN_FACTOR
  if (!Number.isFinite(numericalMargin)) return null
  if (thickness === 0) {
    return {
      broadPhaseCandidates: broadPhase.candidates.length,
      interactions: broadPhase.candidates.map((candidate) => ({
        firstFaceId: candidate.firstFaceId,
        secondFaceId: candidate.secondFaceId,
        relation: candidate.relation,
        hingeEdgeIds: candidate.hingeEdgeIds,
        geometryClass: 'indeterminate',
      })),
      trianglePairTests: 0,
      satTests: 0,
      numericalMargin,
    }
  }

  const prismCache = new Map<string, readonly TrianglePrism[]>()
  let trianglePairTests = 0
  let satTests = 0
  const interactions: FoldPreviewNarrowPhaseInteraction[] = []

  try {
    const prismsForFace = (faceId: string) => {
      const cached = prismCache.get(faceId)
      if (cached) return cached
      const face = facesById.get(faceId)
      const transform = faceTransforms.get(faceId)
      if (!face || !transform) return null
      const prisms = buildTrianglePrisms(face, transform, thickness)
      if (!prisms) return null
      prismCache.set(faceId, prisms)
      return prisms
    }

    for (const candidate of broadPhase.candidates) {
      const firstPrisms = prismsForFace(candidate.firstFaceId)
      const secondPrisms = prismsForFace(candidate.secondFaceId)
      if (!firstPrisms || !secondPrisms) return null

      let geometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass'] | null = null
      pairSearch:
      for (const first of firstPrisms) {
        for (const second of secondPrisms) {
          trianglePairTests += 1
          if (trianglePairTests > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS) return null
          if (!boundsOverlap(first.bounds, second.bounds, numericalMargin)) continue
          satTests += 1
          const intersection = classifyTrianglePrisms(first, second, numericalMargin)
          if (!intersection) return null
          if (intersection === 'penetrating') {
            geometryClass = 'penetrating'
            break pairSearch
          }
          if (
            intersection === 'indeterminate'
            && geometryClass !== 'indeterminate'
          ) {
            geometryClass = 'indeterminate'
          } else if (intersection === 'touching' && !geometryClass) {
            geometryClass = 'touching'
          }
        }
      }
      if (geometryClass) {
        interactions.push({
          firstFaceId: candidate.firstFaceId,
          secondFaceId: candidate.secondFaceId,
          relation: candidate.relation,
          hingeEdgeIds: candidate.hingeEdgeIds,
          geometryClass,
        })
      }
    }
  } catch {
    return null
  }

  return {
    broadPhaseCandidates: broadPhase.candidates.length,
    interactions,
    trianglePairTests,
    satTests,
    numericalMargin,
  }
}

function buildTrianglePrisms(
  face: FoldPreviewCollisionPoseFace,
  transform: Matrix4,
  thickness: number,
): readonly TrianglePrism[] | null {
  const triangles = triangulateFoldPreviewPolygon(face.polygon)
  const halfThickness = thickness / 2
  if (!Number.isFinite(halfThickness) || halfThickness < 0) return null
  const prisms: TrianglePrism[] = []
  for (const triangle of triangles) {
    const top = triangle.map((index) => transformedPoint(
      face.polygon[index].x,
      halfThickness,
      face.polygon[index].z,
      transform,
    ))
    const bottom = triangle.map((index) => transformedPoint(
      face.polygon[index].x,
      -halfThickness,
      face.polygon[index].z,
      transform,
    ))
    if ([...top, ...bottom].some((point) => !point)) return null
    const vertices = [...top, ...bottom] as Vector3[]
    const firstEdge = vertices[1].clone().sub(vertices[0])
    const secondEdge = vertices[2].clone().sub(vertices[1])
    const thirdEdge = vertices[0].clone().sub(vertices[2])
    const baseNormal = normalized(firstEdge.clone().cross(
      vertices[2].clone().sub(vertices[0]),
    ))
    if (!baseNormal) return null
    const extrusion = vertices[3].clone().sub(vertices[0])
    const extrusionDirection = normalized(extrusion)
    if (thickness > 0 && !extrusionDirection) return null

    const baseEdges = [firstEdge, secondEdge, thirdEdge]
      .map(normalized)
    if (baseEdges.some((edge) => !edge)) return null
    const edgeDirections = baseEdges as Vector3[]
    if (extrusionDirection) edgeDirections.push(extrusionDirection)

    const faceAxes = [baseNormal]
    for (const edge of edgeDirections.slice(0, 3)) {
      const sideAxis = normalized(thickness > 0
        ? edge.clone().cross(extrusion)
        : edge.clone().cross(baseNormal))
      if (!sideAxis) return null
      faceAxes.push(sideAxis)
    }
    const bounds = boundsForVertices(vertices)
    if (!bounds) return null
    prisms.push({ vertices, faceAxes, edgeDirections, bounds })
  }
  return prisms.length === triangles.length && prisms.length > 0 ? prisms : null
}

function classifyTrianglePrisms(
  first: TrianglePrism,
  second: TrianglePrism,
  margin: number,
): PrismIntersection | null {
  const axes = [...first.faceAxes, ...second.faceAxes]
  let uncertainAxis = false
  for (const firstEdge of first.edgeDirections) {
    for (const secondEdge of second.edgeDirections) {
      const cross = firstEdge.clone().cross(secondEdge)
      const length = cross.length()
      if (!Number.isFinite(length)) return null
      if (length === 0) continue
      if (length <= PARALLEL_AXIS_TOLERANCE) {
        uncertainAxis = true
        continue
      }
      axes.push(cross.multiplyScalar(1 / length))
    }
  }
  if (axes.length === 0) return null

  let boundaryContact = false
  for (const axis of axes) {
    const firstProjection = projectVertices(first.vertices, axis)
    const secondProjection = projectVertices(second.vertices, axis)
    if (!firstProjection || !secondProjection) return null
    const gap = Math.max(
      secondProjection.min - firstProjection.max,
      firstProjection.min - secondProjection.max,
    )
    if (gap > margin) return 'separated'
    const overlap = Math.min(firstProjection.max, secondProjection.max)
      - Math.max(firstProjection.min, secondProjection.min)
    if (!Number.isFinite(gap) || !Number.isFinite(overlap)) return null
    if (overlap <= margin) boundaryContact = true
  }
  if (uncertainAxis) return 'indeterminate'
  return boundaryContact ? 'touching' : 'penetrating'
}

function boundsForVertices(vertices: readonly Vector3[]) {
  const bounds = {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    minZ: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
    maxZ: Number.NEGATIVE_INFINITY,
  }
  for (const vertex of vertices) {
    bounds.minX = Math.min(bounds.minX, vertex.x)
    bounds.minY = Math.min(bounds.minY, vertex.y)
    bounds.minZ = Math.min(bounds.minZ, vertex.z)
    bounds.maxX = Math.max(bounds.maxX, vertex.x)
    bounds.maxY = Math.max(bounds.maxY, vertex.y)
    bounds.maxZ = Math.max(bounds.maxZ, vertex.z)
  }
  return Object.values(bounds).every(Number.isFinite) ? bounds : null
}

function boundsOverlap(
  first: TrianglePrism['bounds'],
  second: TrianglePrism['bounds'],
  margin: number,
) {
  return second.minX - first.maxX <= margin
    && first.minX - second.maxX <= margin
    && second.minY - first.maxY <= margin
    && first.minY - second.maxY <= margin
    && second.minZ - first.maxZ <= margin
    && first.minZ - second.maxZ <= margin
}

function projectVertices(vertices: readonly Vector3[], axis: Vector3) {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const vertex of vertices) {
    const projection = vertex.dot(axis)
    if (!Number.isFinite(projection)) return null
    min = Math.min(min, projection)
    max = Math.max(max, projection)
  }
  return Number.isFinite(min) && Number.isFinite(max) ? { min, max } : null
}

function transformedPoint(x: number, y: number, z: number, transform: Matrix4) {
  const point = new Vector3(x, y, z).applyMatrix4(transform)
  return [point.x, point.y, point.z].every(Number.isFinite) ? point : null
}

function normalized(vector: Vector3) {
  const length = vector.length()
  return Number.isFinite(length) && length > 0
    ? vector.multiplyScalar(1 / length)
    : null
}

function rigidTransform(transform: Matrix4) {
  const elements = transform.elements
  if (
    !Array.isArray(elements)
    || elements.length !== 16
    || !elements.every(Number.isFinite)
  ) return false
  const first = new Vector3(elements[0], elements[1], elements[2])
  const second = new Vector3(elements[4], elements[5], elements[6])
  const third = new Vector3(elements[8], elements[9], elements[10])
  const determinant = first.dot(second.clone().cross(third))
  return Math.abs(first.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(second.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(third.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(first.dot(second)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(first.dot(third)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(second.dot(third)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(determinant - 1) <= RIGID_TRANSFORM_TOLERANCE
}
