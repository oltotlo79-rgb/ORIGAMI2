import assert from 'node:assert/strict'
import test from 'node:test'

import type { BufferAttribute, BufferGeometry } from 'three'
import {
  FOLD_PREVIEW_BACK_MATERIAL_INDEX,
  FOLD_PREVIEW_FRONT_MATERIAL_INDEX,
  FOLD_PREVIEW_SIDE_MATERIAL_INDEX,
  createFoldPreviewFaceGeometry,
  triangulateFoldPreviewPolygon,
} from '../src/lib/foldPreviewGeometry.ts'

const rectangle = [
  { x: -2, z: -1 },
  { x: -2, z: 1 },
  { x: 2, z: 1 },
  { x: 2, z: -1 },
] as const

test('rectangular face becomes a centred prism with three complete material groups', () => {
  const geometry = createFoldPreviewFaceGeometry(rectangle, 0.2)

  assert.equal(geometry.index, null)
  assert.deepEqual(geometry.groups, [
    { start: 0, count: 6, materialIndex: FOLD_PREVIEW_FRONT_MATERIAL_INDEX },
    { start: 6, count: 6, materialIndex: FOLD_PREVIEW_BACK_MATERIAL_INDEX },
    { start: 12, count: 24, materialIndex: FOLD_PREVIEW_SIDE_MATERIAL_INDEX },
  ])
  assertBounds(geometry, [-2, -0.1, -1], [2, 0.1, 1])
  assertFiniteGeometry(geometry)
  assertGroupNormalsAndWinding(geometry)
  const uv = geometry.getAttribute('uv') as BufferAttribute
  assert.equal(uv.count, geometry.getAttribute('position').count)
  for (let index = 0; index < uv.count; index += 1) {
    assert.ok(uv.getX(index) >= 0 && uv.getX(index) <= 1)
    assert.ok(uv.getY(index) >= 0 && uv.getY(index) <= 1)
  }

  let disposed = false
  geometry.addEventListener('dispose', () => { disposed = true })
  assert.doesNotThrow(() => geometry.dispose())
  assert.equal(disposed, true)
})

test('weak-convex input preserves its collinear boundary point and valid side walls', () => {
  const weakConvex = [
    { x: -2, z: -1 },
    { x: -2, z: 1 },
    { x: 0, z: 1 },
    { x: 2, z: 1 },
    { x: 2, z: -1 },
  ] as const
  const geometry = createFoldPreviewFaceGeometry(weakConvex, 0.4)

  assert.equal(geometry.groups.length, 3)
  assert.equal(geometry.groups[2].count, weakConvex.length * 6)
  assertBounds(geometry, [-2, -0.2, -1], [2, 0.2, 1])
  assertFiniteGeometry(geometry)
  assertGroupNormalsAndWinding(geometry)

  const positions = geometry.getAttribute('position') as BufferAttribute
  const collinearPointOccurrences = Array.from({ length: positions.count }, (_, index) => index)
    .filter((index) => (
      positions.getX(index) === 0
      && Math.abs(positions.getY(index)) === Math.fround(0.2)
      && positions.getZ(index) === 1
    ))
  assert.ok(collinearPointOccurrences.length >= 4)

  let disposed = false
  geometry.addEventListener('dispose', () => { disposed = true })
  geometry.dispose()
  assert.equal(disposed, true)
})

test('a clockwise hole removes both caps and creates inward thickness walls', () => {
  const outer = [
    { x: -3, z: -3 },
    { x: 3, z: -3 },
    { x: 3, z: 3 },
    { x: -3, z: 3 },
  ] as const
  const hole = [
    { x: -1, z: -1 },
    { x: -1, z: 1 },
    { x: 1, z: 1 },
    { x: 1, z: -1 },
  ] as const
  const geometry = createFoldPreviewFaceGeometry(outer, 0.2, [hole])

  assert.equal(geometry.groups.length, 3)
  assert.equal(geometry.groups[2].count, (outer.length + hole.length) * 6)
  assert.ok(geometry.groups[0].count > 6)
  assert.equal(geometry.groups[0].count, geometry.groups[1].count)
  assertBounds(geometry, [-3, -0.1, -3], [3, 0.1, 3])
  assertFiniteGeometry(geometry)
  assertGroupNormalsAndWinding(geometry)
})

test('invalid, non-finite, and degenerate inputs fail deterministically', () => {
  const cases: ReadonlyArray<readonly [string, () => unknown]> = [
    ['too few points', () => createFoldPreviewFaceGeometry(rectangle.slice(0, 2), 0.2)],
    ['non-finite x', () => createFoldPreviewFaceGeometry([
      ...rectangle.slice(0, 3),
      { x: Number.NaN, z: -1 },
    ], 0.2)],
    ['non-finite z', () => createFoldPreviewFaceGeometry([
      ...rectangle.slice(0, 3),
      { x: 2, z: Number.POSITIVE_INFINITY },
    ], 0.2)],
    ['zero thickness', () => createFoldPreviewFaceGeometry(rectangle, 0)],
    ['negative thickness', () => createFoldPreviewFaceGeometry(rectangle, -0.1)],
    ['non-finite thickness', () => createFoldPreviewFaceGeometry(rectangle, Number.NaN)],
    ['repeated point', () => createFoldPreviewFaceGeometry([
      rectangle[0], rectangle[1], rectangle[2], rectangle[1], rectangle[3],
    ], 0.2)],
    ['zero area', () => createFoldPreviewFaceGeometry([
      { x: 0, z: 0 }, { x: 1, z: 0 }, { x: 2, z: 0 },
    ], 0.2)],
  ]

  for (const [label, operation] of cases) {
    assert.throws(operation, RangeError, label)
  }
})

test('collision and rendering share one deterministic double-precision triangulation', () => {
  const concave = [
    { x: 0, z: 0 },
    { x: 3, z: 0 },
    { x: 3, z: 3 },
    { x: 1.5, z: 1.25 },
    { x: 0, z: 3 },
  ] as const
  const triangles = triangulateFoldPreviewPolygon(concave)
  assert.equal(triangles.length, concave.length - 2)
  assert.deepEqual(triangulateFoldPreviewPolygon(concave), triangles)
  assert.ok(triangles.every((triangle) =>
    triangle.length === 3 && new Set(triangle).size === 3))
  assert.throws(
    () => triangulateFoldPreviewPolygon([{ x: 0, z: 0 }, { x: 1, z: 0 }]),
    RangeError,
  )
})

function assertBounds(
  geometry: BufferGeometry,
  expectedMin: readonly [number, number, number],
  expectedMax: readonly [number, number, number],
) {
  geometry.computeBoundingBox()
  assert.ok(geometry.boundingBox)
  assert.deepEqual(geometry.boundingBox.min.toArray(), expectedMin.map(Math.fround))
  assert.deepEqual(geometry.boundingBox.max.toArray(), expectedMax.map(Math.fround))
}

function assertFiniteGeometry(geometry: BufferGeometry) {
  const position = geometry.getAttribute('position') as BufferAttribute
  const normal = geometry.getAttribute('normal') as BufferAttribute
  assert.equal(position.itemSize, 3)
  assert.equal(normal.itemSize, 3)
  assert.equal(position.count, normal.count)
  assert.ok(position.count > 0)
  for (const attribute of [position, normal]) {
    for (const value of attribute.array) assert.ok(Number.isFinite(value))
  }
}

function assertGroupNormalsAndWinding(geometry: BufferGeometry) {
  const position = geometry.getAttribute('position') as BufferAttribute
  const normal = geometry.getAttribute('normal') as BufferAttribute
  let nextStart = 0
  for (const group of geometry.groups) {
    assert.equal(group.start, nextStart)
    assert.equal(group.count % 3, 0)
    nextStart += group.count
    for (let vertex = group.start; vertex < group.start + group.count; vertex += 1) {
      const normalX = normal.getX(vertex)
      const normalY = normal.getY(vertex)
      const normalZ = normal.getZ(vertex)
      assert.ok(Math.abs(Math.hypot(normalX, normalY, normalZ) - 1) < 1e-6)
      if (group.materialIndex === FOLD_PREVIEW_FRONT_MATERIAL_INDEX) {
        assert.deepEqual([normalX, normalY, normalZ], [0, 1, 0])
      } else if (group.materialIndex === FOLD_PREVIEW_BACK_MATERIAL_INDEX) {
        assert.deepEqual([normalX, normalY, normalZ], [0, -1, 0])
      } else {
        assert.equal(group.materialIndex, FOLD_PREVIEW_SIDE_MATERIAL_INDEX)
        assert.equal(normalY, 0)
      }
    }

    for (let vertex = group.start; vertex < group.start + group.count; vertex += 3) {
      const edgeAX = position.getX(vertex + 1) - position.getX(vertex)
      const edgeAY = position.getY(vertex + 1) - position.getY(vertex)
      const edgeAZ = position.getZ(vertex + 1) - position.getZ(vertex)
      const edgeBX = position.getX(vertex + 2) - position.getX(vertex)
      const edgeBY = position.getY(vertex + 2) - position.getY(vertex)
      const edgeBZ = position.getZ(vertex + 2) - position.getZ(vertex)
      const crossX = edgeAY * edgeBZ - edgeAZ * edgeBY
      const crossY = edgeAZ * edgeBX - edgeAX * edgeBZ
      const crossZ = edgeAX * edgeBY - edgeAY * edgeBX
      const alignment = crossX * normal.getX(vertex)
        + crossY * normal.getY(vertex)
        + crossZ * normal.getZ(vertex)
      assert.ok(alignment > 0)
    }
  }
  assert.equal(nextStart, position.count)
}
