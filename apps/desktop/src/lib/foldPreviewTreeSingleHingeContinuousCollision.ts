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
  type FoldPreviewContinuousMotionResult,
  type FoldPreviewContinuousMotionStep,
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
import { createFoldPreviewTreeSceneCollisionPoseKey } from './foldPreviewTreeScenePose.ts'
import {
  MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
  MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES,
  calculateFoldPreviewNarrowPhaseNumericalMargin,
  prepareFoldPreviewNarrowPhase,
  type FoldPreviewFullScanNonAdjacentWitnessSet,
  type FoldPreviewNarrowPhaseInteraction,
  type FoldPreviewNarrowPhaseWitnessCoverage,
  type FoldPreviewNarrowPhaseWitnessSample,
} from './foldPreviewNarrowCollision.ts'

export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_CROSS_TRIANGLE_PAIRS =
  1_000_000
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS =
  1_000_000
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS =
  1_000_000
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_TERMINAL_EVIDENCE_TRIANGLE_PAIRS =
  100_000
export const FOLD_PREVIEW_TREE_SINGLE_HINGE_BLOCKING_SAMPLE_VERSION =
  'tree_single_hinge_blocking_sample_v1'
export const FOLD_PREVIEW_TREE_TERMINAL_FULL_SCAN_BINDING_VERSION =
  'tree_single_hinge_terminal_full_scan_binding_v1'

const HINGE_INTERVAL_NUMERICAL_SAFETY_FACTOR = 8
const MOTION_POSE_EQUIVALENCE_FACTOR = 1_024
const MAX_BLOCKING_SAMPLE_REQUEST_KEY_LENGTH = 8 * 1_024 * 1_024

export type FoldPreviewTreeSingleHingeContinuousRequestIdentity = Readonly<{
  contextKey: string
  sourcePoseRequestKey: string
  generation: number
  requestSequence: number
}>

export type FoldPreviewTreeSingleHingeBlockingFaceTransform = Readonly<{
  faceId: string
  /** Exact detached column-major Matrix4 elements used by the point check. */
  elements: readonly number[]
}>

type FoldPreviewCompleteNonAdjacentWitnessSet = Extract<
  FoldPreviewFullScanNonAdjacentWitnessSet,
  { kind: 'complete' }
>

export type FoldPreviewTreeTerminalFullScanBinding = Readonly<{
  version: typeof FOLD_PREVIEW_TREE_TERMINAL_FULL_SCAN_BINDING_VERSION
  sourcePose: 'blocking_evaluate_point_pose'
  requestIdentityBound: true
  identity: Readonly<{
    projectId: string
    revision: number
    revisionBinding: 'project_response_source_equal_v1'
    fixedFaceId: string
    selectedHingeEdgeId: string
    request: FoldPreviewTreeSingleHingeContinuousRequestIdentity
    blockingPoseRequestKey: string
  }>
  blockingSampleTime: number
  selectedAngleDegrees: number
  collisionThickness: number
  angleVectors: Readonly<{
    start: readonly FoldPreviewHingeAngle[]
    target: readonly FoldPreviewHingeAngle[]
    sample: readonly FoldPreviewHingeAngle[]
  }>
  partition: Readonly<{
    version: 'rerooted_selected_hinge_partition_v1'
    stationaryFaceIds: readonly string[]
    movingFaceIds: readonly string[]
    witnessRelations: readonly Readonly<{
      witnessIndex: number
      firstBody: 'stationary' | 'moving'
      secondBody: 'stationary' | 'moving'
      relation:
        | 'cross_partition'
        | 'stationary_internal'
        | 'moving_internal'
    }>[]
  }>
  evidence: FoldPreviewCompleteNonAdjacentWitnessSet
  safety: Readonly<{
    nonAdjacentScopeOnly: true
    hingeAdjacentPairsIncluded: false
    allWitnessesCrossPartition: boolean
    sameBodyWitnessCount: number
    twoBodyTranslationInputEligible: boolean
    wholeSceneConstraintsRepresented: false
    legalCorrectionPoseGenerated: false
    staticCandidateRevalidated: false
    continuousCandidatePathCertified: false
    autoApplicable: false
  }>
}>

export type FoldPreviewTreeSingleHingeBlockingSample = Readonly<{
  version: typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_BLOCKING_SAMPLE_VERSION
  sourcePose: 'blocking_evaluate_point_pose'
  blockingSampleTime: number
  selectedAngleDegrees: number
  collisionThickness: number
  identity: Readonly<{
    projectId: string
    revision: number
    revisionBinding: 'project_response_source_equal_v1'
    fixedFaceId: string
    selectedHingeEdgeId: string
    request: FoldPreviewTreeSingleHingeContinuousRequestIdentity | null
  }>
  angleVectors: Readonly<{
    start: readonly FoldPreviewHingeAngle[]
    target: readonly FoldPreviewHingeAngle[]
    sample: readonly FoldPreviewHingeAngle[]
  }>
  /** Only the primary blocker's two face transforms are retained. */
  faceTransforms: readonly [
    FoldPreviewTreeSingleHingeBlockingFaceTransform,
    FoldPreviewTreeSingleHingeBlockingFaceTransform,
  ]
  /** Full bounded list, so its global coverage equations remain meaningful. */
  witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[]
  witnessCoverage: FoldPreviewNarrowPhaseWitnessCoverage
  /**
   * The first sample matching the blocking face pair and class, or null when
   * that pair was omitted by the cap or conservative derivation was unavailable.
   */
  primaryWitnessIndex: number | null
  /**
   * Optional complete non-adjacent evidence bound to this exact terminal
   * request and fold-tree partition. Failure never weakens the v1 block.
   */
  terminalFullScanBinding: FoldPreviewTreeTerminalFullScanBinding | null
}>

export type FoldPreviewTreeSingleHingeContinuousBlocker = Readonly<{
  firstFaceId: string
  secondFaceId: string
  relation: 'hinge_adjacent' | 'non_adjacent'
  geometryClass: 'touching' | 'penetrating' | 'indeterminate'
  hingeDecisionKind?: string
  /**
   * Detached analysis-only unsafe pose. Its transforms and pair-local hint
   * must never be sent through the certified scene-application path.
   */
  blockingSample: FoldPreviewTreeSingleHingeBlockingSample | null
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
    /**
     * Optional per-job tightening for the one terminal-only full evidence
     * scan. If the all-face upper bound exceeds it, the v1 block is returned
     * immediately with a null terminal binding.
     */
    maxTerminalEvidenceTrianglePairs?: number
    /**
     * Optional UI/runtime identity for the exact request that created the job.
     * Direct pure-analyzer callers may omit it; UI publication must not.
     */
    requestIdentity?: FoldPreviewTreeSingleHingeContinuousRequestIdentity
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
  maxTerminalEvidenceTrianglePairs: number
  requestIdentity: FoldPreviewTreeSingleHingeContinuousRequestIdentity | null
}>

type LightweightBlocker = Omit<
  FoldPreviewTreeSingleHingeContinuousBlocker,
  'blockingSample'
>

type BlockingSampleSeed = Readonly<{
  blockingSampleTime: number
  selectedAngleDegrees: number
  blocker: LightweightBlocker
  faceTransforms: FoldPreviewTreeSingleHingeBlockingSample['faceTransforms']
  witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[]
  witnessCoverage: FoldPreviewNarrowPhaseWitnessCoverage
  primaryWitnessIndex: number | null
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
    const stationaryFaceIds = Object.freeze(
      stationaryFaces.map((face) => face.id),
    )
    const movingFaceIdSnapshot = Object.freeze([...movingFaceIds])

    return Object.freeze({
      fixedFaceId: tree.rootFaceId,
      selectedHingeEdgeId,
      parentFaceId: selectedJoint.parentFaceId,
      childFaceId: selectedJoint.childFaceId,
      stationaryFaceIds,
      movingFaceIds: movingFaceIdSnapshot,
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
        if (
          resolvedOptions.requestIdentity
          && createFoldPreviewTreeSceneCollisionPoseKey(
            snapshot,
            tree.rootFaceId,
            thickness,
            normalizedAngles,
          ) !== resolvedOptions.requestIdentity.sourcePoseRequestKey
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
        let earliestBlockingSampleSeed: BlockingSampleSeed | null = null
        const pointDecision = (
          time: number,
        ): FoldPreviewContinuousPointDecision<
          LightweightBlocker
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
              const blocker = blockerFor(interaction)
              const seed = captureBlockingSampleSeed(
                time,
                angle,
                blocker,
                faceTransforms,
                result.witnessSamples,
                result.witnessCoverage,
              )
              if (
                seed
                && (
                  !earliestBlockingSampleSeed
                  || seed.blockingSampleTime
                    < earliestBlockingSampleSeed.blockingSampleTime
                )
              ) {
                earliestBlockingSampleSeed = seed
              }
              return {
                kind: 'blocked',
                blocker,
              }
            }
          }
          return unknownReason
            ? { kind: 'indeterminate', reason: unknownReason }
            : { kind: 'safe' }
        }

          const innerJob = createFoldPreviewContinuousMotionJob({
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
          if (!innerJob) return null
          return attachBlockingSampleToJob(
            innerJob,
            () => earliestBlockingSampleSeed,
            (seed) => {
              if (
                !resolvedOptions.requestIdentity
                || pointTrianglePairUpperBound
                > resolvedOptions.maxTerminalEvidenceTrianglePairs
              ) return null
              const terminalTransforms = faceTransformsAt(
                seed.selectedAngleDegrees,
              )
              if (
                !terminalTransforms
                || !blockingFaceTransformsMatch(
                  seed.faceTransforms,
                  terminalTransforms,
                )
              ) return null
              return pointAnalyzer.collectFullScanNonAdjacentWitnessSet(
                terminalTransforms,
                thickness,
              )
            },
            snapshot,
            tree.rootFaceId,
            selectedHingeEdgeId,
            selectedJoint.parentFaceId,
            selectedJoint.childFaceId,
            stationaryFaceIds,
            movingFaceIdSnapshot,
            normalizedAngles,
            targetSelectedAngleDegrees,
            thickness,
            resolvedOptions.requestIdentity,
          )
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

function captureBlockingSampleSeed(
  blockingSampleTime: number,
  selectedAngleDegrees: number,
  blocker: LightweightBlocker,
  faceTransforms: ReadonlyMap<string, Matrix4>,
  witnessSamples: readonly FoldPreviewNarrowPhaseWitnessSample[],
  witnessCoverage: FoldPreviewNarrowPhaseWitnessCoverage,
): BlockingSampleSeed | null {
  if (
    blocker.relation !== 'non_adjacent'
    || (
      blocker.geometryClass !== 'touching'
      && blocker.geometryClass !== 'penetrating'
    )
    || !validUnitTime(blockingSampleTime)
    || !validAngle(selectedAngleDegrees)
    || !validWitnessCoverage(witnessCoverage, witnessSamples.length)
  ) return null
  const firstTransform = snapshotBlockingFaceTransform(
    blocker.firstFaceId,
    faceTransforms.get(blocker.firstFaceId),
  )
  const secondTransform = snapshotBlockingFaceTransform(
    blocker.secondFaceId,
    faceTransforms.get(blocker.secondFaceId),
  )
  if (!firstTransform || !secondTransform) return null
  const samples = Object.freeze([...witnessSamples])
  const primaryWitnessIndex = samples.findIndex(
    (sample) =>
      sample.firstFaceId === blocker.firstFaceId
      && sample.secondFaceId === blocker.secondFaceId
      && sample.geometryClass === blocker.geometryClass,
  )
  return Object.freeze({
    blockingSampleTime,
    selectedAngleDegrees,
    blocker,
    faceTransforms: Object.freeze([
      firstTransform,
      secondTransform,
    ] as const),
    witnessSamples: samples,
    witnessCoverage: Object.freeze({ ...witnessCoverage }),
    primaryWitnessIndex:
      primaryWitnessIndex >= 0 ? primaryWitnessIndex : null,
  })
}

function attachBlockingSampleToJob(
  innerJob: FoldPreviewContinuousMotionJob<LightweightBlocker>,
  getSeed: () => BlockingSampleSeed | null,
  collectTerminalFullScan: (
    seed: BlockingSampleSeed,
  ) => FoldPreviewFullScanNonAdjacentWitnessSet | null,
  model: FoldGraphPreviewModel,
  fixedFaceId: string,
  selectedHingeEdgeId: string,
  parentFaceId: string,
  childFaceId: string,
  stationaryFaceIds: readonly string[],
  movingFaceIds: readonly string[],
  startAngles: readonly FoldPreviewHingeAngle[],
  targetSelectedAngleDegrees: number,
  collisionThickness: number,
  requestIdentity:
    FoldPreviewTreeSingleHingeContinuousRequestIdentity | null,
): FoldPreviewContinuousMotionJob<
  FoldPreviewTreeSingleHingeContinuousBlocker
> {
  type BlockedResult = Extract<
    FoldPreviewContinuousMotionResult<
      FoldPreviewTreeSingleHingeContinuousBlocker
    >,
    { kind: 'blocked' }
  >
  let wrappedBlocked: BlockedResult | null = null

  return Object.freeze({
    step(workBudget: number): FoldPreviewContinuousMotionStep<
      FoldPreviewTreeSingleHingeContinuousBlocker
    > {
      if (wrappedBlocked) return wrappedBlocked
      const step = innerJob.step(workBudget)
      if (step.kind !== 'blocked') return step
      if (!Object.hasOwn(step, 'blocker') || !step.blocker) {
        const result: BlockedResult = Object.freeze({
          kind: 'blocked',
          certifiedSafeThrough: step.certifiedSafeThrough,
          stopTime: step.stopTime,
          unsafeBracket: step.unsafeBracket,
          blockingSampleTime: step.blockingSampleTime,
          stats: step.stats,
        })
        wrappedBlocked = result
        return result
      }
      const lightweight = step.blocker
      let blockingSample: FoldPreviewTreeSingleHingeBlockingSample | null = null
      try {
        const seed = getSeed()
        const baseBlockingSample = seed
          && seed.blockingSampleTime === step.blockingSampleTime
          && sameLightweightBlocker(seed.blocker, lightweight)
          ? buildBlockingSample(
              seed,
              model,
              fixedFaceId,
              selectedHingeEdgeId,
              startAngles,
              targetSelectedAngleDegrees,
              collisionThickness,
              requestIdentity,
            )
          : null
        blockingSample = baseBlockingSample
        if (seed && baseBlockingSample) {
          try {
            const evidence = collectTerminalFullScan(seed)
            const terminalFullScanBinding =
              buildTerminalFullScanBinding(
                evidence,
                seed,
                baseBlockingSample,
                model,
                fixedFaceId,
                selectedHingeEdgeId,
                parentFaceId,
                childFaceId,
                stationaryFaceIds,
                movingFaceIds,
                requestIdentity,
              )
            if (terminalFullScanBinding) {
              blockingSample = deepFreeze({
                ...baseBlockingSample,
                terminalFullScanBinding,
              })
            }
          } catch {
            // Optional evidence failure must leave the v1 sample intact.
          }
        }
      } catch {
        // Explanation failure must never weaken or replace the block itself.
      }
      const blocker: FoldPreviewTreeSingleHingeContinuousBlocker =
        Object.freeze({
          ...lightweight,
          blockingSample,
        })
      const result: BlockedResult = Object.freeze({
        ...step,
        blocker,
      })
      wrappedBlocked = result
      return result
    },
    cancel() {
      innerJob.cancel()
    },
  })
}

function buildBlockingSample(
  seed: BlockingSampleSeed,
  model: FoldGraphPreviewModel,
  fixedFaceId: string,
  selectedHingeEdgeId: string,
  startAngles: readonly FoldPreviewHingeAngle[],
  targetSelectedAngleDegrees: number,
  collisionThickness: number,
  requestIdentity:
    FoldPreviewTreeSingleHingeContinuousRequestIdentity | null,
): FoldPreviewTreeSingleHingeBlockingSample | null {
  const startSelectedAngleDegrees = startAngles.find(
    (angle) => angle.edgeId === selectedHingeEdgeId,
  )?.angleDegrees
  const expectedPrimaryWitnessIndex = seed.witnessSamples.findIndex(
    (sample) => witnessMatchesBlocker(sample, seed.blocker),
  )
  if (
    !validAngle(startSelectedAngleDegrees)
    || !validAngle(targetSelectedAngleDegrees)
    || !validAngle(seed.selectedAngleDegrees)
    || !validUnitTime(seed.blockingSampleTime)
    || seed.selectedAngleDegrees !== (
      startSelectedAngleDegrees
      + (
        targetSelectedAngleDegrees - startSelectedAngleDegrees
      ) * seed.blockingSampleTime
    )
    || !Number.isFinite(collisionThickness)
    || collisionThickness < 0
    || seed.faceTransforms[0].faceId !== seed.blocker.firstFaceId
    || seed.faceTransforms[1].faceId !== seed.blocker.secondFaceId
    || !validWitnessCoverage(
      seed.witnessCoverage,
      seed.witnessSamples.length,
    )
    || seed.primaryWitnessIndex !== (
      expectedPrimaryWitnessIndex >= 0
        ? expectedPrimaryWitnessIndex
        : null
    )
  ) return null
  const start = angleVectorAt(
    startAngles,
    selectedHingeEdgeId,
    startSelectedAngleDegrees,
  )
  const target = angleVectorAt(
    startAngles,
    selectedHingeEdgeId,
    targetSelectedAngleDegrees,
  )
  const sample = angleVectorAt(
    startAngles,
    selectedHingeEdgeId,
    seed.selectedAngleDegrees,
  )
  if (!start || !target || !sample) return null
  return deepFreeze({
    version: FOLD_PREVIEW_TREE_SINGLE_HINGE_BLOCKING_SAMPLE_VERSION,
    sourcePose: 'blocking_evaluate_point_pose',
    blockingSampleTime: seed.blockingSampleTime,
    selectedAngleDegrees: seed.selectedAngleDegrees,
    collisionThickness,
    identity: {
      projectId: model.projectId,
      revision: model.revision,
      revisionBinding: 'project_response_source_equal_v1',
      fixedFaceId,
      selectedHingeEdgeId,
      request: requestIdentity,
    },
    angleVectors: {
      start,
      target,
      sample,
    },
    faceTransforms: seed.faceTransforms,
    witnessSamples: seed.witnessSamples,
    witnessCoverage: seed.witnessCoverage,
    primaryWitnessIndex: seed.primaryWitnessIndex,
    terminalFullScanBinding: null,
  })
}

function buildTerminalFullScanBinding(
  evidence: FoldPreviewFullScanNonAdjacentWitnessSet | null,
  seed: BlockingSampleSeed,
  blockingSample: FoldPreviewTreeSingleHingeBlockingSample,
  model: FoldGraphPreviewModel,
  fixedFaceId: string,
  selectedHingeEdgeId: string,
  parentFaceId: string,
  childFaceId: string,
  stationaryFaceIds: readonly string[],
  movingFaceIds: readonly string[],
  requestIdentity:
    FoldPreviewTreeSingleHingeContinuousRequestIdentity | null,
): FoldPreviewTreeTerminalFullScanBinding | null {
  if (
    !evidence
    || evidence.kind !== 'complete'
    || !requestIdentity
    || blockingSample.terminalFullScanBinding !== null
    || !sameRequestIdentity(
      blockingSample.identity.request,
      requestIdentity,
    )
    || evidence.collisionThickness !== blockingSample.collisionThickness
    || evidence.sourcePose !== 'analyzed_input_pose'
    || evidence.requestIdentityBound !== false
    || evidence.autoApplicable !== false
    || evidence.witnessSamples.length === 0
    || !validCompleteFullScanCoverage(
      evidence,
    )
    || !evidence.witnessSamples.some((sample) =>
      witnessMatchesBlocker(sample, seed.blocker)
    )
  ) return null

  const primaryWitness = blockingSample.primaryWitnessIndex === null
    ? null
    : blockingSample.witnessSamples[blockingSample.primaryWitnessIndex]
  if (
    primaryWitness
    && !evidence.witnessSamples.some((sample) =>
      sameWitnessIdentity(sample, primaryWitness)
    )
  ) return null

  const startPoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    model,
    fixedFaceId,
    blockingSample.collisionThickness,
    blockingSample.angleVectors.start,
  )
  const blockingPoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    model,
    fixedFaceId,
    blockingSample.collisionThickness,
    blockingSample.angleVectors.sample,
  )
  if (
    startPoseRequestKey !== requestIdentity.sourcePoseRequestKey
    || !validBoundedKey(blockingPoseRequestKey)
  ) return null

  const modelFaceIds = model.faces.map((face) => face.id)
  const stationary = snapshotFacePartition(
    stationaryFaceIds,
    modelFaceIds,
  )
  const moving = snapshotFacePartition(
    movingFaceIds,
    modelFaceIds,
  )
  if (
    !stationary
    || !moving
    || stationary.length + moving.length !== modelFaceIds.length
  ) return null
  const stationarySet = new Set(stationary)
  const movingSet = new Set(moving)
  if (
    fixedFaceId !== blockingSample.identity.fixedFaceId
    || selectedHingeEdgeId
      !== blockingSample.identity.selectedHingeEdgeId
    || !stationarySet.has(fixedFaceId)
    || !stationarySet.has(parentFaceId)
    || !movingSet.has(childFaceId)
    || [...stationarySet].some((faceId) => movingSet.has(faceId))
    || modelFaceIds.some((faceId) =>
      !stationarySet.has(faceId) && !movingSet.has(faceId)
    )
    || !selectedHingeMatchesPartition(
      model,
      selectedHingeEdgeId,
      parentFaceId,
      childFaceId,
    )
  ) return null

  const witnessRelations: Array<
    FoldPreviewTreeTerminalFullScanBinding['partition']['witnessRelations'][number]
  > = []
  let sameBodyWitnessCount = 0
  for (
    let witnessIndex = 0;
    witnessIndex < evidence.witnessSamples.length;
    witnessIndex += 1
  ) {
    const sample = evidence.witnessSamples[witnessIndex]
    const firstBody = stationarySet.has(sample.firstFaceId)
      ? 'stationary'
      : movingSet.has(sample.firstFaceId)
        ? 'moving'
        : null
    const secondBody = stationarySet.has(sample.secondFaceId)
      ? 'stationary'
      : movingSet.has(sample.secondFaceId)
        ? 'moving'
        : null
    if (!firstBody || !secondBody) return null
    const relation = firstBody !== secondBody
      ? 'cross_partition'
      : firstBody === 'stationary'
        ? 'stationary_internal'
        : 'moving_internal'
    if (relation !== 'cross_partition') sameBodyWitnessCount += 1
    witnessRelations.push(Object.freeze({
      witnessIndex,
      firstBody,
      secondBody,
      relation,
    }))
  }
  const allWitnessesCrossPartition = sameBodyWitnessCount === 0
  return deepFreeze({
    version: FOLD_PREVIEW_TREE_TERMINAL_FULL_SCAN_BINDING_VERSION,
    sourcePose: 'blocking_evaluate_point_pose',
    requestIdentityBound: true,
    identity: {
      projectId: blockingSample.identity.projectId,
      revision: blockingSample.identity.revision,
      revisionBinding: 'project_response_source_equal_v1',
      fixedFaceId,
      selectedHingeEdgeId,
      request: requestIdentity,
      blockingPoseRequestKey,
    },
    blockingSampleTime: blockingSample.blockingSampleTime,
    selectedAngleDegrees: blockingSample.selectedAngleDegrees,
    collisionThickness: blockingSample.collisionThickness,
    angleVectors: blockingSample.angleVectors,
    partition: {
      version: 'rerooted_selected_hinge_partition_v1',
      stationaryFaceIds: stationary,
      movingFaceIds: moving,
      witnessRelations,
    },
    evidence,
    safety: {
      nonAdjacentScopeOnly: true,
      hingeAdjacentPairsIncluded: false,
      allWitnessesCrossPartition,
      sameBodyWitnessCount,
      twoBodyTranslationInputEligible:
        allWitnessesCrossPartition
        && evidence.witnessSamples.length > 0,
      wholeSceneConstraintsRepresented: false,
      legalCorrectionPoseGenerated: false,
      staticCandidateRevalidated: false,
      continuousCandidatePathCertified: false,
      autoApplicable: false,
    },
  })
}

function validCompleteFullScanCoverage(
  evidence: FoldPreviewCompleteNonAdjacentWitnessSet,
) {
  const coverage = evidence.coverage
  const sampleCount = evidence.witnessSamples.length
  return evidence.algorithm === 'full_non_adjacent_prism_witness_scan_v2'
    && Number.isFinite(evidence.collisionThickness)
    && evidence.collisionThickness > 0
    && Number.isFinite(evidence.numericalMargin)
    && evidence.numericalMargin >= 0
    && coverage.scope
      === 'all_broad_phase_non_adjacent_triangle_pairs_full_scan_v2'
    && coverage.authoritativePairScanComplete === true
    && coverage.allCollisionConstraintsRepresented === true
    && validCount(coverage.broadPhaseCandidateCount)
    && validCount(coverage.expectedTrianglePairCount)
    && coverage.expectedTrianglePairCount
      <= MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_TERMINAL_EVIDENCE_TRIANGLE_PAIRS
    && validCount(coverage.trianglePairTests)
    && validCount(coverage.aabbRejectedPairCount)
    && validCount(coverage.satTests)
    && validCount(coverage.satSeparatedPairCount)
    && validCount(coverage.touchingPairCount)
    && validCount(coverage.penetratingPairCount)
    && coverage.indeterminatePairCount === 0
    && validCount(coverage.eligiblePairCount)
    && validCount(coverage.attemptedPairCount)
    && validCount(coverage.availablePairCount)
    && coverage.unavailablePairCount === 0
    && coverage.omittedByLimitCount === 0
    && coverage.expectedTrianglePairCount === coverage.trianglePairTests
    && coverage.trianglePairTests
      === coverage.aabbRejectedPairCount + coverage.satTests
    && coverage.satTests === coverage.satSeparatedPairCount
      + coverage.touchingPairCount
      + coverage.penetratingPairCount
    && coverage.eligiblePairCount
      === coverage.touchingPairCount + coverage.penetratingPairCount
    && coverage.eligiblePairCount === coverage.attemptedPairCount
    && coverage.attemptedPairCount === coverage.availablePairCount
    && coverage.availablePairCount === sampleCount
    && sampleCount <= MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
}

function snapshotFacePartition(
  values: readonly string[],
  modelFaceIds: readonly string[],
): readonly string[] | null {
  if (!Array.isArray(values) || values.length > modelFaceIds.length) return null
  const allowed = new Set(modelFaceIds)
  const seen = new Set<string>()
  const snapshot: string[] = []
  for (const value of values) {
    if (
      typeof value !== 'string'
      || !allowed.has(value)
      || seen.has(value)
    ) return null
    seen.add(value)
    snapshot.push(value)
  }
  return Object.freeze(snapshot)
}

function selectedHingeMatchesPartition(
  model: FoldGraphPreviewModel,
  selectedHingeEdgeId: string,
  parentFaceId: string,
  childFaceId: string,
) {
  const hinge = model.hinges.find(
    (current) => current.edgeId === selectedHingeEdgeId,
  )
  return hinge
    && (
      (
        hinge.leftFaceId === parentFaceId
        && hinge.rightFaceId === childFaceId
      )
      || (
        hinge.rightFaceId === parentFaceId
        && hinge.leftFaceId === childFaceId
      )
    )
}

function sameRequestIdentity(
  first: FoldPreviewTreeSingleHingeContinuousRequestIdentity | null,
  second: FoldPreviewTreeSingleHingeContinuousRequestIdentity,
) {
  return first?.contextKey === second.contextKey
    && first.sourcePoseRequestKey === second.sourcePoseRequestKey
    && first.generation === second.generation
    && first.requestSequence === second.requestSequence
}

function sameWitnessIdentity(
  first: FoldPreviewNarrowPhaseWitnessSample,
  second: FoldPreviewNarrowPhaseWitnessSample,
) {
  return first.firstFaceId === second.firstFaceId
    && first.secondFaceId === second.secondFaceId
    && first.firstTriangleIndex === second.firstTriangleIndex
    && first.secondTriangleIndex === second.secondTriangleIndex
    && first.geometryClass === second.geometryClass
}

function blockingFaceTransformsMatch(
  expected: FoldPreviewTreeSingleHingeBlockingSample['faceTransforms'],
  actual: ReadonlyMap<string, Matrix4>,
) {
  for (const saved of expected) {
    const transform = actual.get(saved.faceId)
    if (!transform || !Array.isArray(transform.elements)) return false
    for (let index = 0; index < 16; index += 1) {
      if (transform.elements[index] !== saved.elements[index]) return false
    }
  }
  return true
}

function snapshotBlockingFaceTransform(
  faceId: string,
  transform: Matrix4 | undefined,
): FoldPreviewTreeSingleHingeBlockingFaceTransform | null {
  if (!transform || !Array.isArray(transform.elements)) return null
  const elements = transform.elements
  if (elements.length !== 16) return null
  const snapshot: number[] = []
  for (let index = 0; index < 16; index += 1) {
    const value = elements[index]
    if (!Number.isFinite(value)) return null
    snapshot.push(value)
  }
  return Object.freeze({
    faceId,
    elements: Object.freeze(snapshot),
  })
}

function angleVectorAt(
  startAngles: readonly FoldPreviewHingeAngle[],
  selectedHingeEdgeId: string,
  selectedAngleDegrees: number,
): readonly FoldPreviewHingeAngle[] | null {
  if (!validAngle(selectedAngleDegrees)) return null
  let replacements = 0
  const angles = startAngles.map((angle) => {
    const isSelected = angle.edgeId === selectedHingeEdgeId
    if (isSelected) replacements += 1
    return Object.freeze({
      edgeId: angle.edgeId,
      angleDegrees: isSelected
        ? selectedAngleDegrees
        : angle.angleDegrees,
    })
  })
  return replacements === 1 ? Object.freeze(angles) : null
}

function witnessMatchesBlocker(
  sample: FoldPreviewNarrowPhaseWitnessSample | undefined,
  blocker: LightweightBlocker,
) {
  return sample?.firstFaceId === blocker.firstFaceId
    && sample.secondFaceId === blocker.secondFaceId
    && sample.relation === 'non_adjacent'
    && sample.geometryClass === blocker.geometryClass
}

function validWitnessCoverage(
  coverage: FoldPreviewNarrowPhaseWitnessCoverage,
  sampleCount: number,
) {
  return coverage
    && validCount(sampleCount)
    && sampleCount <= MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
    && coverage.scope
      === 'detected_non_adjacent_triangle_pairs_in_authoritative_scan_v1'
    && validCount(coverage.eligiblePairCount)
    && validCount(coverage.attemptedPairCount)
    && validCount(coverage.unavailablePairCount)
    && validCount(coverage.omittedByLimitCount)
    && coverage.eligiblePairCount
      === coverage.attemptedPairCount + coverage.omittedByLimitCount
    && coverage.attemptedPairCount
      === sampleCount + coverage.unavailablePairCount
    && coverage.attemptedPairCount
      <= MAX_FOLD_PREVIEW_NARROW_PHASE_WITNESS_SAMPLES
    && typeof coverage.authoritativePairScanComplete === 'boolean'
}

function sameLightweightBlocker(
  first: LightweightBlocker,
  second: LightweightBlocker,
) {
  return first.firstFaceId === second.firstFaceId
    && first.secondFaceId === second.secondFaceId
    && first.relation === second.relation
    && first.geometryClass === second.geometryClass
    && first.hingeDecisionKind === second.hingeDecisionKind
}

function blockerFor(
  interaction: FoldPreviewNarrowPhaseInteraction,
): LightweightBlocker {
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
  const maxTerminalEvidenceTrianglePairs =
    options.maxTerminalEvidenceTrianglePairs
    ?? MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_TERMINAL_EVIDENCE_TRIANGLE_PAIRS
  const rawRequestIdentity = options.requestIdentity
  const requestIdentity = rawRequestIdentity === undefined
    ? null
    : snapshotRequestIdentity(rawRequestIdentity)
  if (
    !Number.isSafeInteger(maxIntervalPairVisits)
    || maxIntervalPairVisits <= 0
    || maxIntervalPairVisits
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_INTERVAL_PAIR_VISITS
    || !Number.isSafeInteger(maxPointTriangleTests)
    || maxPointTriangleTests <= 0
    || maxPointTriangleTests
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_CONTINUOUS_POINT_TRIANGLE_TESTS
    || !Number.isSafeInteger(maxTerminalEvidenceTrianglePairs)
    || maxTerminalEvidenceTrianglePairs <= 0
    || maxTerminalEvidenceTrianglePairs
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_TERMINAL_EVIDENCE_TRIANGLE_PAIRS
    || (rawRequestIdentity !== undefined && !requestIdentity)
  ) return null
  return {
    motion: {
      maxDepth: options.maxDepth,
      maxIntervalTests: options.maxIntervalTests,
      minTimeSpan: options.minTimeSpan,
    },
    maxIntervalPairVisits,
    maxPointTriangleTests,
    maxTerminalEvidenceTrianglePairs,
    requestIdentity,
  }
}

function snapshotRequestIdentity(
  value: unknown,
): FoldPreviewTreeSingleHingeContinuousRequestIdentity | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const source = value as Record<string, unknown>
  const contextKey = source.contextKey
  const sourcePoseRequestKey = source.sourcePoseRequestKey
  const generation = source.generation
  const requestSequence = source.requestSequence
  if (
    !validBoundedKey(contextKey)
    || !validBoundedKey(sourcePoseRequestKey)
    || !Number.isSafeInteger(generation)
    || (generation as number) < 0
    || !Number.isSafeInteger(requestSequence)
    || (requestSequence as number) <= 0
  ) return null
  return Object.freeze({
    contextKey,
    sourcePoseRequestKey,
    generation: generation as number,
    requestSequence: requestSequence as number,
  })
}

function validBoundedKey(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_BLOCKING_SAMPLE_REQUEST_KEY_LENGTH
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

function validUnitTime(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 1
}

function validCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
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

function deepFreeze<T>(value: T, seen = new WeakSet<object>()): T {
  if (typeof value !== 'object' || value === null) return value
  const object = value as object
  if (seen.has(object)) return value
  seen.add(object)
  for (const key of Reflect.ownKeys(object)) {
    deepFreeze(
      (object as Record<PropertyKey, unknown>)[key],
      seen,
    )
  }
  return Object.freeze(value)
}
