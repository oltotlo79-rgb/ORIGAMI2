import { Vector3 } from 'three'
import { resolveSingleFoldAnchor } from './foldPreviewAnchoring.ts'
import {
  FOLD_PREVIEW_BACK_MATERIAL_INDEX,
  FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
} from './foldPreviewGeometry.ts'
import {
  FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
  prepareFoldPreviewPhysicalGrab,
  type FoldPreviewPhysicalGrabPoint,
  type FoldPreviewPhysicalGrabPrepareReason,
  type FoldPreviewPhysicalGrabRay,
  type FoldPreviewPhysicalGrabSession,
} from './foldPreviewPhysicalGrab.ts'
import type { SingleFoldPreviewModel } from './foldPreviewModel.ts'
import type { FoldPreviewFaceSurfaceHit } from './foldPreviewPicking.ts'
import { calculateSingleFoldPose } from './foldPreviewSingleFoldKinematics.ts'

export type FoldPreviewSingleFoldPhysicalGrabPrepareInput = Readonly<{
  model: SingleFoldPreviewModel
  fixedFaceId: string
  appliedAngleDegrees: number
  contextKey: string
  surfaceHit: FoldPreviewFaceSurfaceHit
  visualThickness: number
  startRay: FoldPreviewPhysicalGrabRay
  minimumOrbitRadius: number
}>

export type FoldPreviewSingleFoldPhysicalGrabPrepareReason =
  | 'invalid_input'
  | 'anchor_unavailable'
  | 'surface_face_mismatch'
  | 'surface_material_unsupported'
  | 'surface_cap_mismatch'
  | 'pose_unavailable'
  | 'pose_mismatch'
  | 'numeric'
  | 'physical_grab_rejected'

export type FoldPreviewSingleFoldPhysicalGrabReady = Readonly<{
  kind: 'ready'
  mapping: typeof FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
  contextKey: string
  fixedFaceId: string
  movingFaceId: string
  hingeEdgeId: string
  appliedAngleDegrees: number
  surface: 'front' | 'back'
  materialIndex: number
  grabLocalPoint: FoldPreviewPhysicalGrabPoint
  grabRestWorldPoint: FoldPreviewPhysicalGrabPoint
  grabWorldPoint: FoldPreviewPhysicalGrabPoint
  session: FoldPreviewPhysicalGrabSession
}>

export type FoldPreviewSingleFoldPhysicalGrabPrepareResult =
  | FoldPreviewSingleFoldPhysicalGrabReady
  | Readonly<{
      kind: 'rejected'
      reason: Exclude<
        FoldPreviewSingleFoldPhysicalGrabPrepareReason,
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
 * Converts one rendered moving-face cap hit into a canonical physical-grab
 * session without treating mutable scene transforms as pose authority.
 */
export function prepareFoldPreviewSingleFoldPhysicalGrab(
  input: FoldPreviewSingleFoldPhysicalGrabPrepareInput,
): FoldPreviewSingleFoldPhysicalGrabPrepareResult {
  if (!validInputShape(input)) return rejected('invalid_input')

  let anchor: ReturnType<typeof resolveSingleFoldAnchor>
  try {
    anchor = resolveSingleFoldAnchor(input.model, input.fixedFaceId)
  } catch {
    return rejected('invalid_input')
  }
  if (!anchor) return rejected('anchor_unavailable')
  if (input.surfaceHit.faceId !== anchor.movingFace.id) {
    return rejected('surface_face_mismatch')
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

  const grabLocalPoint = copyPoint(input.surfaceHit.localPoint)
  const grabRestWorldPoint = {
    x: input.model.hinge.start.x + grabLocalPoint.x,
    y: grabLocalPoint.y,
    z: input.model.hinge.start.z + grabLocalPoint.z,
  }
  if (!finitePoint(grabRestWorldPoint)) return rejected('numeric')

  let pose: ReturnType<typeof calculateSingleFoldPose>
  try {
    pose = calculateSingleFoldPose(
      input.model,
      input.fixedFaceId,
      input.appliedAngleDegrees,
    )
  } catch {
    return rejected('pose_unavailable')
  }
  if (
    !pose
    || pose.fixedFaceId !== anchor.fixedFace.id
    || pose.movingFaceId !== anchor.movingFace.id
    || !finitePoint(pose.axisStart)
    || !finitePoint(pose.axisEnd)
  ) return rejected('pose_unavailable')
  const movingTransform = pose.faceTransforms.get(anchor.movingFace.id)
  if (
    !movingTransform
    || !movingTransform.elements.every(Number.isFinite)
  ) return rejected('pose_unavailable')

  const expectedWorldVector = new Vector3(
    grabRestWorldPoint.x,
    grabRestWorldPoint.y,
    grabRestWorldPoint.z,
  ).applyMatrix4(movingTransform)
  const expectedWorldPoint = {
    x: expectedWorldVector.x,
    y: expectedWorldVector.y,
    z: expectedWorldVector.z,
  }
  if (!finitePoint(expectedWorldPoint)) return rejected('numeric')

  const hingeLength = pointDistance(pose.axisStart, pose.axisEnd)
  const grabScale = pointDistance(grabRestWorldPoint, pose.axisStart)
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
      pose.axisStart.x,
      pose.axisStart.y,
      pose.axisStart.z,
      pose.axisEnd.x,
      pose.axisEnd.y,
      pose.axisEnd.z,
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
    axisStart: pose.axisStart,
    axisEnd: pose.axisEnd,
    movingRotationSign: anchor.movingRotationSign,
    appliedAngleDegrees: input.appliedAngleDegrees,
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
    fixedFaceId: anchor.fixedFace.id,
    movingFaceId: anchor.movingFace.id,
    hingeEdgeId: input.model.hinge.edgeId,
    appliedAngleDegrees: input.appliedAngleDegrees,
    surface,
    materialIndex: input.surfaceHit.materialIndex,
    grabLocalPoint: freezePoint(grabLocalPoint),
    grabRestWorldPoint: freezePoint(grabRestWorldPoint),
    grabWorldPoint: freezePoint(input.surfaceHit.worldPoint),
    session: physicalGrab.session,
  })
}

function validInputShape(
  value: unknown,
): value is FoldPreviewSingleFoldPhysicalGrabPrepareInput {
  if (!value || typeof value !== 'object') return false
  const input = value as Partial<FoldPreviewSingleFoldPhysicalGrabPrepareInput>
  const hit = input.surfaceHit
  return Boolean(
    input.model
    && typeof input.model === 'object'
    && input.model.kind === 'single_fold'
    && typeof input.fixedFaceId === 'string'
    && input.fixedFaceId.length > 0
    && typeof input.contextKey === 'string'
    && input.contextKey.length > 0
    && validAngle(input.appliedAngleDegrees)
    && Number.isFinite(input.visualThickness)
    && (input.visualThickness as number) > 0
    && Number.isFinite(input.minimumOrbitRadius)
    && (input.minimumOrbitRadius as number) > 0
    && validSurfaceHit(hit)
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

function unitVector(value: FoldPreviewPhysicalGrabPoint) {
  const vectorLength = Math.hypot(value.x, value.y, value.z)
  return Number.isFinite(vectorLength)
    && Math.abs(vectorLength - 1) <= UNIT_TOLERANCE
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

function rejected(
  reason: Exclude<
    FoldPreviewSingleFoldPhysicalGrabPrepareReason,
    'physical_grab_rejected'
  >,
): FoldPreviewSingleFoldPhysicalGrabPrepareResult {
  return Object.freeze({ kind: 'rejected', reason })
}
