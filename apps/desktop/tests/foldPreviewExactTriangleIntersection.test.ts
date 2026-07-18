import assert from 'node:assert/strict'
import test from 'node:test'

import {
  provesFoldPreviewBinary64TransversalTriangleIntersection,
  type FoldPreviewBinary64Point,
} from '../src/lib/foldPreviewExactTriangleIntersection.ts'

const horizontal: readonly FoldPreviewBinary64Point[] = [
  { x: -2, y: -2, z: 0 },
  { x: 2, y: -2, z: 0 },
  { x: 0, y: 2, z: 0 },
]
const vertical: readonly FoldPreviewBinary64Point[] = [
  { x: -2, y: 0, z: -2 },
  { x: 2, y: 0, z: -2 },
  { x: 0, y: 0, z: 2 },
]

test('exact binary64 proof recognizes a positive transversal line overlap', () => {
  assert.equal(
    provesFoldPreviewBinary64TransversalTriangleIntersection(
      horizontal,
      vertical,
    ),
    true,
  )
  assert.equal(
    provesFoldPreviewBinary64TransversalTriangleIntersection(
      vertical,
      horizontal,
    ),
    true,
  )
})

test('proof is invariant under every triangle winding and start vertex', () => {
  for (const first of trianglePermutations(horizontal)) {
    for (const second of trianglePermutations(vertical)) {
      assert.equal(
        provesFoldPreviewBinary64TransversalTriangleIntersection(
          first,
          second,
        ),
        true,
      )
    }
  }
})

test('shared features and coplanar overlap never satisfy the transversal proof', () => {
  const coplanarOverlap = horizontal.map(({ x, y, z }) => ({
    x: x / 2,
    y: y / 2,
    z,
  }))
  const sharedEdge = [
    horizontal[0],
    horizontal[1],
    { x: 0, y: -3, z: 1 },
  ]
  const sharedVertex = [
    horizontal[0],
    { x: -3, y: -3, z: 1 },
    { x: -3, y: -1, z: 1 },
  ]

  for (const second of [coplanarOverlap, sharedEdge, sharedVertex]) {
    assert.equal(
      provesFoldPreviewBinary64TransversalTriangleIntersection(
        horizontal,
        second,
      ),
      false,
    )
  }
})

test('straddling sections that meet only at a shared vertex remain contact-only', () => {
  const first = [
    { x: 0, y: 0, z: 0 },
    { x: 2, y: -1, z: 0 },
    { x: 2, y: 1, z: 0 },
  ]
  const second = [
    { x: 0, y: 0, z: 0 },
    { x: -2, y: 0, z: -1 },
    { x: -2, y: 0, z: 1 },
  ]
  assert.equal(
    provesFoldPreviewBinary64TransversalTriangleIntersection(first, second),
    false,
  )
})

test('straddling plane sections with no line overlap are not promoted', () => {
  const displaced = vertical.map(({ x, y, z }) => ({
    x: x + 20,
    y,
    z,
  }))
  assert.equal(
    provesFoldPreviewBinary64TransversalTriangleIntersection(
      horizontal,
      displaced,
    ),
    false,
  )
})

test('a plane-boundary vertex does not hide a positive transversal overlap', () => {
  const planeVertex = [
    { x: -2, y: 0, z: 0 },
    { x: 2, y: -2, z: -1 },
    { x: 0, y: 2, z: 1 },
  ]
  for (const first of trianglePermutations(horizontal)) {
    for (const second of trianglePermutations(planeVertex)) {
      assert.equal(
        provesFoldPreviewBinary64TransversalTriangleIntersection(
          first,
          second,
        ),
        true,
      )
    }
  }
})

test('binary64 conversion stays exact at subnormal and large exponents', () => {
  for (const scale of [Number.MIN_VALUE, 2 ** 500]) {
    const first = horizontal.map(({ x, y, z }) => ({
      x: x * scale,
      y: y * scale,
      z: z * scale,
    }))
    const second = vertical.map(({ x, y, z }) => ({
      x: x * scale,
      y: y * scale,
      z: z * scale,
    }))
    assert.equal(
      provesFoldPreviewBinary64TransversalTriangleIntersection(first, second),
      true,
      `scale ${scale}`,
    )
  }
})

test('degenerate triangles and invalid values fail closed', () => {
  const degenerate = [
    { x: 0, y: 0, z: 0 },
    { x: 1, y: 1, z: 1 },
    { x: 2, y: 2, z: 2 },
  ]
  const invalid = [
    { x: 0, y: 0, z: 0 },
    { x: 1, y: Number.NaN, z: 0 },
    { x: 0, y: 1, z: 0 },
  ]
  for (const second of [degenerate, invalid]) {
    assert.equal(
      provesFoldPreviewBinary64TransversalTriangleIntersection(
        horizontal,
        second,
      ),
      false,
    )
  }
  assert.equal(
    provesFoldPreviewBinary64TransversalTriangleIntersection(
      horizontal,
      vertical.slice(0, 2),
    ),
    false,
  )
})

test('coordinate getters are snapshotted once and hostile access fails closed', () => {
  const reads = new Map<string, number>()
  const snapshotted = vertical.map((point, pointIndex) => {
    const coordinate = (key: 'x' | 'y' | 'z') => ({
      enumerable: true,
      get() {
        const identity = `${pointIndex}:${key}`
        reads.set(identity, (reads.get(identity) ?? 0) + 1)
        return point[key]
      },
    })
    return Object.defineProperties({}, {
      x: coordinate('x'),
      y: coordinate('y'),
      z: coordinate('z'),
    }) as FoldPreviewBinary64Point
  })
  assert.equal(
    provesFoldPreviewBinary64TransversalTriangleIntersection(
      horizontal,
      snapshotted,
    ),
    true,
  )
  assert.deepEqual([...reads.values()], Array(9).fill(1))

  const revoked = Proxy.revocable(vertical[0], {})
  revoked.revoke()
  assert.equal(
    provesFoldPreviewBinary64TransversalTriangleIntersection(
      horizontal,
      [revoked.proxy, vertical[1], vertical[2]],
    ),
    false,
  )
})

function trianglePermutations(
  triangle: readonly FoldPreviewBinary64Point[],
) {
  return [
    [triangle[0], triangle[1], triangle[2]],
    [triangle[1], triangle[2], triangle[0]],
    [triangle[2], triangle[0], triangle[1]],
    [triangle[0], triangle[2], triangle[1]],
    [triangle[2], triangle[1], triangle[0]],
    [triangle[1], triangle[0], triangle[2]],
  ] as const
}
