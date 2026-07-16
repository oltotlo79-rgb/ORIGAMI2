import assert from 'node:assert/strict'
import { performance } from 'node:perf_hooks'
import test from 'node:test'

import {
  DEFAULT_SNAP_SETTINGS,
  createVisibleGrid,
  resolveSnapTarget,
  type ResolveSnapTargetOptions,
  type SnapGrid,
  type SnapKind,
  type SnapSegment,
  type SnapSettings,
} from '../src/lib/snap.ts'
import { createVertexPlacement } from '../src/lib/vertexPlacement.ts'
import {
  createIntersectionSnapIndex,
  type IntersectionSnapSegment,
} from '../src/lib/intersectionSnap.ts'

const EMPTY_GRID: SnapGrid = { xValues: [], yValues: [] }

function only(...kinds: SnapKind[]): SnapSettings {
  return {
    vertex: kinds.includes('vertex'),
    midpoint: kinds.includes('midpoint'),
    edge: kinds.includes('edge'),
    grid: kinds.includes('grid'),
  }
}

function resolve(overrides: Partial<ResolveSnapTargetOptions> = {}) {
  return resolveSnapTarget({
    point: { x: 0, y: 0 },
    scale: 1,
    settings: DEFAULT_SNAP_SETTINGS,
    vertices: [],
    segments: [],
    grid: EMPTY_GRID,
    ...overrides,
  })
}

test('default settings enable every snap kind', () => {
  assert.deepEqual(DEFAULT_SNAP_SETTINGS, {
    vertex: true,
    midpoint: true,
    edge: true,
    grid: true,
  })
  assert.equal(Object.isFrozen(DEFAULT_SNAP_SETTINGS), true)
})

test('kind priority is vertex, midpoint, edge, then grid', () => {
  const segment: SnapSegment = {
    id: 'segment',
    startVertexId: 'start',
    endVertexId: 'end',
    x1: -2,
    y1: 0,
    x2: 2,
    y2: 0,
  }
  const common = {
    vertices: [{ id: 'vertex', x: 9, y: 0 }],
    segments: [segment],
    grid: { xValues: [0], yValues: [0] },
  }

  assert.equal(resolve({ ...common })?.kind, 'vertex')
  assert.equal(resolve({ ...common, settings: only('midpoint', 'edge', 'grid') })?.kind, 'midpoint')
  assert.equal(resolve({ ...common, settings: only('edge', 'grid') })?.kind, 'edge')
  assert.equal(resolve({ ...common, settings: only('grid') })?.kind, 'grid')
})

test('same-kind ties use distance and then lexical key', () => {
  const target = resolve({
    settings: only('vertex'),
    vertices: [
      { id: 'b', x: 1, y: 0 },
      { id: 'a', x: -1, y: 0 },
      { id: 'far', x: 2, y: 0 },
    ],
  })

  assert.equal(target?.key, 'vertex:a')
  assert.deepEqual(target?.point, { x: -1, y: 0 })
})

test('pixel thresholds are inclusive and can be overridden', () => {
  assert.equal(resolve({
    settings: only('vertex'),
    vertices: [{ id: 'limit', x: 10, y: 0 }],
  })?.sourceId, 'limit')
  assert.equal(resolve({
    settings: only('vertex'),
    vertices: [{ id: 'outside', x: 10.001, y: 0 }],
  }), null)
  assert.equal(resolve({
    settings: only('vertex'),
    vertices: [{ id: 'custom', x: 3, y: 0 }],
    thresholdsPx: { vertex: 3 },
  })?.sourceId, 'custom')
})

test('visible grid is origin-aligned across negative coordinates', () => {
  const grid = createVisibleGrid(
    { minX: -23, minY: -12, maxX: 27, maxY: 8 },
    5,
    100,
  )

  assert.deepEqual(grid.xValues, [-20, -10, 0, 10, 20])
  assert.deepEqual(grid.yValues, [-10, 0])
})

test('edge projection uses only a strict segment interior', () => {
  const segment: SnapSegment = {
    id: 'edge',
    startVertexId: 'start',
    endVertexId: 'end',
    x1: 0,
    y1: 0,
    x2: 10,
    y2: 0,
  }

  const interior = resolve({
    point: { x: 4, y: 3 },
    settings: only('edge'),
    segments: [segment],
  })
  assert.deepEqual(interior?.point, { x: 4, y: 0 })
  assert.equal(interior?.distancePx, 3)
  assert.equal(interior?.sourceFraction, 0.4)

  assert.equal(resolve({
    point: { x: 15, y: 3 },
    settings: only('edge'),
    segments: [segment],
  }), null)
  assert.equal(resolve({
    point: { x: 0, y: 1 },
    settings: only('edge'),
    segments: [segment],
  }), null)
})

test('excluded vertex removes itself and its connected segment candidates', () => {
  const connected: SnapSegment = {
    id: 'connected',
    startVertexId: 'moving',
    endVertexId: 'other',
    x1: 0,
    y1: 0,
    x2: 6,
    y2: 0,
  }
  const common = {
    excludedVertexId: 'moving',
    vertices: [
      { id: 'moving', x: 0, y: 0 },
      { id: 'other', x: 6, y: 0 },
    ],
    segments: [connected],
  }

  assert.equal(resolve({ ...common })?.sourceId, 'other')
  assert.equal(resolve({
    ...common,
    point: { x: 3, y: 0 },
    settings: only('midpoint', 'edge'),
  }), null)
})

test('accept filters candidates, including alternate grid intersections', () => {
  const vertex = resolve({
    settings: only('vertex'),
    vertices: [
      { id: 'reject', x: 1, y: 0 },
      { id: 'accept', x: 2, y: 0 },
    ],
    accept: (target) => target.sourceId !== 'reject',
  })
  assert.equal(vertex?.sourceId, 'accept')

  const grid = resolve({
    point: { x: 0.1, y: 0.1 },
    settings: only('grid'),
    grid: { xValues: [0, 1], yValues: [0, 1] },
    accept: (target) => target.key !== 'grid:0:0',
  })
  assert.equal(grid?.key, 'grid:0:1')
})

test('non-finite inputs, zero-length segments, and overflowing geometry are ignored', () => {
  assert.equal(resolve({ point: { x: Number.NaN, y: 0 } }), null)
  assert.equal(resolve({ scale: 0 }), null)
  assert.equal(resolve({ scale: Number.POSITIVE_INFINITY }), null)

  const invalidSegments: SnapSegment[] = [
    {
      id: 'zero',
      startVertexId: 'a',
      endVertexId: 'b',
      x1: 1,
      y1: 1,
      x2: 1,
      y2: 1,
    },
    {
      id: 'overflow',
      startVertexId: 'c',
      endVertexId: 'd',
      x1: -Number.MAX_VALUE,
      y1: 0,
      x2: Number.MAX_VALUE,
      y2: 0,
    },
  ]
  assert.equal(resolve({
    settings: only('midpoint', 'edge'),
    vertices: [{ id: 'bad', x: Number.NaN, y: 0 }],
    segments: invalidSegments,
  }), null)

  assert.equal(resolve({
    settings: only('grid'),
    grid: { xValues: [Number.NaN, 0], yValues: [Number.POSITIVE_INFINITY, 0] },
  })?.key, 'grid:0:0')
})

test('all-off settings always return null', () => {
  assert.equal(resolve({
    settings: only(),
    vertices: [{ id: 'vertex', x: 0, y: 0 }],
    grid: { xValues: [0], yValues: [0] },
  }), null)
})

test('model distances are converted to pixels by scale', () => {
  assert.equal(resolve({
    scale: 5,
    settings: only('vertex'),
    vertices: [{ id: 'limit', x: 2, y: 0 }],
  })?.distancePx, 10)
  assert.equal(resolve({
    scale: 5,
    settings: only('vertex'),
    vertices: [{ id: 'outside', x: 2.01, y: 0 }],
  }), null)

  const tinyScaleGrid = {
    scale: Number.MIN_VALUE,
    settings: only('grid'),
    grid: { xValues: [1], yValues: [1] },
  }
  assert.equal(resolve(tinyScaleGrid)?.key, 'grid:1:1')
  assert.equal(resolve({
    ...tinyScaleGrid,
    accept: () => true,
  })?.key, 'grid:1:1')
  assert.equal(resolve({
    ...tinyScaleGrid,
    thresholdsPx: { grid: Number.MAX_VALUE },
    accept: () => true,
  })?.key, 'grid:1:1')
})

test('tiny scales rank vertices by model distance before inverse key order', () => {
  for (const accept of [undefined, () => true]) {
    const target = resolve({
      scale: Number.MIN_VALUE,
      settings: only('vertex'),
      vertices: [
        { id: 'a-far', x: 0.2, y: 0 },
        { id: 'z-near', x: 0.1, y: 0 },
      ],
      accept,
    })

    assert.equal(target?.sourceId, 'z-near')
    assert.equal(target?.distancePx, 0)
  }
})

test('tiny scales rank midpoint and edge candidates by model distance', () => {
  const segments: SnapSegment[] = [
    {
      id: 'a-far',
      startVertexId: 'far-start',
      endVertexId: 'far-end',
      x1: 0.2,
      y1: -1,
      x2: 0.2,
      y2: 1,
    },
    {
      id: 'z-near',
      startVertexId: 'near-start',
      endVertexId: 'near-end',
      x1: 0.1,
      y1: -1,
      x2: 0.1,
      y2: 1,
    },
  ]

  for (const accept of [undefined, () => true]) {
    for (const kind of ['midpoint', 'edge'] as const) {
      const target = resolve({
        scale: Number.MIN_VALUE,
        settings: only(kind),
        segments,
        accept,
      })

      assert.equal(target?.sourceId, 'z-near')
      assert.equal(target?.distancePx, 0)
    }
  }
})

test('tiny scales rank multiple grid intersections with and without accept', () => {
  const options = {
    scale: Number.MIN_VALUE,
    settings: only('grid'),
    grid: { xValues: [-0.3, 0.2], yValues: [0.4, 0] },
  }

  const direct = resolve(options)
  const accepted = resolve({ ...options, accept: () => true })
  assert.equal(direct?.key, 'grid:0.2:0')
  assert.equal(accepted?.key, 'grid:0.2:0')
  assert.equal(direct?.distancePx, 0)
  assert.equal(accepted?.distancePx, 0)
})

test('visible grid never exceeds the requested value count', () => {
  const grid = createVisibleGrid(
    { minX: -1000, minY: -1000, maxX: 1000, maxY: 1000 },
    10_000,
    3,
  )

  assert.ok(grid.xValues.length <= 3)
  assert.ok(grid.yValues.length <= 3)
  assert.deepEqual(grid.xValues, [-1000, 0, 1000])
  assert.deepEqual(grid.yValues, [-1000, 0, 1000])
  assert.deepEqual(
    createVisibleGrid({ minX: 0, minY: 0, maxX: Number.POSITIVE_INFINITY, maxY: 1 }),
    EMPTY_GRID,
  )
})

test('large vertex and segment sets resolve without candidate arrays', () => {
  const vertices = Array.from({ length: 10_000 }, (_, index) => ({
    id: `v${index}`,
    x: index + 0.25,
    y: 0,
  }))
  assert.equal(resolve({
    point: { x: 9999.25, y: 0 },
    settings: only('vertex'),
    vertices,
  })?.sourceId, 'v9999')

  const segments = Array.from({ length: 10_000 }, (_, index): SnapSegment => ({
    id: `s${index}`,
    startVertexId: `a${index}`,
    endVertexId: `b${index}`,
    x1: 0,
    y1: index * 100,
    x2: 1,
    y2: index * 100,
  }))
  assert.equal(resolve({
    point: { x: 0.5, y: 0 },
    settings: only('edge'),
    segments,
  })?.sourceId, 's0')
})

test('raw and grid-snapped points remain ordinary vertex additions', () => {
  assert.deepEqual(createVertexPlacement({ x: 12, y: 34 }, null, []), {
    operation: 'add',
    x: 12,
    y: 34,
  })
  assert.deepEqual(createVertexPlacement(
    { x: 20, y: 40 },
    {
      key: 'grid:20:40',
      kind: 'grid',
      point: { x: 20, y: 40 },
      distancePx: 2,
    },
    [],
  ), {
    operation: 'add',
    x: 20,
    y: 40,
  })
})

test('midpoint and edge targets become atomic edge splits', () => {
  const horizontal: SnapSegment = {
    id: 'horizontal',
    startVertexId: 'a',
    endVertexId: 'b',
    x1: 10,
    y1: 20,
    x2: 30,
    y2: 20,
  }
  const vertical: SnapSegment = {
    id: 'vertical',
    startVertexId: 'c',
    endVertexId: 'd',
    x1: -4,
    y1: 10,
    x2: -4,
    y2: 50,
  }

  assert.deepEqual(createVertexPlacement(
    { x: 20, y: 20 },
    {
      key: 'midpoint:horizontal',
      kind: 'midpoint',
      point: { x: 20, y: 20 },
      distancePx: 1,
      sourceId: 'horizontal',
      sourceFraction: 0.5,
    },
    [horizontal, vertical],
  ), {
    operation: 'split-edge',
    edgeId: 'horizontal',
    fraction: 0.5,
  })
  assert.deepEqual(createVertexPlacement(
    { x: -4, y: 40 },
    {
      key: 'edge:vertical',
      kind: 'edge',
      point: { x: -4, y: 40 },
      distancePx: 1,
      sourceId: 'vertical',
      sourceFraction: 0.75,
    },
    [horizontal, vertical],
  ), {
    operation: 'split-edge',
    edgeId: 'vertical',
    fraction: 0.75,
  })

  const reversedTarget = resolve({
    point: { x: 2, y: 1 },
    settings: only('edge'),
    segments: [{
      id: 'reversed',
      startVertexId: 'end',
      endVertexId: 'start',
      x1: 10,
      y1: 0,
      x2: 0,
      y2: 0,
    }],
  })
  assert.equal(reversedTarget?.sourceFraction, 0.8)
  assert.deepEqual(createVertexPlacement(
    reversedTarget?.point ?? { x: Number.NaN, y: Number.NaN },
    reversedTarget,
    [{
      id: 'reversed',
      startVertexId: 'end',
      endVertexId: 'start',
      x1: 10,
      y1: 0,
      x2: 0,
      y2: 0,
    }],
  ), {
    operation: 'split-edge',
    edgeId: 'reversed',
    fraction: 0.8,
  })
})

test('malformed edge snap metadata never degrades to an overlapping free vertex', () => {
  const segment: SnapSegment = {
    id: 'edge',
    startVertexId: 'a',
    endVertexId: 'b',
    x1: 0,
    y1: 0,
    x2: 10,
    y2: 0,
  }
  assert.equal(createVertexPlacement(
    { x: 5, y: 0 },
    { key: 'edge:missing', kind: 'edge', point: { x: 5, y: 0 }, distancePx: 0 },
    [segment],
  ), null)
  assert.equal(createVertexPlacement(
    { x: 5, y: 0 },
    {
      key: 'edge:edge',
      kind: 'edge',
      point: { x: 5, y: 0 },
      distancePx: 0,
      sourceId: 'edge',
    },
    [segment],
  ), null)
  assert.equal(createVertexPlacement(
    { x: 0, y: 0 },
    {
      key: 'edge:edge',
      kind: 'edge',
      point: { x: 0, y: 0 },
      distancePx: 0,
      sourceId: 'edge',
      sourceFraction: 0,
    },
    [segment],
  ), null)
})

function intersectionSegment(
  id: string,
  startVertexId: string,
  endVertexId: string,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
): IntersectionSnapSegment {
  return { id, startVertexId, endVertexId, x1, y1, x2, y2 }
}

function queryIntersection(
  segments: readonly IntersectionSnapSegment[],
  point = { x: 0, y: 0 },
  thresholdPx = 8,
) {
  return createIntersectionSnapIndex(segments).query({
    point,
    scale: 1,
    thresholdPx,
  })
}

test('proper intersections expose canonical edge IDs, fractions, point, and key', () => {
  const horizontal = intersectionSegment('z-edge', 'h1', 'h2', 0, 0, 10, 0)
  const vertical = intersectionSegment('a-edge', 'v1', 'v2', 2, -5, 2, 5)
  const result = queryIntersection([horizontal, vertical], { x: 2, y: 0 })

  assert.equal(result.truncated, false)
  assert.equal(result.candidateSegmentCount, 2)
  assert.equal(result.testedPairCount, 1)
  assert.deepEqual(result.target, {
    kind: 'intersection',
    classification: 'proper',
    key: 'intersection:["a-edge","z-edge"]',
    point: { x: 2, y: 0 },
    distancePx: 0,
    sourceEdges: [
      { id: 'a-edge', fraction: 0.5 },
      { id: 'z-edge', fraction: 0.2 },
    ],
  })
})

test('proper intersection output is deterministic across input order and reversed edges', () => {
  const horizontal = intersectionSegment('z-edge', 'h2', 'h1', 10, 0, 0, 0)
  const vertical = intersectionSegment('a-edge', 'v2', 'v1', 2, 5, 2, -5)
  const forward = queryIntersection([horizontal, vertical], { x: 2, y: 0 })
  const shuffled = queryIntersection([vertical, horizontal], { x: 2, y: 0 })

  assert.deepEqual(shuffled, forward)
  assert.deepEqual(forward.target?.sourceEdges, [
    { id: 'a-edge', fraction: 0.5 },
    { id: 'z-edge', fraction: 0.8 },
  ])
})

test('equal-distance proper intersections use the canonical key as a stable tie-breaker', () => {
  const segments = [
    intersectionSegment('b', 'b1', 'b2', -1.5, 0, -0.5, 0),
    intersectionSegment('c', 'c1', 'c2', -1, -1, -1, 1),
    intersectionSegment('a', 'a1', 'a2', 0.5, 0, 1.5, 0),
    intersectionSegment('z', 'z1', 'z2', 1, -1, 1, 1),
  ]
  const first = queryIntersection(segments, { x: 0, y: 0 }, 2)
  const second = queryIntersection([...segments].reverse(), { x: 0, y: 0 }, 2)

  assert.equal(first.target?.key, 'intersection:["a","z"]')
  assert.deepEqual(second, first)
})

test('intersection accept filters choose the next ranked candidate deterministically', () => {
  const segments = [
    intersectionSegment('left-horizontal', 'lh1', 'lh2', -1.5, 0, -0.5, 0),
    intersectionSegment('left-vertical', 'lv1', 'lv2', -1, -1, -1, 1),
    intersectionSegment('right-horizontal', 'rh1', 'rh2', 1.5, 0, 2.5, 0),
    intersectionSegment('right-vertical', 'rv1', 'rv2', 2, -1, 2, 1),
  ]
  const index = createIntersectionSnapIndex(segments)
  const common = {
    point: { x: 0, y: 0 },
    scale: 1,
    thresholdPx: 3,
  }
  const unfiltered = index.query(common)
  const acceptAll = index.query({ ...common, accept: () => true })
  const paperInteriorOnly = index.query({
    ...common,
    accept: (target) => target.point.x > 0,
  })

  assert.equal(unfiltered.target?.point.x, -1)
  assert.deepEqual(acceptAll, unfiltered)
  assert.equal(paperInteriorOnly.target?.point.x, 2)
  assert.equal(paperInteriorOnly.truncated, false)
  assert.deepEqual(
    index.query({ ...common, accept: () => false }),
    { target: null, candidateSegmentCount: 4, testedPairCount: 6, truncated: false },
  )
})

test('T junctions, endpoints, shared vertices, parallel lines, and overlaps are excluded', () => {
  const excludedPairs: readonly (readonly [IntersectionSnapSegment, IntersectionSnapSegment])[] = [
    [
      intersectionSegment('t-base', 'a', 'b', 0, 0, 10, 0),
      intersectionSegment('t-stem', 'c', 'd', 5, 0, 5, 5),
    ],
    [
      intersectionSegment('endpoint-a', 'a', 'b', 0, 0, 5, 0),
      intersectionSegment('endpoint-b', 'c', 'd', 5, 0, 5, 5),
    ],
    [
      intersectionSegment('shared-a', 'a', 'shared', 0, 0, 5, 0),
      intersectionSegment('shared-b', 'shared', 'b', 5, 0, 5, 5),
    ],
    [
      intersectionSegment('parallel-a', 'a', 'b', 0, 0, 10, 0),
      intersectionSegment('parallel-b', 'c', 'd', 0, 1, 10, 1),
    ],
    [
      intersectionSegment('overlap-a', 'a', 'b', 0, 0, 10, 0),
      intersectionSegment('overlap-b', 'c', 'd', 4, 0, 12, 0),
    ],
  ]

  for (const pair of excludedPairs) {
    assert.equal(queryIntersection(pair, { x: 5, y: 0 }, 20).target, null)
  }
})

test('invalid, zero-length, duplicate-ID, and overflowing segments are safely ignored', () => {
  const filtered = createIntersectionSnapIndex([
    intersectionSegment('zero', 'a', 'b', 1, 1, 1, 1),
    intersectionSegment('same-vertex', 'a', 'a', 0, 0, 1, 1),
    intersectionSegment('nonfinite', 'a', 'b', 0, 0, Number.NaN, 1),
    intersectionSegment('duplicate', 'a', 'b', 0, 0, 1, 1),
    intersectionSegment('duplicate', 'c', 'd', 0, 1, 1, 0),
  ])
  assert.equal(filtered.segmentCount, 0)

  const overflow = queryIntersection([
    intersectionSegment('huge-horizontal', 'a', 'b', -1e200, 0, 1e200, 0),
    intersectionSegment('huge-vertical', 'c', 'd', 0, -1e200, 0, 1e200),
  ], { x: 0, y: 0 }, 1)
  assert.equal(overflow.target, null)
  assert.equal(overflow.truncated, false)
})

test('large finite coordinates retain a proper strict-interior intersection', () => {
  const magnitude = 1e150
  const result = queryIntersection([
    intersectionSegment('horizontal', 'a', 'b', -magnitude, 0, magnitude, 0),
    intersectionSegment('vertical', 'c', 'd', 0, -magnitude, 0, magnitude),
  ], { x: 0, y: 0 }, 1)

  assert.deepEqual(result.target?.point, { x: 0, y: 0 })
  assert.deepEqual(result.target?.sourceEdges, [
    { id: 'horizontal', fraction: 0.5 },
    { id: 'vertical', fraction: 0.5 },
  ])
})

test('proper intersection distance uses inclusive pixel thresholds and scale', () => {
  const segments = [
    intersectionSegment('horizontal', 'a', 'b', 0, 0, 10, 0),
    intersectionSegment('vertical', 'c', 'd', 5, -5, 5, 5),
  ]
  const index = createIntersectionSnapIndex(segments)
  const atLimit = index.query({
    point: { x: 5, y: 3 },
    scale: 2,
    thresholdPx: 6,
  })
  assert.equal(atLimit.target?.distancePx, 6)
  assert.equal(index.query({
    point: { x: 5, y: 3 },
    scale: 2,
    thresholdPx: 5.999,
  }).target, null)
  assert.equal(index.query({
    point: { x: 5, y: 3 },
    scale: 0,
  }).target, null)
})

test('dense local geometry stops at the configured pair-test limit', () => {
  const dense = Array.from({ length: 200 }, (_, index) =>
    intersectionSegment(`edge-${String(index).padStart(3, '0')}`, `a-${index}`, `b-${index}`, -10, 0, 10, 0))
  const result = createIntersectionSnapIndex(dense).query({
    point: { x: 0, y: 0 },
    scale: 1,
    thresholdPx: 1,
    maxPairTests: 17,
  })

  assert.equal(result.candidateSegmentCount, 200)
  assert.equal(result.testedPairCount, 17)
  assert.equal(result.truncated, true)
  assert.equal(result.target, null)
})

test('truncated searches never expose a provisional target regardless of accept', () => {
  const crowded = [
    intersectionSegment('a-horizontal', 'a1', 'a2', -10, 0, 10, 0),
    intersectionSegment('b-vertical', 'b1', 'b2', 0, -10, 0, 10),
    ...Array.from({ length: 20 }, (_, index) =>
      intersectionSegment(`z-${String(index).padStart(2, '0')}`, `z1-${index}`, `z2-${index}`, -10, 0, 10, 0)),
  ]
  const index = createIntersectionSnapIndex(crowded)

  for (const scale of [1, Number.MIN_VALUE]) {
    for (const accept of [undefined, () => true, () => false]) {
      const result = index.query({
        point: { x: 0, y: 0 },
        scale,
        thresholdPx: 1,
        maxPairTests: 1,
        accept,
      })
      assert.equal(result.testedPairCount, 1)
      assert.equal(result.truncated, true)
      assert.equal(result.target, null)
    }
  }
})

test('10,000 sparse segments use local AABB queries within a bounded time', () => {
  const segments: IntersectionSnapSegment[] = []
  for (let index = 0; index < 5_000; index += 1) {
    const x = index * 10
    const id = String(index).padStart(4, '0')
    segments.push(
      intersectionSegment(`h-${id}`, `hl-${id}`, `hr-${id}`, x - 1, 0, x + 1, 0),
      intersectionSegment(`v-${id}`, `vb-${id}`, `vt-${id}`, x, -1, x, 1),
    )
  }

  const buildStarted = performance.now()
  const index = createIntersectionSnapIndex(segments)
  const buildElapsed = performance.now() - buildStarted
  assert.equal(index.segmentCount, 10_000)
  assert.ok(buildElapsed < 5_000, `10,000-segment index build took ${buildElapsed}ms`)

  const queryStarted = performance.now()
  for (let sample = 0; sample < 250; sample += 1) {
    const sourceIndex = sample * 19
    const result = index.query({
      point: { x: sourceIndex * 10, y: 0 },
      scale: 1,
      thresholdPx: 0.1,
    })
    assert.equal(result.candidateSegmentCount, 2)
    assert.equal(result.testedPairCount, 1)
    assert.equal(result.truncated, false)
    assert.equal(result.target?.classification, 'proper')
  }
  const queryElapsed = performance.now() - queryStarted
  assert.ok(queryElapsed < 2_000, `250 local intersection queries took ${queryElapsed}ms`)
})
