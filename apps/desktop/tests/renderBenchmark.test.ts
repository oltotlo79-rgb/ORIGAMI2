import assert from 'node:assert/strict'
import test from 'node:test'

import {
  measureBenchmarkPayloadBytes,
  prepareBenchmarkRenderData,
  type BenchmarkPatternData,
} from '../src/lib/renderBenchmark.ts'

function fixture(): BenchmarkPatternData {
  return {
    requested_edge_count: 2,
    vertex_count: 3,
    edge_count: 2,
    vertices: [
      { id: 'v0', position: { x: -1, y: 2 } },
      { id: 'v1', position: { x: 3, y: 2 } },
      { id: 'v2', position: { x: 3, y: 6 } },
    ],
    edges: [
      { id: 'e0', start: 'v0', end: 'v1', kind: 'mountain' },
      { id: 'e1', start: 'v1', end: 'v2', kind: 'valley' },
    ],
  }
}

test('benchmark response becomes renderable lines with exact bounds', () => {
  const result = prepareBenchmarkRenderData(fixture())
  assert.equal(result.requestedEdgeCount, 2)
  assert.deepEqual(result.bounds, { minX: -1, minY: 2, maxX: 3, maxY: 6 })
  assert.deepEqual(result.vertices[1], { id: 'v1', x: 3, y: 2 })
  assert.deepEqual(result.lines[1], {
    id: 'e1',
    startVertexId: 'v1',
    endVertexId: 'v2',
    x1: 3,
    y1: 2,
    x2: 3,
    y2: 6,
    kind: 'valley',
  })
})

test('benchmark preparation rejects malformed topology before rendering', () => {
  assert.throws(
    () => prepareBenchmarkRenderData({
      ...fixture(),
      edges: [{ id: 'e0', start: 'v0', end: 'missing', kind: 'mountain' }],
      edge_count: 1,
    }),
    /missing vertex/,
  )
  assert.throws(
    () => prepareBenchmarkRenderData({
      ...fixture(),
      vertices: [
        { id: 'v0', position: { x: 0, y: 0 } },
        { id: 'v0', position: { x: 1, y: 0 } },
      ],
      vertex_count: 2,
      edges: [],
      edge_count: 0,
    }),
    /duplicated/,
  )
})

test('benchmark preparation expands zero-area bounds and supports empty data', () => {
  const point = prepareBenchmarkRenderData({
    requested_edge_count: 0,
    vertex_count: 1,
    edge_count: 0,
    vertices: [{ id: 'v0', position: { x: 4, y: 7 } }],
    edges: [],
  })
  assert.deepEqual(point.bounds, { minX: 3.5, minY: 6.5, maxX: 4.5, maxY: 7.5 })

  const empty = prepareBenchmarkRenderData({
    requested_edge_count: 0,
    vertex_count: 0,
    edge_count: 0,
    vertices: [],
    edges: [],
  })
  assert.deepEqual(empty.bounds, { minX: 0, minY: 0, maxX: 1, maxY: 1 })
})

test('benchmark payload reports its UTF-8 transfer size', () => {
  const data = fixture()
  assert.equal(measureBenchmarkPayloadBytes(data), Buffer.byteLength(JSON.stringify(data), 'utf8'))
})
