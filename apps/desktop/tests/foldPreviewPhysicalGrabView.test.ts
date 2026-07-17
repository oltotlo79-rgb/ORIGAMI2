import assert from 'node:assert/strict'
import test from 'node:test'

import { PerspectiveCamera, Vector3 } from 'three'
import {
  prepareFoldPreviewPhysicalGrab,
  type FoldPreviewPhysicalGrabPoint,
  type FoldPreviewPhysicalGrabSession,
} from '../src/lib/foldPreviewPhysicalGrab.ts'
import {
  canBeginFoldPreviewPhysicalGrabInView,
  snapshotFoldPreviewPhysicalGrabView,
  type FoldPreviewPhysicalGrabViewport,
} from '../src/lib/foldPreviewPhysicalGrabView.ts'

const VIEWPORT: FoldPreviewPhysicalGrabViewport = Object.freeze({
  left: 20,
  top: 30,
  width: 1_000,
  height: 800,
  clientWidth: 1_000,
  clientHeight: 800,
})

test('front-facing 0- and 180-degree endpoint grabs remain controllable', () => {
  const camera = frontCamera()
  assert.equal(
    canBeginFoldPreviewPhysicalGrabInView(
      camera,
      sessionAt(0),
      VIEWPORT,
    ),
    true,
  )
  assert.equal(
    canBeginFoldPreviewPhysicalGrabInView(
      camera,
      sessionAt(180),
      VIEWPORT,
    ),
    true,
  )
})

test('radius and one-degree motion thresholds are inclusive CSS-pixel boundaries', () => {
  const camera = frontCamera()
  const session = sessionAt(0)
  const center = project(camera, session.orbitCenter)
  const current = project(camera, orbitPoint(session, 0))
  const adjacent = project(camera, orbitPoint(session, 1))
  const radius = Math.hypot(
    current.x - center.x,
    current.y - center.y,
  )
  const oneDegreeMotion = Math.hypot(
    adjacent.x - current.x,
    adjacent.y - current.y,
  )

  assert.equal(canBeginFoldPreviewPhysicalGrabInView(
    camera,
    session,
    VIEWPORT,
    {
      minimumRadiusPixels: radius,
      minimumOneDegreeMotionPixels: oneDegreeMotion,
    },
  ), true)
  assert.equal(canBeginFoldPreviewPhysicalGrabInView(
    camera,
    session,
    VIEWPORT,
    {
      minimumRadiusPixels: radius + 1e-9,
      minimumOneDegreeMotionPixels: 0,
    },
  ), false)
  assert.equal(canBeginFoldPreviewPhysicalGrabInView(
    camera,
    session,
    VIEWPORT,
    {
      minimumRadiusPixels: 0,
      minimumOneDegreeMotionPixels: oneDegreeMotion + 1e-9,
    },
  ), false)
})

test('side-on, clip-external, and malformed views fail closed', () => {
  const sideOn = new PerspectiveCamera(50, 1, 0.1, 100)
  sideOn.position.set(5, 0, 0)
  sideOn.lookAt(0, 0, 0)
  sideOn.updateProjectionMatrix()
  sideOn.updateMatrixWorld(true)
  assert.equal(
    canBeginFoldPreviewPhysicalGrabInView(
      sideOn,
      sessionAt(0),
      VIEWPORT,
    ),
    false,
  )

  const lookingAway = new PerspectiveCamera(50, 1, 0.1, 100)
  lookingAway.position.set(0, 0, 5)
  lookingAway.lookAt(0, 0, 10)
  lookingAway.updateProjectionMatrix()
  lookingAway.updateMatrixWorld(true)
  assert.equal(
    canBeginFoldPreviewPhysicalGrabInView(
      lookingAway,
      sessionAt(0),
      VIEWPORT,
    ),
    false,
  )
  assert.equal(
    canBeginFoldPreviewPhysicalGrabInView(
      frontCamera(),
      sessionAt(0),
      { ...VIEWPORT, width: 0 },
    ),
    false,
  )
  assert.equal(
    canBeginFoldPreviewPhysicalGrabInView(
      frontCamera(),
      sessionAt(0),
      VIEWPORT,
      {
        minimumRadiusPixels: Number.NaN,
        minimumOneDegreeMotionPixels: 0.2,
      },
    ),
    false,
  )
})

test('view guards change with camera, target, viewport, and requested angle', () => {
  const camera = frontCamera()
  const target = { x: 0, y: 0, z: 0 }
  const baseline = snapshotFoldPreviewPhysicalGrabView(
    camera,
    target,
    VIEWPORT,
    30,
  )
  assert.ok(baseline)
  assert.equal(snapshotFoldPreviewPhysicalGrabView(
    camera,
    target,
    VIEWPORT,
    30,
  ), baseline)
  assert.notEqual(snapshotFoldPreviewPhysicalGrabView(
    camera,
    target,
    { ...VIEWPORT, left: VIEWPORT.left + 1 },
    30,
  ), baseline)
  assert.notEqual(snapshotFoldPreviewPhysicalGrabView(
    camera,
    { ...target, x: 1 },
    VIEWPORT,
    30,
  ), baseline)
  assert.notEqual(snapshotFoldPreviewPhysicalGrabView(
    camera,
    target,
    VIEWPORT,
    31,
  ), baseline)

  camera.position.x = 1
  camera.lookAt(0, 0, 0)
  assert.notEqual(snapshotFoldPreviewPhysicalGrabView(
    camera,
    target,
    VIEWPORT,
    30,
  ), baseline)
  assert.equal(snapshotFoldPreviewPhysicalGrabView(
    camera,
    target,
    { ...VIEWPORT, clientHeight: 0 },
    30,
  ), null)
  assert.equal(snapshotFoldPreviewPhysicalGrabView(
    camera,
    target,
    VIEWPORT,
    Number.NaN,
  ), null)
})

function frontCamera() {
  const camera = new PerspectiveCamera(50, 1, 0.1, 100)
  camera.position.set(0, 0, 5)
  camera.lookAt(0, 0, 0)
  camera.updateProjectionMatrix()
  camera.updateMatrixWorld(true)
  return camera
}

function sessionAt(angleDegrees: number) {
  const grabRestWorldPoint = { x: 1, y: 0, z: 0 }
  const grabWorldPoint = {
    x: Math.cos(angleDegrees * Math.PI / 180),
    y: Math.sin(angleDegrees * Math.PI / 180),
    z: 0,
  }
  const result = prepareFoldPreviewPhysicalGrab({
    contextKey: 'view-context',
    axisStart: { x: 0, y: 0, z: 0 },
    axisEnd: { x: 0, y: 0, z: 1 },
    movingRotationSign: 1,
    appliedAngleDegrees: angleDegrees,
    grabRestWorldPoint,
    grabWorldPoint,
    startRay: {
      origin: { x: grabWorldPoint.x, y: grabWorldPoint.y, z: 5 },
      direction: { x: 0, y: 0, z: -1 },
      minimumDistance: 0,
      maximumDistance: Number.POSITIVE_INFINITY,
    },
    minimumOrbitRadius: 0.01,
  })
  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(result.reason)
  return result.session
}

function orbitPoint(
  session: FoldPreviewPhysicalGrabSession,
  angleDegrees: number,
): FoldPreviewPhysicalGrabPoint {
  const angle = angleDegrees * Math.PI / 180
  const cosine = Math.cos(angle)
  const sine = Math.sin(angle)
  return {
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
}

function project(
  camera: PerspectiveCamera,
  point: FoldPreviewPhysicalGrabPoint,
) {
  const projected = new Vector3(point.x, point.y, point.z).project(camera)
  return {
    x: VIEWPORT.left + (projected.x + 1) * VIEWPORT.width / 2,
    y: VIEWPORT.top + (1 - projected.y) * VIEWPORT.height / 2,
  }
}
