import { rerootFoldPreviewTree } from './foldPreviewAnchoring.ts'
import {
  MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
  MAX_FOLD_PREVIEW_COLLISION_FACES,
} from './foldPreviewCollision.ts'
import type {
  FoldPreviewHingeAngle,
  FoldPreviewTreeKinematics,
} from './foldPreviewKinematics.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from './foldPreviewModel.ts'
import {
  MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
} from './foldPreviewNarrowCollision.ts'

export const FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION =
  'tree_single_hinge_motion_v1'
export const MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_ID_LENGTH = 512
export const MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH = 8 * 1_024 * 1_024

export type FoldPreviewTreeMotionContextInput = Readonly<{
  model: FoldGraphPreviewModel
  fixedFaceId: string
  selectedHingeEdgeId: string
  appliedAngles: readonly FoldPreviewHingeAngle[]
  collisionThickness: number
  visualThickness: number
}>

export type FoldPreviewTreeMotionContext = Readonly<{
  version: typeof FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION
  contextKey: string
  model: FoldGraphPreviewModel
  tree: FoldPreviewTreeKinematics
  fixedFaceId: string
  selectedHingeEdgeId: string
  selectedAngleDegrees: number
  appliedAngles: readonly FoldPreviewHingeAngle[]
  nonSelectedAngles: readonly FoldPreviewHingeAngle[]
  collisionThickness: number
  visualThickness: number
}>

export type FoldPreviewTreeMotionTargetDifference =
  | Readonly<{
      kind: 'same'
      targetAngles: readonly FoldPreviewHingeAngle[]
    }>
  | Readonly<{
      kind: 'selected_only'
      targetSelectedAngleDegrees: number
      targetAngles: readonly FoldPreviewHingeAngle[]
    }>
  | Readonly<{
      kind: 'invalid_or_multiple'
      reason:
        | 'invalid_target_vector'
        | 'non_selected_change'
        | 'multiple_changes'
      changedHingeEdgeIds: readonly string[]
    }>

type FoldPreviewTreeModelSnapshot = Omit<
  FoldGraphPreviewModel,
  'kinematics'
> & Readonly<{
  kinematics: FoldPreviewTreeKinematics
}>

const preparedContexts = new WeakSet<object>()

/**
 * Snapshots the complete input for one selected-hinge tree motion.
 *
 * The opaque key deliberately excludes only the selected hinge magnitude.
 * Every identity, thickness, and non-selected angle that can change the
 * selected hinge's world motion remains part of the key.
 */
export function prepareFoldPreviewTreeMotionContext(
  input: FoldPreviewTreeMotionContextInput,
): FoldPreviewTreeMotionContext | null {
  try {
    if (!isRecord(input)) return null
    const rawModel = input.model
    const rawFixedFaceId = input.fixedFaceId
    const rawSelectedHingeEdgeId = input.selectedHingeEdgeId
    const rawAppliedAngles = input.appliedAngles
    const rawCollisionThickness = input.collisionThickness
    const rawVisualThickness = input.visualThickness

    const model = snapshotTreeModel(rawModel)
    if (!model) return null
    if (
      !validId(rawFixedFaceId)
      || !validId(rawSelectedHingeEdgeId)
      || !validCollisionThickness(rawCollisionThickness)
      || !validVisualThickness(rawVisualThickness)
    ) return null
    const fixedFaceId = rawFixedFaceId
    const selectedHingeEdgeId = rawSelectedHingeEdgeId
    const collisionThickness = normalizeZero(rawCollisionThickness)
    const visualThickness = normalizeZero(rawVisualThickness)

    const tree = rerootFoldPreviewTree(
      model.kinematics,
      fixedFaceId,
    )
    if (
      !tree
      || !modelMatchesTree(model, tree)
      || !tree.joints.some(
        (joint) => joint.hinge.edgeId === selectedHingeEdgeId,
      )
    ) return null

    const expectedEdgeIds = canonicalHingeEdgeIds(model.hinges)
    const appliedAngles = normalizeCompleteAngles(
      rawAppliedAngles,
      expectedEdgeIds,
    )
    if (!appliedAngles) return null
    const selectedAngleDegrees = appliedAngles.find(
      (angle) => angle.edgeId === selectedHingeEdgeId,
    )?.angleDegrees
    if (!validAngle(selectedAngleDegrees)) return null

    const nonSelectedAngles = appliedAngles.filter(
      (angle) => angle.edgeId !== selectedHingeEdgeId,
    )
    if (nonSelectedAngles.length !== appliedAngles.length - 1) return null
    const contextKey = createContextKey({
      model,
      fixedFaceId,
      selectedHingeEdgeId,
      collisionThickness,
      visualThickness,
      nonSelectedAngles,
    })
    if (!contextKey) return null

    const context = deepFreeze({
      version: FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION,
      contextKey,
      model,
      tree,
      fixedFaceId,
      selectedHingeEdgeId,
      selectedAngleDegrees,
      appliedAngles: copyAngles(appliedAngles),
      nonSelectedAngles: copyAngles(nonSelectedAngles),
      collisionThickness,
      visualThickness,
    }) as FoldPreviewTreeMotionContext
    preparedContexts.add(context)
    return context
  } catch {
    return null
  }
}

/**
 * Returns one complete canonical vector with only the selected magnitude
 * replaced. Invalid or forged contexts and out-of-range angles fail closed.
 */
export function replaceFoldPreviewTreeMotionSelectedAngle(
  context: FoldPreviewTreeMotionContext,
  selectedAngleDegrees: number,
): readonly FoldPreviewHingeAngle[] | null {
  try {
    if (
      !isPreparedContext(context)
      || !validAngle(selectedAngleDegrees)
    ) return null
    const normalizedAngle = normalizeZero(selectedAngleDegrees)
    let replacementCount = 0
    const result = context.appliedAngles.map((angle) => {
      if (angle.edgeId !== context.selectedHingeEdgeId) {
        return {
          edgeId: angle.edgeId,
          angleDegrees: angle.angleDegrees,
        }
      }
      replacementCount += 1
      return {
        edgeId: angle.edgeId,
        angleDegrees: normalizedAngle,
      }
    })
    return replacementCount === 1
      ? deepFreeze(result)
      : null
  } catch {
    return null
  }
}

/**
 * Classifies an arbitrary complete target vector against the applied snapshot.
 *
 * A one-hinge runner may accept only `same` or `selected_only`. A valid change
 * to any non-selected hinge shares the fail-closed result with multi-hinge
 * changes so callers cannot accidentally route it through the scalar runner.
 */
export function classifyFoldPreviewTreeMotionTarget(
  context: FoldPreviewTreeMotionContext,
  targetAngles: readonly FoldPreviewHingeAngle[],
): FoldPreviewTreeMotionTargetDifference {
  try {
    if (!isPreparedContext(context)) return invalidTarget()
    const expectedEdgeIds = context.appliedAngles.map(
      (angle) => angle.edgeId,
    )
    const normalizedTarget = normalizeCompleteAngles(
      targetAngles,
      expectedEdgeIds,
    )
    if (!normalizedTarget) return invalidTarget()

    const changedHingeEdgeIds: string[] = []
    for (let index = 0; index < context.appliedAngles.length; index += 1) {
      const applied = context.appliedAngles[index]
      const target = normalizedTarget[index]
      if (
        applied.edgeId !== target.edgeId
        || applied.angleDegrees !== target.angleDegrees
      ) changedHingeEdgeIds.push(applied.edgeId)
    }

    if (changedHingeEdgeIds.length === 0) {
      return deepFreeze({
        kind: 'same',
        targetAngles: copyAngles(normalizedTarget),
      })
    }
    if (
      changedHingeEdgeIds.length === 1
      && changedHingeEdgeIds[0] === context.selectedHingeEdgeId
    ) {
      const targetSelectedAngleDegrees = normalizedTarget.find(
        (angle) => angle.edgeId === context.selectedHingeEdgeId,
      )?.angleDegrees
      if (!validAngle(targetSelectedAngleDegrees)) return invalidTarget()
      return deepFreeze({
        kind: 'selected_only',
        targetSelectedAngleDegrees,
        targetAngles: copyAngles(normalizedTarget),
      })
    }
    return deepFreeze({
      kind: 'invalid_or_multiple',
      reason: changedHingeEdgeIds.length === 1
        ? 'non_selected_change'
        : 'multiple_changes',
      changedHingeEdgeIds: [...changedHingeEdgeIds],
    })
  } catch {
    return invalidTarget()
  }
}

function createContextKey(input: Readonly<{
  model: FoldPreviewTreeModelSnapshot
  fixedFaceId: string
  selectedHingeEdgeId: string
  collisionThickness: number
  visualThickness: number
  nonSelectedAngles: readonly FoldPreviewHingeAngle[]
}>) {
  const key = JSON.stringify([
    FOLD_PREVIEW_TREE_MOTION_CONTEXT_VERSION,
    input.model.projectId,
    input.model.revision,
    input.model.kind,
    input.model.kinematics.kind,
    input.fixedFaceId,
    input.collisionThickness,
    input.visualThickness,
    input.selectedHingeEdgeId,
    input.nonSelectedAngles.map((angle) => [
      angle.edgeId,
      angle.angleDegrees,
    ]),
  ])
  return typeof key === 'string'
    && key.length > 0
    && key.length <= MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_KEY_LENGTH
    ? key
    : null
}

function snapshotTreeModel(
  value: unknown,
): FoldPreviewTreeModelSnapshot | null {
  if (!isRecord(value)) return null
  const model = value
  const rawKind = model.kind
  const rawProjectId = model.projectId
  const rawRevision = model.revision
  const rawWorldUnitsPerMillimetre = model.worldUnitsPerMillimetre
  const rawPaperCenter = model.paperCenter
  const rawWorldBounds = model.worldBounds
  const rawFaces = model.faces
  const rawHinges = model.hinges
  const rawKinematics = model.kinematics
  const paperCenter = snapshotPaperCenter(rawPaperCenter)
  const worldBounds = snapshotWorldBounds(rawWorldBounds)
  if (
    rawKind !== 'fold_graph'
    || !validId(rawProjectId)
    || !validRevision(rawRevision)
    || !isPositiveFinite(rawWorldUnitsPerMillimetre)
    || !paperCenter
    || !worldBounds
    || !Array.isArray(rawFaces)
    || !Array.isArray(rawHinges)
    || !isRecord(rawKinematics)
  ) return null
  const rawKinematicsKind = rawKinematics.kind
  const rawRootFaceId = rawKinematics.rootFaceId
  const rawJoints = rawKinematics.joints
  if (
    rawKinematicsKind !== 'tree'
    || !validId(rawRootFaceId)
    || !Array.isArray(rawJoints)
  ) return null

  const faceCount = rawFaces.length
  const hingeCount = rawHinges.length
  const jointCount = rawJoints.length
  if (
    !validBoundedArrayLength(
      faceCount,
      2,
      MAX_FOLD_PREVIEW_COLLISION_FACES,
    )
    || !validBoundedArrayLength(
      hingeCount,
      1,
      MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
    )
    || !validBoundedArrayLength(
      jointCount,
      1,
      MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES,
    )
    || jointCount !== hingeCount
    || faceCount !== jointCount + 1
  ) return null

  const vertexPositions = new Map<string, Readonly<{ x: number; z: number }>>()
  const faces: FoldPreviewFaceModel[] = []
  const faceIds = new Set<string>()
  let vertexCount = 0
  for (let faceIndex = 0; faceIndex < faceCount; faceIndex += 1) {
    const rawFace = rawFaces[faceIndex]
    if (!isRecord(rawFace)) return null
    const rawFaceId = rawFace.id
    const rawPolygon = rawFace.polygon
    if (
      !validId(rawFaceId)
      || faceIds.has(rawFaceId)
      || !Array.isArray(rawPolygon)
    ) return null
    const polygonLength = rawPolygon.length
    if (
      !validBoundedArrayLength(
        polygonLength,
        3,
        MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES,
      )
      || vertexCount
        > MAX_FOLD_PREVIEW_NARROW_PHASE_PREPARED_VERTICES - polygonLength
    ) return null
    vertexCount += polygonLength

    const polygon: FoldPreviewFaceModel['polygon'][number][] = []
    const faceVertexIds = new Set<string>()
    for (let pointIndex = 0; pointIndex < polygonLength; pointIndex += 1) {
      const point = snapshotPoint(rawPolygon[pointIndex])
      if (!point || faceVertexIds.has(point.vertexId)) return null
      const existing = vertexPositions.get(point.vertexId)
      if (
        existing
        && (existing.x !== point.x || existing.z !== point.z)
      ) return null
      vertexPositions.set(point.vertexId, { x: point.x, z: point.z })
      faceVertexIds.add(point.vertexId)
      polygon.push(point)
    }
    faceIds.add(rawFaceId)
    faces.push({ id: rawFaceId, polygon })
  }

  const hinges: FoldPreviewHingeModel[] = []
  const hingesById = new Map<string, FoldPreviewHingeModel>()
  for (let hingeIndex = 0; hingeIndex < hingeCount; hingeIndex += 1) {
    const hinge = snapshotHinge(rawHinges[hingeIndex])
    if (
      !hinge
      || hingesById.has(hinge.edgeId)
      || !faceIds.has(hinge.leftFaceId)
      || !faceIds.has(hinge.rightFaceId)
    ) return null
    hinges.push(hinge)
    hingesById.set(hinge.edgeId, hinge)
  }

  if (
    !faceIds.has(rawRootFaceId)
    || hinges.length !== jointCount
    || faces.length !== jointCount + 1
  ) return null
  const joints: FoldPreviewTreeKinematics['joints'][number][] = []
  for (let jointIndex = 0; jointIndex < jointCount; jointIndex += 1) {
    const rawJoint = rawJoints[jointIndex]
    if (!isRecord(rawJoint)) return null
    const rawParentFaceId = rawJoint.parentFaceId
    const rawChildFaceId = rawJoint.childFaceId
    const rawJointHinge = rawJoint.hinge
    const rawChildRotationSign = rawJoint.childRotationSign
    if (
      !validId(rawParentFaceId)
      || !validId(rawChildFaceId)
      || !faceIds.has(rawParentFaceId)
      || !faceIds.has(rawChildFaceId)
      || (
        rawChildRotationSign !== 1
        && rawChildRotationSign !== -1
      )
    ) return null
    const hinge = snapshotHinge(rawJointHinge)
    if (!hinge) return null
    joints.push({
      parentFaceId: rawParentFaceId,
      childFaceId: rawChildFaceId,
      hinge,
      childRotationSign: rawChildRotationSign,
    })
  }
  const tree: FoldPreviewTreeKinematics = {
    kind: 'tree',
    rootFaceId: rawRootFaceId,
    joints,
  }
  const snapshot: FoldPreviewTreeModelSnapshot = {
    kind: 'fold_graph',
    projectId: rawProjectId,
    revision: rawRevision,
    worldUnitsPerMillimetre: normalizeZero(rawWorldUnitsPerMillimetre),
    paperCenter,
    worldBounds,
    faces,
    hinges,
    kinematics: tree,
  }
  return modelMatchesTree(snapshot, tree) ? snapshot : null
}

function snapshotHinge(value: unknown): FoldPreviewHingeModel | null {
  if (!isRecord(value)) return null
  const rawEdgeId = value.edgeId
  const rawLeftFaceId = value.leftFaceId
  const rawRightFaceId = value.rightFaceId
  const rawStart = value.start
  const rawEnd = value.end
  const rawAxis = value.axis
  const rawAssignment = value.assignment
  const rawRotationSign = value.rotationSign
  const start = snapshotPoint(rawStart)
  const end = snapshotPoint(rawEnd)
  const axis = snapshotAxis(rawAxis)
  if (
    !validId(rawEdgeId)
    || !validId(rawLeftFaceId)
    || !validId(rawRightFaceId)
    || rawLeftFaceId === rawRightFaceId
    || !start
    || !end
    || start.vertexId === end.vertexId
    || !axis
    || (rawAssignment !== 'mountain' && rawAssignment !== 'valley')
    || (rawRotationSign !== 1 && rawRotationSign !== -1)
    || rawRotationSign !== (rawAssignment === 'mountain' ? 1 : -1)
  ) return null
  const snapshot: FoldPreviewHingeModel = {
    edgeId: rawEdgeId,
    leftFaceId: rawLeftFaceId,
    rightFaceId: rawRightFaceId,
    start,
    end,
    axis,
    assignment: rawAssignment,
    rotationSign: rawRotationSign,
  }
  return validHingeGeometry(snapshot) ? snapshot : null
}

function modelMatchesTree(
  model: FoldPreviewTreeModelSnapshot,
  tree: FoldPreviewTreeKinematics,
) {
  if (
    model.faces.length !== tree.joints.length + 1
    || model.hinges.length !== tree.joints.length
  ) return false
  const faceIds = new Set(model.faces.map((face) => face.id))
  if (
    faceIds.size !== model.faces.length
    || !faceIds.has(tree.rootFaceId)
  ) return false
  const modelHingesById = new Map(
    model.hinges.map((hinge) => [hinge.edgeId, hinge]),
  )
  if (modelHingesById.size !== model.hinges.length) return false

  const reachedFaces = new Set<string>([tree.rootFaceId])
  const reachedHinges = new Set<string>()
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
      !reachedFaces.has(joint.parentFaceId)
      || reachedFaces.has(joint.childFaceId)
      || !faceIds.has(joint.childFaceId)
      || reachedHinges.has(joint.hinge.edgeId)
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
    reachedFaces.add(joint.childFaceId)
    reachedHinges.add(joint.hinge.edgeId)
  }
  return reachedFaces.size === faceIds.size
    && reachedHinges.size === modelHingesById.size
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

function normalizeCompleteAngles(
  value: unknown,
  canonicalEdgeIds: readonly string[],
): readonly FoldPreviewHingeAngle[] | null {
  if (!Array.isArray(value)) return null
  const angleCount = value.length
  if (
    !Number.isSafeInteger(angleCount)
    || angleCount !== canonicalEdgeIds.length
    || angleCount > MAX_FOLD_PREVIEW_COLLISION_ADJACENCIES
  ) return null
  const expected = new Set(canonicalEdgeIds)
  if (expected.size !== canonicalEdgeIds.length) return null
  const byEdgeId = new Map<string, number>()
  for (let angleIndex = 0; angleIndex < angleCount; angleIndex += 1) {
    const rawAngle = value[angleIndex]
    if (!isRecord(rawAngle)) return null
    const rawEdgeId = rawAngle.edgeId
    const rawAngleDegrees = rawAngle.angleDegrees
    if (
      !validId(rawEdgeId)
      || !expected.has(rawEdgeId)
      || byEdgeId.has(rawEdgeId)
      || !validAngle(rawAngleDegrees)
    ) return null
    byEdgeId.set(
      rawEdgeId,
      normalizeZero(rawAngleDegrees),
    )
  }
  const normalized = canonicalEdgeIds.map((edgeId) => {
    const angleDegrees = byEdgeId.get(edgeId)
    return validAngle(angleDegrees)
      ? { edgeId, angleDegrees }
      : null
  })
  return normalized.some((angle) => angle === null)
    ? null
    : normalized as readonly FoldPreviewHingeAngle[]
}

function canonicalHingeEdgeIds(
  hinges: readonly FoldPreviewHingeModel[],
) {
  return hinges
    .map((hinge) => hinge.edgeId)
    .sort(compareText)
}

function copyAngles(
  angles: readonly FoldPreviewHingeAngle[],
) {
  return angles.map((angle) => ({
    edgeId: angle.edgeId,
    angleDegrees: angle.angleDegrees,
  }))
}

function invalidTarget(): FoldPreviewTreeMotionTargetDifference {
  return deepFreeze({
    kind: 'invalid_or_multiple',
    reason: 'invalid_target_vector',
    changedHingeEdgeIds: [],
  })
}

function isPreparedContext(
  value: unknown,
): value is FoldPreviewTreeMotionContext {
  return isRecord(value) && preparedContexts.has(value)
}

function snapshotPoint(
  value: unknown,
): FoldPreviewHingeModel['start'] | null {
  if (!isRecord(value)) return null
  const rawVertexId = value.vertexId
  const rawX = value.x
  const rawZ = value.z
  return validId(rawVertexId)
    && finiteNumber(rawX)
    && finiteNumber(rawZ)
    ? {
        vertexId: rawVertexId,
        x: normalizeZero(rawX),
        z: normalizeZero(rawZ),
      }
    : null
}

function snapshotAxis(
  value: unknown,
): FoldPreviewHingeModel['axis'] | null {
  if (!isRecord(value)) return null
  const rawX = value.x
  const rawZ = value.z
  return finiteNumber(rawX) && finiteNumber(rawZ)
    ? { x: normalizeZero(rawX), z: normalizeZero(rawZ) }
    : null
}

function validHingeGeometry(hinge: FoldPreviewHingeModel) {
  const deltaX = hinge.end.x - hinge.start.x
  const deltaZ = hinge.end.z - hinge.start.z
  const segmentLength = Math.hypot(deltaX, deltaZ)
  const axisLength = Math.hypot(hinge.axis.x, hinge.axis.z)
  if (!(segmentLength > 0) || !(axisLength > 0)) return false
  const scale = segmentLength * axisLength
  const cross = deltaX * hinge.axis.z - deltaZ * hinge.axis.x
  const dot = deltaX * hinge.axis.x + deltaZ * hinge.axis.z
  return Number.isFinite(scale)
    && Number.isFinite(cross)
    && Number.isFinite(dot)
    && dot > 0
    && Math.abs(cross) <= scale * Number.EPSILON * 16
}

function snapshotPaperCenter(
  value: unknown,
): FoldGraphPreviewModel['paperCenter'] | null {
  if (!isRecord(value)) return null
  const rawX = value.x
  const rawY = value.y
  return finiteNumber(rawX) && finiteNumber(rawY)
    ? { x: normalizeZero(rawX), y: normalizeZero(rawY) }
    : null
}

function snapshotWorldBounds(
  value: unknown,
): FoldGraphPreviewModel['worldBounds'] | null {
  if (!isRecord(value)) return null
  const rawMinX = value.minX
  const rawMinZ = value.minZ
  const rawMaxX = value.maxX
  const rawMaxZ = value.maxZ
  return finiteNumber(rawMinX)
    && finiteNumber(rawMinZ)
    && finiteNumber(rawMaxX)
    && finiteNumber(rawMaxZ)
    && rawMinX < rawMaxX
    && rawMinZ < rawMaxZ
    ? {
        minX: normalizeZero(rawMinX),
        minZ: normalizeZero(rawMinZ),
        maxX: normalizeZero(rawMaxX),
        maxZ: normalizeZero(rawMaxZ),
      }
    : null
}

function validCollisionThickness(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
}

function validVisualThickness(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_FOLD_PREVIEW_TREE_MOTION_CONTEXT_ID_LENGTH
    && value.trim().length > 0
}

function isPositiveFinite(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
}

function finiteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function validBoundedArrayLength(
  value: unknown,
  minimum: number,
  maximum: number,
): value is number {
  return Number.isSafeInteger(value)
    && (value as number) >= minimum
    && (value as number) <= maximum
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}

function normalizeZero(value: number) {
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
