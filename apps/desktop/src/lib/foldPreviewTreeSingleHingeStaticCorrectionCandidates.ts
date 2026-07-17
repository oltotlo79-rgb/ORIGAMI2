import { Vector3 } from 'three'
import { collectFoldTreeDependentFaces } from './foldPreviewAnchoring.ts'
import type {
  FoldPreviewCollisionAdjacency,
} from './foldPreviewCollision.ts'
import type {
  FoldPreviewHingeContactConstraint,
} from './foldPreviewHingeCollision.ts'
import { triangulateFoldPreviewPolygon } from './foldPreviewGeometry.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
  type FoldPreviewTreePose,
} from './foldPreviewKinematics.ts'
import {
  prepareFoldPreviewNarrowPhase,
  type FoldPreviewFullScanNonAdjacentWitnessSet,
  type FoldPreviewNarrowPhaseResult,
} from './foldPreviewNarrowCollision.ts'
import {
  deriveFoldPreviewSingleHingeRotationFitSeeds,
  MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS,
  type FoldPreviewSingleHingeRotationFitSeed,
} from './foldPreviewSingleHingeRotationFitSeeds.ts'
import {
  replaceFoldPreviewTreeMotionSelectedAngle,
  type FoldPreviewTreeMotionContext,
} from './foldPreviewTreeMotionContext.ts'
import {
  createFoldPreviewTreeSceneCollisionPoseKey,
} from './foldPreviewTreeScenePose.ts'
import type {
  FoldPreviewTreeTerminalFullScanBinding,
} from './foldPreviewTreeSingleHingeContinuousCollision.ts'
import {
  deriveFoldPreviewTwoBodyCorrectionCandidate,
  type FoldPreviewTwoBodyCorrectionCandidate,
} from './foldPreviewTwoBodyCorrectionCandidate.ts'

export const FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION =
  'tree_single_hinge_static_correction_candidates_v1'
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES =
  MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS
export const MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS =
  1_000_000

const MATERIAL_POINT_EQUIVALENCE_FACTOR = 4_096

type Point = Readonly<{ x: number; y: number; z: number }>

export type FoldPreviewTreeSingleHingeStaticCorrectionAnalysis = Readonly<{
  broadPhaseCandidateCount: number
  broadPhaseNonAdjacentCandidateCount: number
  broadPhaseHingeAdjacentCandidateCount: number
  interactionCount: number
  allowedHingeInteractionCount: number
  trianglePairTests: number
  satTests: number
  numericalMargin: number
  fullScanBroadPhaseCandidateCount: number
  fullScanExpectedTrianglePairCount: number
  fullScanTrianglePairTests: number
  fullScanAabbRejectedPairCount: number
  fullScanSatTests: number
  fullScanSatSeparatedPairCount: number
}>

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidate = Readonly<{
  rank: number
  sourceSeedRank: number
  source: FoldPreviewSingleHingeRotationFitSeed['source']
  pose: Readonly<{
    poseRequestKey: string
    selectedAngleDegrees: number
    appliedAngles: readonly FoldPreviewHingeAngle[]
  }>
  fit: Readonly<{
    signedDeltaDegrees: number
    signedRotationRadians: number
    residualSquared: number
    residualRms: number
    improvementSquared: number
    improvementRatio: number
  }>
  staticAnalysis: FoldPreviewTreeSingleHingeStaticCorrectionAnalysis
  safety: Readonly<{
    modelIdentityBound: true
    completeLegalAngleVectorGenerated: true
    legalCorrectionPoseGenerated: true
    collisionConstraintsRevalidated: true
    hingeContactPolicySatisfied: true
    wholeSceneStaticClear: true
    staticCandidateRevalidated: true
    continuousCandidatePathCertified: false
    sceneApplied: false
    autoApplicable: false
  }>
}>

export type FoldPreviewTreeSingleHingeStaticCorrectionCandidates = Readonly<{
  version:
    typeof FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION
  kind: 'statically_revalidated_single_hinge_correction_candidates'
  sourceIdentity: Readonly<{
    projectId: string
    revision: number
    fixedFaceId: string
    selectedHingeEdgeId: string
    contextKey: string
    sourcePoseRequestKey: string
    blockingPoseRequestKey: string
    generation: number
    requestSequence: number
    blockingSampleTime: number
    sourceSelectedAngleDegrees: number
    blockingSelectedAngleDegrees: number
    collisionThickness: number
  }>
  sourcePartition: Readonly<{
    version: 'rerooted_selected_hinge_partition_v1'
    stationaryFaceIds: readonly string[]
    movingFaceIds: readonly string[]
  }>
  commonTranslation: Readonly<{
    version: FoldPreviewTwoBodyCorrectionCandidate['version']
    translation: Point
    magnitude: number
    certifiedMagnitudeUpperBound: number
    clearance: number
    maximumTranslation: number
    constraintCount: number
    solver: Readonly<{
      method: FoldPreviewTwoBodyCorrectionCandidate['solver']['method']
      seedMethod: FoldPreviewTwoBodyCorrectionCandidate['solver']['seedMethod']
      activeConstraintIndices: readonly number[]
      activeSetSize: 1 | 2 | 3
      evaluatedActiveSetCount: number
      maximumActiveSetCount: number
    }>
  }>
  rotationFit: Readonly<{
    version: 'single_hinge_rotation_fit_seeds_v1'
    method: 'bounded_finite_rotation_least_squares_v1'
    objective: 'moving_material_points_match_common_translation'
    maximumAngleDeltaDegrees: number
    angleDomain: Readonly<{
      minimumDegrees: number
      maximumDegrees: number
    }>
    worldAxis: Readonly<{
      point: Point
      direction: Point
    }>
    childRotationSign: 1 | -1
    movingPointCount: number
    baselineResidualSquared: number
    baselineResidualRms: number
    evaluatedCandidateCount: number
    seedCount: number
  }>
  staticValidationWork: Readonly<{
    strategy: 'full_non_adjacent_then_hinge_policy_v1'
    maximumTrianglePairVisits:
      typeof MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
    plannedTrianglePairVisitUpperBound: number
    actualTrianglePairVisits: number
    fullScanCount: number
    narrowScanCount: number
  }>
  candidates: readonly FoldPreviewTreeSingleHingeStaticCorrectionCandidate[]
  safety: Readonly<{
    modelIdentityBound: true
    sourcePoseIdentityVerified: true
    blockingPoseIdentityVerified: true
    partitionRevalidated: true
    completeLegalAngleVectorsGenerated: true
    legalCorrectionPoseGenerated: true
    collisionConstraintsRevalidated: true
    hingeContactPolicySatisfied: true
    wholeSceneStaticClear: true
    staticCandidateRevalidated: true
    continuousCandidatePathCertified: false
    sceneApplied: false
    autoApplicable: false
  }>
}>

type VerifiedContext = Readonly<{
  sourceAngles: readonly FoldPreviewHingeAngle[]
  blockingAngles: readonly FoldPreviewHingeAngle[]
  sourcePoseRequestKey: string
  blockingPoseRequestKey: string
  blockingPose: FoldPreviewTreePose
  selectedJoint: FoldPreviewTreeMotionContext['tree']['joints'][number]
  sourceSelectedAngleDegrees: number
}>

type VerifiedPartition = Readonly<{
  stationaryFaceIds: readonly string[]
  movingFaceIds: readonly string[]
}>

/**
 * Rebinds terminal two-body evidence to one authentic tree-motion context and
 * retains only complete one-hinge angle vectors that are collision-free in a
 * fresh whole-scene static analysis.
 *
 * The returned candidates remain analysis-only. In particular, this function
 * does not apply a scene pose and does not certify the path from the current or
 * blocking pose to any returned candidate.
 */
export function deriveFoldPreviewTreeSingleHingeStaticCorrectionCandidates(
  context: FoldPreviewTreeMotionContext,
  binding: FoldPreviewTreeTerminalFullScanBinding,
  clearance: number,
  maximumTranslation: number,
  maximumAngleDeltaDegrees: number,
): FoldPreviewTreeSingleHingeStaticCorrectionCandidates | null {
  try {
    if (!validPositiveAngleDelta(maximumAngleDeltaDegrees)) return null

    // This is the sole read boundary for the raw terminal binding. Everything
    // below is reconstructed from the detached, internally revalidated result.
    const translationCandidate =
      deriveFoldPreviewTwoBodyCorrectionCandidate(
        binding,
        clearance,
        maximumTranslation,
      )
    if (!translationCandidate) return null

    const verifiedContext = verifyContextAndPoses(
      context,
      translationCandidate,
    )
    if (!verifiedContext) return null
    const partition = verifyPartition(
      context,
      translationCandidate,
      verifiedContext.selectedJoint,
    )
    if (!partition) return null

    const worldAxis = worldSelectedHingeAxis(
      verifiedContext.blockingPose,
      verifiedContext.selectedJoint,
    )
    if (!worldAxis) return null
    const movingPoints = collectMovingWorldMaterialPoints(
      context,
      verifiedContext.blockingPose,
      partition.movingFaceIds,
    )
    if (!movingPoints) return null

    const fit = deriveFoldPreviewSingleHingeRotationFitSeeds({
      axis: worldAxis,
      childRotationSign: verifiedContext.selectedJoint.childRotationSign,
      blockingAngleDegrees:
        translationCandidate.sourceIdentity.selectedAngleDegrees,
      maximumAngleDeltaDegrees,
      translation: translationCandidate.translation,
      movingPoints,
    })
    if (!fit) return null

    const trianglePairUpperBound =
      allFaceTrianglePairUpperBound(context)
    const plannedTrianglePairVisitUpperBound =
      trianglePairUpperBound === null
        ? null
        : boundedProduct(
            trianglePairUpperBound,
            fit.seeds.length * 2,
          )
    if (
      plannedTrianglePairVisitUpperBound === null
      || plannedTrianglePairVisitUpperBound <= 0
      || plannedTrianglePairVisitUpperBound
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
    ) return null

    const analyzer = prepareStaticAnalyzer(context)
    if (!analyzer) return null
    const candidates: FoldPreviewTreeSingleHingeStaticCorrectionCandidate[] = []
    let actualTrianglePairVisits = 0
    let fullScanCount = 0
    let narrowScanCount = 0
    for (const seed of fit.seeds) {
      const appliedAngles = replaceFoldPreviewTreeMotionSelectedAngle(
        context,
        seed.angleDegrees,
      )
      if (!appliedAngles) return null
      const poseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
        context.model,
        context.fixedFaceId,
        context.collisionThickness,
        appliedAngles,
      )
      if (!poseRequestKey) return null
      const pose = calculateFoldTreePoseWithAngles(context.tree, {
        kind: 'per_hinge',
        angles: appliedAngles,
      })
      if (!pose) return null

      const fullScan = analyzer.collectFullScanNonAdjacentWitnessSet(
        pose.faceTransforms,
        context.collisionThickness,
      )
      if (!fullScan) return null
      fullScanCount += 1
      const afterFullScan = boundedVisitSum(
        actualTrianglePairVisits,
        fullScan.coverage.trianglePairTests,
      )
      if (afterFullScan === null) return null
      actualTrianglePairVisits = afterFullScan
      if (!fullNonAdjacentScanIsClear(fullScan)) continue
      const result = analyzer.analyze(
        pose.faceTransforms,
        context.collisionThickness,
      )
      if (!result) return null
      narrowScanCount += 1
      const afterNarrowScan = boundedVisitSum(
        actualTrianglePairVisits,
        result.trianglePairTests,
      )
      if (afterNarrowScan === null) return null
      actualTrianglePairVisits = afterNarrowScan
      const staticAnalysis = staticClearAnalysis(result, fullScan)
      if (!staticAnalysis) continue

      candidates.push(deepFreeze({
        rank: candidates.length + 1,
        sourceSeedRank: seed.rank,
        source: seed.source,
        pose: {
          poseRequestKey,
          selectedAngleDegrees: seed.angleDegrees,
          appliedAngles: copyAngles(appliedAngles),
        },
        fit: {
          signedDeltaDegrees: seed.signedDeltaDegrees,
          signedRotationRadians: seed.signedRotationRadians,
          residualSquared: seed.residualSquared,
          residualRms: seed.residualRms,
          improvementSquared: seed.improvementSquared,
          improvementRatio: seed.improvementRatio,
        },
        staticAnalysis,
        safety: {
          modelIdentityBound: true,
          completeLegalAngleVectorGenerated: true,
          legalCorrectionPoseGenerated: true,
          collisionConstraintsRevalidated: true,
          hingeContactPolicySatisfied: true,
          wholeSceneStaticClear: true,
          staticCandidateRevalidated: true,
          continuousCandidatePathCertified: false,
          sceneApplied: false,
          autoApplicable: false,
        },
      }))
    }
    if (
      candidates.length === 0
      || candidates.length
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES
    ) return null

    return deepFreeze({
      version:
        FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_CORRECTION_CANDIDATES_VERSION,
      kind: 'statically_revalidated_single_hinge_correction_candidates',
      sourceIdentity: {
        projectId: translationCandidate.sourceIdentity.projectId,
        revision: translationCandidate.sourceIdentity.revision,
        fixedFaceId: translationCandidate.sourceIdentity.fixedFaceId,
        selectedHingeEdgeId:
          translationCandidate.sourceIdentity.selectedHingeEdgeId,
        contextKey: translationCandidate.sourceIdentity.contextKey,
        sourcePoseRequestKey: verifiedContext.sourcePoseRequestKey,
        blockingPoseRequestKey: verifiedContext.blockingPoseRequestKey,
        generation: translationCandidate.sourceIdentity.generation,
        requestSequence:
          translationCandidate.sourceIdentity.requestSequence,
        blockingSampleTime:
          translationCandidate.sourceIdentity.blockingSampleTime,
        sourceSelectedAngleDegrees:
          verifiedContext.sourceSelectedAngleDegrees,
        blockingSelectedAngleDegrees:
          translationCandidate.sourceIdentity.selectedAngleDegrees,
        collisionThickness:
          translationCandidate.sourceIdentity.collisionThickness,
      },
      sourcePartition: {
        version: translationCandidate.sourcePartition.version,
        stationaryFaceIds: [...partition.stationaryFaceIds],
        movingFaceIds: [...partition.movingFaceIds],
      },
      commonTranslation: {
        version: translationCandidate.version,
        translation: copyPoint(translationCandidate.translation),
        magnitude: translationCandidate.magnitude,
        certifiedMagnitudeUpperBound:
          translationCandidate.certifiedMagnitudeUpperBound,
        clearance: translationCandidate.clearance,
        maximumTranslation: translationCandidate.maximumTranslation,
        constraintCount: translationCandidate.constraints.length,
        solver: {
          method: translationCandidate.solver.method,
          seedMethod: translationCandidate.solver.seedMethod,
          activeConstraintIndices: [
            ...translationCandidate.solver.activeConstraintIndices,
          ],
          activeSetSize: translationCandidate.solver.activeSetSize,
          evaluatedActiveSetCount:
            translationCandidate.solver.evaluatedActiveSetCount,
          maximumActiveSetCount:
            translationCandidate.solver.maximumActiveSetCount,
        },
      },
      rotationFit: {
        version: fit.version,
        method: fit.analysis.method,
        objective: fit.analysis.objective,
        maximumAngleDeltaDegrees: fit.maximumAngleDeltaDegrees,
        angleDomain: {
          minimumDegrees: fit.angleDomain.minimumDegrees,
          maximumDegrees: fit.angleDomain.maximumDegrees,
        },
        worldAxis: {
          point: copyPoint(worldAxis.point),
          direction: copyPoint(worldAxis.direction),
        },
        childRotationSign:
          verifiedContext.selectedJoint.childRotationSign,
        movingPointCount: movingPoints.length,
        baselineResidualSquared: fit.baselineResidualSquared,
        baselineResidualRms: fit.baselineResidualRms,
        evaluatedCandidateCount: fit.evaluatedCandidateCount,
        seedCount: fit.seeds.length,
      },
      staticValidationWork: {
        strategy: 'full_non_adjacent_then_hinge_policy_v1',
        maximumTrianglePairVisits:
          MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS,
        plannedTrianglePairVisitUpperBound,
        actualTrianglePairVisits,
        fullScanCount,
        narrowScanCount,
      },
      candidates,
      safety: {
        modelIdentityBound: true,
        sourcePoseIdentityVerified: true,
        blockingPoseIdentityVerified: true,
        partitionRevalidated: true,
        completeLegalAngleVectorsGenerated: true,
        legalCorrectionPoseGenerated: true,
        collisionConstraintsRevalidated: true,
        hingeContactPolicySatisfied: true,
        wholeSceneStaticClear: true,
        staticCandidateRevalidated: true,
        continuousCandidatePathCertified: false,
        sceneApplied: false,
        autoApplicable: false,
      },
    })
  } catch {
    return null
  }
}

function verifyContextAndPoses(
  context: FoldPreviewTreeMotionContext,
  candidate: FoldPreviewTwoBodyCorrectionCandidate,
): VerifiedContext | null {
  const sourceSelectedAngleDegrees = context.selectedAngleDegrees
  const sourceAngles = replaceFoldPreviewTreeMotionSelectedAngle(
    context,
    sourceSelectedAngleDegrees,
  )
  if (
    !sourceAngles
    || context.version !== 'tree_single_hinge_motion_v1'
    || context.model.kind !== 'fold_graph'
    || context.model.projectId !== candidate.sourceIdentity.projectId
    || context.model.revision !== candidate.sourceIdentity.revision
    || context.fixedFaceId !== candidate.sourceIdentity.fixedFaceId
    || context.selectedHingeEdgeId
      !== candidate.sourceIdentity.selectedHingeEdgeId
    || context.contextKey !== candidate.sourceIdentity.contextKey
    || context.collisionThickness
      !== candidate.sourceIdentity.collisionThickness
    || !sameAngles(sourceAngles, context.appliedAngles)
  ) return null
  const selectedJoint = context.tree.joints.find(
    (joint) => joint.hinge.edgeId === context.selectedHingeEdgeId,
  )
  if (!selectedJoint) return null

  const sourcePoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    context.model,
    context.fixedFaceId,
    context.collisionThickness,
    sourceAngles,
  )
  if (
    !sourcePoseRequestKey
    || sourcePoseRequestKey !== candidate.sourceIdentity.sourcePoseRequestKey
  ) return null
  const blockingAngles = replaceFoldPreviewTreeMotionSelectedAngle(
    context,
    candidate.sourceIdentity.selectedAngleDegrees,
  )
  if (!blockingAngles) return null
  const blockingPoseRequestKey = createFoldPreviewTreeSceneCollisionPoseKey(
    context.model,
    context.fixedFaceId,
    context.collisionThickness,
    blockingAngles,
  )
  if (
    !blockingPoseRequestKey
    || blockingPoseRequestKey
      !== candidate.sourceIdentity.blockingPoseRequestKey
  ) return null
  const blockingPose = calculateFoldTreePoseWithAngles(context.tree, {
    kind: 'per_hinge',
    angles: blockingAngles,
  })
  if (!blockingPose) return null
  return Object.freeze({
    sourceAngles: copyAngles(sourceAngles),
    blockingAngles: copyAngles(blockingAngles),
    sourcePoseRequestKey,
    blockingPoseRequestKey,
    blockingPose,
    selectedJoint,
    sourceSelectedAngleDegrees,
  })
}

function verifyPartition(
  context: FoldPreviewTreeMotionContext,
  candidate: FoldPreviewTwoBodyCorrectionCandidate,
  selectedJoint: FoldPreviewTreeMotionContext['tree']['joints'][number],
): VerifiedPartition | null {
  const movingFaceIds = collectFoldTreeDependentFaces(
    context.tree,
    context.selectedHingeEdgeId,
  )
  if (!movingFaceIds || movingFaceIds.length === 0) return null
  const moving = new Set(movingFaceIds)
  const stationaryFaceIds = context.model.faces
    .map((face) => face.id)
    .filter((faceId) => !moving.has(faceId))
  if (
    moving.size !== movingFaceIds.length
    || stationaryFaceIds.length === 0
    || !stationaryFaceIds.includes(context.fixedFaceId)
    || !stationaryFaceIds.includes(selectedJoint.parentFaceId)
    || !moving.has(selectedJoint.childFaceId)
    || !sameIds(
      stationaryFaceIds,
      candidate.sourcePartition.stationaryFaceIds,
    )
    || !sameIds(
      movingFaceIds,
      candidate.sourcePartition.movingFaceIds,
    )
  ) return null
  const allFaceIds = context.model.faces.map((face) => face.id)
  if (
    new Set(allFaceIds).size !== allFaceIds.length
    || stationaryFaceIds.length + movingFaceIds.length !== allFaceIds.length
    || allFaceIds.some((faceId) =>
      !moving.has(faceId) && !stationaryFaceIds.includes(faceId))
  ) return null
  for (const joint of context.tree.joints) {
    const parentMoving = moving.has(joint.parentFaceId)
    const childMoving = moving.has(joint.childFaceId)
    if (
      parentMoving !== childMoving
      && joint.hinge.edgeId !== context.selectedHingeEdgeId
    ) return null
  }
  return Object.freeze({
    stationaryFaceIds: Object.freeze([...stationaryFaceIds]),
    movingFaceIds: Object.freeze([...movingFaceIds]),
  })
}

function worldSelectedHingeAxis(
  pose: FoldPreviewTreePose,
  joint: FoldPreviewTreeMotionContext['tree']['joints'][number],
): Readonly<{ point: Point; direction: Point }> | null {
  const parentTransform = pose.faceTransforms.get(joint.parentFaceId)
  const hingeTransform = pose.hingeTransforms.get(joint.hinge.edgeId)
  if (
    !parentTransform
    || !hingeTransform
    || !sameMatrix(parentTransform.elements, hingeTransform.elements)
  ) return null
  const start = new Vector3(
    joint.hinge.start.x,
    0,
    joint.hinge.start.z,
  ).applyMatrix4(parentTransform)
  const end = new Vector3(
    joint.hinge.end.x,
    0,
    joint.hinge.end.z,
  ).applyMatrix4(parentTransform)
  const direction = end.clone().sub(start)
  const length = direction.length()
  if (
    !finiteVector(start)
    || !finiteVector(end)
    || !Number.isFinite(length)
    || length <= 0
  ) return null
  direction.multiplyScalar(1 / length)
  if (!finiteVector(direction)) return null
  return Object.freeze({
    point: freezePoint(start.x, start.y, start.z),
    direction: freezePoint(direction.x, direction.y, direction.z),
  })
}

function collectMovingWorldMaterialPoints(
  context: FoldPreviewTreeMotionContext,
  pose: FoldPreviewTreePose,
  movingFaceIds: readonly string[],
) {
  const moving = new Set(movingFaceIds)
  const points = new Map<string, Point>()
  const jointFactor = Math.max(1, context.tree.joints.length)
  for (const face of context.model.faces) {
    if (!moving.has(face.id)) continue
    const transform = pose.faceTransforms.get(face.id)
    if (!transform) return null
    for (const vertex of face.polygon) {
      const world = new Vector3(vertex.x, 0, vertex.z)
        .applyMatrix4(transform)
      if (!finiteVector(world)) return null
      const position = freezePoint(world.x, world.y, world.z)
      const existing = points.get(vertex.vertexId)
      if (
        existing
        && !equivalentPoint(existing, position, jointFactor)
      ) return null
      if (!existing) points.set(vertex.vertexId, position)
    }
  }
  if (points.size === 0) return null
  return Object.freeze(
    [...points.entries()]
      .sort(([first], [second]) => compareText(first, second))
      .map(([id, position]) => Object.freeze({ id, position })),
  )
}

function prepareStaticAnalyzer(context: FoldPreviewTreeMotionContext) {
  const adjacencies: FoldPreviewCollisionAdjacency[] =
    context.model.hinges.map((hinge) => ({
      edgeId: hinge.edgeId,
      firstFaceId: hinge.leftFaceId,
      secondFaceId: hinge.rightFaceId,
    }))
  const constraints: FoldPreviewHingeContactConstraint[] =
    context.model.hinges.map((hinge) => ({
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
  return prepareFoldPreviewNarrowPhase(
    context.model.faces,
    adjacencies,
    constraints,
  )
}

function allFaceTrianglePairUpperBound(
  context: FoldPreviewTreeMotionContext,
) {
  let previousTriangleCount = 0
  let pairCount = 0
  for (const face of context.model.faces) {
    const triangleCount =
      triangulateFoldPreviewPolygon(face.polygon).length
    if (!Number.isSafeInteger(triangleCount) || triangleCount <= 0) {
      return null
    }
    const contribution = boundedProduct(
      previousTriangleCount,
      triangleCount,
    )
    if (contribution === null) return null
    pairCount += contribution
    previousTriangleCount += triangleCount
    if (
      !Number.isSafeInteger(pairCount)
      || !Number.isSafeInteger(previousTriangleCount)
      || pairCount
        > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS
    ) return null
  }
  return pairCount
}

function boundedProduct(first: number, second: number) {
  if (
    !Number.isSafeInteger(first)
    || first < 0
    || !Number.isSafeInteger(second)
    || second < 0
    || (first !== 0 && second > Number.MAX_SAFE_INTEGER / first)
  ) return null
  const result = first * second
  return Number.isSafeInteger(result) ? result : null
}

function boundedVisitSum(first: number, second: number) {
  if (
    !Number.isSafeInteger(first)
    || first < 0
    || !Number.isSafeInteger(second)
    || second < 0
    || second
      > MAX_FOLD_PREVIEW_TREE_SINGLE_HINGE_STATIC_TRIANGLE_PAIR_VISITS - first
  ) return null
  return first + second
}

function fullNonAdjacentScanIsClear(
  value: FoldPreviewFullScanNonAdjacentWitnessSet | null,
): value is Extract<
  FoldPreviewFullScanNonAdjacentWitnessSet,
  { kind: 'complete' }
> {
  if (!value || value.kind !== 'complete') return false
  const coverage = value.coverage
  return value.sourcePose === 'analyzed_input_pose'
    && value.requestIdentityBound === false
    && value.autoApplicable === false
    && value.witnessSamples.length === 0
    && coverage.authoritativePairScanComplete === true
    && coverage.allCollisionConstraintsRepresented === true
    && coverage.indeterminatePairCount === 0
    && coverage.touchingPairCount === 0
    && coverage.penetratingPairCount === 0
    && coverage.eligiblePairCount === 0
    && coverage.attemptedPairCount === 0
    && coverage.availablePairCount === 0
    && coverage.unavailablePairCount === 0
    && coverage.omittedByLimitCount === 0
    && coverage.expectedTrianglePairCount === coverage.trianglePairTests
    && coverage.trianglePairTests
      === coverage.aabbRejectedPairCount + coverage.satTests
    && coverage.satTests === coverage.satSeparatedPairCount
}

function staticClearAnalysis(
  result: FoldPreviewNarrowPhaseResult,
  fullScan: Extract<
    FoldPreviewFullScanNonAdjacentWitnessSet,
    { kind: 'complete' }
  >,
): FoldPreviewTreeSingleHingeStaticCorrectionAnalysis | null {
  let allowedHingeInteractionCount = 0
  for (const interaction of result.interactions) {
    if (
      interaction.relation !== 'hinge_adjacent'
      || interaction.geometryClass === 'indeterminate'
      || interaction.hingeDecision?.kind !== 'allowed_by_hinge_model'
    ) return null
    allowedHingeInteractionCount += 1
  }
  if (
    result.witnessSamples.length !== 0
    || result.witnessCoverage.eligiblePairCount !== 0
    || result.witnessCoverage.attemptedPairCount !== 0
    || result.witnessCoverage.unavailablePairCount !== 0
    || result.witnessCoverage.omittedByLimitCount !== 0
    || allowedHingeInteractionCount !== result.interactions.length
  ) return null
  const coverage = fullScan.coverage
  return deepFreeze({
    broadPhaseCandidateCount: result.broadPhaseCandidates,
    broadPhaseNonAdjacentCandidateCount:
      result.broadPhaseNonAdjacentCandidates,
    broadPhaseHingeAdjacentCandidateCount:
      result.broadPhaseHingeAdjacentCandidates,
    interactionCount: result.interactions.length,
    allowedHingeInteractionCount,
    trianglePairTests: result.trianglePairTests,
    satTests: result.satTests,
    numericalMargin: result.numericalMargin,
    fullScanBroadPhaseCandidateCount:
      coverage.broadPhaseCandidateCount,
    fullScanExpectedTrianglePairCount:
      coverage.expectedTrianglePairCount,
    fullScanTrianglePairTests: coverage.trianglePairTests,
    fullScanAabbRejectedPairCount:
      coverage.aabbRejectedPairCount,
    fullScanSatTests: coverage.satTests,
    fullScanSatSeparatedPairCount:
      coverage.satSeparatedPairCount,
  })
}

function sameAngles(
  first: readonly FoldPreviewHingeAngle[],
  second: readonly FoldPreviewHingeAngle[],
) {
  if (first.length !== second.length) return false
  for (let index = 0; index < first.length; index += 1) {
    if (
      first[index].edgeId !== second[index].edgeId
      || first[index].angleDegrees !== second[index].angleDegrees
    ) return false
  }
  return true
}

function sameIds(first: readonly string[], second: readonly string[]) {
  return first.length === second.length
    && first.every((value, index) => value === second[index])
}

function sameMatrix(first: readonly number[], second: readonly number[]) {
  return first.length === 16
    && second.length === 16
    && first.every((value, index) =>
      Number.isFinite(value) && value === second[index])
}

function equivalentPoint(first: Point, second: Point, jointFactor: number) {
  const scale = Math.max(
    1,
    Math.abs(first.x),
    Math.abs(first.y),
    Math.abs(first.z),
    Math.abs(second.x),
    Math.abs(second.y),
    Math.abs(second.z),
  )
  const tolerance = scale
    * Number.EPSILON
    * MATERIAL_POINT_EQUIVALENCE_FACTOR
    * jointFactor
  return Number.isFinite(tolerance)
    && Math.abs(first.x - second.x) <= tolerance
    && Math.abs(first.y - second.y) <= tolerance
    && Math.abs(first.z - second.z) <= tolerance
}

function copyAngles(angles: readonly FoldPreviewHingeAngle[]) {
  return angles.map((angle) => Object.freeze({
    edgeId: angle.edgeId,
    angleDegrees: angle.angleDegrees,
  }))
}

function copyPoint(value: Point): Point {
  return Object.freeze({
    x: canonicalZero(value.x),
    y: canonicalZero(value.y),
    z: canonicalZero(value.z),
  })
}

function freezePoint(x: number, y: number, z: number): Point {
  return Object.freeze({
    x: canonicalZero(x),
    y: canonicalZero(y),
    z: canonicalZero(z),
  })
}

function finiteVector(value: Vector3) {
  return Number.isFinite(value.x)
    && Number.isFinite(value.y)
    && Number.isFinite(value.z)
}

function validPositiveAngleDelta(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
    && value <= 180
}

function canonicalZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function compareText(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
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
