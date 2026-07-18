import { Vector3, type Matrix4 } from 'three'
import type {
  FoldPreviewCollisionAdjacency,
  FoldPreviewCollisionPoint,
} from './foldPreviewCollision'
import type { FoldPreviewTriangleIndices } from './foldPreviewGeometry'

export type FoldPreviewHingeContactConstraint = Readonly<{
  edgeId: string
  leftFaceId: string
  rightFaceId: string
  start: Readonly<{
    vertexId: string
    x: number
    z: number
  }>
  end: Readonly<{
    vertexId: string
    x: number
    z: number
  }>
  thicknessRule: 'centered_mid_surface_v1'
}>

export type FoldPreviewHingeContactDecision =
  | Readonly<{
      kind: 'allowed_by_hinge_model'
      hingeEdgeId: string
      geometry:
        | 'boundary_contact'
        | 'corridor_overlap'
        | 'flat_surface_stack'
      thicknessRule: 'centered_mid_surface_v1'
    }>
  | Readonly<{
      kind: 'outside_hinge_penetration'
      hingeEdgeId: string
    }>
  | Readonly<{
      kind: 'outside_hinge_contact'
      hingeEdgeId: string
    }>
  | Readonly<{
      kind: 'indeterminate'
      hingeEdgeIds: readonly string[]
      reason:
        | 'zero_thickness'
        | 'missing_constraint'
        | 'multiple_shared_hinges'
        | 'pose_mismatch'
        | 'unsupported_flat_fold'
        | 'layer_offset_unmodeled'
        | 'numerical_geometry'
        | 'corridor_boundary'
        | 'non_hinge_triangle'
        | 'incomplete_pair_scan'
        | 'pair_geometry_mismatch'
        | 'flat_pose_penetration'
    }>

export type FoldPreviewHingeContactPair = Readonly<{
  firstTriangleIndex: number
  secondTriangleIndex: number
  firstVertices: readonly Vector3[]
  secondVertices: readonly Vector3[]
  geometryClass: 'touching' | 'penetrating' | 'indeterminate'
}>

export type FoldPreviewStaticHingeTrianglePairSupportDecision =
  | Readonly<{
      kind: 'proven_static_hinge_support'
      hingeEdgeId: string
      thicknessRule: 'centered_mid_surface_v1'
    }>
  | Readonly<{
      kind: 'not_proven'
      hingeEdgeIds: readonly string[]
      reason:
        | 'missing_constraint'
        | 'multiple_shared_hinges'
        | 'malformed_request'
        | 'face_pair_mismatch'
        | 'triangle_index_out_of_range'
        | 'material_half_slabs_not_proven'
        | 'finite_hinge_segment_not_proven'
    }>

export type FoldPreviewStaticHingeTrianglePairSupportRequest = Readonly<{
  firstFaceId: string
  secondFaceId: string
  hingeEdgeIds: readonly string[]
  firstTriangleIndex: number
  secondTriangleIndex: number
}>

export type FoldPreviewHingeContactPolicy = Readonly<{
  classify(input: Readonly<{
    firstFaceId: string
    secondFaceId: string
    hingeEdgeIds: readonly string[]
    faceTransforms: ReadonlyMap<string, Matrix4>
    thickness: number
    numericalMargin: number
    testedTrianglePairs: number
    pairs: readonly FoldPreviewHingeContactPair[]
  }>): FoldPreviewHingeContactDecision
  /**
   * Proves only a pose-independent source-geometry property: both requested
   * triangles lie wholly in their opposing material half-slabs and between
   * the finite hinge endpoints for centered_mid_surface_v1.
   *
   * This is not a motion-safety certificate. A continuous caller must
   * separately prove a rigid shared-axis path and reject any interval that
   * reaches the 180-degree centered-slab singularity.
   */
  proveStaticTrianglePairSupport(
    input: FoldPreviewStaticHingeTrianglePairSupportRequest,
  ): FoldPreviewStaticHingeTrianglePairSupportDecision
}>

export type FoldPreviewHingePolicyFace = Readonly<{
  id: string
  polygon: readonly FoldPreviewCollisionPoint[]
  triangles: readonly FoldPreviewTriangleIndices[]
}>

/**
 * Returns the centered-slab radius h / cos(theta / 2).
 *
 * Callers provide the already validated cosine so point and continuous hinge
 * policies use the same finite-radius model near a flat fold.
 */
export function calculateFoldPreviewCenteredSlabHingeRadius(
  thickness: number,
  cosineHalfAngle: number,
): number | null {
  if (
    !Number.isFinite(thickness)
    || thickness < 0
    || !Number.isFinite(cosineHalfAngle)
    || cosineHalfAngle <= 0
    || cosineHalfAngle > 1
  ) return null
  const radius = (thickness / 2) / cosineHalfAngle
  return Number.isFinite(radius) && radius >= 0 ? radius : null
}

type PreparedPolicyFace = Readonly<{
  id: string
  polygon: readonly FoldPreviewCollisionPoint[]
  triangles: readonly FoldPreviewTriangleIndices[]
}>

type PreparedConstraint = Readonly<{
  edgeId: string
  leftFaceId: string
  rightFaceId: string
  start: FoldPreviewHingeContactConstraint['start']
  end: FoldPreviewHingeContactConstraint['end']
  thicknessRule: 'centered_mid_surface_v1'
  leftTriangleProofs: readonly PreparedTriangleHingeProof[]
  rightTriangleProofs: readonly PreparedTriangleHingeProof[]
}>

type PreparedTriangleHingeProof = Readonly<{
  materialSide: boolean
  axiallyBounded: boolean
}>

type HingeFrame = Readonly<{
  start: Vector3
  axis: Vector3
  normal: Vector3
  binormal: Vector3
  length: number
  innerRadius: number
  outerRadius: number
  outerMargin: number
  flat: boolean
  flatFold: boolean
  zeroThickness: boolean
}>

type ProjectionRange = Readonly<{ min: number; max: number }>
type CorridorPlacement = 'inside' | 'outside' | 'boundary'

const RIGID_TRANSFORM_TOLERANCE = 1e-10
const HINGE_LOCAL_NUMERICAL_MARGIN_FACTOR = 256
const HINGE_WORLD_ROUNDING_MARGIN_FACTOR = 32

function snapshotPolicyFaces(
  faces: readonly FoldPreviewHingePolicyFace[],
): readonly PreparedPolicyFace[] | null {
  const snapshots: PreparedPolicyFace[] = []
  const faceIds = new Set<string>()
  for (const face of faces) {
    if (
      !face
      || !validId(face.id)
      || faceIds.has(face.id)
      || !Array.isArray(face.polygon)
      || face.polygon.length < 3
      || !Array.isArray(face.triangles)
      || face.triangles.length === 0
    ) return null
    const polygon = face.polygon.map((point) => {
      if (
        !point
        || !Number.isFinite(point.x)
        || !Number.isFinite(point.z)
        || (point.vertexId !== undefined && !validId(point.vertexId))
      ) throw new RangeError('invalid hinge policy polygon')
      return point.vertexId === undefined
        ? { x: point.x, z: point.z }
        : { vertexId: point.vertexId, x: point.x, z: point.z }
    })
    const triangles: FoldPreviewTriangleIndices[] = []
    for (const rawTriangle of face.triangles) {
      const triangle: readonly unknown[] = rawTriangle
      if (
        !Array.isArray(triangle)
        || triangle.length !== 3
        || !triangle.every((index) =>
          Number.isSafeInteger(index) && index >= 0 && index < polygon.length)
        || new Set(triangle).size !== 3
      ) return null
      triangles.push([
        triangle[0] as number,
        triangle[1] as number,
        triangle[2] as number,
      ])
    }
    snapshots.push({ id: face.id, polygon, triangles })
    faceIds.add(face.id)
  }
  return snapshots
}

/**
 * Builds the origami-specific policy separately from broad-phase adjacency.
 * Every supplied adjacency must have one complete immutable constraint.
 */
export function prepareFoldPreviewHingeContactPolicy(
  faces: readonly FoldPreviewHingePolicyFace[],
  adjacencies: readonly FoldPreviewCollisionAdjacency[],
  constraints: readonly FoldPreviewHingeContactConstraint[],
): FoldPreviewHingeContactPolicy | null {
  try {
    if (
      !Array.isArray(faces)
      || !Array.isArray(adjacencies)
      || !Array.isArray(constraints)
      || constraints.length !== adjacencies.length
    ) return null
    const faceSnapshots = snapshotPolicyFaces(faces)
    if (!faceSnapshots) return null
    const facesById = new Map(faceSnapshots.map((face) => [face.id, face]))
    const adjacencyById = new Map(adjacencies.map((adjacency) => [
      adjacency.edgeId,
      adjacency,
    ]))
    if (adjacencyById.size !== adjacencies.length) return null

    const constraintsById = new Map<string, PreparedConstraint>()
    for (const constraint of constraints) {
      if (!validConstraintRecord(constraint) || constraintsById.has(constraint.edgeId)) {
        return null
      }
      const adjacency = adjacencyById.get(constraint.edgeId)
      const leftFace = facesById.get(constraint.leftFaceId)
      const rightFace = facesById.get(constraint.rightFaceId)
      if (
        !adjacency
        || !leftFace
        || !rightFace
        || !sameFacePair(adjacency, constraint)
      ) return null
      const leftBoundary = resolveBoundaryEdge(leftFace, constraint)
      const rightBoundary = resolveBoundaryEdge(rightFace, constraint)
      if (
        !leftBoundary
        || !rightBoundary
        || leftBoundary.direction === rightBoundary.direction
        || Math.sign(leftBoundary.thirdVertexSide)
          === Math.sign(rightBoundary.thirdVertexSide)
      ) return null
      const leftTriangleProofs = proveFaceTrianglesWithinHingeHalfSlab(
        leftFace,
        constraint,
        Math.sign(leftBoundary.thirdVertexSide),
      )
      const rightTriangleProofs = proveFaceTrianglesWithinHingeHalfSlab(
        rightFace,
        constraint,
        Math.sign(rightBoundary.thirdVertexSide),
      )
      if (!leftTriangleProofs || !rightTriangleProofs) return null
      constraintsById.set(constraint.edgeId, {
        edgeId: constraint.edgeId,
        leftFaceId: constraint.leftFaceId,
        rightFaceId: constraint.rightFaceId,
        start: { ...constraint.start },
        end: { ...constraint.end },
        thicknessRule: constraint.thicknessRule,
        leftTriangleProofs,
        rightTriangleProofs,
      })
    }
    if (constraintsById.size !== adjacencyById.size) return null

    return Object.freeze({
      classify(input) {
        return classifyHingeContact(facesById, constraintsById, input)
      },
      proveStaticTrianglePairSupport(input) {
        try {
          return proveStaticTrianglePairSupport(
            facesById,
            constraintsById,
            input,
          )
        } catch {
          return staticSupportNotProven([], 'malformed_request')
        }
      },
    })
  } catch {
    return null
  }
}

function proveStaticTrianglePairSupport(
  facesById: ReadonlyMap<string, PreparedPolicyFace>,
  constraintsById: ReadonlyMap<string, PreparedConstraint>,
  input: FoldPreviewStaticHingeTrianglePairSupportRequest,
): FoldPreviewStaticHingeTrianglePairSupportDecision {
  if (!input || !Array.isArray(input.hingeEdgeIds)) {
    return staticSupportNotProven([], 'malformed_request')
  }
  const hingeEdgeIds = [...input.hingeEdgeIds]
  if (hingeEdgeIds.length !== 1) {
    return staticSupportNotProven(
      hingeEdgeIds.filter(validId),
      hingeEdgeIds.length === 0
        ? 'missing_constraint'
        : 'multiple_shared_hinges',
    )
  }
  const hingeEdgeId = hingeEdgeIds[0]
  if (!validId(hingeEdgeId)) {
    return staticSupportNotProven([], 'malformed_request')
  }
  const constraint = constraintsById.get(hingeEdgeId)
  if (!constraint) {
    return staticSupportNotProven(hingeEdgeIds, 'missing_constraint')
  }
  if (
    !validId(input.firstFaceId)
    || !validId(input.secondFaceId)
    || !sameIds(
      input.firstFaceId,
      input.secondFaceId,
      constraint.leftFaceId,
      constraint.rightFaceId,
    )
  ) {
    return staticSupportNotProven(hingeEdgeIds, 'face_pair_mismatch')
  }
  const firstFace = facesById.get(input.firstFaceId)
  const secondFace = facesById.get(input.secondFaceId)
  if (!firstFace || !secondFace) {
    return staticSupportNotProven(hingeEdgeIds, 'face_pair_mismatch')
  }
  if (
    !Number.isSafeInteger(input.firstTriangleIndex)
    || input.firstTriangleIndex < 0
    || input.firstTriangleIndex >= firstFace.triangles.length
    || !Number.isSafeInteger(input.secondTriangleIndex)
    || input.secondTriangleIndex < 0
    || input.secondTriangleIndex >= secondFace.triangles.length
  ) {
    return staticSupportNotProven(hingeEdgeIds, 'triangle_index_out_of_range')
  }
  const proof = staticPairHingeProof(
    input.firstTriangleIndex,
    input.secondTriangleIndex,
    constraint,
    input.firstFaceId,
  )
  if (!proof.left || !proof.right) {
    return staticSupportNotProven(hingeEdgeIds, 'triangle_index_out_of_range')
  }
  if (!proof.left.materialSide || !proof.right.materialSide) {
    return staticSupportNotProven(
      hingeEdgeIds,
      'material_half_slabs_not_proven',
    )
  }
  if (!proof.left.axiallyBounded || !proof.right.axiallyBounded) {
    return staticSupportNotProven(
      hingeEdgeIds,
      'finite_hinge_segment_not_proven',
    )
  }
  return {
    kind: 'proven_static_hinge_support',
    hingeEdgeId,
    thicknessRule: constraint.thicknessRule,
  }
}

function classifyHingeContact(
  facesById: ReadonlyMap<string, PreparedPolicyFace>,
  constraintsById: ReadonlyMap<string, PreparedConstraint>,
  input: Readonly<{
    firstFaceId: string
    secondFaceId: string
    hingeEdgeIds: readonly string[]
    faceTransforms: ReadonlyMap<string, Matrix4>
    thickness: number
    numericalMargin: number
    testedTrianglePairs: number
    pairs: readonly FoldPreviewHingeContactPair[]
  }>,
): FoldPreviewHingeContactDecision {
  const hingeEdgeIds = Array.isArray(input?.hingeEdgeIds)
    ? [...input.hingeEdgeIds]
    : []
  if (hingeEdgeIds.length !== 1) {
    return indeterminate(
      hingeEdgeIds,
      hingeEdgeIds.length === 0 ? 'missing_constraint' : 'multiple_shared_hinges',
    )
  }
  if (!Number.isFinite(input.thickness) || input.thickness < 0) {
    return indeterminate(hingeEdgeIds, 'numerical_geometry')
  }
  if (!Number.isFinite(input.numericalMargin) || input.numericalMargin < 0) {
    return indeterminate(hingeEdgeIds, 'numerical_geometry')
  }
  const constraint = constraintsById.get(hingeEdgeIds[0])
  if (
    !constraint
    || !sameIds(
      input.firstFaceId,
      input.secondFaceId,
      constraint.leftFaceId,
      constraint.rightFaceId,
    )
  ) return indeterminate(hingeEdgeIds, 'missing_constraint')
  const firstFace = facesById.get(input.firstFaceId)
  const secondFace = facesById.get(input.secondFaceId)
  if (!firstFace || !secondFace) {
    return indeterminate(hingeEdgeIds, 'missing_constraint')
  }
  const expectedTrianglePairs = firstFace.triangles.length * secondFace.triangles.length
  if (
    !Number.isSafeInteger(expectedTrianglePairs)
    || !Number.isSafeInteger(input.testedTrianglePairs)
    || input.testedTrianglePairs !== expectedTrianglePairs
    || !Array.isArray(input.pairs)
    || input.pairs.length > input.testedTrianglePairs
  ) return indeterminate(hingeEdgeIds, 'incomplete_pair_scan')

  const frameResult = createHingeFrame(
    constraint,
    input.faceTransforms,
    input.thickness,
    input.numericalMargin,
  )
  if (frameResult.kind === 'error') {
    return indeterminate(hingeEdgeIds, frameResult.reason)
  }
  if (!Array.isArray(input.pairs) || input.pairs.length === 0) {
    return indeterminate(hingeEdgeIds, 'numerical_geometry')
  }
  const pairKeys = new Set<string>()
  for (const pair of input.pairs) {
    if (
      !pair
      || !Number.isSafeInteger(pair.firstTriangleIndex)
      || !Number.isSafeInteger(pair.secondTriangleIndex)
    ) continue
    const pairKey = `${pair.firstTriangleIndex}:${pair.secondTriangleIndex}`
    if (pairKeys.has(pairKey)) {
      return indeterminate(hingeEdgeIds, 'pair_geometry_mismatch')
    }
    pairKeys.add(pairKey)
  }

  let outsidePenetration = false
  let outsideContact = false
  let hasPenetration = false
  let hasContact = false
  let unresolvedReason: Extract<
    FoldPreviewHingeContactDecision,
    { kind: 'indeterminate' }
  >['reason'] | null = null

  for (const pair of input.pairs) {
    if (
      !validPair(pair)
      || !pairMatchesPreparedGeometry(
        pair,
        firstFace,
        secondFace,
        input.faceTransforms,
        input.thickness,
      )
    ) {
      unresolvedReason ??= 'pair_geometry_mismatch'
      continue
    }
    const hingeProof = pairHingeProof(
      pair,
      constraint,
      input.firstFaceId,
    )
    // Preparation proves which triangles stay entirely in their face's
    // material half-plane. Any pair with both proofs is a subset of the two
    // analytic half-infinite slabs, so every radial intersection is inside
    // h / cos(theta / 2), even when triangulation represents the same legal
    // hinge overlap through pairs other than the shared-edge pair.
    const placement = hingeProof.radiallyBounded
      ? classifyFiniteHingePlacement(
          pair,
          frameResult.frame,
          hingeProof.axiallyBounded,
        )
      : classifyCorridorPlacement(pair, frameResult.frame)
    if (placement === 'outside') {
      if (pair.geometryClass === 'penetrating') {
        outsidePenetration = true
      } else if (pair.geometryClass === 'touching') {
        outsideContact = true
      } else {
        unresolvedReason ??= 'numerical_geometry'
      }
      continue
    }
    if (placement === 'boundary') {
      unresolvedReason ??= 'corridor_boundary'
      continue
    }
    if (!hingeProof.radiallyBounded) {
      unresolvedReason ??= 'non_hinge_triangle'
      continue
    }
    if (frameResult.frame.zeroThickness) {
      if (!hingeProof.axiallyBounded) {
        unresolvedReason ??= 'corridor_boundary'
        continue
      }
      if (frameResult.frame.flatFold) {
        // Exact opposite normals establish the 180-degree pose, but they do
        // not by themselves prove positive-area material overlap. Preserve
        // boundary/unknown pairs and require at least one authoritative
        // coplanar-area penetration before publishing a flat surface stack.
        if (pair.geometryClass === 'penetrating') hasPenetration = true
        else hasContact = true
        continue
      }
      if (pair.geometryClass === 'indeterminate') {
        // Away from the exact flat-fold pose, two rigid faces sharing the
        // verified axis can intersect only on that axis.  The opposing
        // material-half-plane and finite-axis proofs therefore resolve a
        // numerically marginal surface pair as the intended hinge boundary.
        hasContact = true
        continue
      }
      if (
        pair.geometryClass === 'penetrating'
      ) {
        outsidePenetration = true
        continue
      }
      hasContact = true
      continue
    }
    if (frameResult.frame.flat && pair.geometryClass === 'penetrating') {
      unresolvedReason ??= 'flat_pose_penetration'
      continue
    }
    // A genuine positive-thickness hinge pair has boundary contact at zero
    // angle and volume overlap for 0 < theta < pi. This analytic fact safely
    // resolves SAT's near-parallel indeterminate result for this pair only.
    if (frameResult.frame.flat) hasContact = true
    else hasPenetration = true
  }

  if (outsidePenetration) {
    return {
      kind: 'outside_hinge_penetration',
      hingeEdgeId: constraint.edgeId,
    }
  }
  if (unresolvedReason) return indeterminate(hingeEdgeIds, unresolvedReason)
  if (outsideContact) {
    return {
      kind: 'outside_hinge_contact',
      hingeEdgeId: constraint.edgeId,
    }
  }
  if (frameResult.frame.flatFold && !hasPenetration) {
    return indeterminate(hingeEdgeIds, 'numerical_geometry')
  }
  if (!hasPenetration && !hasContact) {
    return indeterminate(hingeEdgeIds, 'numerical_geometry')
  }
  return {
    kind: 'allowed_by_hinge_model',
    hingeEdgeId: constraint.edgeId,
    geometry: hasPenetration
      ? frameResult.frame.flatFold
        ? 'flat_surface_stack'
        : 'corridor_overlap'
      : 'boundary_contact',
    thicknessRule: constraint.thicknessRule,
  }
}

function createHingeFrame(
  constraint: PreparedConstraint,
  transforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
  margin: number,
):
  | Readonly<{ kind: 'ready'; frame: HingeFrame }>
  | Readonly<{
      kind: 'error'
      reason: Extract<
        FoldPreviewHingeContactDecision,
        { kind: 'indeterminate' }
      >['reason']
    }> {
  if (!transforms) return { kind: 'error', reason: 'pose_mismatch' }
  const leftTransform = transforms.get(constraint.leftFaceId)
  const rightTransform = transforms.get(constraint.rightFaceId)
  if (
    !leftTransform
    || !rightTransform
    || !rigidTransform(leftTransform)
    || !rigidTransform(rightTransform)
  ) {
    return { kind: 'error', reason: 'pose_mismatch' }
  }
  const leftStart = transformedRestPoint(constraint.start, leftTransform)
  const rightStart = transformedRestPoint(constraint.start, rightTransform)
  const leftEnd = transformedRestPoint(constraint.end, leftTransform)
  const rightEnd = transformedRestPoint(constraint.end, rightTransform)
  if (!leftStart || !rightStart || !leftEnd || !rightEnd) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  const hingeMargins = hingeFrameNumericalMargin(
    [leftStart, rightStart, leftEnd, rightEnd],
    margin,
    Math.max(
      1,
      thickness,
      Math.hypot(
        constraint.end.x - constraint.start.x,
        constraint.end.z - constraint.start.z,
      ),
    ),
  )
  if (hingeMargins === null) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  const hingeMargin = hingeMargins.numericalMargin
  const hingeTopologyMargin = hingeMargins.topologyMargin
  const startError = leftStart.distanceTo(rightStart)
  const endError = leftEnd.distanceTo(rightEnd)
  if (
    !Number.isFinite(startError)
    || !Number.isFinite(endError)
    || startError > hingeTopologyMargin
    || endError > hingeTopologyMargin
  ) return { kind: 'error', reason: 'pose_mismatch' }

  const start = leftStart.clone().add(rightStart).multiplyScalar(0.5)
  const end = leftEnd.clone().add(rightEnd).multiplyScalar(0.5)
  const axis = end.clone().sub(start)
  const length = axis.length()
  if (!Number.isFinite(length) || length <= hingeMargin) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  axis.multiplyScalar(1 / length)

  const leftNormal = transformedNormal(leftTransform)
  const rightNormal = transformedNormal(rightTransform)
  if (!leftNormal || !rightNormal) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  const axisNormalDot = axis.dot(leftNormal)
  const rightAxisNormalDot = axis.dot(rightNormal)
  if (
    !Number.isFinite(axisNormalDot)
    || !Number.isFinite(rightAxisNormalDot)
    || Math.abs(axisNormalDot) > RIGID_TRANSFORM_TOLERANCE
    || Math.abs(rightAxisNormalDot) > RIGID_TRANSFORM_TOLERANCE
  ) {
    return { kind: 'error', reason: 'pose_mismatch' }
  }
  const normal = leftNormal.clone().addScaledVector(axis, -axisNormalDot)
  const normalLength = normal.length()
  if (!Number.isFinite(normalLength) || normalLength === 0) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  normal.multiplyScalar(1 / normalLength)
  const binormal = axis.clone().cross(normal)
  const binormalLength = binormal.length()
  if (!Number.isFinite(binormalLength) || binormalLength === 0) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  binormal.multiplyScalar(1 / binormalLength)

  const rawNormalDot = leftNormal.dot(rightNormal)
  const normalCross = leftNormal.clone().cross(rightNormal)
  if (!Number.isFinite(rawNormalDot) || !finiteVector(normalCross)) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  const normalDot = Math.max(-1, Math.min(1, rawNormalDot))
  const cosineHalfAngle = Math.sqrt(Math.max(0, (1 + normalDot) / 2))
  if (!Number.isFinite(cosineHalfAngle)) {
    return { kind: 'error', reason: 'numerical_geometry' }
  }
  const poseError = Math.max(startError, endError)
  const outerMargin = hingeMargin + poseError
  const exactlyParallel = normalCross.x === 0
    && normalCross.y === 0
    && normalCross.z === 0
  const numericalFlatFoldSingularity =
    cosineHalfAngle <= Math.sqrt(Number.EPSILON)
  const exactFlatFold = exactlyParallel && normalDot === -1
  if (numericalFlatFoldSingularity && thickness > 0) {
    return { kind: 'error', reason: 'layer_offset_unmodeled' }
  }
  if (thickness === 0) {
    return {
      kind: 'ready',
      frame: {
        start,
        axis,
        normal,
        binormal,
        length,
        innerRadius: 0,
        outerRadius: outerMargin,
        outerMargin,
        flat: !exactFlatFold
          && exactlyParallel
          && 1 - normalDot <= RIGID_TRANSFORM_TOLERANCE
          && poseError <= hingeTopologyMargin,
        flatFold: exactFlatFold,
        zeroThickness: true,
      },
    }
  }
  const radius = calculateFoldPreviewCenteredSlabHingeRadius(
    thickness,
    cosineHalfAngle,
  )
  // The finite hinge segment is a physical cap, not a tolerance band. A
  // radius even strictly above L is unresolved; an absolute-world margin
  // must never turn it into an allowed corridor after a common translation.
  if (radius === null || radius > length) {
    return { kind: 'error', reason: 'layer_offset_unmodeled' }
  }
  // A dot-product tolerance alone creates a finite "flat" angle band even
  // though every representable non-zero fold has a valid analytic corridor.
  // Exact parallelism is tested component-wise so subnormal cross components
  // are not lost by squaring them in Vector3.length().
  const flat = exactlyParallel
    && 1 - normalDot <= RIGID_TRANSFORM_TOLERANCE
    && poseError <= hingeTopologyMargin
  const innerRadius = flat ? radius : radius - outerMargin
  const outerRadius = radius + outerMargin
  if (
    !Number.isFinite(innerRadius)
    || !Number.isFinite(outerRadius)
    || innerRadius < 0
  ) return { kind: 'error', reason: 'corridor_boundary' }

  return {
    kind: 'ready',
    frame: {
      start,
      axis,
      normal,
      binormal,
      length,
      innerRadius,
      outerRadius,
      outerMargin,
      flat,
      flatFold: false,
      zeroThickness: false,
    },
  }
}

function hingeFrameNumericalMargin(
  points: readonly Vector3[],
  authoritativeMargin: number,
  localScaleFloor: number,
): Readonly<{
  numericalMargin: number
  topologyMargin: number
}> | null {
  if (
    points.length === 0
    || !points.every(finiteVector)
    || !Number.isFinite(authoritativeMargin)
    || authoritativeMargin < 0
    || !Number.isFinite(localScaleFloor)
    || localScaleFloor <= 0
  ) return null
  const origin = points[0]
  let localScale = localScaleFloor
  let worldScale = 1
  for (const point of points) {
    localScale = Math.max(
      localScale,
      Math.abs(point.x - origin.x),
      Math.abs(point.y - origin.y),
      Math.abs(point.z - origin.z),
    )
    worldScale = Math.max(
      worldScale,
      Math.abs(point.x),
      Math.abs(point.y),
      Math.abs(point.z),
    )
  }
  const localMargin = localScale
    * Number.EPSILON
    * HINGE_LOCAL_NUMERICAL_MARGIN_FACTOR
  const storedWorldRoundingMargin = worldScale
    * Number.EPSILON
    * HINGE_WORLD_ROUNDING_MARGIN_FACTOR
  if (
    !Number.isFinite(localMargin)
    || !Number.isFinite(storedWorldRoundingMargin)
  ) return null
  const numericalMargin = Math.min(
    authoritativeMargin,
    Math.max(localMargin, storedWorldRoundingMargin),
  )
  // Absolute-world ULP coverage remains necessary for corridor projections,
  // but cannot certify that two current hinge endpoints are the same point.
  // A non-zero topology mismatch is accepted only inside the local feature
  // error bound; otherwise the generic transform input fails closed.
  const topologyMargin = Math.min(authoritativeMargin, localMargin)
  return Number.isFinite(numericalMargin) && Number.isFinite(topologyMargin)
    ? { numericalMargin, topologyMargin }
    : null
}

function classifyFiniteHingePlacement(
  pair: FoldPreviewHingeContactPair,
  frame: HingeFrame,
  staticallyAxiallyBounded: boolean,
): CorridorPlacement {
  // Rigid transforms preserve the hinge-axis coordinate of every rest point.
  // When both source triangles lie wholly between the two endpoints, their
  // intersection does too; avoid turning harmless projection round-off at an
  // endpoint into a corridor-boundary failure.
  if (staticallyAxiallyBounded) return 'inside'
  const axial = intersectRanges(
    projectRange(pair.firstVertices, frame.start, frame.axis),
    projectRange(pair.secondVertices, frame.start, frame.axis),
    frame.outerMargin,
  )
  if (!axial) return 'boundary'
  if (
    axial.max < -frame.outerMargin
    || axial.min > frame.length + frame.outerMargin
  ) return 'outside'
  if (axial.min >= 0 && axial.max <= frame.length) return 'inside'
  return 'boundary'
}

function classifyCorridorPlacement(
  pair: FoldPreviewHingeContactPair,
  frame: HingeFrame,
): CorridorPlacement {
  const axial = intersectRanges(
    projectRange(pair.firstVertices, frame.start, frame.axis),
    projectRange(pair.secondVertices, frame.start, frame.axis),
    frame.outerMargin,
  )
  const normal = intersectRanges(
    projectRange(pair.firstVertices, frame.start, frame.normal),
    projectRange(pair.secondVertices, frame.start, frame.normal),
    frame.outerMargin,
  )
  const binormal = intersectRanges(
    projectRange(pair.firstVertices, frame.start, frame.binormal),
    projectRange(pair.secondVertices, frame.start, frame.binormal),
    frame.outerMargin,
  )
  if (!axial || !normal || !binormal) return 'boundary'

  const minimumRadius = Math.hypot(
    distanceFromZero(normal),
    distanceFromZero(binormal),
  )
  if (
    axial.max < -frame.outerMargin
    || axial.min > frame.length + frame.outerMargin
    || minimumRadius > frame.outerRadius
  ) return 'outside'

  const maximumRadius = Math.max(
    Math.hypot(normal.min, binormal.min),
    Math.hypot(normal.min, binormal.max),
    Math.hypot(normal.max, binormal.min),
    Math.hypot(normal.max, binormal.max),
  )
  if (
    axial.min >= 0
    && axial.max <= frame.length
    && Number.isFinite(maximumRadius)
    && (
      frame.flat
        ? maximumRadius <= frame.innerRadius
        : maximumRadius < frame.innerRadius
    )
  ) return 'inside'
  return 'boundary'
}

function projectRange(
  vertices: readonly Vector3[],
  origin: Vector3,
  axis: Vector3,
): ProjectionRange | null {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const vertex of vertices) {
    if (!finiteVector(vertex)) return null
    const x = vertex.x - origin.x
    const y = vertex.y - origin.y
    const z = vertex.z - origin.z
    const projection = x * axis.x + y * axis.y + z * axis.z
    if (!Number.isFinite(projection)) return null
    min = Math.min(min, projection)
    max = Math.max(max, projection)
  }
  return Number.isFinite(min) && Number.isFinite(max) ? { min, max } : null
}

function intersectRanges(
  first: ProjectionRange | null,
  second: ProjectionRange | null,
  margin: number,
): ProjectionRange | null {
  if (!first || !second) return null
  const min = Math.max(first.min, second.min)
  const max = Math.min(first.max, second.max)
  if (!Number.isFinite(min) || !Number.isFinite(max) || min > max + margin) return null
  return min <= max ? { min, max } : null
}

function distanceFromZero(range: ProjectionRange) {
  if (range.min <= 0 && range.max >= 0) return 0
  return Math.min(Math.abs(range.min), Math.abs(range.max))
}

function pairHingeProof(
  pair: FoldPreviewHingeContactPair,
  constraint: PreparedConstraint,
  firstFaceId: string,
) {
  const proof = staticPairHingeProof(
    pair.firstTriangleIndex,
    pair.secondTriangleIndex,
    constraint,
    firstFaceId,
  )
  return {
    radiallyBounded: Boolean(
      proof.left?.materialSide && proof.right?.materialSide,
    ),
    // The interaction is the intersection of both triangles.  If either
    // triangle lies wholly between the finite hinge endpoints, that
    // intersection is axially bounded as well.  Requiring both triangles to
    // be bounded incorrectly rejects a normal polygon triangulation whose
    // non-hinge triangle extends past one endpoint but only meets the other
    // face at the shared hinge endpoint.
    axiallyBounded: Boolean(
      proof.left?.axiallyBounded || proof.right?.axiallyBounded,
    ),
  }
}

function staticPairHingeProof(
  firstTriangleIndex: number,
  secondTriangleIndex: number,
  constraint: PreparedConstraint,
  firstFaceId: string,
): Readonly<{
  left: PreparedTriangleHingeProof | undefined
  right: PreparedTriangleHingeProof | undefined
}> {
  const firstIsLeft = firstFaceId === constraint.leftFaceId
  const left = firstIsLeft
    ? constraint.leftTriangleProofs[firstTriangleIndex]
    : constraint.leftTriangleProofs[secondTriangleIndex]
  const right = firstIsLeft
    ? constraint.rightTriangleProofs[secondTriangleIndex]
    : constraint.rightTriangleProofs[firstTriangleIndex]
  return { left, right }
}

function proveFaceTrianglesWithinHingeHalfSlab(
  face: PreparedPolicyFace,
  constraint: FoldPreviewHingeContactConstraint,
  materialSideSign: number,
): readonly PreparedTriangleHingeProof[] | null {
  if (materialSideSign !== -1 && materialSideSign !== 1) return null
  const edgeX = constraint.end.x - constraint.start.x
  const edgeZ = constraint.end.z - constraint.start.z
  const edgeLengthSquared = edgeX * edgeX + edgeZ * edgeZ
  if (
    !Number.isFinite(edgeX)
    || !Number.isFinite(edgeZ)
    || !Number.isFinite(edgeLengthSquared)
    || edgeLengthSquared <= 0
  ) return null

  const proofs: PreparedTriangleHingeProof[] = []
  for (const triangle of face.triangles) {
    let materialSide = true
    let axiallyBounded = true
    for (const vertexIndex of triangle) {
      const vertex = face.polygon[vertexIndex]
      if (!vertex) return null
      const relativeX = vertex.x - constraint.start.x
      const relativeZ = vertex.z - constraint.start.z
      const side = edgeX * relativeZ - edgeZ * relativeX
      const axialProjection = edgeX * relativeX + edgeZ * relativeZ
      if (!Number.isFinite(side) || !Number.isFinite(axialProjection)) return null
      materialSide &&= materialSideSign * side >= 0
      axiallyBounded &&= axialProjection >= 0
        && axialProjection <= edgeLengthSquared
    }
    proofs.push({ materialSide, axiallyBounded })
  }
  return proofs.length === face.triangles.length ? proofs : null
}

function resolveBoundaryEdge(
  face: FoldPreviewHingePolicyFace,
  constraint: FoldPreviewHingeContactConstraint,
) {
  if (
    !face
    || !Array.isArray(face.polygon)
    || !Array.isArray(face.triangles)
    || face.polygon.length < 3
  ) return null
  const startMatches = matchingVertexIndices(face.polygon, constraint.start)
  const endMatches = matchingVertexIndices(face.polygon, constraint.end)
  if (startMatches.length !== 1 || endMatches.length !== 1) return null
  const startIndex = startMatches[0]
  const endIndex = endMatches[0]
  const nextStart = (startIndex + 1) % face.polygon.length
  const nextEnd = (endIndex + 1) % face.polygon.length
  const direction = nextStart === endIndex ? 1 : nextEnd === startIndex ? -1 : 0
  if (direction === 0) return null
  const triangleMatches = face.triangles
    .filter((triangle) =>
      triangle.includes(startIndex) && triangle.includes(endIndex))
  if (triangleMatches.length !== 1) return null
  const hingeTriangle = triangleMatches[0]
  const thirdVertexIndices = hingeTriangle.filter((index: number) =>
    index !== startIndex && index !== endIndex)
  if (thirdVertexIndices.length !== 1) return null
  const thirdVertex = face.polygon[thirdVertexIndices[0]]
  const edgeX = constraint.end.x - constraint.start.x
  const edgeZ = constraint.end.z - constraint.start.z
  const thirdVertexSide = edgeX * (thirdVertex.z - constraint.start.z)
    - edgeZ * (thirdVertex.x - constraint.start.x)
  if (!Number.isFinite(thirdVertexSide) || thirdVertexSide === 0) return null
  return {
    direction,
    thirdVertexSide,
  }
}

function matchingVertexIndices(
  polygon: readonly FoldPreviewCollisionPoint[],
  endpoint: FoldPreviewHingeContactConstraint['start'],
) {
  const byId = polygon
    .map((point, index) => ({ point, index }))
    .filter(({ point }) => point?.vertexId === endpoint.vertexId)
  if (byId.length !== 1) return []
  return byId[0].point.x === endpoint.x && byId[0].point.z === endpoint.z
    ? [byId[0].index]
    : []
}

function validConstraintRecord(
  constraint: FoldPreviewHingeContactConstraint,
) {
  if (
    !constraint
    || !validId(constraint.edgeId)
    || !validId(constraint.leftFaceId)
    || !validId(constraint.rightFaceId)
    || constraint.leftFaceId === constraint.rightFaceId
    || constraint.thicknessRule !== 'centered_mid_surface_v1'
    || !validEndpoint(constraint.start)
    || !validEndpoint(constraint.end)
    || constraint.start.vertexId === constraint.end.vertexId
  ) return false
  const deltaX = constraint.end.x - constraint.start.x
  const deltaZ = constraint.end.z - constraint.start.z
  return Number.isFinite(deltaX)
    && Number.isFinite(deltaZ)
    && Math.hypot(deltaX, deltaZ) > 0
}

function validEndpoint(endpoint: FoldPreviewHingeContactConstraint['start']) {
  return Boolean(
    endpoint
    && validId(endpoint.vertexId)
    && Number.isFinite(endpoint.x)
    && Number.isFinite(endpoint.z),
  )
}

function sameFacePair(
  adjacency: FoldPreviewCollisionAdjacency,
  constraint: FoldPreviewHingeContactConstraint,
) {
  return sameIds(
    adjacency.firstFaceId,
    adjacency.secondFaceId,
    constraint.leftFaceId,
    constraint.rightFaceId,
  )
}

function sameIds(first: string, second: string, left: string, right: string) {
  return (first === left && second === right) || (first === right && second === left)
}

function transformedRestPoint(
  point: FoldPreviewHingeContactConstraint['start'],
  transform: Matrix4,
) {
  const transformed = new Vector3(point.x, 0, point.z).applyMatrix4(transform)
  return finiteVector(transformed) ? transformed : null
}

function transformedNormal(transform: Matrix4) {
  const normal = new Vector3(0, 1, 0).transformDirection(transform)
  return finiteVector(normal) && normal.lengthSq() > 0 ? normal : null
}

function validPair(pair: FoldPreviewHingeContactPair) {
  return Boolean(
    pair
    && Number.isSafeInteger(pair.firstTriangleIndex)
    && pair.firstTriangleIndex >= 0
    && Number.isSafeInteger(pair.secondTriangleIndex)
    && pair.secondTriangleIndex >= 0
    && Array.isArray(pair.firstVertices)
    && pair.firstVertices.length === 6
    && Array.isArray(pair.secondVertices)
    && pair.secondVertices.length === 6
    && (
      pair.geometryClass === 'touching'
      || pair.geometryClass === 'penetrating'
      || pair.geometryClass === 'indeterminate'
    ),
  )
}

function pairMatchesPreparedGeometry(
  pair: FoldPreviewHingeContactPair,
  firstFace: PreparedPolicyFace,
  secondFace: PreparedPolicyFace,
  transforms: ReadonlyMap<string, Matrix4>,
  thickness: number,
) {
  const firstTransform = transforms?.get(firstFace.id)
  const secondTransform = transforms?.get(secondFace.id)
  if (!firstTransform || !secondTransform) return false
  const firstExpected = expectedPrismVertices(
    firstFace,
    pair.firstTriangleIndex,
    firstTransform,
    thickness,
  )
  const secondExpected = expectedPrismVertices(
    secondFace,
    pair.secondTriangleIndex,
    secondTransform,
    thickness,
  )
  return Boolean(
    firstExpected
    && secondExpected
    && sameVertices(pair.firstVertices, firstExpected)
    && sameVertices(pair.secondVertices, secondExpected),
  )
}

function expectedPrismVertices(
  face: PreparedPolicyFace,
  triangleIndex: number,
  transform: Matrix4,
  thickness: number,
): readonly Vector3[] | null {
  const triangle = face.triangles[triangleIndex]
  const halfThickness = thickness / 2
  if (
    !triangle
    || !Number.isFinite(halfThickness)
    || halfThickness < 0
  ) return null
  const top = triangle.map((index) =>
    transformedFacePoint(face.polygon[index], halfThickness, transform))
  const bottom = triangle.map((index) =>
    transformedFacePoint(face.polygon[index], -halfThickness, transform))
  if ([...top, ...bottom].some((point) => !point)) return null
  return [...top, ...bottom] as Vector3[]
}

function transformedFacePoint(
  point: FoldPreviewCollisionPoint,
  y: number,
  transform: Matrix4,
) {
  const transformed = new Vector3(point.x, y, point.z).applyMatrix4(transform)
  return finiteVector(transformed) ? transformed : null
}

function sameVertices(first: readonly Vector3[], second: readonly Vector3[]) {
  return first.length === second.length && first.every((point, index) => {
    const expected = second[index]
    return finiteVector(point)
      && point.x === expected.x
      && point.y === expected.y
      && point.z === expected.z
  })
}

function finiteVector(vector: Vector3) {
  return vector
    && Number.isFinite(vector.x)
    && Number.isFinite(vector.y)
    && Number.isFinite(vector.z)
}

function rigidTransform(transform: Matrix4) {
  const elements = transform?.elements
  if (
    !Array.isArray(elements)
    || elements.length !== 16
    || !elements.every(Number.isFinite)
  ) return false
  const first = new Vector3(elements[0], elements[1], elements[2])
  const second = new Vector3(elements[4], elements[5], elements[6])
  const third = new Vector3(elements[8], elements[9], elements[10])
  const determinant = first.dot(second.clone().cross(third))
  return Math.abs(elements[3]) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(elements[7]) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(elements[11]) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(elements[15] - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(first.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(second.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(third.lengthSq() - 1) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(first.dot(second)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(first.dot(third)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(second.dot(third)) <= RIGID_TRANSFORM_TOLERANCE
    && Math.abs(determinant - 1) <= RIGID_TRANSFORM_TOLERANCE
}

function indeterminate(
  hingeEdgeIds: readonly string[],
  reason: Extract<
    FoldPreviewHingeContactDecision,
    { kind: 'indeterminate' }
  >['reason'],
): FoldPreviewHingeContactDecision {
  return {
    kind: 'indeterminate',
    hingeEdgeIds: [...hingeEdgeIds],
    reason,
  }
}

function staticSupportNotProven(
  hingeEdgeIds: readonly string[],
  reason: Extract<
    FoldPreviewStaticHingeTrianglePairSupportDecision,
    { kind: 'not_proven' }
  >['reason'],
): FoldPreviewStaticHingeTrianglePairSupportDecision {
  return {
    kind: 'not_proven',
    hingeEdgeIds: [...hingeEdgeIds],
    reason,
  }
}

function validId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}
