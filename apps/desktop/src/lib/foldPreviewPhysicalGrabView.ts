import { Vector3, type Camera } from 'three'
import type {
  FoldPreviewPhysicalGrabPoint,
  FoldPreviewPhysicalGrabSession,
} from './foldPreviewPhysicalGrab.ts'

export const FOLD_PREVIEW_PHYSICAL_GRAB_MINIMUM_RADIUS_PIXELS = 8
export const FOLD_PREVIEW_PHYSICAL_GRAB_MINIMUM_ONE_DEGREE_MOTION_PIXELS = 0.2

export type FoldPreviewPhysicalGrabViewport = Readonly<{
  left: number
  top: number
  width: number
  height: number
  clientWidth: number
  clientHeight: number
}>

export type FoldPreviewPhysicalGrabViewThresholds = Readonly<{
  minimumRadiusPixels: number
  minimumOneDegreeMotionPixels: number
}>

const DEFAULT_THRESHOLDS: FoldPreviewPhysicalGrabViewThresholds =
  Object.freeze({
    minimumRadiusPixels:
      FOLD_PREVIEW_PHYSICAL_GRAB_MINIMUM_RADIUS_PIXELS,
    minimumOneDegreeMotionPixels:
      FOLD_PREVIEW_PHYSICAL_GRAB_MINIMUM_ONE_DEGREE_MOTION_PIXELS,
  })

/**
 * Rejects grabs whose circular motion is too small or too edge-on to control
 * reliably in the current CSS-pixel viewport.
 */
export function canBeginFoldPreviewPhysicalGrabInView(
  camera: Camera,
  session: FoldPreviewPhysicalGrabSession,
  viewport: FoldPreviewPhysicalGrabViewport,
  thresholds: FoldPreviewPhysicalGrabViewThresholds = DEFAULT_THRESHOLDS,
) {
  if (
    !validViewport(viewport)
    || !validThresholds(thresholds)
    || !Number.isFinite(session?.appliedAngleDegrees)
  ) return false
  try {
    camera.updateMatrixWorld(true)
    const applied = session.appliedAngleDegrees
    const current = physicalGrabOrbitPoint(session, applied)
    const center = projectPhysicalGrabPoint(
      camera,
      session.orbitCenter,
      viewport,
    )
    const currentProjected = current
      ? projectPhysicalGrabPoint(camera, current, viewport)
      : null
    if (!center || !currentProjected) return false
    const radiusPixels = Math.hypot(
      currentProjected.x - center.x,
      currentProjected.y - center.y,
    )
    if (
      !Number.isFinite(radiusPixels)
      || radiusPixels < thresholds.minimumRadiusPixels
    ) return false

    const adjacentAngles = [
      Math.max(0, applied - 1),
      Math.min(180, applied + 1),
    ].filter((angle) => angle !== applied)
    if (adjacentAngles.length === 0) return false
    let maximumMotionPixels = 0
    for (const angle of adjacentAngles) {
      const point = physicalGrabOrbitPoint(session, angle)
      const projected = point
        ? projectPhysicalGrabPoint(camera, point, viewport)
        : null
      if (!projected) return false
      maximumMotionPixels = Math.max(
        maximumMotionPixels,
        Math.hypot(
          projected.x - currentProjected.x,
          projected.y - currentProjected.y,
        ),
      )
    }
    return Number.isFinite(maximumMotionPixels)
      && maximumMotionPixels
        >= thresholds.minimumOneDegreeMotionPixels
  } catch {
    return false
  }
}

/**
 * Produces an opaque exact guard for every view input used to turn CSS
 * pointer coordinates into a world ray.
 */
export function snapshotFoldPreviewPhysicalGrabView(
  camera: Camera,
  controlsTarget: FoldPreviewPhysicalGrabPoint,
  viewport: FoldPreviewPhysicalGrabViewport,
  requestedAngleDegrees: number,
) {
  if (
    !finitePoint(controlsTarget)
    || !validViewport(viewport)
    || !validAngle(requestedAngleDegrees)
  ) return null
  try {
    camera.updateMatrixWorld(true)
    const values = [
      ...camera.matrixWorld.elements,
      ...camera.projectionMatrix.elements,
      controlsTarget.x,
      controlsTarget.y,
      controlsTarget.z,
      viewport.clientWidth,
      viewport.clientHeight,
      viewport.left,
      viewport.top,
      viewport.width,
      viewport.height,
      requestedAngleDegrees,
    ]
    return values.every(Number.isFinite)
      ? values
          .map((value) => Object.is(value, -0) ? 0 : value)
          .join(',')
      : null
  } catch {
    return null
  }
}

function physicalGrabOrbitPoint(
  session: FoldPreviewPhysicalGrabSession,
  angleDegrees: number,
) {
  if (!validAngle(angleDegrees)) return null
  const angle = angleDegrees * Math.PI / 180
  const cosine = Math.cos(angle)
  const sine = Math.sin(angle)
  const point = {
    x: session.orbitCenter.x + session.orbitRadius * (
      session.restRadialUnit.x * cosine
      + session.positiveTangentUnit.x * sine
    ),
    y: session.orbitCenter.y + session.orbitRadius * (
      session.restRadialUnit.y * cosine
      + session.positiveTangentUnit.y * sine
    ),
    z: session.orbitCenter.z + session.orbitRadius * (
      session.restRadialUnit.z * cosine
      + session.positiveTangentUnit.z * sine
    ),
  }
  return finitePoint(point) ? point : null
}

function projectPhysicalGrabPoint(
  camera: Camera,
  point: FoldPreviewPhysicalGrabPoint,
  viewport: FoldPreviewPhysicalGrabViewport,
) {
  if (!finitePoint(point)) return null
  const projected = new Vector3(point.x, point.y, point.z).project(camera)
  if (
    ![projected.x, projected.y, projected.z].every(Number.isFinite)
    || projected.z < -1
    || projected.z > 1
  ) return null
  const x = viewport.left
    + (projected.x + 1) * viewport.width / 2
  const y = viewport.top
    + (1 - projected.y) * viewport.height / 2
  return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null
}

function validViewport(
  viewport: FoldPreviewPhysicalGrabViewport,
) {
  return Boolean(
    viewport
    && Number.isFinite(viewport.left)
    && Number.isFinite(viewport.top)
    && Number.isFinite(viewport.width)
    && viewport.width > 0
    && Number.isFinite(viewport.height)
    && viewport.height > 0
    && Number.isSafeInteger(viewport.clientWidth)
    && viewport.clientWidth > 0
    && Number.isSafeInteger(viewport.clientHeight)
    && viewport.clientHeight > 0,
  )
}

function validThresholds(
  thresholds: FoldPreviewPhysicalGrabViewThresholds,
) {
  return Boolean(
    thresholds
    && Number.isFinite(thresholds.minimumRadiusPixels)
    && thresholds.minimumRadiusPixels >= 0
    && Number.isFinite(thresholds.minimumOneDegreeMotionPixels)
    && thresholds.minimumOneDegreeMotionPixels >= 0,
  )
}

function finitePoint(value: unknown): value is FoldPreviewPhysicalGrabPoint {
  if (!value || typeof value !== 'object') return false
  const point = value as Partial<FoldPreviewPhysicalGrabPoint>
  return Number.isFinite(point.x)
    && Number.isFinite(point.y)
    && Number.isFinite(point.z)
}

function validAngle(value: unknown): value is number {
  return Number.isFinite(value)
    && (value as number) >= 0
    && (value as number) <= 180
}
