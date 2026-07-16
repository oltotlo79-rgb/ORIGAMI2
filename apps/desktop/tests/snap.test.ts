import assert from 'node:assert/strict'
import { performance } from 'node:perf_hooks'
import test from 'node:test'

import {
  DEFAULT_SNAP_SETTINGS,
  createVisibleGrid,
  prioritizeAdditionSnapTargets,
  resolveUniqueSnapAnchor,
  resolveSnapTarget,
  toggleSnapSetting,
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
    intersection: kinds.includes('intersection'),
    midpoint: kinds.includes('midpoint'),
    horizontal: kinds.includes('horizontal'),
    vertical: kinds.includes('vertical'),
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
    intersection: true,
    midpoint: true,
    horizontal: true,
    vertical: true,
    edge: true,
    grid: true,
  })
  assert.equal(Object.isFrozen(DEFAULT_SNAP_SETTINGS), true)
})

test('intersection snapping is independently toggleable and defaults on', () => {
  const disabled = toggleSnapSetting(DEFAULT_SNAP_SETTINGS, 'intersection')
  assert.equal(DEFAULT_SNAP_SETTINGS.intersection, true)
  assert.equal(disabled.intersection, false)
  assert.deepEqual(toggleSnapSetting(disabled, 'intersection'), DEFAULT_SNAP_SETTINGS)
  for (const kind of ['vertex', 'midpoint', 'horizontal', 'vertical', 'edge', 'grid'] as const) {
    assert.equal(disabled[kind], true)
  }
})

test('horizontal and vertical snapping are independently toggleable and default on', () => {
  const horizontalOff = toggleSnapSetting(DEFAULT_SNAP_SETTINGS, 'horizontal')
  const verticalOff = toggleSnapSetting(DEFAULT_SNAP_SETTINGS, 'vertical')

  assert.equal(horizontalOff.horizontal, false)
  assert.equal(horizontalOff.vertical, true)
  assert.equal(verticalOff.horizontal, true)
  assert.equal(verticalOff.vertical, false)
  assert.deepEqual(toggleSnapSetting(horizontalOff, 'horizontal'), DEFAULT_SNAP_SETTINGS)
  assert.deepEqual(toggleSnapSetting(verticalOff, 'vertical'), DEFAULT_SNAP_SETTINGS)
})

test('a drawing anchor requires one finite vertex record with the selected ID', () => {
  const selected = { id: 'selected', x: -0, y: 12 }
  assert.deepEqual(resolveUniqueSnapAnchor([selected], selected.id), selected)
  assert.equal(resolveUniqueSnapAnchor([selected], null), undefined)
  assert.equal(resolveUniqueSnapAnchor([selected], 'missing'), undefined)
  assert.equal(resolveUniqueSnapAnchor([
    selected,
    { ...selected },
  ], selected.id), undefined)
  assert.equal(resolveUniqueSnapAnchor([
    { ...selected, x: Number.NaN },
  ], selected.id), undefined)
})

test('kind priority is vertex, midpoint, direction, edge, then grid', () => {
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
    anchor: { id: 'anchor', x: 1, y: 1 },
  }

  assert.equal(resolve({ ...common })?.kind, 'vertex')
  assert.equal(resolve({
    ...common,
    settings: only('midpoint', 'horizontal', 'vertical', 'edge', 'grid'),
  })?.kind, 'midpoint')
  assert.equal(resolve({
    ...common,
    settings: only('horizontal', 'vertical', 'edge', 'grid'),
  })?.kind, 'horizontal')
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

test('direction targets carry a deterministic anchor guide and preserve the free coordinate', () => {
  const anchor = { id: 'anchor:one', x: 1, y: 5 }
  const horizontal = resolve({
    point: { x: 4, y: 7 },
    settings: only('horizontal', 'vertical'),
    anchor,
  })
  assert.deepEqual(horizontal, {
    key: 'horizontal:"anchor:one"',
    kind: 'horizontal',
    point: { x: 4, y: 5 },
    distancePx: 2,
    sourceId: 'anchor:one',
    anchorId: 'anchor:one',
    anchorPoint: { x: 1, y: 5 },
  })

  const vertical = resolve({
    point: { x: 3, y: 10 },
    settings: only('vertical'),
    anchor,
  })
  assert.deepEqual(vertical, {
    key: 'vertical:"anchor:one"',
    kind: 'vertical',
    point: { x: 1, y: 10 },
    distancePx: 2,
    sourceId: 'anchor:one',
    anchorId: 'anchor:one',
    anchorPoint: { x: 1, y: 5 },
  })
})

test('equal direction corrections prefer horizontal deterministically', () => {
  const common = {
    point: { x: 4, y: 8 },
    anchor: { id: 'anchor', x: 1, y: 5 },
  }

  assert.equal(resolve({
    ...common,
    settings: only('horizontal', 'vertical'),
  })?.kind, 'horizontal')
  assert.equal(resolve({
    ...common,
    settings: only('vertical'),
  })?.kind, 'vertical')
})

test('horizontal and vertical use independent inclusive pixel thresholds', () => {
  const anchor = { id: 'anchor', x: 4, y: 4 }
  assert.equal(resolve({
    point: { x: 0, y: 0 },
    scale: 2,
    settings: only('horizontal'),
    anchor,
  })?.distancePx, 8)
  assert.equal(resolve({
    point: { x: 0, y: 0 },
    scale: 2,
    settings: only('horizontal'),
    anchor,
    thresholdsPx: { horizontal: 7.999 },
  }), null)
  assert.equal(resolve({
    point: { x: 0, y: 0 },
    scale: 2,
    settings: only('vertical'),
    anchor,
    thresholdsPx: { vertical: 8 },
  })?.distancePx, 8)
  assert.equal(resolve({
    point: { x: 0, y: 0 },
    scale: 2,
    settings: only('vertical'),
    anchor,
    thresholdsPx: { vertical: 7.999 },
  }), null)
})

test('direction accept filters fall back to the other axis and then lower priorities', () => {
  const common = {
    point: { x: 1, y: 2 },
    anchor: { id: 'anchor', x: 0, y: 0 },
    settings: only('horizontal', 'vertical', 'edge'),
    segments: [{
      id: 'edge',
      startVertexId: 'left',
      endVertexId: 'right',
      x1: -5,
      y1: 0,
      x2: 5,
      y2: 0,
    }],
  }

  assert.equal(resolve(common)?.kind, 'vertical')
  assert.equal(resolve({
    ...common,
    accept: (target) => target.kind !== 'vertical',
  })?.kind, 'horizontal')
  assert.equal(resolve({
    ...common,
    accept: (target) => target.kind !== 'horizontal' && target.kind !== 'vertical',
  })?.kind, 'edge')
})

test('explicit drag-origin anchors remain valid when their vertex is excluded', () => {
  const target = resolve({
    point: { x: 4, y: 1 },
    settings: only('horizontal'),
    anchor: { id: 'moving', x: 0, y: 0 },
    excludedVertexId: 'moving',
    vertices: [{ id: 'moving', x: 0, y: 0 }],
  })

  assert.equal(target?.kind, 'horizontal')
  assert.deepEqual(target?.point, { x: 4, y: 0 })
  assert.equal(target?.anchorId, 'moving')
})

test('missing, empty, non-finite, and overflowing direction anchors are ignored', () => {
  const common = { settings: only('horizontal', 'vertical') }
  assert.equal(resolve(common), null)
  assert.equal(resolve({ ...common, anchor: { id: '', x: 0, y: 0 } }), null)
  assert.equal(resolve({
    ...common,
    anchor: { id: 42 as never, x: 0, y: 0 },
  }), null)
  assert.equal(resolve({
    ...common,
    anchor: { id: 'nan', x: Number.NaN, y: 0 },
  }), null)
  assert.equal(resolve({
    point: { x: -Number.MAX_VALUE, y: 0 },
    settings: only('vertical'),
    anchor: { id: 'overflow', x: Number.MAX_VALUE, y: 0 },
  }), null)
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

test('direction-only snapping stays constant-time with 10,000 unrelated vertices', () => {
  const vertices = Array.from({ length: 10_000 }, (_, index) => ({
    id: `v${index}`,
    x: index,
    y: -index,
  }))
  const options = {
    point: { x: 2, y: 3 },
    scale: 1,
    settings: only('horizontal', 'vertical'),
    vertices,
    segments: [],
    grid: EMPTY_GRID,
    anchor: { id: 'anchor', x: 0, y: 0 },
  } satisfies ResolveSnapTargetOptions

  const started = performance.now()
  for (let query = 0; query < 10_000; query += 1) {
    assert.equal(resolveSnapTarget(options)?.kind, 'vertical')
  }
  const elapsed = performance.now() - started
  assert.ok(elapsed < 2_000, `10,000 direction queries took ${elapsed}ms`)
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

  const horizontal = resolve({
    point: { x: 12, y: 42 },
    settings: only('horizontal'),
    anchor: { id: 'anchor', x: 5, y: 40 },
  })
  assert.deepEqual(createVertexPlacement(
    horizontal?.point ?? { x: Number.NaN, y: Number.NaN },
    horizontal,
    [],
  ), {
    operation: 'add',
    x: 12,
    y: 40,
  })
})

test('direction snaps split one coincident edge instead of adding an overlapping vertex', () => {
  const forward = {
    id: 'boundary-edge',
    startVertexId: 'left',
    endVertexId: 'right',
    x1: 0,
    y1: 0,
    x2: 100,
    y2: 0,
  }
  const target = resolve({
    point: { x: 25, y: 2 },
    settings: only('horizontal'),
    anchor: { id: 'anchor', x: 0, y: 0 },
    segments: [forward],
  })
  assert.equal(target?.kind, 'horizontal')
  assert.deepEqual(createVertexPlacement(target?.point ?? { x: 25, y: 0 }, target, [forward]), {
    operation: 'split-edge',
    edgeId: forward.id,
    fraction: 0.25,
  })

  const reversed = {
    ...forward,
    id: 'reversed-edge',
    startVertexId: forward.endVertexId,
    endVertexId: forward.startVertexId,
    x1: forward.x2,
    x2: forward.x1,
  }
  assert.deepEqual(createVertexPlacement(target?.point ?? { x: 25, y: 0 }, target, [reversed]), {
    operation: 'split-edge',
    edgeId: reversed.id,
    fraction: 0.75,
  })

  const verticalEdge = {
    id: 'vertical-edge',
    startVertexId: 'top',
    endVertexId: 'bottom',
    x1: 0,
    y1: 0,
    x2: 0,
    y2: 100,
  }
  const verticalTarget = resolve({
    point: { x: 2, y: 25 },
    settings: only('vertical'),
    anchor: { id: 'anchor', x: 0, y: 0 },
  })
  assert.ok(verticalTarget?.kind === 'vertical')
  assert.deepEqual(createVertexPlacement(verticalTarget.point, verticalTarget, [verticalEdge]), {
    operation: 'split-edge',
    edgeId: verticalEdge.id,
    fraction: 0.25,
  })
})

test('direction placement rejects endpoint, multi-edge, duplicate-ID, and malformed ambiguity', () => {
  const base = {
    id: 'base',
    startVertexId: 'left',
    endVertexId: 'right',
    x1: 0,
    y1: 0,
    x2: 100,
    y2: 0,
  }
  const target = resolve({
    point: { x: 25, y: 2 },
    settings: only('horizontal'),
    anchor: { id: 'anchor', x: 0, y: 0 },
  })
  assert.ok(target?.kind === 'horizontal')
  assert.equal(createVertexPlacement(target.point, target, [
    base,
    { ...base, id: 'overlap', startVertexId: 'other-left', endVertexId: 'other-right' },
  ]), null)
  assert.equal(createVertexPlacement(target.point, target, [
    base,
    { ...base, x1: 200, x2: 300, startVertexId: 'far-left', endVertexId: 'far-right' },
  ]), null)
  assert.equal(createVertexPlacement(target.point, {
    ...target,
    anchorId: 'wrong-anchor',
  }, [base]), null)

  const endpointTarget = resolve({
    point: { x: 0, y: 2 },
    settings: only('horizontal'),
    anchor: { id: 'anchor', x: 0, y: 0 },
  })
  assert.ok(endpointTarget?.kind === 'horizontal')
  assert.equal(createVertexPlacement(endpointTarget.point, endpointTarget, [base]), null)
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

test('T junctions expose the exact existing endpoint in every orientation', () => {
  const cases = [
    {
      branch: intersectionSegment('a-branch', 'junction-start-a', 'tip-a', 5, 0, 5, 5),
      base: intersectionSegment('m-base', 'left-a', 'right-a', 0, 0, 10, 0),
      junctionVertexId: 'junction-start-a',
      sourceEdges: [
        { id: 'a-branch', fraction: 0 },
        { id: 'm-base', fraction: 0.5 },
      ],
    },
    {
      branch: intersectionSegment('a-branch', 'tip-b', 'junction-end-a', 5, 5, 5, 0),
      base: intersectionSegment('m-base', 'right-b', 'left-b', 10, 0, 0, 0),
      junctionVertexId: 'junction-end-a',
      sourceEdges: [
        { id: 'a-branch', fraction: 1 },
        { id: 'm-base', fraction: 0.5 },
      ],
    },
    {
      branch: intersectionSegment('z-branch', 'junction-start-z', 'tip-c', 5, 0, 5, -5),
      base: intersectionSegment('m-base', 'right-c', 'left-c', 10, 0, 0, 0),
      junctionVertexId: 'junction-start-z',
      sourceEdges: [
        { id: 'm-base', fraction: 0.5 },
        { id: 'z-branch', fraction: 0 },
      ],
    },
    {
      branch: intersectionSegment('z-branch', 'tip-d', 'junction-end-z', 5, -5, 5, 0),
      base: intersectionSegment('m-base', 'left-d', 'right-d', 0, 0, 10, 0),
      junctionVertexId: 'junction-end-z',
      sourceEdges: [
        { id: 'm-base', fraction: 0.5 },
        { id: 'z-branch', fraction: 1 },
      ],
    },
  ] as const

  for (const entry of cases) {
    const forward = queryIntersection([entry.branch, entry.base], { x: 5, y: 0 })
    const reversedInput = queryIntersection([entry.base, entry.branch], { x: 5, y: 0 })
    assert.deepEqual(reversedInput, forward)
    assert.deepEqual(forward.target, {
      kind: 'intersection',
      classification: 't-junction',
      key: `intersection:${JSON.stringify([
        entry.sourceEdges[0].id,
        entry.sourceEdges[1].id,
      ])}`,
      point: { x: 5, y: 0 },
      distancePx: 0,
      sourceEdges: entry.sourceEdges,
      junctionVertexId: entry.junctionVertexId,
    })
  }
})

test('addition snap priority is T repair, vertex, proper, midpoint, direction, edge, then grid', () => {
  const crossing = [
    intersectionSegment('a-edge', 'a1', 'a2', 0, -2, 0, 2),
    intersectionSegment('b-edge', 'b1', 'b2', -2, 0, 2, 0),
  ]
  const proper = queryIntersection(crossing).target
  assert.equal(proper?.classification, 'proper')
  assert.ok(proper)
  const tJunctionSegments = [
    intersectionSegment('c-base', 'left', 'right', -2, 0, 2, 0),
    intersectionSegment('d-branch', 'junction', 'tip', 0, 0, 0, 2),
  ]
  const tJunction = queryIntersection(tJunctionSegments).target
  assert.equal(tJunction?.classification, 't-junction')
  assert.ok(tJunction)
  const vertex = resolve({
    settings: only('vertex'),
    vertices: [{ id: 'vertex', x: 0, y: 0 }],
  })
  const junctionVertex = resolve({
    settings: only('vertex'),
    vertices: [{ id: 'junction', x: 0, y: 0 }],
  })
  const midpoint = resolve({
    settings: only('midpoint'),
    segments: [{
      id: 'midpoint',
      startVertexId: 'm1',
      endVertexId: 'm2',
      x1: -1,
      y1: 0,
      x2: 1,
      y2: 0,
    }],
  })
  const edge = resolve({
    settings: only('edge'),
    point: { x: 0, y: 1 },
    segments: [{
      id: 'edge',
      startVertexId: 'e1',
      endVertexId: 'e2',
      x1: -1,
      y1: 0,
      x2: 1,
      y2: 0,
    }],
  })
  const grid = resolve({
    settings: only('grid'),
    grid: { xValues: [0], yValues: [0] },
  })
  const direction = resolve({
    point: { x: 0, y: 1 },
    settings: only('horizontal'),
    anchor: { id: 'anchor', x: 2, y: 0 },
  })

  assert.equal(
    prioritizeAdditionSnapTargets(junctionVertex, tJunction)?.classification,
    't-junction',
  )
  assert.equal(prioritizeAdditionSnapTargets(vertex, tJunction), vertex)
  assert.equal(prioritizeAdditionSnapTargets(vertex, proper), vertex)
  for (const lowerPriority of [midpoint, direction, edge, grid]) {
    assert.equal(
      prioritizeAdditionSnapTargets(lowerPriority, proper)?.kind,
      'intersection',
    )
    assert.equal(prioritizeAdditionSnapTargets(lowerPriority, null), lowerPriority)
  }
  assert.equal(prioritizeAdditionSnapTargets(null, proper), proper)
  assert.equal(prioritizeAdditionSnapTargets(null, null), null)
})

test('proper intersection placement carries only validated canonical edge IDs', () => {
  const segments = [
    intersectionSegment('a-edge', 'a1', 'a2', 0, -2, 0, 2),
    intersectionSegment('z-edge', 'z1', 'z2', -2, 0, 2, 0),
  ]
  const target = queryIntersection(segments).target
  assert.ok(target)
  assert.deepEqual(createVertexPlacement(target.point, target, segments), {
    operation: 'connect-intersection',
    firstEdgeId: 'a-edge',
    secondEdgeId: 'z-edge',
  })

  const invalidTargets = [
    { ...target, sourceEdges: [target.sourceEdges[1], target.sourceEdges[0]] },
    {
      ...target,
      sourceEdges: [target.sourceEdges[0], { ...target.sourceEdges[1], id: 'a-edge' }],
    },
    {
      ...target,
      sourceEdges: [{ ...target.sourceEdges[0], fraction: 0 }, target.sourceEdges[1]],
    },
    { ...target, point: { x: Number.NaN, y: 0 } },
  ]
  for (const invalid of invalidTargets) {
    assert.equal(createVertexPlacement(invalid.point, invalid, segments), null)
  }
  assert.equal(createVertexPlacement({ x: 1, y: 1 }, target, segments), null)
  assert.equal(createVertexPlacement(target.point, target, segments.slice(0, 1)), null)

  const sharedVertexSegments = [
    { ...segments[0], endVertexId: 'shared' },
    { ...segments[1], startVertexId: 'shared' },
  ]
  assert.equal(createVertexPlacement(target.point, target, sharedVertexSegments), null)
})

test('T-junction placement validates the junction vertex and both canonical edge IDs', () => {
  const segments = [
    intersectionSegment('a-base', 'left', 'right', 0, 0, 10, 0),
    intersectionSegment('z-branch', 'junction', 'tip', 5, 0, 5, 5),
  ]
  const target = queryIntersection(segments, { x: 5, y: 0 }).target
  assert.equal(target?.classification, 't-junction')
  assert.ok(target && target.classification === 't-junction')
  assert.deepEqual(createVertexPlacement(target.point, target, segments), {
    operation: 'connect-t-junction',
    firstEdgeId: 'a-base',
    secondEdgeId: 'z-branch',
    junctionVertexId: 'junction',
  })

  const malformed = [
    { ...target, junctionVertexId: 'tip' },
    { ...target, key: 'intersection:["wrong","key"]' },
    { ...target, distancePx: Number.NaN },
    { ...target, sourceEdges: [target.sourceEdges[1], target.sourceEdges[0]] },
    {
      ...target,
      sourceEdges: [
        target.sourceEdges[0],
        { ...target.sourceEdges[1], fraction: 0.25 },
      ],
    },
    {
      ...target,
      sourceEdges: [
        { ...target.sourceEdges[0], fraction: 0 },
        target.sourceEdges[1],
      ],
    },
    { ...target, point: { x: 5, y: 0.25 } },
  ]
  for (const invalid of malformed) {
    assert.equal(createVertexPlacement(invalid.point, invalid, segments), null)
  }
  assert.equal(createVertexPlacement(
    target.point,
    { ...target, classification: 'unknown' } as never,
    segments,
  ), null)
  assert.equal(createVertexPlacement(target.point, target, segments.slice(0, 1)), null)

  const alreadyConnected = [
    { ...segments[0], endVertexId: 'junction' },
    segments[1],
  ]
  assert.equal(createVertexPlacement(target.point, target, alreadyConnected), null)

  const duplicatePosition = [
    ...segments,
    intersectionSegment('other', 'different-id', 'other-tip', 5, 0, 7, 2),
  ]
  assert.equal(createVertexPlacement(target.point, target, duplicatePosition), null)

  const inconsistentJunction = [
    ...segments,
    intersectionSegment('other', 'junction', 'other-tip', 6, 0, 7, 2),
  ]
  assert.equal(createVertexPlacement(target.point, target, inconsistentJunction), null)
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

test('equal-distance T repairs outrank proper intersections deterministically', () => {
  const segments = [
    intersectionSegment('p-horizontal', 'h1', 'h2', -2, 0, 2, 0),
    intersectionSegment('p-vertical', 'v1', 'v2', 0, -2, 0, 2),
    intersectionSegment('a-branch', 'junction', 'tip', 0, 0, 1, 1),
  ]
  const forward = queryIntersection(segments)
  const reversed = queryIntersection([...segments].reverse())

  assert.equal(forward.target?.classification, 't-junction')
  assert.equal(forward.target?.key, 'intersection:["a-branch","p-horizontal"]')
  assert.deepEqual(reversed, forward)
})

test('intersection accept filters choose the next ranked candidate deterministically', () => {
  const segments = [
    intersectionSegment('left-horizontal', 'lh1', 'lh2', -1.5, 0, -0.5, 0),
    intersectionSegment('left-vertical', 'left-junction', 'lv2', -1, 0, -1, 1),
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
  assert.equal(unfiltered.target?.classification, 't-junction')
  assert.deepEqual(acceptAll, unfiltered)
  assert.equal(paperInteriorOnly.target?.point.x, 2)
  assert.equal(paperInteriorOnly.target?.classification, 'proper')
  assert.equal(paperInteriorOnly.truncated, false)
  assert.deepEqual(
    index.query({ ...common, accept: () => false }),
    { target: null, candidateSegmentCount: 4, testedPairCount: 6, truncated: false },
  )
})

test('endpoint-endpoint, shared vertices, parallel lines, and overlaps are excluded', () => {
  const excludedPairs: readonly (readonly [IntersectionSnapSegment, IntersectionSnapSegment])[] = [
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

test('already-connected T topology and same-position vertex ambiguity are excluded', () => {
  const alreadyConnected = [
    intersectionSegment('left', 'left-tip', 'junction', 0, 0, 5, 0),
    intersectionSegment('right', 'junction', 'right-tip', 5, 0, 10, 0),
    intersectionSegment('branch', 'junction', 'branch-tip', 5, 0, 5, 5),
  ]
  assert.equal(queryIntersection(alreadyConnected, { x: 5, y: 0 }).target, null)

  const ambiguousEndpoints = [
    intersectionSegment('base', 'base-left', 'base-right', 0, 0, 10, 0),
    intersectionSegment('branch-a', 'junction-a', 'tip-a', 5, 0, 5, 5),
    intersectionSegment('branch-b', 'junction-b', 'tip-b', 5, 0, 6, -5),
  ]
  assert.equal(queryIntersection(ambiguousEndpoints, { x: 5, y: 0 }).target, null)

  const base = intersectionSegment('base', 'base-left', 'base-right', 0, 0, 10, 0)
  const branch = intersectionSegment('branch', 'junction', 'tip', 5, 0, 5, 5)
  const isolatedAmbiguity = createIntersectionSnapIndex(
    [base, branch],
    [
      { id: 'junction', x: 5, y: 0 },
      { id: 'isolated-duplicate', x: 5, y: 0 },
    ],
  ).query({ point: { x: 5, y: 0 }, scale: 1 })
  assert.equal(isolatedAmbiguity.target, null)
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
