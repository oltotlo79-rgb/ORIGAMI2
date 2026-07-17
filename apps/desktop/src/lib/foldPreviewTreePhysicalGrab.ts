import { Vector3, type Matrix4 } from 'three'
import {
  collectFoldTreeDependentFaces,
  rerootFoldPreviewTree,
} from './foldPreviewAnchoring.ts'
import {
  FOLD_PREVIEW_BACK_MATERIAL_INDEX,
  FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
} from './foldPreviewGeometry.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
  type FoldPreviewTreeAngleInput,
  type FoldPreviewTreeKinematics,
} from './foldPreviewKinematics.ts'
import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  prepareFoldPreviewPhysicalGrab,
  type FoldPreviewPhysicalGrabPoint,
  type FoldPreviewPhysicalGrabPrepareReason,
  type FoldPreviewPhysicalGrabRay,
  type FoldPreviewPhysicalGrabSession,
} from './foldPreviewPhysicalGrab.ts'
import type { FoldGraphPreviewModel } from './foldPreviewModel.ts'
import type { FoldPreviewFaceSurfaceHit } from './foldPreviewPicking.ts'

export type FoldPreviewTreePhysicalGrabPrepareInput = Readonly<{
  model: FoldGraphPreviewModel
  fixedFaceId: string
  selectedHingeEdgeId: string
  appliedAngles: FoldPreviewTreeAngleInput
  contextKey: string
  surfaceHit: FoldPreviewFaceSurfaceHit
  visualThickness: number
  startRay: FoldPreviewPhysicalGrabRay
  minimumOrbitRadius: number
}>

export type FoldPreviewTreePhysicalGrabPrepareReason =
  | 'invalid_input'
  | 'tree_unavailable'
  | 'selected_hinge_unavailable'
  | 'surface_face_not_dependent'
  | 'surface_material_unsupported'
  | 'surface_cap_mismatch'
  | 'pose_unavailable'
  | 'pose_mismatch'
  | 'numeric'
  | 'physical_grab_rejected'

export type FoldPreviewTreePhysicalGrabReady = Readonly<{
  kind: 'ready'
  mapping: typeof FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
  contextKey: string
  fixedFaceId: string
  hingeEdgeId: string
  parentFaceId: string
  childFaceId: string
  dependentFaceIds: readonly string[]
  surfaceFaceId: string
  appliedAngleDegrees: number
  appliedAngles: readonly FoldPreviewHingeAngle[]
  surface: 'front' | 'back'
  materialIndex: number
  grabLocalPoint: FoldPreviewPhysicalGrabPoint
  /**
   * World point with the selected hinge at zero while every other hinge keeps
   * its applied angle. It is the canonical rest point for this one-DOF grab.
   */
  grabRestWorldPoint: FoldPreviewPhysicalGrabPoint
  grabWorldPoint: FoldPreviewPhysicalGrabPoint
  session: FoldPreviewPhysicalGrabSession
}>

export type FoldPreviewTreePhysicalGrabPrepareResult =
  | FoldPreviewTreePhysicalGrabReady
  | Readonly<{
      kind: 'rejected'
      reason: Exclude<
        FoldPreviewTreePhysicalGrabPrepareReason,
        'physical_grab_rejected'
      >
    }>
  | Readonly<{
      kind: 'rejected'
      reason: 'physical_grab_rejected'
      physicalGrabReason: FoldPreviewPhysicalGrabPrepareReason
    }>

const CAP_RELATIVE_TOLERANCE = 1e-7
const POSE_RELATIVE_TOLERANCE = 1e-7
const NUMERIC_TOLERANCE_FACTOR = 256
const UNIT_TOLERANCE = 1e-10

/**
 * Converts a cap hit on the selected hinge's moving subtree into one rigid
 * circular grab session. Other hinge angles remain fixed while this session
 * resolves only the selected hinge's 0–180 degree magnitude.
 */
export function prepareFoldPreviewTreePhysicalGrab(
  input: FoldPreviewTreePhysicalGrabPrepareInput,
): FoldPreviewTreePhysicalGrabPrepareResult {
  if (!validInputShape(input)) return rejected('invalid_input')

  let tree: FoldPreviewTreeKinematics | null
  try {
    tree = input.model.kinematics.kind === 'tree'
      ? rerootFoldPreviewTree(input.model.kinematics, input.fixedFaceId)
      : null
  } catch {
    return rejected('tree_unavailable')
  }
  if (!tree || !modelMatchesTree(input.model, tree)) {
    return rejected('tree_unavailable')
  }

  const selectedJoint = tree.joints.find(
    (joint) => joint.hinge.edgeId === input.selectedHingeEdgeId,
  )
  if (!selectedJoint) return rejected('selected_hinge_unavailable')

  let dependentFaceIds: readonly string[] | null
  try {
    dependentFaceIds = collectFoldTreeDependentFaces(
      tree,
      input.selectedHingeEdgeId,
    )
  } catch {
    return rejected('tree_unavailable')
  }
  if (!dependentFaceIds || dependentFaceIds.length === 0) {
    return rejected('tree_unavailable')
  }
  if (!dependentFaceIds.includes(input.surfaceHit.faceId)) {
    return rejected('surface_face_not_dependent')
  }

  const surface = input.surfaceHit.materialIndex === FOLD_PREVIEW_FRONT_MATERIAL_INDEX
    ? 'front'
    : input.surfaceHit.materialIndex === FOLD_PREVIEW_BACK_MATERIAL_INDEX
      ? 'back'
      : null
  if (!surface) return rejected('surface_material_unsupported')

  const halfThickness = input.visualThickness / 2
  const expectedCapY = Math.fround(surface === 'front' ? halfThickness : -halfThickness)
  if (
    !Number.isFinite(halfThickness)
    || halfThickness <= 0
    || !Number.isFinite(expectedCapY)
    || expectedCapY === 0
  ) return rejected('numeric')
  const capTolerance = strictTolerance(
    input.visualThickness,
    maximumMagnitude([
      input.visualThickness,
      input.surfaceHit.localPoint.x,
      input.surfaceHit.localPoint.y,
      input.surfaceHit.localPoint.z,
    ]),
    CAP_RELATIVE_TOLERANCE,
  )
  if (capTolerance === null) return rejected('numeric')
  if (Math.abs(input.surfaceHit.localPoint.y - expectedCapY) > capTolerance) {
    return rejected('surface_cap_mismatch')
  }

  const normalizedAngles = normalizeAngles(tree, input.appliedAngles)
  if (!normalizedAngles) return rejected('invalid_input')
  const selectedAngle = normalizedAngles.find(
    (angle) => angle.edgeId === input.selectedHingeEdgeId,
  )?.angleDegrees
  if (!validAngle(selectedAngle)) return rejected('invalid_input')
  const zeroSelectedAngles = normalizedAngles.map((angle) => ({
    edgeId: angle.edgeId,
    angleDegrees: angle.edgeId === input.selectedHingeEdgeId
      ? 0
      : angle.angleDegrees,
  }))

  let appliedPose: ReturnType<typeof calculateFoldTreePoseWithAngles>
  let zeroSelectedPose: ReturnType<typeof calculateFoldTreePoseWithAngles>
  try {
    appliedPose = calculateFoldTreePoseWithAngles(tree, {
      kind: 'per_hinge',
      angles: normalizedAngles,
    })
    zeroSelectedPose = calculateFoldTreePoseWithAngles(tree, {
      kind: 'per_hinge',
      angles: zeroSelectedAngles,
    })
  } catch {
    return rejected('pose_unavailable')
  }
  if (!appliedPose || !zeroSelectedPose) return rejected('pose_unavailable')

  const appliedFaceTransform = appliedPose.faceTransforms.get(input.surfaceHit.faceId)
  const zeroSelectedFaceTransform =
    zeroSelectedPose.faceTransforms.get(input.surfaceHit.faceId)
  const hingeTransform = zeroSelectedPose.hingeTransforms.get(
    input.selectedHingeEdgeId,
  )
  if (
    !finiteMatrix(appliedFaceTransform)
    || !finiteMatrix(zeroSelectedFaceTransform)
    || !finiteMatrix(hingeTransform)
  ) return rejected('pose_unavailable')

  const grabLocalPoint = copyPoint(input.surfaceHit.localPoint)
  const grabRestWorldPoint = transformPoint(
    grabLocalPoint,
    zeroSelectedFaceTransform,
  )
  const expectedWorldPoint = transformPoint(
    grabLocalPoint,
    appliedFaceTransform,
  )
  const axisStart = transformPoint({
    x: selectedJoint.hinge.start.x,
    y: 0,
    z: selectedJoint.hinge.start.z,
  }, hingeTransform)
  const axisEnd = transformPoint({
    x: selectedJoint.hinge.end.x,
    y: 0,
    z: selectedJoint.hinge.end.z,
  }, hingeTransform)
  if (
    !grabRestWorldPoint
    || !expectedWorldPoint
    || !axisStart
    || !axisEnd
  ) return rejected('numeric')

  const hingeLength = pointDistance(axisStart, axisEnd)
  const grabScale = pointDistance(grabRestWorldPoint, axisStart)
  if (!Number.isFinite(hingeLength) || !Number.isFinite(grabScale)) {
    return rejected('numeric')
  }
  const poseTolerance = strictTolerance(
    Math.max(
      input.visualThickness,
      input.minimumOrbitRadius,
      hingeLength,
      grabScale,
    ),
    maximumMagnitude([
      axisStart.x,
      axisStart.y,
      axisStart.z,
      axisEnd.x,
      axisEnd.y,
      axisEnd.z,
      grabRestWorldPoint.x,
      grabRestWorldPoint.y,
      grabRestWorldPoint.z,
      input.surfaceHit.worldPoint.x,
      input.surfaceHit.worldPoint.y,
      input.surfaceHit.worldPoint.z,
    ]),
    POSE_RELATIVE_TOLERANCE,
  )
  if (poseTolerance === null) return rejected('numeric')
  if (pointDistance(expectedWorldPoint, input.surfaceHit.worldPoint) > poseTolerance) {
    return rejected('pose_mismatch')
  }

  const physicalGrab = prepareFoldPreviewPhysicalGrab({
    contextKey: input.contextKey,
    axisStart,
    axisEnd,
    movingRotationSign: selectedJoint.childRotationSign,
    appliedAngleDegrees: selectedAngle,
    grabRestWorldPoint,
    grabWorldPoint: input.surfaceHit.worldPoint,
    startRay: input.startRay,
    minimumOrbitRadius: input.minimumOrbitRadius,
  })
  if (physicalGrab.kind !== 'ready') {
    return Object.freeze({
      kind: 'rejected',
      reason: 'physical_grab_rejected',
      physicalGrabReason: physicalGrab.reason,
    })
  }

  return Object.freeze({
    kind: 'ready',
    mapping: FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
    contextKey: input.contextKey,
    fixedFaceId: tree.rootFaceId,
    hingeEdgeId: selectedJoint.hinge.edgeId,
    parentFaceId: selectedJoint.parentFaceId,
    childFaceId: selectedJoint.childFaceId,
    dependentFaceIds: Object.freeze([...dependentFaceIds]),
    surfaceFaceId: input.surfaceHit.faceId,
    appliedAngleDegrees: selectedAngle,
    appliedAngles: freezeAngles(normalizedAngles),
    surface,
    materialIndex: input.surfaceHit.materialIndex,
    grabLocalPoint: freezePoint(grabLocalPoint),
    grabRestWorldPoint: freezePoint(grabRestWorldPoint),
    grabWorldPoint: freezePoint(input.surfaceHit.worldPoint),
    session: physicalGrab.session,
  })
}

function normalizeAngles(
  tree: FoldPreviewTreeKinematics,
  input: FoldPreviewTreeAngleInput,
): readonly FoldPreviewHingeAngle[] | null {
  if (!input || typeof input !== 'object') return null
  if (input.kind === 'uniform') {
    if (!validAngle(input.angleDegrees)) return null
    return tree.joints.map((joint) => ({
      edgeId: joint.hinge.edgeId,
      angleDegrees: input.angleDegrees,
    }))
  }
  if (
    input.kind !== 'per_hinge'
    || !Array.isArray(input.angles)
    || input.angles.length !== tree.joints.length
  ) return null

  const byEdgeId = new Map<string, number>()
  for (const angle of input.angles) {
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
    normalized.push({ edgeId: joint.hinge.edgeId, angleDegrees })
  }
  return normalized
}

function modelMatchesTree(
  model: FoldGraphPreviewModel,
  tree: FoldPreviewTreeKinematics,
) {
  if (
    !Array.isArray(model.faces)
    || !Array.isArray(model.hinges)
    || model.faces.length !== tree.joints.length + 1
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
  const modelHingesById = new Map<
    string,
    FoldGraphPreviewModel['hinges'][number]
  >()
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
    const jointFacePairMatchesHinge = (
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
      || !jointFacePairMatchesHinge
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
  first: FoldGraphPreviewModel['hinges'][number],
  second: FoldGraphPreviewModel['hinges'][number],
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

function validInputShape(
  value: unknown,
): value is FoldPreviewTreePhysicalGrabPrepareInput {
  if (!value || typeof value !== 'object') return false
  const input = value as Partial<FoldPreviewTreePhysicalGrabPrepareInput>
  return Boolean(
    input.model
    && typeof input.model === 'object'
    && input.model.kind === 'fold_graph'
    && typeof input.fixedFaceId === 'string'
    && input.fixedFaceId.length > 0
    && typeof input.selectedHingeEdgeId === 'string'
    && input.selectedHingeEdgeId.length > 0
    && input.appliedAngles
    && typeof input.appliedAngles === 'object'
    && typeof input.contextKey === 'string'
    && input.contextKey.length > 0
    && Number.isFinite(input.visualThickness)
    && (input.visualThickness as number) > 0
    && Number.isFinite(input.minimumOrbitRadius)
    && (input.minimumOrbitRadius as number) > 0
    && validSurfaceHit(input.surfaceHit)
    && validRay(input.startRay),
  )
}

function validSurfaceHit(value: unknown): value is FoldPreviewFaceSurfaceHit {
  if (!value || typeof value !== 'object') return false
  const hit = value as Partial<FoldPreviewFaceSurfaceHit>
  return typeof hit.faceId === 'string'
    && hit.faceId.length > 0
    && finitePoint(hit.worldPoint)
    && finitePoint(hit.localPoint)
    && Number.isFinite(hit.distance)
    && (hit.distance as number) >= 0
    && Number.isSafeInteger(hit.materialIndex)
    && (hit.materialIndex as number) >= 0
}

function validRay(value: unknown): value is FoldPreviewPhysicalGrabRay {
  if (!value || typeof value !== 'object') return false
  const ray = value as Partial<FoldPreviewPhysicalGrabRay>
  return finitePoint(ray.origin)
    && finitePoint(ray.direction)
    && unitVector(ray.direction)
    && Number.isFinite(ray.minimumDistance)
    && (ray.minimumDistance as number) >= 0
    && (
      ray.maximumDistance === Number.POSITIVE_INFINITY
      || Number.isFinite(ray.maximumDistance)
    )
    && (ray.maximumDistance as number) > (ray.minimumDistance as number)
}

function validAngle(value: unknown): value is number {
  return Number.isFinite(value)
    && (value as number) >= 0
    && (value as number) <= 180
}

function finitePoint(value: unknown): value is FoldPreviewPhysicalGrabPoint {
  if (!value || typeof value !== 'object') return false
  const point = value as Partial<FoldPreviewPhysicalGrabPoint>
  return Number.isFinite(point.x)
    && Number.isFinite(point.y)
    && Number.isFinite(point.z)
}

function finiteMatrix(
  value: Matrix4 | null | undefined,
): value is Matrix4 {
  return Boolean(value && value.elements.every(Number.isFinite))
}

function unitVector(value: FoldPreviewPhysicalGrabPoint) {
  const vectorLength = Math.hypot(value.x, value.y, value.z)
  return Number.isFinite(vectorLength)
    && Math.abs(vectorLength - 1) <= UNIT_TOLERANCE
}

function transformPoint(
  point: FoldPreviewPhysicalGrabPoint,
  transform: Matrix4,
): FoldPreviewPhysicalGrabPoint | null {
  const transformed = new Vector3(point.x, point.y, point.z).applyMatrix4(transform)
  const result = { x: transformed.x, y: transformed.y, z: transformed.z }
  return finitePoint(result) ? result : null
}

function strictTolerance(
  relativeScale: number,
  coordinateScale: number,
  relativeTolerance: number,
) {
  const relative = relativeScale * relativeTolerance
  const numericFloor = Number.EPSILON
    * coordinateScale
    * NUMERIC_TOLERANCE_FACTOR
  if (
    !Number.isFinite(relative)
    || relative <= 0
    || !Number.isFinite(numericFloor)
    || numericFloor > relative
  ) return null
  return Math.max(relative, numericFloor)
}

function maximumMagnitude(values: readonly number[]) {
  let maximum = 0
  for (const value of values) {
    const magnitude = Math.abs(value)
    if (!Number.isFinite(magnitude)) return Number.POSITIVE_INFINITY
    maximum = Math.max(maximum, magnitude)
  }
  return maximum
}

function pointDistance(
  first: FoldPreviewPhysicalGrabPoint,
  second: FoldPreviewPhysicalGrabPoint,
) {
  return Math.hypot(
    first.x - second.x,
    first.y - second.y,
    first.z - second.z,
  )
}

function copyPoint(value: FoldPreviewPhysicalGrabPoint) {
  return { x: value.x, y: value.y, z: value.z }
}

function freezePoint(
  value: FoldPreviewPhysicalGrabPoint,
): FoldPreviewPhysicalGrabPoint {
  return Object.freeze(copyPoint(value))
}

function freezeAngles(
  values: readonly FoldPreviewHingeAngle[],
): readonly FoldPreviewHingeAngle[] {
  return Object.freeze(values.map((value) => Object.freeze({
    edgeId: value.edgeId,
    angleDegrees: value.angleDegrees,
  })))
}

function rejected(
  reason: Exclude<
    FoldPreviewTreePhysicalGrabPrepareReason,
    'physical_grab_rejected'
  >,
): FoldPreviewTreePhysicalGrabPrepareResult {
  return Object.freeze({ kind: 'rejected', reason })
}
