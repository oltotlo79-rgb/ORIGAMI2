import { Matrix4, Vector3, type Matrix4 as ThreeMatrix4 } from 'three'
import {
  collectFoldTreeDependentFaces,
  rerootFoldPreviewTree,
} from './foldPreviewAnchoring.ts'
import {
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
  type FoldPreviewCollisionAdjacency,
} from './foldPreviewCollision.ts'
import {
  createFoldPreviewContinuousMotionJob,
  type FoldPreviewContinuousMotionJob,
  type FoldPreviewContinuousMotionOptions,
  type FoldPreviewContinuousPointDecision,
} from './foldPreviewContinuousMotion.ts'
import { findFoldPreviewSingleAxisSweptAabb } from './foldPreviewContinuousInterval.ts'
import {
  prepareFoldPreviewHingeContactPolicy,
  type FoldPreviewHingeContactConstraint,
  type FoldPreviewHingePolicyFace,
} from './foldPreviewHingeCollision.ts'
import {
  triangulateFoldPreviewPolygon,
  type FoldPreviewTriangleIndices,
} from './foldPreviewGeometry.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
  type FoldPreviewTreeKinematics,
} from './foldPreviewKinematics.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from './foldPreviewModel.ts'
import {
  MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
  calculateFoldPreviewNarrowPhaseNumericalMargin,
  prepareFoldPreviewNarrowPhase,
} from './foldPreviewNarrowCollision.ts'

export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_CROSS_TRIANGLE_PAIRS =
  1_000_000
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS =
  1_000_000
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS =
  1_000_000

const HINGE_INTERVAL_NUMERICAL_SAFETY_FACTOR = 8
const MOTION_POSE_EQUIVALENCE_FACTOR = 1_024

export type FoldPreviewTreeSingleHingeContinuousBlocker = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  geometryClass: 'touching' | 'penetrating' | 'indeterminate'
  hingeDecisionKind?: string
}>

export type FoldPreviewTreeSingleHingeContinuousOptions =
  FoldPreviewContinuousMotionOptions & Readonly<{
    /**
     * Optional per-job tightening of the hard cumulative interval pair cap.
     * This counts every cross-cut triangle pair visited by interval callbacks.
     */
    maxIntervalPairVisits?: number
    /**
     * Optional per-job tightening of the hard cumulative narrow-phase triangle
     * test cap. A point call starts only when its complete worst-case pair scan
     * fits in the remaining budget.
     */
    maxPointTriangleTests?: number
  }>

export type FoldPreviewTreeSingleHingeContinuousAnalyzer = Readonly<{
  fixedFaceId: string
  selectedHingeEdgeId: string
  parentFaceId: string
  childFaceId: string
  stationaryFaceIds: readonly string[]
  movingFaceIds: readonly string[]
  crossTrianglePairs: number
  staticallySupportedTrianglePairs: number
  createJob(
    startAngles: readonly FoldPreviewHingeAngle[],
    targetSelectedAngleDegrees: number,
    thickness: number,
    options?: FoldPreviewTreeSingleHingeContinuousOptions,
  ): FoldPreviewContinuousMotionJob<
    FoldPreviewTreeSingleHingeContinuousBlocker
  > | null
}>

type PreparedFace = Readonly<{
  id: string
  polygon: FoldPreviewFaceModel['polygon']
  triangles: readonly FoldPreviewTriangleIndices[]
}>

type Bounds = Readonly<{
  minX: number
  minY: number
  minZ: number
  maxX: number
  maxY: number
  maxZ: number
}>

type WorldAxis = Readonly<{
  start: Readonly<{ x: number; y: number; z: number }>
  end: Readonly<{ x: number; y: number; z: number }>
}>

type ResolvedJobOptions = Readonly<{
  motion: FoldPreviewContinuousMotionOptions
  maxIntervalPairVisits: number
  maxPointTriangleTests: number
}>

/**
 * Prepares collision analysis for one selected hinge in a validated fold tree.
 *
 * The selected child subtree is the only moving body. Every other hinge angle
 * remains fixed, so the subtree receives one common rigid world-axis rotation.
 * Point checks include every face; interval checks need only cross-cut pairs.
 */
export function prepareFoldPreviewTreeSingleHingeContinuousCollision(
  model: FoldGraphPreviewModel,
  fixedFaceId: string,
  selectedHingeEdgeId: string,
): FoldPreviewTreeSingleHingeContinuousAnalyzer | null {
  try {
    const snapshot = snapshotModel(model)
    if (
      !snapshot
      || typeof fixedFaceId !== 'string'
      || fixedFaceId.length === 0
      || typeof selectedHingeEdgeId !== 'string'
      || selectedHingeEdgeId.length === 0
      || snapshot.kinematics.kind !== 'tree'
    ) return null

    const tree = rerootFoldPreviewTree(snapshot.kinematics, fixedFaceId)
    if (!tree || !modelMatchesTree(snapshot, tree)) return null
    const selectedJoint = tree.joints.find(
      (joint) => joint.hinge.edgeId === selectedHingeEdgeId,
    )
    if (!selectedJoint) return null

    const movingFaceIds = collectFoldTreeDependentFaces(
      tree,
      selectedHingeEdgeId,
    )
    if (!movingFaceIds || movingFaceIds.length === 0) return null
    const movingFaceIdSet = new Set(movingFaceIds)
    if (
      movingFaceIdSet.size !== movingFaceIds.length
      || movingFaceIdSet.has(selectedJoint.parentFaceId)
      || !movingFaceIdSet.has(selectedJoint.childFaceId)
    ) return null

    const preparedFaces: PreparedFace[] = snapshot.faces.map((face) => ({
      id: face.id,
      polygon: face.polygon,
      triangles: triangulateFoldPreviewPolygon(face.polygon),
    }))
    if (
      preparedFaces.some((face) => face.triangles.length === 0)
      || new Set(preparedFaces.map((face) => face.id)).size
        !== preparedFaces.length
    ) return null
    const facesById = new Map(preparedFaces.map((face) => [face.id, face]))
    if (
      facesById.size !== tree.joints.length + 1
      || movingFaceIds.some((faceId) => !facesById.has(faceId))
    ) return null

    const stationaryFaces = preparedFaces.filter(
      (face) => !movingFaceIdSet.has(face.id),
    )
    const movingFaces = preparedFaces.filter(
      (face) => movingFaceIdSet.has(face.id),
    )
    const selectedParentFace = facesById.get(selectedJoint.parentFaceId)
    const selectedChildFace = facesById.get(selectedJoint.childFaceId)
    if (
      stationaryFaces.length === 0
      || movingFaces.length === 0
      || !selectedParentFace
      || !selectedChildFace
      || movingFaceIdSet.has(selectedParentFace.id)
      || !movingFaceIdSet.has(selectedChildFace.id)
      || !onlySelectedHingeCrossesCut(
        snapshot.hinges,
        movingFaceIdSet,
        selectedHingeEdgeId,
      )
    ) return null

    const adjacency: readonly FoldPreviewCollisionAdjacency[] =
      snapshot.hinges.map((hinge) => ({
        edgeId: hinge.edgeId,
        firstFaceId: hinge.leftFaceId,
        secondFaceId: hinge.rightFaceId,
      }))
    const constraints: readonly FoldPreviewHingeContactConstraint[] =
      snapshot.hinges.map((hinge) => ({
        edgeId: hinge.edgeId,
        leftFaceId: hinge.leftFaceId,
        rightFaceId: hinge.rightFaceId,
        start: {
          vertexId: hinge.start.vertexId,
          x: hinge.start.x,
          z: hinge.start.z,
        },
        end: {
          vertexId: hinge.end.vertexId,
          x: hinge.end.x,
          z: hinge.end.z,
        },
        thicknessRule: 'centered_mid_surface_v1',
      }))
    const policyFaces: FoldPreviewHingePolicyFace[] =
      preparedFaces.map((face) => ({
        id: face.id,
        polygon: face.polygon,
        triangles: face.triangles,
      }))
    const hingePolicy = prepareFoldPreviewHingeContactPolicy(
      policyFaces,
      adjacency,
      constraints,
    )
    const pointAnalyzer = prepareFoldPreviewNarrowPhase(
      snapshot.faces,
      adjacency,
      constraints,
    )
    if (!hingePolicy || !pointAnalyzer) return null

    const crossTrianglePairs = trianglePairProduct(
      stationaryFaces,
      movingFaces,
    )
    const pointTrianglePairUpperBound =
      allFaceTrianglePairUpperBound(preparedFaces)
    if (
      crossTrianglePairs === null
      || crossTrianglePairs <= 0
      || crossTrianglePairs
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_CROSS_TRIANGLE_PAIRS
      || pointTrianglePairUpperBound === null
      || pointTrianglePairUpperBound <= 0
      || pointTrianglePairUpperBound
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS
    ) return null

    const staticallySupportedPairs =
      selectedParentFace.triangles.map((_, firstTriangleIndex) =>
        selectedChildFace.triangles.map((__, secondTriangleIndex) =>
          hingePolicy.proveStaticTrianglePairSupport({
            firstFaceId: selectedParentFace.id,
            secondFaceId: selectedChildFace.id,
            hingeEdgeIds: [selectedHingeEdgeId],
            firstTriangleIndex,
            secondTriangleIndex,
          }).kind === 'proven_static_hinge_support'))
    const staticallySupportedTrianglePairs =
      staticallySupportedPairs.reduce(
        (total, row) => total + row.filter(Boolean).length,
        0,
      )

    return Object.freeze({
      fixedFaceId: tree.rootFaceId,
      selectedHingeEdgeId,
      parentFaceId: selectedJoint.parentFaceId,
      childFaceId: selectedJoint.childFaceId,
      stationaryFaceIds: Object.freeze(
        stationaryFaces.map((face) => face.id),
      ),
      movingFaceIds: Object.freeze([...movingFaceIds]),
      crossTrianglePairs,
      staticallySupportedTrianglePairs,
      createJob(
        startAngles: readonly FoldPreviewHingeAngle[],
        targetSelectedAngleDegrees: number,
        thickness: number,
        options: FoldPreviewTreeSingleHingeContinuousOptions = {},
      ) {
        try {
          const resolvedOptions = resolveJobOptions(options)
          const normalizedAngles = normalizeAngles(tree, startAngles)
        if (
          !resolvedOptions
          || !normalizedAngles
          || !validAngle(targetSelectedAngleDegrees)
          || !Number.isFinite(thickness)
          || thickness < 0
        ) return null
        const startSelectedAngle = normalizedAngles.find(
          (angle) => angle.edgeId === selectedHingeEdgeId,
        )?.angleDegrees
        if (!validAngle(startSelectedAngle)) return null

        const basePose = calculateFoldTreePoseWithAngles(tree, {
          kind: 'per_hinge',
          angles: normalizedAngles,
        })
        const selectedHingeTransform =
          basePose?.hingeTransforms.get(selectedHingeEdgeId)
        if (!basePose || !selectedHingeTransform) return null
        const worldAxis = transformHingeAxis(
          selectedJoint.hinge,
          selectedHingeTransform,
        )
        if (!worldAxis) return null

        const coordinateScale = continuousCoordinateScaleUpperBound(
          preparedFaces,
          basePose.faceTransforms,
          movingFaceIdSet,
          worldAxis,
          thickness,
        )
        const numericalMargin = coordinateScale === null
          ? null
          : calculateFoldPreviewNarrowPhaseNumericalMargin(coordinateScale)
        if (numericalMargin === null) return null
        const staticSupportNumericallySafe = thickness
          >= numericalMargin * HINGE_INTERVAL_NUMERICAL_SAFETY_FACTOR
        const stationaryBounds = triangleBoundsAtPose(
          stationaryFaces,
          basePose.faceTransforms,
          worldAxis,
          thickness,
          0,
        )
        if (!stationaryBounds) return null

        const angleAt = (time: number) =>
          startSelectedAngle
          + (targetSelectedAngleDegrees - startSelectedAngle) * time
        const faceTransformsAt = (
          selectedAngleDegrees: number,
        ): ReadonlyMap<string, Matrix4> | null => {
          if (!validAngle(selectedAngleDegrees)) return null
          const deltaRadians = (
            selectedAngleDegrees - startSelectedAngle
          ) * selectedJoint.childRotationSign * Math.PI / 180
          const movingRotation = deltaRadians === 0
            ? new Matrix4()
            : worldAxisRotation(worldAxis, deltaRadians)
          if (!movingRotation) return null
          const transforms = new Map<string, Matrix4>()
          for (const face of preparedFaces) {
            const baseTransform = basePose.faceTransforms.get(face.id)
            if (!baseTransform) return null
            const transform = movingFaceIdSet.has(face.id)
              ? movingRotation.clone().multiply(baseTransform)
              : baseTransform.clone()
            if (!transform.elements.every(Number.isFinite)) return null
            transforms.set(face.id, transform)
          }
          return transforms.size === preparedFaces.length ? transforms : null
        }
        for (const probeAngle of [
          targetSelectedAngleDegrees,
          startSelectedAngle
            + (targetSelectedAngleDegrees - startSelectedAngle) / 2,
        ]) {
          const rigidSubtreeTransforms = faceTransformsAt(probeAngle)
          const corePose = calculateFoldTreePoseWithAngles(tree, {
            kind: 'per_hinge',
            angles: normalizedAngles.map((angle) => ({
              edgeId: angle.edgeId,
              angleDegrees: angle.edgeId === selectedHingeEdgeId
                ? probeAngle
                : angle.angleDegrees,
            })),
          })
          if (
            !rigidSubtreeTransforms
            || !corePose
            || !equivalentFaceTransforms(
              preparedFaces,
              rigidSubtreeTransforms,
              corePose.faceTransforms,
            )
          ) return null
        }

        let pointTriangleTests = 0
        let intervalPairVisits = 0
        const pointDecision = (
          time: number,
        ): FoldPreviewContinuousPointDecision<
          FoldPreviewTreeSingleHingeContinuousBlocker
        > => {
          const angle = angleAt(time)
          if (!validAngle(angle)) {
            return { kind: 'indeterminate', reason: 'invalid_interpolated_angle' }
          }
          if (
            pointTriangleTests
            > resolvedOptions.maxPointTriangleTests
              - pointTrianglePairUpperBound
          ) {
            return {
              kind: 'indeterminate',
              reason: 'tree_point_triangle_work_limit',
            }
          }
          const faceTransforms = faceTransformsAt(angle)
          if (!faceTransforms) {
            return { kind: 'indeterminate', reason: 'pose_unavailable' }
          }
          const result = pointAnalyzer.analyze(faceTransforms, thickness)
          if (!result) {
            return {
              kind: 'indeterminate',
              reason: 'point_collision_unavailable',
            }
          }
          if (
            !Number.isSafeInteger(result.trianglePairTests)
            || result.trianglePairTests < 0
            || result.trianglePairTests > pointTrianglePairUpperBound
          ) {
            return {
              kind: 'indeterminate',
              reason: 'point_collision_work_accounting',
            }
          }
          pointTriangleTests += result.trianglePairTests

          let unknownReason: string | null = null
          for (const interaction of result.interactions) {
            if (interaction.relation === 'hinge_adjacent') {
              const decision = interaction.hingeDecision
              if (decision?.kind === 'allowed_by_hinge_model') continue
              if (
                decision?.kind === 'outside_hinge_penetration'
                || decision?.kind === 'outside_hinge_contact'
              ) {
                return {
                  kind: 'blocked',
                  blocker: blockerFor(interaction),
                }
              }
              unknownReason ??= decision?.kind === 'indeterminate'
                ? `hinge_${decision.reason}`
                : 'hinge_decision_unavailable'
              continue
            }
            if (interaction.geometryClass === 'indeterminate') {
              unknownReason ??= 'non_adjacent_geometry_indeterminate'
            } else {
              return {
                kind: 'blocked',
                blocker: blockerFor(interaction),
              }
            }
          }
          return unknownReason
            ? { kind: 'indeterminate', reason: unknownReason }
            : { kind: 'safe' }
        }

          return createFoldPreviewContinuousMotionJob({
            evaluatePoint: pointDecision,
            certifyInterval(startTime, endTime) {
            const startAngle = angleAt(startTime)
            const endAngle = angleAt(endTime)
            if (!validAngle(startAngle) || !validAngle(endAngle)) {
              return {
                kind: 'indeterminate',
                reason: 'invalid_interpolated_interval',
              }
            }
            if (Math.max(startAngle, endAngle) >= 180) {
              return { kind: 'unresolved' }
            }
            if (
              staticallySupportedTrianglePairs > 0
              && !staticSupportNumericallySafe
            ) {
              return {
                kind: 'indeterminate',
                reason: 'hinge_interval_numerical_margin',
              }
            }

            const midpointTime = startTime + (endTime - startTime) / 2
            const midpointAngle = angleAt(midpointTime)
            const midpointTransforms = faceTransformsAt(midpointAngle)
            if (!midpointTransforms) {
              return {
                kind: 'indeterminate',
                reason: 'midpoint_pose_unavailable',
              }
            }
            const movingBounds = triangleBoundsAtPose(
              movingFaces,
              midpointTransforms,
              worldAxis,
              thickness,
              Math.abs(endAngle - startAngle) * Math.PI / 180,
            )
            if (!movingBounds) {
              return {
                kind: 'indeterminate',
                reason: 'swept_bounds_unavailable',
              }
            }

            for (const stationaryFace of stationaryFaces) {
              const firstBounds = stationaryBounds.get(stationaryFace.id)
              if (!firstBounds) {
                return {
                  kind: 'indeterminate',
                  reason: 'swept_bounds_unavailable',
                }
              }
              for (const movingFace of movingFaces) {
                const secondBounds = movingBounds.get(movingFace.id)
                if (!secondBounds) {
                  return {
                    kind: 'indeterminate',
                    reason: 'swept_bounds_unavailable',
                  }
                }
                for (
                  let firstTriangleIndex = 0;
                  firstTriangleIndex < stationaryFace.triangles.length;
                  firstTriangleIndex += 1
                ) {
                  for (
                    let secondTriangleIndex = 0;
                    secondTriangleIndex < movingFace.triangles.length;
                    secondTriangleIndex += 1
                  ) {
                    if (
                      intervalPairVisits
                      >= resolvedOptions.maxIntervalPairVisits
                    ) {
                      return {
                        kind: 'indeterminate',
                        reason: 'tree_interval_pair_work_limit',
                      }
                    }
                    intervalPairVisits += 1
                    if (
                      stationaryFace.id === selectedParentFace.id
                      && movingFace.id === selectedChildFace.id
                      && staticallySupportedPairs[firstTriangleIndex]
                        ?.[secondTriangleIndex] === true
                    ) continue
                    const first = firstBounds[firstTriangleIndex]
                    const second = secondBounds[secondTriangleIndex]
                    if (!first || !second) {
                      return {
                        kind: 'indeterminate',
                        reason: 'swept_bounds_unavailable',
                      }
                    }
                    if (!strictlySeparated(first, second)) {
                      return { kind: 'unresolved' }
                    }
                  }
                }
              }
            }
            return { kind: 'clear' }
            },
          }, resolvedOptions.motion)
        } catch {
          return null
        }
      },
    })
  } catch {
    return null
  }
}

function triangleBoundsAtPose(
  faces: readonly PreparedFace[],
  faceTransforms: ReadonlyMap<string, Matrix4>,
  worldAxis: WorldAxis,
  thickness: number,
  angularSpanRadians: number,
): ReadonlyMap<string, readonly Bounds[]> | null {
  const result = new Map<string, readonly Bounds[]>()
  for (const face of faces) {
    const transform = faceTransforms.get(face.id)
    if (!transform) return null
    const faceBounds: Bounds[] = []
    for (const triangle of face.triangles) {
      const bounds = findFoldPreviewSingleAxisSweptAabb(
        prismVertices(face, triangle, transform, thickness),
        worldAxis.start,
        worldAxis.end,
        angularSpanRadians,
      )
      if (!bounds) return null
      faceBounds.push(bounds)
    }
    result.set(face.id, faceBounds)
  }
  return result.size === faces.length ? result : null
}

function prismVertices(
  face: PreparedFace,
  triangle: FoldPreviewTriangleIndices,
  transform: ThreeMatrix4,
  thickness: number,
) {
  const halfThickness = thickness / 2
  const vertices: Vector3[] = []
  for (const y of [halfThickness, -halfThickness]) {
    for (const index of triangle) {
      const point = face.polygon[index]
      vertices.push(new Vector3(point.x, y, point.z).applyMatrix4(transform))
    }
  }
  return vertices
}

function transformHingeAxis(
  hinge: FoldPreviewHingeModel,
  transform: Matrix4,
): WorldAxis | null {
  const start = new Vector3(
    hinge.start.x,
    0,
    hinge.start.z,
  ).applyMatrix4(transform)
  const end = new Vector3(
    hinge.end.x,
    0,
    hinge.end.z,
  ).applyMatrix4(transform)
  const length = start.distanceTo(end)
  if (
    ![start.x, start.y, start.z, end.x, end.y, end.z, length]
      .every(Number.isFinite)
    || length <= 0
  ) return null
  return {
    start: { x: start.x, y: start.y, z: start.z },
    end: { x: end.x, y: end.y, z: end.z },
  }
}

function worldAxisRotation(
  axis: WorldAxis,
  radians: number,
): Matrix4 | null {
  if (!Number.isFinite(radians)) return null
  const direction = new Vector3(
    axis.end.x - axis.start.x,
    axis.end.y - axis.start.y,
    axis.end.z - axis.start.z,
  )
  const length = direction.length()
  if (!Number.isFinite(length) || length <= 0) return null
  direction.multiplyScalar(1 / length)
  const rotation = new Matrix4()
    .makeTranslation(axis.start.x, axis.start.y, axis.start.z)
    .multiply(new Matrix4().makeRotationAxis(direction, radians))
    .multiply(new Matrix4().makeTranslation(
      -axis.start.x,
      -axis.start.y,
      -axis.start.z,
    ))
  return rotation.elements.every(Number.isFinite) ? rotation : null
}

function continuousCoordinateScaleUpperBound(
  faces: readonly PreparedFace[],
  baseTransforms: ReadonlyMap<string, Matrix4>,
  movingFaceIds: ReadonlySet<string>,
  worldAxis: WorldAxis,
  thickness: number,
): number | null {
  const halfThickness = thickness / 2
  const axisOriginRadius = Math.hypot(
    worldAxis.start.x,
    worldAxis.start.y,
    worldAxis.start.z,
  )
  if (
    !Number.isFinite(halfThickness)
    || !Number.isFinite(axisOriginRadius)
  ) return null
  let scale = Math.max(
    1,
    Math.abs(worldAxis.start.x),
    Math.abs(worldAxis.start.y),
    Math.abs(worldAxis.start.z),
    Math.abs(worldAxis.end.x),
    Math.abs(worldAxis.end.y),
    Math.abs(worldAxis.end.z),
  )
  for (const face of faces) {
    const transform = baseTransforms.get(face.id)
    if (!transform) return null
    for (const point of face.polygon) {
      for (const y of halfThickness === 0
        ? [0]
        : [-halfThickness, halfThickness]) {
        const world = new Vector3(point.x, y, point.z).applyMatrix4(transform)
        if (![world.x, world.y, world.z].every(Number.isFinite)) return null
        const bound = movingFaceIds.has(face.id)
          ? axisOriginRadius + Math.hypot(
              world.x - worldAxis.start.x,
              world.y - worldAxis.start.y,
              world.z - worldAxis.start.z,
            )
          : Math.max(
              Math.abs(world.x),
              Math.abs(world.y),
              Math.abs(world.z),
            )
        if (!Number.isFinite(bound)) return null
        scale = Math.max(scale, bound)
      }
    }
  }
  return Number.isFinite(scale) ? scale : null
}

function blockerFor(
  interaction: Readonly<{
    firstFaceId: string
    secondFaceId: string
    relation: 'hinge_adjacent' | 'non_adjacent'
    geometryClass: 'touching' | 'penetrating' | 'indeterminate'
    hingeDecision?: Readonly<{ kind: string }>
  }>,
): FoldPreviewTreeSingleHingeContinuousBlocker {
  return Object.freeze({
    firstFaceId: interaction.firstFaceId,
    secondFaceId: interaction.secondFaceId,
    relation: interaction.relation,
    geometryClass: interaction.geometryClass,
    ...(interaction.hingeDecision
      ? { hingeDecisionKind: interaction.hingeDecision.kind }
      : {}),
  })
}

function strictlySeparated(first: Bounds, second: Bounds) {
  return first.maxX < second.minX
    || second.maxX < first.minX
    || first.maxY < second.minY
    || second.maxY < first.minY
    || first.maxZ < second.minZ
    || second.maxZ < first.minZ
}

function equivalentFaceTransforms(
  faces: readonly PreparedFace[],
  first: ReadonlyMap<string, Matrix4>,
  second: ReadonlyMap<string, Matrix4>,
) {
  if (first.size !== faces.length || second.size !== faces.length) return false
  for (const face of faces) {
    const firstTransform = first.get(face.id)
    const secondTransform = second.get(face.id)
    if (!firstTransform || !secondTransform) return false
    for (let index = 0; index < 16; index += 1) {
      const firstValue = firstTransform.elements[index]
      const secondValue = secondTransform.elements[index]
      const scale = Math.max(
        1,
        Math.abs(firstValue),
        Math.abs(secondValue),
      )
      const tolerance = scale
        * Number.EPSILON
        * MOTION_POSE_EQUIVALENCE_FACTOR
      if (
        !Number.isFinite(firstValue)
        || !Number.isFinite(secondValue)
        || !Number.isFinite(tolerance)
        || Math.abs(firstValue - secondValue) > tolerance
      ) return false
    }
  }
  return true
}

function normalizeAngles(
  tree: FoldPreviewTreeKinematics,
  input: readonly FoldPreviewHingeAngle[],
): readonly FoldPreviewHingeAngle[] | null {
  if (!Array.isArray(input) || input.length !== tree.joints.length) return null
  const byEdgeId = new Map<string, number>()
  for (const angle of input) {
    if (
      !angle
      || typeof angle !== 'object'
      || typeof angle.edgeId !== 'string'
      || angle.edgeId.length === 0
      || byEdgeId.has(angle.edgeId)
      || !validAngle(angle.angleDegrees)
    ) return null
    byEdgeId.set(angle.edgeId, angle.angleDegrees)
  }
  const normalized: FoldPreviewHingeAngle[] = []
  for (const joint of tree.joints) {
    const angleDegrees = byEdgeId.get(joint.hinge.edgeId)
    if (!validAngle(angleDegrees)) return null
    normalized.push({
      edgeId: joint.hinge.edgeId,
      angleDegrees,
    })
  }
  return normalized
}

function resolveJobOptions(
  options: FoldPreviewTreeSingleHingeContinuousOptions,
): ResolvedJobOptions | null {
  if (!options || typeof options !== 'object' || Array.isArray(options)) return null
  const maxIntervalPairVisits = options.maxIntervalPairVisits
    ?? MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS
  const maxPointTriangleTests = options.maxPointTriangleTests
    ?? MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS
  if (
    !Number.isSafeInteger(maxIntervalPairVisits)
    || maxIntervalPairVisits <= 0
    || maxIntervalPairVisits
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS
    || !Number.isSafeInteger(maxPointTriangleTests)
    || maxPointTriangleTests <= 0
    || maxPointTriangleTests
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS
  ) return null
  return {
    motion: {
      maxDepth: options.maxDepth,
      maxIntervalTests: options.maxIntervalTests,
      minTimeSpan: options.minTimeSpan,
    },
    maxIntervalPairVisits,
    maxPointTriangleTests,
  }
}

function trianglePairProduct(
  firstFaces: readonly PreparedFace[],
  secondFaces: readonly PreparedFace[],
): number | null {
  const firstTriangles = triangleCount(firstFaces)
  const secondTriangles = triangleCount(secondFaces)
  if (firstTriangles === null || secondTriangles === null) return null
  const product = firstTriangles * secondTriangles
  return Number.isSafeInteger(product) ? product : null
}

function allFaceTrianglePairUpperBound(
  faces: readonly PreparedFace[],
): number | null {
  let total = 0
  for (let firstIndex = 0; firstIndex < faces.length; firstIndex += 1) {
    for (
      let secondIndex = firstIndex + 1;
      secondIndex < faces.length;
      secondIndex += 1
    ) {
      const pairs = faces[firstIndex].triangles.length
        * faces[secondIndex].triangles.length
      total += pairs
      if (!Number.isSafeInteger(pairs) || !Number.isSafeInteger(total)) {
        return null
      }
    }
  }
  return total
}

function triangleCount(faces: readonly PreparedFace[]): number | null {
  let count = 0
  for (const face of faces) {
    count += face.triangles.length
    if (!Number.isSafeInteger(count)) return null
  }
  return count
}

function onlySelectedHingeCrossesCut(
  hinges: readonly FoldPreviewHingeModel[],
  movingFaceIds: ReadonlySet<string>,
  selectedHingeEdgeId: string,
) {
  let crossingHinges = 0
  for (const hinge of hinges) {
    const crosses = movingFaceIds.has(hinge.leftFaceId)
      !== movingFaceIds.has(hinge.rightFaceId)
    if (!crosses) continue
    if (hinge.edgeId !== selectedHingeEdgeId) return false
    crossingHinges += 1
  }
  return crossingHinges === 1
}

function validAngle(value: unknown): value is number {
  return Number.isFinite(value)
    && (value as number) >= 0
    && (value as number) <= 180
}

function snapshotModel(
  model: FoldGraphPreviewModel,
): FoldGraphPreviewModel | null {
  try {
    if (!model || typeof model !== 'object') return null

    const kind = model.kind
    const sourceFaces = model.faces
    const sourceHinges = model.hinges
    const sourceKinematics = model.kinematics
    if (
      kind !== 'fold_graph'
      || !Array.isArray(sourceFaces)
      || !Array.isArray(sourceHinges)
      || !sourceKinematics
      || typeof sourceKinematics !== 'object'
    ) return null

    const kinematicsKind = sourceKinematics.kind
    if (kinematicsKind !== 'tree') return null
    const sourceJoints = sourceKinematics.joints
    if (!Array.isArray(sourceJoints)) return null

    // Snapshot every collection length once. All collection-size limits and
    // tree cardinalities are rejected before any element or vertex is copied.
    const faceCount = sourceFaces.length
    const hingeCount = sourceHinges.length
    const jointCount = sourceJoints.length
    if (
      !Number.isSafeInteger(faceCount)
      || faceCount < 1
      || faceCount > MAX_FOLD_PREVIEW_COLLISION_FACES
      || !Number.isSafeInteger(hingeCount)
      || hingeCount > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
      || !Number.isSafeInteger(jointCount)
      || jointCount > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
      || faceCount !== jointCount + 1
      || hingeCount !== jointCount
    ) return null

    const sourceFaceSnapshots: {
      id: string
      polygon: FoldPreviewFaceModel['polygon']
      polygonLength: number
    }[] = []
    let preparedVertexCount = 0
    for (let faceIndex = 0; faceIndex < faceCount; faceIndex += 1) {
      const face = sourceFaces[faceIndex]
      if (!face || typeof face !== 'object') return null
      const id = face.id
      const sourcePolygon = face.polygon
      if (
        typeof id !== 'string'
        || id.length === 0
        || !Array.isArray(sourcePolygon)
      ) return null
      const polygonLength = sourcePolygon.length
      if (
        !Number.isSafeInteger(polygonLength)
        || polygonLength < 3
        || polygonLength
          > MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES
            - preparedVertexCount
      ) return null
      preparedVertexCount += polygonLength
      sourceFaceSnapshots.push({ id, polygon: sourcePolygon, polygonLength })
    }

    // Vertex objects are copied only after the cumulative preparation cap has
    // been established across every polygon.
    const faces: FoldPreviewFaceModel[] = []
    for (
      let faceIndex = 0;
      faceIndex < sourceFaceSnapshots.length;
      faceIndex += 1
    ) {
      const sourceFace = sourceFaceSnapshots[faceIndex]
      if (!sourceFace) return null
      const polygon: FoldPreviewFaceModel['polygon'][number][] = []
      for (
        let pointIndex = 0;
        pointIndex < sourceFace.polygonLength;
        pointIndex += 1
      ) {
        const point = snapshotPoint(sourceFace.polygon[pointIndex])
        if (!point) return null
        polygon.push(point)
      }
      faces.push({ id: sourceFace.id, polygon })
    }

    const hinges: FoldPreviewHingeModel[] = []
    for (let hingeIndex = 0; hingeIndex < hingeCount; hingeIndex += 1) {
      const hinge = snapshotHinge(sourceHinges[hingeIndex])
      if (!hinge) return null
      hinges.push(hinge)
    }

    const joints: FoldPreviewTreeKinematics['joints'][number][] = []
    for (let jointIndex = 0; jointIndex < jointCount; jointIndex += 1) {
      const sourceJoint = sourceJoints[jointIndex]
      if (!sourceJoint || typeof sourceJoint !== 'object') return null
      const parentFaceId = sourceJoint.parentFaceId
      const childFaceId = sourceJoint.childFaceId
      const sourceHinge = sourceJoint.hinge
      const childRotationSign = sourceJoint.childRotationSign
      const hinge = snapshotHinge(sourceHinge)
      if (
        typeof parentFaceId !== 'string'
        || parentFaceId.length === 0
        || typeof childFaceId !== 'string'
        || childFaceId.length === 0
        || !hinge
        || (childRotationSign !== 1 && childRotationSign !== -1)
      ) return null
      joints.push({
        parentFaceId,
        childFaceId,
        hinge,
        childRotationSign,
      })
    }

    const projectId = model.projectId
    const revision = model.revision
    const worldUnitsPerMillimetre = model.worldUnitsPerMillimetre
    const sourcePaperCenter = model.paperCenter
    const sourceWorldBounds = model.worldBounds
    const rootFaceId = sourceKinematics.rootFaceId
    if (
      typeof projectId !== 'string'
      || projectId.length === 0
      || !Number.isSafeInteger(revision)
      || !Number.isFinite(worldUnitsPerMillimetre)
      || worldUnitsPerMillimetre <= 0
      || !sourcePaperCenter
      || typeof sourcePaperCenter !== 'object'
      || !sourceWorldBounds
      || typeof sourceWorldBounds !== 'object'
      || typeof rootFaceId !== 'string'
      || rootFaceId.length === 0
    ) return null
    const paperCenterX = sourcePaperCenter.x
    const paperCenterY = sourcePaperCenter.y
    const minX = sourceWorldBounds.minX
    const minZ = sourceWorldBounds.minZ
    const maxX = sourceWorldBounds.maxX
    const maxZ = sourceWorldBounds.maxZ
    if (
      !Number.isFinite(paperCenterX)
      || !Number.isFinite(paperCenterY)
      || !Number.isFinite(minX)
      || !Number.isFinite(minZ)
      || !Number.isFinite(maxX)
      || !Number.isFinite(maxZ)
      || minX > maxX
      || minZ > maxZ
    ) return null

    return {
      kind: 'fold_graph',
      projectId,
      revision,
      worldUnitsPerMillimetre,
      paperCenter: { x: paperCenterX, y: paperCenterY },
      worldBounds: { minX, minZ, maxX, maxZ },
      faces,
      hinges,
      kinematics: {
        kind: 'tree',
        rootFaceId,
        joints,
      },
    }
  } catch {
    return null
  }
}

function snapshotHinge(hinge: unknown): FoldPreviewHingeModel | null {
  if (!hinge || typeof hinge !== 'object') return null
  const sourceHinge = hinge as FoldPreviewHingeModel
  const edgeId = sourceHinge.edgeId
  const leftFaceId = sourceHinge.leftFaceId
  const rightFaceId = sourceHinge.rightFaceId
  const sourceStart = sourceHinge.start
  const sourceEnd = sourceHinge.end
  const sourceAxis = sourceHinge.axis
  const assignment = sourceHinge.assignment
  const rotationSign = sourceHinge.rotationSign
  if (
    typeof edgeId !== 'string'
    || edgeId.length === 0
    || typeof leftFaceId !== 'string'
    || leftFaceId.length === 0
    || typeof rightFaceId !== 'string'
    || rightFaceId.length === 0
    || !sourceAxis
    || typeof sourceAxis !== 'object'
    || (assignment !== 'mountain' && assignment !== 'valley')
    || (rotationSign !== 1 && rotationSign !== -1)
  ) return null
  const start = snapshotPoint(sourceStart)
  const end = snapshotPoint(sourceEnd)
  const axisX = sourceAxis.x
  const axisZ = sourceAxis.z
  if (!start || !end || !Number.isFinite(axisX) || !Number.isFinite(axisZ)) {
    return null
  }
  return {
    edgeId,
    leftFaceId,
    rightFaceId,
    start,
    end,
    axis: { x: axisX, z: axisZ },
    assignment,
    rotationSign,
  }
}

function snapshotPoint(
  point: unknown,
): FoldPreviewFaceModel['polygon'][number] | null {
  if (!point || typeof point !== 'object') return null
  const sourcePoint = point as FoldPreviewFaceModel['polygon'][number]
  const vertexId = sourcePoint.vertexId
  const x = sourcePoint.x
  const z = sourcePoint.z
  if (
    typeof vertexId !== 'string'
    || vertexId.length === 0
    || !Number.isFinite(x)
    || !Number.isFinite(z)
  ) return null
  return { vertexId, x, z }
}

function modelMatchesTree(
  model: FoldGraphPreviewModel,
  tree: FoldPreviewTreeKinematics,
) {
  if (
    model.faces.length !== tree.joints.length + 1
    || model.hinges.length !== tree.joints.length
  ) return false
  const faceIds = new Set<string>()
  for (const face of model.faces) {
    if (
      !face
      || typeof face.id !== 'string'
      || face.id.length === 0
      || faceIds.has(face.id)
    ) return false
    faceIds.add(face.id)
  }
  if (!faceIds.has(tree.rootFaceId)) return false

  const modelHingesById = new Map<string, FoldPreviewHingeModel>()
  for (const hinge of model.hinges) {
    if (
      !hinge
      || typeof hinge.edgeId !== 'string'
      || hinge.edgeId.length === 0
      || modelHingesById.has(hinge.edgeId)
    ) return false
    modelHingesById.set(hinge.edgeId, hinge)
  }
  const treeHingeIds = new Set<string>()
  for (const joint of tree.joints) {
    const modelHinge = modelHingesById.get(joint.hinge.edgeId)
    const assignmentSign = joint.hinge.assignment === 'mountain'
      ? 1
      : joint.hinge.assignment === 'valley'
        ? -1
        : null
    const jointPairMatches = (
      joint.parentFaceId === joint.hinge.leftFaceId
      && joint.childFaceId === joint.hinge.rightFaceId
    ) || (
      joint.parentFaceId === joint.hinge.rightFaceId
      && joint.childFaceId === joint.hinge.leftFaceId
    )
    if (
      !faceIds.has(joint.parentFaceId)
      || !faceIds.has(joint.childFaceId)
      || treeHingeIds.has(joint.hinge.edgeId)
      || !modelHinge
      || !sameHinge(modelHinge, joint.hinge)
      || !jointPairMatches
      || assignmentSign === null
      || joint.hinge.rotationSign !== assignmentSign
      || joint.childRotationSign !== (
        joint.parentFaceId === joint.hinge.leftFaceId
          ? joint.hinge.rotationSign
          : -joint.hinge.rotationSign
      )
    ) return false
    treeHingeIds.add(joint.hinge.edgeId)
  }
  return treeHingeIds.size === modelHingesById.size
}

function sameHinge(
  first: FoldPreviewHingeModel,
  second: FoldPreviewHingeModel,
) {
  return first.edgeId === second.edgeId
    && first.leftFaceId === second.leftFaceId
    && first.rightFaceId === second.rightFaceId
    && first.start.vertexId === second.start.vertexId
    && first.start.x === second.start.x
    && first.start.z === second.start.z
    && first.end.vertexId === second.end.vertexId
    && first.end.x === second.end.x
    && first.end.z === second.end.z
    && first.axis.x === second.axis.x
    && first.axis.z === second.axis.z
    && first.assignment === second.assignment
    && first.rotationSign === second.rotationSign
}
