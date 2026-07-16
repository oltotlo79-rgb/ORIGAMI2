import assert from 'node:assert/strict'
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
