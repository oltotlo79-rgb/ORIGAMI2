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
