import { Matrix4, Vector3, type Matrix4 as ThreeMatrix4 } from 'three'
import {
  createFoldPreviewContinuousMotionJob,
  type FoldPreviewContinuousMotionJob,
  type FoldPreviewContinuousMotionOptions,
  type FoldPreviewContinuousPointDecision,
} from './foldPreviewContinuousMotion.ts'
import { findFoldPreviewSingleAxisSweptAabb } from './foldPreviewContinuousInterval.ts'
import type { FoldPreviewCollisionAdjacency } from './foldPreviewCollision.ts'
import {
  prepareFoldPreviewHingeContactPolicy,
  type FoldPreviewHingeContactConstraint,
  type FoldPreviewHingePolicyFace,
} from './foldPreviewHingeCollision.ts'
import {
  triangulateFoldPreviewPolygon,
  type FoldPreviewTriangleIndices,
} from './foldPreviewGeometry.ts'
import type {
  FoldPreviewFaceModel,
  SingleFoldPreviewModel,
} from './foldPreviewModel.ts'
import {
  calculateFoldPreviewNarrowPhaseNumericalMargin,
  prepareFoldPreviewNarrowPhase,
} from './foldPreviewNarrowCollision.ts'
import { calculateSingleFoldPose } from './foldPreviewSingleFoldKinematics.ts'

export const MAX_FOLD_PREVIEW_SINGLE_FOLD_CONTINUOUS_TRIANGLE_PAIRS = 1_000_000
const HINGE_INTERVAL_NUMERICAL_SAFETY_FACTOR = 8

export type FoldPreviewSingleFoldContinuousBlocker = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  geometryClass: 'touching' | 'penetrating' | 'indeterminate'
  hingeDecisionKind?: string
}>

export type FoldPreviewSingleFoldContinuousAnalyzer = Readonly<{
  fixedFaceId: string
  movingFaceId: string
  trianglePairs: number
  staticallySupportedTrianglePairs: number
  createJob(
    startAngleDegrees: number,
    targetAngleDegrees: number,
    thickness: number,
    options?: FoldPreviewContinuousMotionOptions,
  ): FoldPreviewContinuousMotionJob<FoldPreviewSingleFoldContinuousBlocker> | null
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

/**
 * Prepares continuous collision analysis for one validated two-face fold.
 *
 * The generated job follows a linear angle path. It can skip a triangle pair
 * only when the hinge policy proves static finite support and the whole time
 * interval stays below the centered-slab 180-degree singularity. Every other
 * pair needs strict separation between conservative swept AABBs.
 */
export function prepareFoldPreviewSingleFoldContinuousCollision(
  model: SingleFoldPreviewModel,
  fixedFaceId: string,
): FoldPreviewSingleFoldContinuousAnalyzer | null {
  try {
    const snapshot = snapshotModel(model)
    if (!snapshot) return null
    const initialPose = calculateSingleFoldPose(snapshot, fixedFaceId, 0)
    if (!initialPose) return null

    const preparedFaces: PreparedFace[] = snapshot.faces.map((face) => ({
      id: face.id,
      polygon: face.polygon,
      triangles: triangulateFoldPreviewPolygon(face.polygon),
    }))
    const facesById = new Map(preparedFaces.map((face) => [face.id, face]))
    if (facesById.size !== 2) return null
    const fixedFace = facesById.get(initialPose.fixedFaceId)
    const movingFace = facesById.get(initialPose.movingFaceId)
    if (!fixedFace || !movingFace) return null

    const adjacency: readonly FoldPreviewCollisionAdjacency[] = [{
      edgeId: snapshot.hinge.edgeId,
      firstFaceId: snapshot.hinge.leftFaceId,
      secondFaceId: snapshot.hinge.rightFaceId,
    }]
    const constraint: readonly FoldPreviewHingeContactConstraint[] = [{
      edgeId: snapshot.hinge.edgeId,
      leftFaceId: snapshot.hinge.leftFaceId,
      rightFaceId: snapshot.hinge.rightFaceId,
      start: {
        vertexId: snapshot.hinge.start.vertexId,
        x: snapshot.hinge.start.x,
        z: snapshot.hinge.start.z,
      },
      end: {
        vertexId: snapshot.hinge.end.vertexId,
        x: snapshot.hinge.end.x,
        z: snapshot.hinge.end.z,
      },
      thicknessRule: 'centered_mid_surface_v1',
    }]
    const policyFaces: FoldPreviewHingePolicyFace[] = preparedFaces.map((face) => ({
      id: face.id,
      polygon: face.polygon,
      triangles: face.triangles,
    }))
    const hingePolicy = prepareFoldPreviewHingeContactPolicy(
      policyFaces,
      adjacency,
      constraint,
    )
    const pointAnalyzer = prepareFoldPreviewNarrowPhase(
      snapshot.faces,
      adjacency,
      constraint,
    )
    if (!hingePolicy || !pointAnalyzer) return null

    const trianglePairs = fixedFace.triangles.length * movingFace.triangles.length
    if (
      !Number.isSafeInteger(trianglePairs)
      || trianglePairs <= 0
      || trianglePairs > MAX_FOLD_PREVIEW_SINGLE_FOLD_CONTINUOUS_TRIANGLE_PAIRS
    ) return null
    const staticallySupportedPairs = fixedFace.triangles.map(
      (_, firstTriangleIndex) =>
        movingFace.triangles.map((__, secondTriangleIndex) =>
          hingePolicy.proveStaticTrianglePairSupport({
            firstFaceId: fixedFace.id,
            secondFaceId: movingFace.id,
            hingeEdgeIds: [snapshot.hinge.edgeId],
            firstTriangleIndex,
            secondTriangleIndex,
          }).kind === 'proven_static_hinge_support'),
    )
    const staticallySupportedTrianglePairs = staticallySupportedPairs.reduce(
      (total, row) => total + row.filter(Boolean).length,
      0,
    )

    return Object.freeze({
      fixedFaceId: fixedFace.id,
      movingFaceId: movingFace.id,
      trianglePairs,
      staticallySupportedTrianglePairs,
      createJob(
        startAngleDegrees: number,
        targetAngleDegrees: number,
        thickness: number,
        options: FoldPreviewContinuousMotionOptions = {},
      ) {
        if (
          !validAngle(startAngleDegrees)
          || !validAngle(targetAngleDegrees)
          || !Number.isFinite(thickness)
          || thickness < 0
        ) return null
        const axisStart = {
          x: snapshot.hinge.start.x,
          y: 0,
          z: snapshot.hinge.start.z,
        }
        const axisEnd = {
          x: snapshot.hinge.end.x,
          y: 0,
          z: snapshot.hinge.end.z,
        }
        const coordinateScale = continuousCoordinateScaleUpperBound(
          snapshot,
          thickness,
        )
        const numericalMargin = coordinateScale === null
          ? null
          : calculateFoldPreviewNarrowPhaseNumericalMargin(coordinateScale)
        if (numericalMargin === null) return null
        // The pointwise hinge frame subtracts margin + endpoint pose error
        // from the analytic corridor radius. Endpoint error is itself bounded
        // by one margin. Requiring four times that theoretical minimum, via
        // this factor of eight against full thickness, also covers transform
        // round-off across the complete rotation interval.
        const staticSupportNumericallySafe = thickness
          >= numericalMargin * HINGE_INTERVAL_NUMERICAL_SAFETY_FACTOR
        const fixedTransform = new Matrix4()
        const fixedBounds = fixedFace.triangles.map((triangle) =>
          findFoldPreviewSingleAxisSweptAabb(
            prismVertices(fixedFace, triangle, fixedTransform, thickness),
            axisStart,
            axisEnd,
            0,
          ))
        if (fixedBounds.some((bounds) => !bounds)) return null

        const angleAt = (time: number) =>
          startAngleDegrees + (targetAngleDegrees - startAngleDegrees) * time
        const pointDecision = (
          time: number,
        ): FoldPreviewContinuousPointDecision<FoldPreviewSingleFoldContinuousBlocker> => {
          const angle = angleAt(time)
          if (!validAngle(angle)) {
            return { kind: 'indeterminate', reason: 'invalid_interpolated_angle' }
          }
          const pose = calculateSingleFoldPose(snapshot, fixedFace.id, angle)
          if (!pose) return { kind: 'indeterminate', reason: 'pose_unavailable' }
          const result = pointAnalyzer.analyze(pose.faceTransforms, thickness)
          if (!result) {
            return { kind: 'indeterminate', reason: 'point_collision_unavailable' }
          }
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
            // Subdivide toward, but never certify through, the exact singular
            // target. The point evaluator will make the terminal reason clear.
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
            const pose = calculateSingleFoldPose(
              snapshot,
              fixedFace.id,
              midpointAngle,
            )
            const movingTransform = pose?.faceTransforms.get(movingFace.id)
            if (!pose || !movingTransform) {
              return { kind: 'indeterminate', reason: 'midpoint_pose_unavailable' }
            }
            const angularSpanRadians = Math.abs(endAngle - startAngle)
              * Math.PI
              / 180
            const movingBounds = movingFace.triangles.map((triangle) =>
              findFoldPreviewSingleAxisSweptAabb(
                prismVertices(movingFace, triangle, movingTransform, thickness),
                pose.axisStart,
                pose.axisEnd,
                angularSpanRadians,
              ))
            if (movingBounds.some((bounds) => !bounds)) {
              return {
                kind: 'indeterminate',
                reason: 'swept_bounds_unavailable',
              }
            }

            for (
              let firstTriangleIndex = 0;
              firstTriangleIndex < fixedFace.triangles.length;
              firstTriangleIndex += 1
            ) {
              for (
                let secondTriangleIndex = 0;
                secondTriangleIndex < movingFace.triangles.length;
                secondTriangleIndex += 1
              ) {
                if (
                  staticallySupportedPairs[firstTriangleIndex][secondTriangleIndex]
                ) continue
                const first = fixedBounds[firstTriangleIndex]
                const second = movingBounds[secondTriangleIndex]
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
            return { kind: 'clear' }
          },
        }, options)
      },
    })
  } catch {
    return null
  }
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

function blockerFor(
  interaction: Readonly<{
    firstFaceId: string
    secondFaceId: string
    relation: 'hinge_adjacent' | 'non_adjacent'
    geometryClass: 'touching' | 'penetrating' | 'indeterminate'
    hingeDecision?: Readonly<{ kind: string }>
  }>,
): FoldPreviewSingleFoldContinuousBlocker {
  return {
    firstFaceId: interaction.firstFaceId,
    secondFaceId: interaction.secondFaceId,
    relation: interaction.relation,
    geometryClass: interaction.geometryClass,
    ...(interaction.hingeDecision
      ? { hingeDecisionKind: interaction.hingeDecision.kind }
      : {}),
  }
}

function strictlySeparated(first: Bounds, second: Bounds) {
  return first.maxX < second.minX
    || second.maxX < first.minX
    || first.maxY < second.minY
    || second.maxY < first.minY
    || first.maxZ < second.minZ
    || second.maxZ < first.minZ
}

function snapshotModel(
  model: SingleFoldPreviewModel,
): SingleFoldPreviewModel | null {
  if (!model || model.kind !== 'single_fold' || !Array.isArray(model.faces)) return null
  const faces = model.faces.map((face) => ({
    id: face.id,
    polygon: face.polygon.map((point) => ({ ...point })),
  }))
  if (faces.length !== 2) return null
  const fixedFace = faces.find((face) => face.id === model.fixedFace.id)
  const movingFace = faces.find((face) => face.id === model.movingFace.id)
  if (!fixedFace || !movingFace) return null
  return {
    kind: 'single_fold',
    projectId: model.projectId,
    revision: model.revision,
    worldUnitsPerMillimetre: model.worldUnitsPerMillimetre,
    paperCenter: { ...model.paperCenter },
    worldBounds: { ...model.worldBounds },
    faces: [faces[0], faces[1]],
    fixedFace,
    movingFace,
    hinge: {
      ...model.hinge,
      start: { ...model.hinge.start },
      end: { ...model.hinge.end },
      axis: { ...model.hinge.axis },
    },
  }
}

function validAngle(value: number) {
  return Number.isFinite(value) && value >= 0 && value <= 180
}

function continuousCoordinateScaleUpperBound(
  model: SingleFoldPreviewModel,
  thickness: number,
): number | null {
  const axisX = model.hinge.start.x
  const axisZ = model.hinge.start.z
  const axisOriginRadius = Math.hypot(axisX, axisZ)
  const halfThickness = thickness / 2
  if (
    !Number.isFinite(axisOriginRadius)
    || !Number.isFinite(halfThickness)
  ) return null

  let scale = 1
  for (const face of model.faces) {
    for (const point of face.polygon) {
      const radius = Math.hypot(
        point.x - axisX,
        halfThickness,
        point.z - axisZ,
      )
      const bound = axisOriginRadius + radius
      if (!Number.isFinite(bound)) return null
      scale = Math.max(scale, bound)
    }
  }
  return scale
}
