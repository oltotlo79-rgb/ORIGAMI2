import assert from 'node:assert/strict'
import { test } from 'node:test'
import { analyzeGenericSkeletonTree } from '../src/lib/genericSkeletonTree.ts'

const point = (x: number, y: number) => ({ x_tenths_mm: x, y_tenths_mm: y })
const segment = (id: number, ax: number, ay: number, bx: number, by: number) => ({
  id, start: point(ax, ay), end: point(bx, by),
})

test('confirmed branch trees are invariant to segment storage order', () => {
  const tree = [segment(3, 0, 0, 0, 10), segment(1, 0, 0, -10, 0), segment(2, 0, 0, 10, 0)]
  assert.deepEqual(analyzeGenericSkeletonTree(tree), { status: 'tree', pointCount: 4, edgeCount: 3 })
  assert.deepEqual(analyzeGenericSkeletonTree([...tree].reverse()), analyzeGenericSkeletonTree(tree))
})

test('cycles duplicate edges disconnected graphs and zero-length bars fail closed', () => {
  assert.equal(analyzeGenericSkeletonTree(Array.from({ length: 17 }, (_, id) =>
    segment(id, id, 0, id + 1, 0))).status, 'resource_limit')
  assert.equal(analyzeGenericSkeletonTree([
    segment(1, 0, 0, 10, 0), segment(2, 10, 0, 5, 10), segment(3, 5, 10, 0, 0),
  ]).status, 'cycle')
  assert.equal(analyzeGenericSkeletonTree([
    segment(1, 0, 0, 10, 0), segment(2, 10, 0, 0, 0),
  ]).status, 'duplicate_edge')
  assert.equal(analyzeGenericSkeletonTree([
    segment(1, 0, 0, 10, 0), segment(2, 20, 0, 30, 0),
  ]).status, 'disconnected')
  assert.equal(analyzeGenericSkeletonTree([segment(1, 0, 0, 0, 0)]).status, 'degenerate')
})
