import assert from 'node:assert/strict'
import test from 'node:test'

import {
  BufferGeometry,
  LineBasicMaterial,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Raycaster,
  Vector2,
  Vector3,
} from 'three'
import {
  pickFoldPreviewFaceSurface,
  pickFoldPreviewTarget,
  type FoldPreviewPickObject,
} from '../src/lib/foldPreviewPicking.ts'

test('hinges take selection priority over an intersected face', () => {
  const { camera, hinge, face } = fixture()
  assert.deepEqual(pickFoldPreviewTarget(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    [{ id: 'hinge', object: hinge }],
    [{ id: 'face', object: face }],
  ), { kind: 'hinge', edgeId: 'hinge' })
})

test('faces remain pickable when no hinge intersects the pointer', () => {
  const { camera, face } = fixture()
  assert.deepEqual(pickFoldPreviewTarget(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    [],
    [{ id: 'face', object: face }],
  ), { kind: 'face', faceId: 'face' })
})

test('empty space and invalid pointer requests return no target', () => {
  const { camera, hinge, face } = fixture()
  const hinges = [{ id: 'hinge', object: hinge }]
  const faces = [{ id: 'face', object: face }]
  assert.equal(pickFoldPreviewTarget(
    new Raycaster(),
    camera,
    new Vector2(0.95, 0.95),
    hinges,
    faces,
  ), null)
  assert.equal(pickFoldPreviewTarget(
    new Raycaster(),
    camera,
    new Vector2(Number.NaN, 0),
    hinges,
    faces,
  ), null)
  assert.equal(pickFoldPreviewTarget(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    hinges,
    faces,
    0,
  ), null)
})

test('duplicate IDs or objects fail closed before raycasting', () => {
  const { camera, hinge, face } = fixture()
  const duplicateIds: FoldPreviewPickObject[] = [
    { id: 'same', object: hinge },
    { id: 'same', object: face },
  ]
  const duplicateObjects: FoldPreviewPickObject[] = [
    { id: 'first', object: hinge },
    { id: 'second', object: hinge },
  ]
  assert.equal(pickFoldPreviewTarget(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    duplicateIds,
    [],
  ), null)
  assert.equal(pickFoldPreviewTarget(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    duplicateObjects,
    [],
  ), null)
})

test('surface picking returns one detached frozen world-space hit', () => {
  const { camera, face } = fixture()
  const result = pickFoldPreviewFaceSurface(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    [{ id: 'face', object: face }],
  )

  assert.deepEqual(result, {
    faceId: 'face',
    worldPoint: { x: 0, y: 0, z: 0 },
    distance: 5,
    materialIndex: 0,
  })
  assert.ok(Object.isFrozen(result))
  assert.ok(Object.isFrozen(result?.worldPoint))
})

test('surface picking chooses the nearest face without exposing its intersection', () => {
  const { camera, face } = fixture()
  const farther = new Mesh(new PlaneGeometry(2, 2), new MeshBasicMaterial())
  farther.position.z = -1
  farther.updateMatrixWorld(true)
  const result = pickFoldPreviewFaceSurface(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    [
      { id: 'farther', object: farther },
      { id: 'nearest', object: face },
    ],
  )

  assert.equal(result?.faceId, 'nearest')
  assert.equal(result?.distance, 5)
})

test('surface picking rejects invalid pointers, targets, and intersection values', () => {
  const { camera, face } = fixture()
  assert.equal(pickFoldPreviewFaceSurface(
    new Raycaster(),
    camera,
    new Vector2(Number.NaN, 0),
    [{ id: 'face', object: face }],
  ), null)
  assert.equal(pickFoldPreviewFaceSurface(
    new Raycaster(),
    camera,
    new Vector2(0, 0),
    [
      { id: 'same', object: face },
      { id: 'same', object: new Mesh(new PlaneGeometry()) },
    ],
  ), null)

  const malformedRaycaster = {
    setFromCamera() {},
    intersectObjects() {
      return [{
        distance: 1,
        point: new Vector3(Number.POSITIVE_INFINITY, 0, 0),
        object: face,
        face: { materialIndex: 0 },
      }]
    },
  } as unknown as Raycaster
  assert.equal(pickFoldPreviewFaceSurface(
    malformedRaycaster,
    camera,
    new Vector2(0, 0),
    [{ id: 'face', object: face }],
  ), null)
})

test('surface picking contains raycaster failures', () => {
  const { camera, face } = fixture()
  const throwingRaycaster = {
    setFromCamera() {
      throw new Error('camera failure')
    },
    intersectObjects() {
      assert.fail('intersection must not run')
    },
  } as unknown as Raycaster
  assert.equal(pickFoldPreviewFaceSurface(
    throwingRaycaster,
    camera,
    new Vector2(0, 0),
    [{ id: 'face', object: face }],
  ), null)
})

function fixture() {
  const camera = new PerspectiveCamera(50, 1, 0.1, 100)
  camera.position.set(0, 0, 5)
  camera.lookAt(0, 0, 0)
  camera.updateProjectionMatrix()
  camera.updateMatrixWorld(true)

  const hingeGeometry = new BufferGeometry().setFromPoints([
    new Vector3(-0.6, 0, 0.05),
    new Vector3(0.6, 0, 0.05),
  ])
  const hinge = new LineSegments(hingeGeometry, new LineBasicMaterial())
  hinge.updateMatrixWorld(true)
  const face = new Mesh(new PlaneGeometry(2, 2), new MeshBasicMaterial())
  face.updateMatrixWorld(true)
  return { camera, hinge, face }
}
