import assert from 'node:assert/strict'
import test from 'node:test'

import { Vector3 } from 'three'
import {
  FOLD_PREVIEW_BACK_MATERIAL_INDEX,
  FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
  FOLD_PREVIEW_SIDE_MATERIAL_INDEX,
} from '../src/lib/foldPreviewGeometry.ts'
import {
  calculateFoldTreePoseWithAngles,
  type FoldPreviewHingeAngle,
  type FoldPreviewTreeAngleInput,
} from '../src/lib/foldPreviewKinematics.ts'
import {
  resolveFoldPreviewPhysicalGrabTarget,
  type FoldPreviewPhysicalGrabPoint,
  type FoldPreviewPhysicalGrabRay,
} from '../src/lib/foldPreviewPhysicalGrab.ts'
import type {
  FoldGraphPreviewModel,
  FoldPreviewFaceModel,
  FoldPreviewHingeModel,
} from '../src/lib/foldPreviewModel.ts'
import type { FoldPreviewFaceSurfaceHit } from '../src/lib/foldPreviewPicking.ts'
import {
  prepareFoldPreviewTreePhysicalGrab,
  type FoldPreviewTreePhysicalGrabPrepareInput,
} from '../src/lib/foldPreviewTreePhysicalGrab.ts'
import { rerootFoldPreviewTree } from '../src/lib/foldPreviewAnchoring.ts'

const VISUAL_THICKNESS = 0.1
const MINIMUM_ORBIT_RADIUS = 0.01

test('a descendant cap becomes a selected-hinge session while downstream angles stay fixed', () => {
  const input = validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    surfaceFaceId: 'leaf',
    appliedAngles: {
      kind: 'per_hinge',
      angles: [
        { edgeId: 'hinge-x', angleDegrees: 35 },
        { edgeId: 'hinge-z', angleDegrees: 55 },
      ],
    },
  })
  const result = prepareFoldPreviewTreePhysicalGrab(input)

  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(`prepare failed: ${result.reason}`)
  assert.equal(result.mapping, 'physical_grab_v2')
  assert.equal(result.fixedFaceId, 'root')
  assert.equal(result.hingeEdgeId, 'hinge-z')
  assert.equal(result.parentFaceId, 'root')
  assert.equal(result.childFaceId, 'middle')
  assert.deepEqual(result.dependentFaceIds, ['middle', 'leaf'])
  assert.equal(result.surfaceFaceId, 'leaf')
  assert.equal(result.appliedAngleDegrees, 55)
  assert.deepEqual(result.appliedAngles, [
    { edgeId: 'hinge-z', angleDegrees: 55 },
    { edgeId: 'hinge-x', angleDegrees: 35 },
  ])
  assert.equal(result.session.movingRotationSign, 1)

  const start = resolveFoldPreviewPhysicalGrabTarget(result.session, {
    contextKey: 'tree-context',
    referenceAngleDegrees: 55,
    ray: input.startRay,
  })
  assert.equal(start.kind, 'unverified_target')
  if (start.kind === 'unverified_target') {
    assert.ok(Math.abs(start.rawAngleDegrees - 55) < 1e-6)
  }

  const flatLocal = result.grabLocalPoint
  assert.ok(
    pointDistance(result.grabRestWorldPoint, flatLocal) > 0.01,
    'the downstream non-zero hinge must remain in the selected-hinge zero baseline',
  )
})

test('front and back caps work at 0, 60, and 180 degrees on a non-commuting child hinge', () => {
  for (const materialIndex of [
    FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
    FOLD_PREVIEW_BACK_MATERIAL_INDEX,
  ]) {
    for (const angleDegrees of [0, 60, 180]) {
      const input = validInput({
        fixedFaceId: 'root',
        selectedHingeEdgeId: 'hinge-x',
        surfaceFaceId: 'leaf',
        appliedAngles: {
          kind: 'per_hinge',
          angles: [
            { edgeId: 'hinge-z', angleDegrees: 50 },
            { edgeId: 'hinge-x', angleDegrees },
          ],
        },
        materialIndex,
      })
      const result = prepareFoldPreviewTreePhysicalGrab(input)
      assert.equal(
        result.kind,
        'ready',
        `${materialIndex}/${angleDegrees}: ${
          result.kind === 'rejected' ? result.reason : 'ready'
        }`,
      )
      if (result.kind !== 'ready') continue
      assert.equal(result.appliedAngleDegrees, angleDegrees)
      assert.equal(result.session.movingRotationSign, -1)
      assert.equal(
        result.surface,
        materialIndex === FOLD_PREVIEW_FRONT_MATERIAL_INDEX ? 'front' : 'back',
      )
    }
  }
})

test('uniform angles normalize to a complete deterministic vector', () => {
  const result = prepareFoldPreviewTreePhysicalGrab(validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-x',
    surfaceFaceId: 'leaf',
    appliedAngles: { kind: 'uniform', angleDegrees: 42 },
  }))

  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(`prepare failed: ${result.reason}`)
  assert.deepEqual(result.appliedAngles, [
    { edgeId: 'hinge-z', angleDegrees: 42 },
    { edgeId: 'hinge-x', angleDegrees: 42 },
  ])
  assert.equal(result.appliedAngleDegrees, 42)
})

test('rerooting flips the selected joint and only accepts its new dependent side', () => {
  const input = validInput({
    fixedFaceId: 'leaf',
    selectedHingeEdgeId: 'hinge-z',
    surfaceFaceId: 'root',
    appliedAngles: {
      kind: 'per_hinge',
      angles: [
        { edgeId: 'hinge-z', angleDegrees: 70 },
        { edgeId: 'hinge-x', angleDegrees: 25 },
      ],
    },
  })
  const result = prepareFoldPreviewTreePhysicalGrab(input)

  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(`prepare failed: ${result.reason}`)
  assert.equal(result.fixedFaceId, 'leaf')
  assert.equal(result.parentFaceId, 'middle')
  assert.equal(result.childFaceId, 'root')
  assert.deepEqual(result.dependentFaceIds, ['root'])
  assert.equal(result.session.movingRotationSign, -1)

  const fixedSideHit = validInput({
    fixedFaceId: 'leaf',
    selectedHingeEdgeId: 'hinge-z',
    surfaceFaceId: 'middle',
    appliedAngles: input.appliedAngles,
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab(fixedSideHit), {
    kind: 'rejected',
    reason: 'surface_face_not_dependent',
  })
})

test('fixed-side parent, side-wall, and off-cap hits fail closed', () => {
  const parentHit = validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-x',
    surfaceFaceId: 'middle',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab(parentHit), {
    kind: 'rejected',
    reason: 'surface_face_not_dependent',
  })

  const sideHit = validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-x',
    surfaceFaceId: 'leaf',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...sideHit,
    surfaceHit: {
      ...sideHit.surfaceHit,
      materialIndex: FOLD_PREVIEW_SIDE_MATERIAL_INDEX,
    },
  }), {
    kind: 'rejected',
    reason: 'surface_material_unsupported',
  })

  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...sideHit,
    surfaceHit: {
      ...sideHit.surfaceHit,
      localPoint: {
        ...sideHit.surfaceHit.localPoint,
        y: 0,
      },
    },
  }), {
    kind: 'rejected',
    reason: 'surface_cap_mismatch',
  })
})

test('scene-pose disagreement is rejected before the physical solver', () => {
  const input = validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    surfaceFaceId: 'leaf',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    surfaceHit: {
      ...input.surfaceHit,
      worldPoint: {
        ...input.surfaceHit.worldPoint,
        x: input.surfaceHit.worldPoint.x + 0.1,
      },
    },
  }), {
    kind: 'rejected',
    reason: 'pose_mismatch',
  })
})

test('unknown topology, incomplete angle vectors, and cyclic models fail closed', () => {
  const input = validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    surfaceFaceId: 'leaf',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    fixedFaceId: 'missing-face',
  }), {
    kind: 'rejected',
    reason: 'tree_unavailable',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    selectedHingeEdgeId: 'missing-hinge',
  }), {
    kind: 'rejected',
    reason: 'selected_hinge_unavailable',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    appliedAngles: {
      kind: 'per_hinge',
      angles: [{ edgeId: 'hinge-z', angleDegrees: 40 }],
    },
  }), {
    kind: 'rejected',
    reason: 'invalid_input',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    appliedAngles: {
      kind: 'per_hinge',
      angles: [
        { edgeId: 'hinge-z', angleDegrees: 40 },
        { edgeId: 'hinge-z', angleDegrees: 40 },
      ],
    },
  }), {
    kind: 'rejected',
    reason: 'invalid_input',
  })
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    appliedAngles: { kind: 'uniform', angleDegrees: 181 },
  }), {
    kind: 'rejected',
    reason: 'invalid_input',
  })

  const cyclicModel: FoldGraphPreviewModel = {
    ...input.model,
    kinematics: { kind: 'static_cycle', reason: 'cyclic_hinge_graph' },
  }
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    model: cyclicModel,
  }), {
    kind: 'rejected',
    reason: 'tree_unavailable',
  })
})

test('render hinges and kinematic joints must share geometry, incidence, and fold direction', () => {
  const input = validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    surfaceFaceId: 'leaf',
  })
  assert.equal(input.model.kinematics.kind, 'tree')
  if (input.model.kinematics.kind !== 'tree') throw new Error('fixture is not a tree')

  const shiftedRenderHinge = {
    ...input.model.hinges[0],
    end: {
      ...input.model.hinges[0].end,
      z: input.model.hinges[0].end.z + 0.25,
    },
  }
  const shiftedRenderModel: FoldGraphPreviewModel = {
    ...input.model,
    hinges: [shiftedRenderHinge, input.model.hinges[1]],
  }
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    model: shiftedRenderModel,
  }), {
    kind: 'rejected',
    reason: 'tree_unavailable',
  })

  const wrongIncidenceHinge = {
    ...input.model.hinges[0],
    rightFaceId: 'leaf',
  }
  const wrongIncidenceModel: FoldGraphPreviewModel = {
    ...input.model,
    hinges: [wrongIncidenceHinge, input.model.hinges[1]],
    kinematics: {
      ...input.model.kinematics,
      joints: [
        {
          ...input.model.kinematics.joints[0],
          hinge: wrongIncidenceHinge,
        },
        input.model.kinematics.joints[1],
      ],
    },
  }
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    model: wrongIncidenceModel,
  }), {
    kind: 'rejected',
    reason: 'tree_unavailable',
  })

  const reversedJointSignModel: FoldGraphPreviewModel = {
    ...input.model,
    kinematics: {
      ...input.model.kinematics,
      joints: [
        {
          ...input.model.kinematics.joints[0],
          childRotationSign: -1,
        },
        input.model.kinematics.joints[1],
      ],
    },
  }
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    model: reversedJointSignModel,
  }), {
    kind: 'rejected',
    reason: 'tree_unavailable',
  })

  const contradictoryAssignmentHinge = {
    ...input.model.hinges[0],
    assignment: 'valley' as const,
  }
  const contradictoryAssignmentModel: FoldGraphPreviewModel = {
    ...input.model,
    hinges: [contradictoryAssignmentHinge, input.model.hinges[1]],
    kinematics: {
      ...input.model.kinematics,
      joints: [
        {
          ...input.model.kinematics.joints[0],
          hinge: contradictoryAssignmentHinge,
        },
        input.model.kinematics.joints[1],
      ],
    },
  }
  assert.deepEqual(prepareFoldPreviewTreePhysicalGrab({
    ...input,
    model: contradictoryAssignmentModel,
  }), {
    kind: 'rejected',
    reason: 'tree_unavailable',
  })
})

test('ready output is deeply frozen and detached from mutable hit and angle input', () => {
  const input = validInput({
    fixedFaceId: 'root',
    selectedHingeEdgeId: 'hinge-z',
    surfaceFaceId: 'leaf',
  })
  const result = prepareFoldPreviewTreePhysicalGrab(input)
  assert.equal(result.kind, 'ready')
  if (result.kind !== 'ready') assert.fail(`prepare failed: ${result.reason}`)

  assert.ok(Object.isFrozen(result))
  assert.ok(Object.isFrozen(result.dependentFaceIds))
  assert.ok(Object.isFrozen(result.appliedAngles))
  assert.ok(result.appliedAngles.every(Object.isFrozen))
  assert.ok(Object.isFrozen(result.grabLocalPoint))
  assert.ok(Object.isFrozen(result.grabRestWorldPoint))
  assert.ok(Object.isFrozen(result.grabWorldPoint))
  assert.ok(Object.isFrozen(result.session))

  ;(input.surfaceHit.localPoint as { x: number }).x = 99
  ;(input.surfaceHit.worldPoint as { y: number }).y = 99
  if (input.appliedAngles.kind === 'per_hinge') {
    ;(input.appliedAngles.angles[0] as { angleDegrees: number }).angleDegrees = 99
  }
  assert.notEqual(result.grabLocalPoint.x, 99)
  assert.notEqual(result.grabWorldPoint.y, 99)
  assert.notEqual(result.appliedAngles[0].angleDegrees, 99)
})

type ValidInputOptions = Readonly<{
  fixedFaceId: string
  selectedHingeEdgeId: string
  surfaceFaceId: string
  appliedAngles?: FoldPreviewTreeAngleInput
  materialIndex?: number
}>

function validInput(
  options: ValidInputOptions,
): FoldPreviewTreePhysicalGrabPrepareInput {
  const model = treeModel()
  assert.equal(model.kinematics.kind, 'tree')
  if (model.kinematics.kind !== 'tree') throw new Error('fixture is not a tree')
  const tree = rerootFoldPreviewTree(model.kinematics, options.fixedFaceId)
  assert.ok(tree)
  const appliedAngles = options.appliedAngles ?? {
    kind: 'per_hinge',
    angles: [
      { edgeId: 'hinge-x', angleDegrees: 35 },
      { edgeId: 'hinge-z', angleDegrees: 55 },
    ],
  }
  const normalizedAngles = normalizeFixtureAngles(tree, appliedAngles)
  const pose = calculateFoldTreePoseWithAngles(tree, {
    kind: 'per_hinge',
    angles: normalizedAngles,
  })
  assert.ok(pose)
  const faceTransform = pose.faceTransforms.get(options.surfaceFaceId)
  const joint = tree.joints.find(
    (candidate) => candidate.hinge.edgeId === options.selectedHingeEdgeId,
  )
  assert.ok(faceTransform && joint)
  const hingeTransform = pose.hingeTransforms.get(options.selectedHingeEdgeId)
  assert.ok(hingeTransform)

  const materialIndex =
    options.materialIndex ?? FOLD_PREVIEW_FRONT_MATERIAL_INDEX
  const capY = materialIndex === FOLD_PREVIEW_BACK_MATERIAL_INDEX
    ? Math.fround(-VISUAL_THICKNESS / 2)
    : Math.fround(VISUAL_THICKNESS / 2)
  const localPoint = localGrabPoint(options.surfaceFaceId, capY)
  const world = new Vector3(
    localPoint.x,
    localPoint.y,
    localPoint.z,
  ).applyMatrix4(faceTransform)
  const axisStart = new Vector3(
    joint.hinge.start.x,
    0,
    joint.hinge.start.z,
  ).applyMatrix4(hingeTransform)
  const axisEnd = new Vector3(
    joint.hinge.end.x,
    0,
    joint.hinge.end.z,
  ).applyMatrix4(hingeTransform)
  const axis = axisEnd.sub(axisStart).normalize()
  const surfaceHit: FoldPreviewFaceSurfaceHit = {
    faceId: options.surfaceFaceId,
    localPoint,
    worldPoint: { x: world.x, y: world.y, z: world.z },
    distance: 5,
    materialIndex,
  }
  return {
    model,
    fixedFaceId: options.fixedFaceId,
    selectedHingeEdgeId: options.selectedHingeEdgeId,
    appliedAngles,
    contextKey: 'tree-context',
    surfaceHit,
    visualThickness: VISUAL_THICKNESS,
    startRay: rayAlongAxis(surfaceHit.worldPoint, axis),
    minimumOrbitRadius: MINIMUM_ORBIT_RADIUS,
  }
}

function normalizeFixtureAngles(
  tree: Extract<FoldGraphPreviewModel['kinematics'], { kind: 'tree' }>,
  input: FoldPreviewTreeAngleInput,
): readonly FoldPreviewHingeAngle[] {
  if (input.kind === 'uniform') {
    return tree.joints.map((joint) => ({
      edgeId: joint.hinge.edgeId,
      angleDegrees: input.angleDegrees,
    }))
  }
  const byEdgeId = new Map(input.angles.map((angle) => [
    angle.edgeId,
    angle.angleDegrees,
  ]))
  return tree.joints.map((joint) => ({
    edgeId: joint.hinge.edgeId,
    angleDegrees: byEdgeId.get(joint.hinge.edgeId) ?? Number.NaN,
  }))
}

function rayAlongAxis(
  point: FoldPreviewPhysicalGrabPoint,
  axis: Vector3,
): FoldPreviewPhysicalGrabRay {
  return {
    origin: {
      x: point.x + axis.x * 5,
      y: point.y + axis.y * 5,
      z: point.z + axis.z * 5,
    },
    direction: { x: -axis.x, y: -axis.y, z: -axis.z },
    minimumDistance: 0,
    maximumDistance: Number.POSITIVE_INFINITY,
  }
}

function localGrabPoint(
  faceId: string,
  y: number,
): FoldPreviewPhysicalGrabPoint {
  if (faceId === 'root') return { x: -0.75, y, z: 0.4 }
  if (faceId === 'middle') return { x: 0.5, y, z: 0.4 }
  return { x: 1.45, y, z: 0.65 }
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

function treeModel(): FoldGraphPreviewModel {
  const root = face('root', -1, 0)
  const middle = face('middle', 0, 1)
  const leaf = face('leaf', 1, 2)
  const hingeZ: FoldPreviewHingeModel = {
    edgeId: 'hinge-z',
    leftFaceId: 'root',
    rightFaceId: 'middle',
    start: { vertexId: 'z-start', x: 0, z: -1 },
    end: { vertexId: 'z-end', x: 0, z: 1 },
    axis: { x: 0, z: 1 },
    assignment: 'mountain',
    rotationSign: 1,
  }
  const hingeX: FoldPreviewHingeModel = {
    edgeId: 'hinge-x',
    leftFaceId: 'middle',
    rightFaceId: 'leaf',
    start: { vertexId: 'x-start', x: 0, z: 0 },
    end: { vertexId: 'x-end', x: 1, z: 0 },
    axis: { x: 1, z: 0 },
    assignment: 'valley',
    rotationSign: -1,
  }
  return {
    kind: 'fold_graph',
    projectId: 'project',
    revision: 1,
    worldUnitsPerMillimetre: 1,
    paperCenter: { x: 0.5, y: 0 },
    worldBounds: { minX: -1, minZ: -1, maxX: 2, maxZ: 1 },
    faces: [root, middle, leaf],
    hinges: [hingeZ, hingeX],
    kinematics: {
      kind: 'tree',
      rootFaceId: 'root',
      joints: [
        {
          parentFaceId: 'root',
          childFaceId: 'middle',
          hinge: hingeZ,
          childRotationSign: 1,
        },
        {
          parentFaceId: 'middle',
          childFaceId: 'leaf',
          hinge: hingeX,
          childRotationSign: -1,
        },
      ],
    },
  }
}

function face(
  id: string,
  minimumX: number,
  maximumX: number,
): FoldPreviewFaceModel {
  return {
    id,
    polygon: [
      { vertexId: `${id}-a`, x: minimumX, z: -1 },
      { vertexId: `${id}-b`, x: maximumX, z: -1 },
      { vertexId: `${id}-c`, x: maximumX, z: 1 },
      { vertexId: `${id}-d`, x: minimumX, z: 1 },
    ],
  }
}
