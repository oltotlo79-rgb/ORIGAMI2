import assert from 'node:assert/strict'
import test from 'node:test'

import {
  Group,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  Raycaster,
  Vector2,
  Vector3,
} from 'three'
import {
  FOLD_PREVIEW_BACK_MATERIAL_INDEX,
  FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
  FOLD_PREVIEW_SIDE_MATERIAL_INDEX,
  createFoldPreviewFaceGeometry,
} from '../src/lib/foldPreviewGeometry.ts'
import type {
  FoldPreviewPhysicalGrabPoint,
  FoldPreviewPhysicalGrabRay,
} from '../src/lib/foldPreviewPhysicalGrab.ts'
import type {
  FoldPreviewFaceModel,
  SingleFoldPreviewModel,
} from '../src/lib/foldPreviewModel.ts'
import {
  pickFoldPreviewFaceSurface,
  type FoldPreviewFaceSurfaceHit,
} from '../src/lib/foldPreviewPicking.ts'
import { calculateSingleFoldPose } from '../src/lib/foldPreviewSingleFoldKinematics.ts'
import {
  prepareFoldPreviewSingleFoldPhysicalGrab,
  type FoldPreviewSingleFoldPhysicalGrabPrepareInput,
} from '../src/lib/foldPreviewSingleFoldPhysicalGrab.ts'

const VISUAL_THICKNESS = 0.1
const MINIMUM_ORBIT_RADIUS = 0.01

test('both fixed faces and assignments accept front and back caps at 0, 60, and 180 degrees', () => {
  for (const rotationSign of [1, -1] as const) {
    const model = singleFoldModel(rotationSign)
    for (const fixedFaceId of ['left', 'right']) {
      for (const materialIndex of [
        FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
        FOLD_PREVIEW_BACK_MATERIAL_INDEX,
      ]) {
        for (const appliedAngleDegrees of [0, 60, 180]) {
          const input = validInput(
            model,
            fixedFaceId,
            appliedAngleDegrees,
            materialIndex,
          )
          const result = prepareFoldPreviewSingleFoldPhysicalGrab(input)
          assert.equal(result.kind, 'ready')
          if (result.kind !== 'ready') {
            assert.fail(
              `${fixedFaceId}/${rotationSign}/${materialIndex}/${appliedAngleDegrees}: ${result.reason}`,
            )
          }
          const expectedMovingFaceId = fixedFaceId === 'left' ? 'right' : 'left'
          const expectedSign = fixedFaceId === 'left' ? rotationSign : -rotationSign
          assert.equal(result.mapping, 'physical_grab_v2')
          assert.equal(result.contextKey, 'context')
          assert.equal(result.fixedFaceId, fixedFaceId)
          assert.equal(result.movingFaceId, expectedMovingFaceId)
          assert.equal(result.hingeEdgeId, 'hinge')
          assert.equal(result.appliedAngleDegrees, appliedAngleDegrees)
          assert.equal(result.materialIndex, materialIndex)
          assert.equal(
            result.surface,
            materialIndex === FOLD_PREVIEW_FRONT_MATERIAL_INDEX ? 'front' : 'back',
          )
          assert.equal(result.session.movingRotationSign, expectedSign)
          assert.deepEqual(result.grabLocalPoint, input.surfaceHit.localPoint)
          assert.equal(
            result.grabRestWorldPoint.y,
            materialIndex === FOLD_PREVIEW_FRONT_MATERIAL_INDEX
              ? Math.fround(VISUAL_THICKNESS / 2)
              : Math.fround(-VISUAL_THICKNESS / 2),
          )
        }
      }
    }
  }
})

test('a preferred closed-prism hit reaches the moving back cap at an exact 180-degree overlap', () => {
  const model = singleFoldModel(1)
  const materials = [
    new MeshBasicMaterial(),
    new MeshBasicMaterial(),
    new MeshBasicMaterial(),
  ]
  const fixedGeometry = createFoldPreviewFaceGeometry(
    model.faces[0].polygon,
    VISUAL_THICKNESS,
  )
  const movingGeometry = createFoldPreviewFaceGeometry(
    model.faces[1].polygon.map((point) => ({
      x: point.x - model.hinge.start.x,
      z: point.z - model.hinge.start.z,
    })),
    VISUAL_THICKNESS,
  )
  try {
    const fixed = new Mesh(fixedGeometry, materials)
    const moving = new Mesh(movingGeometry, materials)
    const pivot = new Group()
    pivot.position.set(model.hinge.start.x, 0, model.hinge.start.z)
    pivot.quaternion.setFromAxisAngle(
      new Vector3(
        model.hinge.end.x - model.hinge.start.x,
        0,
        model.hinge.end.z - model.hinge.start.z,
      ).normalize(),
      Math.PI,
    )
    pivot.add(moving)
    fixed.updateMatrixWorld(true)
    pivot.updateMatrixWorld(true)

    const camera = new PerspectiveCamera(36, 1, 0.1, 100)
    camera.position.set(5.4, 4.7, 6.4)
    camera.lookAt(0, 0, 0)
    camera.updateProjectionMatrix()
    camera.updateMatrixWorld(true)
    const expectedWorld = moving.localToWorld(new Vector3(
      0.75,
      Math.fround(-VISUAL_THICKNESS / 2),
      1,
    ))
    const projected = expectedWorld.clone().project(camera)
    const pointer = new Vector2(projected.x, projected.y)
    const raycaster = new Raycaster()
    const surfaceHit = pickFoldPreviewFaceSurface(
      raycaster,
      camera,
      pointer,
      [
        { id: 'left', object: fixed },
        { id: 'right', object: moving },
      ],
      'right',
    )
    assert.ok(surfaceHit)
    assert.equal(surfaceHit.faceId, 'right')
    assert.equal(
      surfaceHit.materialIndex,
      FOLD_PREVIEW_BACK_MATERIAL_INDEX,
    )

    raycaster.setFromCamera(pointer, camera)
    const result = prepareFoldPreviewSingleFoldPhysicalGrab({
      model,
      fixedFaceId: 'left',
      appliedAngleDegrees: 180,
      contextKey: 'context',
      surfaceHit,
      visualThickness: VISUAL_THICKNESS,
      startRay: {
        origin: {
          x: raycaster.ray.origin.x,
          y: raycaster.ray.origin.y,
          z: raycaster.ray.origin.z,
        },
        direction: {
          x: raycaster.ray.direction.x,
          y: raycaster.ray.direction.y,
          z: raycaster.ray.direction.z,
        },
        minimumDistance: raycaster.near,
        maximumDistance: raycaster.far,
      },
      minimumOrbitRadius: MINIMUM_ORBIT_RADIUS,
    })
    assert.equal(result.kind, 'ready')
  } finally {
    fixedGeometry.dispose()
    movingGeometry.dispose()
    for (const material of materials) material.dispose()
  }
})

test('only the resolved moving face can start a physical grab', () => {
  const model = singleFoldModel(1)
  const input = validInput(model, 'left', 45)
  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...input,
    surfaceHit: {
      ...input.surfaceHit,
      faceId: 'left',
    },
  }), {
    kind: 'rejected',
    reason: 'surface_face_mismatch',
  })

  const rightFixed = validInput(model, 'right', 45)
  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...rightFixed,
    surfaceHit: {
      ...rightFixed.surfaceHit,
      faceId: 'right',
    },
  }), {
    kind: 'rejected',
    reason: 'surface_face_mismatch',
  })
})

test('side walls and unknown material groups cannot start a grab', () => {
  const input = validInput(singleFoldModel(1), 'left', 45)
  for (const materialIndex of [FOLD_PREVIEW_SIDE_MATERIAL_INDEX, 9]) {
    assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
      ...input,
      surfaceHit: {
        ...input.surfaceHit,
        materialIndex,
      },
    }), {
      kind: 'rejected',
      reason: 'surface_material_unsupported',
    })
  }
})

test('front and back material indices require their matching local cap height', () => {
  const input = validInput(singleFoldModel(1), 'left', 45)
  for (const [materialIndex, y] of [
    [FOLD_PREVIEW_FRONT_MATERIAL_INDEX, -VISUAL_THICKNESS / 2],
    [FOLD_PREVIEW_BACK_MATERIAL_INDEX, VISUAL_THICKNESS / 2],
    [FOLD_PREVIEW_FRONT_MATERIAL_INDEX, VISUAL_THICKNESS / 2 + 1e-5],
  ] as const) {
    assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
      ...input,
      surfaceHit: {
        ...input.surfaceHit,
        materialIndex,
        localPoint: {
          ...input.surfaceHit.localPoint,
          y,
        },
      },
    }), {
      kind: 'rejected',
      reason: 'surface_cap_mismatch',
    })
  }
})

test('canonical pose verification rejects a stale or forged world hit', () => {
  const input = validInput(singleFoldModel(1), 'left', 30)
  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...input,
    surfaceHit: {
      ...input.surfaceHit,
      worldPoint: {
        ...input.surfaceHit.worldPoint,
        y: input.surfaceHit.worldPoint.y + 0.1,
      },
    },
  }), {
    kind: 'rejected',
    reason: 'pose_mismatch',
  })
})

test('unknown anchors, empty context, and malformed runtime records fail closed', () => {
  const input = validInput(singleFoldModel(1), 'left', 30)
  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...input,
    fixedFaceId: 'unknown',
  }), {
    kind: 'rejected',
    reason: 'anchor_unavailable',
  })
  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...input,
    contextKey: '',
  }), {
    kind: 'rejected',
    reason: 'invalid_input',
  })

  for (const malformed of [
    null,
    { ...input, appliedAngleDegrees: Number.NaN },
    { ...input, visualThickness: 0 },
    { ...input, minimumOrbitRadius: Number.POSITIVE_INFINITY },
    {
      ...input,
      surfaceHit: {
        ...input.surfaceHit,
        localPoint: { x: Number.NaN, y: 0, z: 0 },
      },
    },
    {
      ...input,
      startRay: {
        ...input.startRay,
        direction: { x: 0, y: 0, z: -2 },
      },
    },
  ]) {
    assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab(malformed as never), {
      kind: 'rejected',
      reason: 'invalid_input',
    })
  }
})

test('unrepresentable cap and coordinate scales are rejected as numeric', () => {
  const input = validInput(singleFoldModel(1), 'left', 30)
  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...input,
    visualThickness: 1e-50,
    surfaceHit: {
      ...input.surfaceHit,
      localPoint: {
        ...input.surfaceHit.localPoint,
        y: 0,
      },
    },
  }), {
    kind: 'rejected',
    reason: 'numeric',
  })

  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...input,
    surfaceHit: {
      ...input.surfaceHit,
      localPoint: {
        ...input.surfaceHit.localPoint,
        x: 1e15,
      },
    },
  }), {
    kind: 'rejected',
    reason: 'numeric',
  })
})

test('solver-level start rejection remains explicit and stable', () => {
  const input = validInput(singleFoldModel(1), 'left', 30)
  assert.deepEqual(prepareFoldPreviewSingleFoldPhysicalGrab({
    ...input,
    minimumOrbitRadius: 10,
  }), {
    kind: 'rejected',
    reason: 'physical_grab_rejected',
    physicalGrabReason: 'grab_on_axis',
  })
})

test('ready snapshots are deeply frozen and detached from mutable hit input', () => {
  const input = validInput(singleFoldModel(1), 'left', 45)
  const result = prepareFoldPreviewSingleFoldPhysicalGrab(input)
  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(`prepare failed: ${result.reason}`)

  assert.ok(Object.isFrozen(result))
  assert.ok(Object.isFrozen(result.grabLocalPoint))
  assert.ok(Object.isFrozen(result.grabRestWorldPoint))
  assert.ok(Object.isFrozen(result.grabWorldPoint))
  assert.ok(Object.isFrozen(result.session))

  ;(input.surfaceHit.localPoint as { x: number }).x = 99
  ;(input.surfaceHit.worldPoint as { y: number }).y = 99
  assert.notEqual(result.grabLocalPoint.x, 99)
  assert.notEqual(result.grabWorldPoint.y, 99)
})

function validInput(
  model: SingleFoldPreviewModel,
  fixedFaceId: 'left' | 'right',
  appliedAngleDegrees: number,
  materialIndex = FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
): FoldPreviewSingleFoldPhysicalGrabPrepareInput {
  const movingFaceId = fixedFaceId === 'left' ? 'right' : 'left'
  const restX = fixedFaceId === 'left' ? 0.75 : -0.75
  const capY = materialIndex === FOLD_PREVIEW_BACK_MATERIAL_INDEX
    ? Math.fround(-VISUAL_THICKNESS / 2)
    : Math.fround(VISUAL_THICKNESS / 2)
  const localPoint = {
    x: restX - model.hinge.start.x,
    y: capY,
    z: -model.hinge.start.z,
  }
  const restPoint = new Vector3(restX, capY, 0)
  const pose = calculateSingleFoldPose(model, fixedFaceId, appliedAngleDegrees)
  assert.ok(pose)
  const movingTransform = pose.faceTransforms.get(movingFaceId)
  assert.ok(movingTransform)
  const world = restPoint.applyMatrix4(movingTransform)
  const surfaceHit: FoldPreviewFaceSurfaceHit = {
    faceId: movingFaceId,
    localPoint,
    worldPoint: { x: world.x, y: world.y, z: world.z },
    distance: 5,
    materialIndex,
  }
  return {
    model,
    fixedFaceId,
    appliedAngleDegrees,
    contextKey: 'context',
    surfaceHit,
    visualThickness: VISUAL_THICKNESS,
    startRay: rayThrough(surfaceHit.worldPoint),
    minimumOrbitRadius: MINIMUM_ORBIT_RADIUS,
  }
}

function rayThrough(point: FoldPreviewPhysicalGrabPoint): FoldPreviewPhysicalGrabRay {
  return {
    origin: { x: point.x, y: point.y, z: point.z + 5 },
    direction: { x: 0, y: 0, z: -1 },
    minimumDistance: 0,
    maximumDistance: Number.POSITIVE_INFINITY,
  }
}

function singleFoldModel(rotationSign: 1 | -1): SingleFoldPreviewModel {
  const left: FoldPreviewFaceModel = {
    id: 'left',
    polygon: [
      { vertexId: 'start', x: 0, z: -1 },
      { vertexId: 'left', x: -1, z: 0 },
      { vertexId: 'end', x: 0, z: 1 },
    ],
  }
  const right: FoldPreviewFaceModel = {
    id: 'right',
    polygon: [
      { vertexId: 'end', x: 0, z: 1 },
      { vertexId: 'right', x: 1, z: 0 },
      { vertexId: 'start', x: 0, z: -1 },
    ],
  }
  return {
    kind: 'single_fold',
    projectId: 'project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0, y: 0 },
    worldBounds: { minX: -1, minZ: -1, maxX: 1, maxZ: 1 },
    faces: [left, right],
    fixedFace: left,
    movingFace: right,
    hinge: {
      edgeId: 'hinge',
      leftFaceId: 'left',
      rightFaceId: 'right',
      start: left.polygon[0],
      end: left.polygon[2],
      axis: { x: 0, z: 1 },
      assignment: rotationSign === 1 ? 'mountain' : 'valley',
      rotationSign,
    },
  }
}
