import { Vector3, type Matrix4 } from 'three'
import {
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
  calculateFoldPreviewBroadPhaseNumericalMargin,
  findFoldPreviewPoseBroadPhaseCandidates,
  type FoldPreviewBroadPhaseResult,
  type FoldPreviewCollisionAdjacency,
  type FoldPreviewCollisionPoseFace,
} from './foldPreviewCollision.ts'
import {
  triangulateFoldPreviewPolygon,
  type FoldPreviewTriangleIndices,
} from './foldPreviewGeometry.ts'
import {
  prepareFoldPreviewHingeContactPolicy,
  type FoldPreviewHingeContactConstraint,
  type FoldPreviewHingeContactDecision,
  type FoldPreviewHingeContactPair,
  type FoldPreviewHingeContactPolicy,
} from './foldPreviewHingeCollision.ts'
import {
  deriveFoldPreviewTrianglePrismWitness,
  type FoldPreviewTrianglePrismWitness,
  type FoldPreviewWitnessFrame,
} from './foldPreviewNarrowCollisionWitness.ts'

export const MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS = 1_000_000
/** Bounds synchronous deep-copy and triangulation during preview setup. */
export const MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES = 100_000
/** Bounds explanatory derivation work independently of collision classification. */
export const MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES = 16

const SAT_MARGIN_FACTOR = 4
const PARALLEL_AXIS_TOLERANCE = Number.EPSILON * 128
const RIGID_TRANSFORM_TOLERANCE = 1e-10

/**
 * Returns the exact SAT/hinge-policy margin for an upper bound on all absolute
 * world coordinates in a pose.
 */
export function calculateFoldPreviewNarrowPhaseNumericalMargin(
  coordinateScale: number,
): number | null {
  const broadPhaseMargin =
    calculateFoldPreviewBroadPhaseNumericalMargin(coordinateScale)
  if (broadPhaseMargin === null) return null
  const margin = broadPhaseMargin * SAT_MARGIN_FACTOR
  return Number.isFinite(margin) ? margin : null
}

export type FoldPreviewNarrowPhaseInteraction = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  hingeEdgeIds: readonly string[]
  geometryClass: 'touching' | 'penetrating' | 'indeterminate'
  hingeDecision?: FoldPreviewHingeContactDecision
}>

export type FoldPreviewNarrowPhaseWitnessSample = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'non_adjacent'
  firstTriangleIndex: number
  secondTriangleIndex: number
  geometryClass: 'touching' | 'penetrating'
  witness: FoldPreviewTrianglePrismWitness
}>

export type FoldPreviewNarrowPhaseWitnessCoverage = Readonly<{
  scope: 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1'
  /** Definitive tested pairs matching their final face-interaction severity. */
  eligiblePairCount: number
  /** Eligible pairs submitted to the bounded witness derivation helper. */
  attemptedPairCount: number
  /** Attempted pairs for which conservative witness derivation returned null. */
  unavailablePairCount: number
  /** Eligible pairs not submitted because the independent limit was reached. */
  omittedByLimitCount: number
  /**
   * False when an authoritative non-adjacent scan stopped at its first
   * penetration or no positive-thickness SAT scan was performed.
   */
  authoritativePairScanComplete: boolean
}>

export type FoldPreviewNarrowPhaseResult = Readonly<{
  broadPhaseCandidates: number
  broadPhaseNonAdjacentCandidates: number
  broadPhaseHingeAdjacentCandidates: number
  interactions: readonly FoldPreviewNarrowPhaseInteraction[]
  trianglePairTests: number
  satTests: number
  numericalMargin: number
  witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[]
  witnessCoverage: FoldPreviewNarrowPhaseWitnessCoverage
}>

export type FoldPreviewNarrowPhaseAnalyzer = Readonly<{
  analyze(
    faceTransforms: ReadonlyMap<string, Matrix4>,
    thickness: number,
  ): FoldPreviewNarrowPhaseResult | null
}>

type PreparedFoldPreviewNarrowPhaseFace = Readonly<{
  id: string
  polygon: FoldPreviewCollisionPoseFace['polygon']
  triangles: readonly FoldPreviewTriangleIndices[]
}>

type FoldPreviewNarrowPhaseFace = Readonly<{
  id: string
  polygon: FoldPreviewCollisionPoseFace['polygon']
  triangles?: readonly FoldPreviewTriangleIndices[]
}>

type TrianglePrism = Readonly<{
  triangleIndex: number
  vertices: readonly Vector3[]
  faceAxes: readonly Vector3[]
  edgeDirections: readonly Vector3[]
  witnessFrame: FoldPreviewWitnessFrame | null
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

type WitnessPairSeed = Readonly<{
  first: TrianglePrism
  second: TrianglePrism
}>

type EligibleWitnessPairSeed = WitnessPairSeed & Readonly<{
  firstFaceId: string
  secondFaceId: string
  geometryClass: 'touching' | 'penetrating'
}>

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
  try {
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
    return refineFoldPreviewNarrowPhase(
      faces,
      faceTransforms,
      thickness,
      broadPhase,
      null,
    )
  } catch {
    return null
  }
}

/**
 * Snapshots and triangulates pose-independent collision inputs once.
 *
 * The returned analyzer deliberately does not retain a pose or thickness:
 * every synchronous call validates one exact immutable transform map, rebuilds
 * world bounds, and reruns the broad and narrow phases.
 */
export function prepareFoldPreviewNarrowPhase(
  faces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  hingeConstraints?: readonly FoldPreviewHingeContactConstraint[],
): FoldPreviewNarrowPhaseAnalyzer | null {
  const prepared = snapshotNarrowPhaseInputs(faces, adjacencies, hingeConstraints)
  if (!prepared) return null
  const {
    preparedFaces,
    poseFaces,
    adjacencySnapshot,
    hingeContactPolicy,
  } = prepared

  return Object.freeze({
    analyze(
      faceTransforms: ReadonlyMap<string, Matrix4>,
      thickness: number,
    ): FoldPreviewNarrowPhaseResult | null {
      try {
        if (!Number.isFinite(thickness) || thickness < 0) return null
        return analyzePreparedFoldPreviewNarrowPhase(
          preparedFaces,
          poseFaces,
          adjacencySnapshot,
          faceTransforms,
          thickness,
          hingeContactPolicy,
        )
      } catch {
        return null
      }
    },
  })
}

function analyzePreparedFoldPreviewNarrowPhase(
  preparedFaces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  poseFaces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  hingeContactPolicy: FoldPreviewHingeContactPolicy | null,
): FoldPreviewNarrowPhaseResult | null {
  const broadPhase = findFoldPreviewPoseBroadPhaseCandidates(
    poseFaces,
    faceTransforms,
    thickness,
    adjacencies,
  )
  if (!broadPhase) return null
  if (!validateRigidTransforms(preparedFaces, faceTransforms)) return null
  return refineFoldPreviewNarrowPhase(
    preparedFaces,
    faceTransforms,
    thickness,
    broadPhase,
    hingeContactPolicy,
  )
}

function refineFoldPreviewNarrowPhase(
  faces: readonly FoldPreviewNarrowPhaseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  broadPhase: FoldPreviewBroadPhaseResult,
  hingeContactPolicy: FoldPreviewHingeContactPolicy | null,
): FoldPreviewNarrowPhaseResult | null {
  const facesById = new Map(faces.map((face) => [face.id, face]))
  if (facesById.size !== faces.length) return null
  const numericalMargin = broadPhase.numericalMargin * SAT_MARGIN_FACTOR
  if (!Number.isFinite(numericalMargin)) return null
  const broadPhaseHingeAdjacentCandidates = broadPhase.candidates.reduce(
    (count, candidate) => count + Number(candidate.relation === 'hinge_adjacent'),
    0,
  )
  const broadPhaseNonAdjacentCandidates = broadPhase.candidates.length
    - broadPhaseHingeAdjacentCandidates
  if (thickness === 0) {
    return {
      broadPhaseCandidates: broadPhase.candidates.length,
      broadPhaseNonAdjacentCandidates,
      broadPhaseHingeAdjacentCandidates,
      interactions: broadPhase.candidates.map((candidate) => {
        const interaction: FoldPreviewNarrowPhaseInteraction = {
          firstFaceId: candidate.firstFaceId,
          secondFaceId: candidate.secondFaceId,
          relation: candidate.relation,
          hingeEdgeIds: candidate.hingeEdgeIds,
          geometryClass: 'indeterminate',
        }
        if (candidate.relation === 'hinge_adjacent' && hingeContactPolicy) {
          return {
            ...interaction,
            hingeDecision: hingeContactPolicy.classify({
              firstFaceId: candidate.firstFaceId,
              secondFaceId: candidate.secondFaceId,
              hingeEdgeIds: candidate.hingeEdgeIds,
              faceTransforms,
              thickness,
              numericalMargin,
              testedTrianglePairs: 0,
              pairs: [],
            }),
          }
        }
        return interaction
      }),
      trianglePairTests: 0,
      satTests: 0,
      numericalMargin,
      witnessSamples: Object.freeze([]),
      witnessCoverage: freezeWitnessCoverage({
        eligiblePairCount: 0,
        attemptedPairCount: 0,
        unavailablePairCount: 0,
        omittedByLimitCount: 0,
        authoritativePairScanComplete: false,
      }),
    }
  }

  const prismCache = new Map<string, readonly TrianglePrism[]>()
  let trianglePairTests = 0
  let satTests = 0
  const interactions: FoldPreviewNarrowPhaseInteraction[] = []
  const witnessSamples: FoldPreviewNarrowPhaseWitnessSample[] = []
  const penetratingEligibleSeeds: EligibleWitnessPairSeed[] = []
  const touchingEligibleSeeds: EligibleWitnessPairSeed[] = []
  let eligibleWitnessPairCount = 0
  let attemptedWitnessPairCount = 0
  let unavailableWitnessPairCount = 0
  let performedNonAdjacentSatScan = false
  let nonAdjacentPairScansComplete = true

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
      const hingePairs: FoldPreviewHingeContactPair[] = []
      const touchingWitnessSeeds: WitnessPairSeed[] = []
      const penetratingWitnessSeeds: WitnessPairSeed[] = []
      let touchingWitnessPairCount = 0
      let penetratingWitnessPairCount = 0
      let candidateTrianglePairTests = 0
      pairSearch:
      for (const first of firstPrisms) {
        for (const second of secondPrisms) {
          candidateTrianglePairTests += 1
          trianglePairTests += 1
          if (trianglePairTests > MAX_FOLD_PREVIEW_NARROW_PHASE_TRIANGLE_TESTS) return null
          if (!boundsOverlap(first.bounds, second.bounds, numericalMargin)) continue
          satTests += 1
          if (candidate.relation === 'non_adjacent') {
            performedNonAdjacentSatScan = true
          }
          const intersection = classifyTrianglePrisms(first, second, numericalMargin)
          if (!intersection) return null
          if (intersection === 'separated') continue
          if (candidate.relation === 'non_adjacent') {
            if (intersection === 'touching') {
              touchingWitnessPairCount += 1
              if (
                touchingWitnessSeeds.length
                < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
              ) {
                touchingWitnessSeeds.push(Object.freeze({ first, second }))
              }
            } else if (intersection === 'penetrating') {
              penetratingWitnessPairCount += 1
              if (
                penetratingWitnessSeeds.length
                < MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
              ) {
                penetratingWitnessSeeds.push(Object.freeze({ first, second }))
              }
            }
          }
          if (candidate.relation === 'hinge_adjacent' && hingeContactPolicy) {
            hingePairs.push({
              firstTriangleIndex: first.triangleIndex,
              secondTriangleIndex: second.triangleIndex,
              firstVertices: first.vertices,
              secondVertices: second.vertices,
              geometryClass: intersection,
            })
          }
          if (intersection === 'penetrating') {
            geometryClass = 'penetrating'
            if (!hingeContactPolicy || candidate.relation !== 'hinge_adjacent') {
              break pairSearch
            }
            continue
          }
          if (
            intersection === 'indeterminate'
            && geometryClass !== 'penetrating'
            && geometryClass !== 'indeterminate'
          ) {
            geometryClass = 'indeterminate'
          } else if (intersection === 'touching' && !geometryClass) {
            geometryClass = 'touching'
          }
        }
      }
      if (candidate.relation === 'non_adjacent') {
        const candidatePairCount = firstPrisms.length * secondPrisms.length
        if (
          !Number.isSafeInteger(candidatePairCount)
          || candidatePairCount < candidateTrianglePairTests
        ) return null
        if (candidateTrianglePairTests !== candidatePairCount) {
          nonAdjacentPairScansComplete = false
        }
        const definitiveClass = geometryClass === 'touching'
          || geometryClass === 'penetrating'
          ? geometryClass
          : null
        if (definitiveClass) {
          const pairCount = definitiveClass === 'penetrating'
            ? penetratingWitnessPairCount
            : touchingWitnessPairCount
          const seeds = definitiveClass === 'penetrating'
            ? penetratingWitnessSeeds
            : touchingWitnessSeeds
          eligibleWitnessPairCount += pairCount
          const eligibleSeeds = definitiveClass === 'penetrating'
            ? penetratingEligibleSeeds
            : touchingEligibleSeeds
          for (const seed of seeds) {
            if (
              eligibleSeeds.length
              >= MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
            ) break
            eligibleSeeds.push(Object.freeze({
              firstFaceId: candidate.firstFaceId,
              secondFaceId: candidate.secondFaceId,
              geometryClass: definitiveClass,
              first: seed.first,
              second: seed.second,
            }))
          }
        }
      }
      if (geometryClass) {
        let interaction: FoldPreviewNarrowPhaseInteraction = {
          firstFaceId: candidate.firstFaceId,
          secondFaceId: candidate.secondFaceId,
          relation: candidate.relation,
          hingeEdgeIds: candidate.hingeEdgeIds,
          geometryClass,
        }
        if (candidate.relation === 'hinge_adjacent' && hingeContactPolicy) {
          const hingeDecision = hingeContactPolicy.classify({
            firstFaceId: candidate.firstFaceId,
            secondFaceId: candidate.secondFaceId,
            hingeEdgeIds: candidate.hingeEdgeIds,
            faceTransforms,
            thickness,
            numericalMargin,
            testedTrianglePairs: candidateTrianglePairTests,
            pairs: hingePairs,
          })
          interaction = {
            ...interaction,
            geometryClass: hingeDecision.kind === 'allowed_by_hinge_model'
              ? hingeDecision.geometry === 'boundary_contact'
                ? 'touching'
                : 'penetrating'
              : interaction.geometryClass,
            hingeDecision,
          }
        }
        interactions.push(interaction)
      }
    }

    for (const eligibleSeeds of [
      penetratingEligibleSeeds,
      touchingEligibleSeeds,
    ]) {
      const remainingAttempts =
        MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
        - attemptedWitnessPairCount
      const attemptCount = Math.min(eligibleSeeds.length, remainingAttempts)
      attemptedWitnessPairCount += attemptCount
      for (let index = 0; index < attemptCount; index += 1) {
        const seed = eligibleSeeds[index]
        if (!seed.first.witnessFrame) {
          unavailableWitnessPairCount += 1
          continue
        }
        const witness = deriveFoldPreviewTrianglePrismWitness({
          firstVertices: seed.first.vertices,
          secondVertices: seed.second.vertices,
          firstFrame: seed.first.witnessFrame,
          numericalMargin,
          authoritativeGeometryClass: seed.geometryClass,
        })
        if (!witness) {
          unavailableWitnessPairCount += 1
          continue
        }
        witnessSamples.push(Object.freeze({
          firstFaceId: seed.firstFaceId,
          secondFaceId: seed.secondFaceId,
          relation: 'non_adjacent',
          firstTriangleIndex: seed.first.triangleIndex,
          secondTriangleIndex: seed.second.triangleIndex,
          geometryClass: seed.geometryClass,
          witness,
        }))
      }
    }
  } catch {
    return null
  }

  return {
    broadPhaseCandidates: broadPhase.candidates.length,
    broadPhaseNonAdjacentCandidates,
    broadPhaseHingeAdjacentCandidates,
    interactions,
    trianglePairTests,
    satTests,
    numericalMargin,
    witnessSamples: Object.freeze(witnessSamples),
    witnessCoverage: freezeWitnessCoverage({
      eligiblePairCount: eligibleWitnessPairCount,
      attemptedPairCount: attemptedWitnessPairCount,
      unavailablePairCount: unavailableWitnessPairCount,
      omittedByLimitCount:
        eligibleWitnessPairCount - attemptedWitnessPairCount,
      authoritativePairScanComplete:
        performedNonAdjacentSatScan && nonAdjacentPairScansComplete,
    }),
  }
}

function snapshotNarrowPhaseInputs(
  faces: readonly FoldPreviewCollisionPoseFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  hingeConstraints: readonly FoldPreviewHingeContactConstraint[] | undefined,
) {
  try {
    if (
      !Array.isArray(faces)
      || !Array.isArray(adjacencies)
      || faces.length > MAX_FOLD_PREVIEW_COLLISION_FACES
      || adjacencies.length > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
    ) return null

    const faceIds = new Set<string>()
    const preparedFaces: PreparedFoldPreviewNarrowPhaseFace[] = []
    let vertexCount = 0
    for (const face of faces) {
      if (
        !face
        || !validId(face.id)
        || faceIds.has(face.id)
        || !Array.isArray(face.polygon)
        || face.polygon.length < 3
      ) return null
      vertexCount += face.polygon.length
      if (
        !Number.isSafeInteger(vertexCount)
        || vertexCount > MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES
      ) return null

      const polygon = face.polygon.map((
        point: FoldPreviewCollisionPoseFace['polygon'][number],
      ) => {
        if (
          !point
          || !Number.isFinite(point.x)
          || !Number.isFinite(point.z)
          || (point.vertexId !== undefined && !validId(point.vertexId))
        ) {
          throw new RangeError('invalid collision polygon point')
        }
        return point.vertexId === undefined
          ? { x: point.x, z: point.z }
          : { vertexId: point.vertexId, x: point.x, z: point.z }
      })
      const triangles = triangulateFoldPreviewPolygon(polygon).map((triangle) =>
        [...triangle] as FoldPreviewTriangleIndices)
      preparedFaces.push({
        id: face.id,
        polygon,
        triangles,
      })
      faceIds.add(face.id)
    }

    const edgeIds = new Set<string>()
    const adjacencySnapshot: FoldPreviewCollisionAdjacency[] = []
    for (const adjacency of adjacencies) {
      if (
        !adjacency
        || !validId(adjacency.edgeId)
        || edgeIds.has(adjacency.edgeId)
        || !faceIds.has(adjacency.firstFaceId)
        || !faceIds.has(adjacency.secondFaceId)
        || adjacency.firstFaceId === adjacency.secondFaceId
      ) return null
      adjacencySnapshot.push({
        edgeId: adjacency.edgeId,
        firstFaceId: adjacency.firstFaceId,
        secondFaceId: adjacency.secondFaceId,
      })
      edgeIds.add(adjacency.edgeId)
    }

    const poseFaces = preparedFaces.map((face) => ({
      id: face.id,
      polygon: face.polygon,
    }))
    const hingeContactPolicy = hingeConstraints === undefined
      ? null
      : prepareFoldPreviewHingeContactPolicy(
          preparedFaces,
          adjacencySnapshot,
          hingeConstraints,
        )
    if (hingeConstraints !== undefined && !hingeContactPolicy) return null
    return {
      preparedFaces,
      poseFaces,
      adjacencySnapshot,
      hingeContactPolicy,
    }
  } catch {
    return null
  }
}

function validateRigidTransforms(
  faces: readonly PreparedFoldPreviewNarrowPhaseFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
) {
  if (!faceTransforms || faceTransforms.size !== faces.length) return false
  for (const face of faces) {
    const transform = faceTransforms.get(face.id)
    if (!transform || !rigidTransform(transform)) return false
  }
  return true
}

function buildTrianglePrisms(
  face: FoldPreviewNarrowPhaseFace,
  transform: Matrix4,
  thickness: number,
): readonly TrianglePrism[] | null {
  const triangles = face.triangles ?? triangulateFoldPreviewPolygon(face.polygon)
  const halfThickness = thickness / 2
  if (!Number.isFinite(halfThickness) || halfThickness < 0) return null
  const witnessFrame = witnessFrameForTransform(transform)
  const prisms: TrianglePrism[] = []
  for (let triangleIndex = 0; triangleIndex < triangles.length; triangleIndex += 1) {
    const triangle = triangles[triangleIndex]
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
    prisms.push({
      triangleIndex,
      vertices,
      faceAxes,
      edgeDirections,
      witnessFrame,
      bounds,
    })
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

function witnessFrameForTransform(
  transform: Matrix4,
): FoldPreviewWitnessFrame | null {
  try {
    const elements = transform.elements
    if (!Array.isArray(elements) || elements.length !== 16) return null
    const xAxisX = elements[0]
    const xAxisY = elements[1]
    const xAxisZ = elements[2]
    const yAxisX = elements[4]
    const yAxisY = elements[5]
    const yAxisZ = elements[6]
    const zAxisX = elements[8]
    const zAxisY = elements[9]
    const zAxisZ = elements[10]
    const values = [
      xAxisX,
      xAxisY,
      xAxisZ,
      yAxisX,
      yAxisY,
      yAxisZ,
      zAxisX,
      zAxisY,
      zAxisZ,
    ]
    if (!values.every(Number.isFinite)) return null
    return Object.freeze({
      xAxis: Object.freeze({
        x: xAxisX,
        y: xAxisY,
        z: xAxisZ,
      }),
      yAxis: Object.freeze({
        x: yAxisX,
        y: yAxisY,
        z: yAxisZ,
      }),
      zAxis: Object.freeze({
        x: zAxisX,
        y: zAxisY,
        z: zAxisZ,
      }),
    })
  } catch {
    return null
  }
}

function freezeWitnessCoverage(
  value: Omit<FoldPreviewNarrowPhaseWitnessCoverage, 'scope'>,
): FoldPreviewNarrowPhaseWitnessCoverage {
  return Object.freeze({
    scope: 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1',
    ...value,
  })
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

function validId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}
