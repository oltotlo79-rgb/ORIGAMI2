import assert from 'node:assert/strict'
import test from 'node:test'

import {
  createPointSpatialIndex,
  createSegmentSpatialIndex,
} from '../src/lib/nearestSpatialIndex.ts'
import {
  DEFAULT_SNAP_SETTINGS,
  createSnapSpatialIndex,
  resolveSnapTarget,
  type ResolveSnapTargetOptions,
  type SnapSegment,
  type SnapVertex,
} from '../src/lib/snap.ts'

test('empty indexes and invalid queries return an empty deterministic result', () => {
  const points = createPointSpatialIndex([])
  const segments = createSegmentSpatialIndex([])
  assert.equal(points.size, 0)
  assert.equal(segments.size, 0)
  assert.deepEqual(points.searchNearest({ point: { x: 0, y: 0 }, radius: 1 }), {
    match: null,
    visitedNodes: 0,
    testedEntries: 0,
  })
  assert.deepEqual(segments.withinRadius({ point: { x: 0, y: 0 }, radius: 1 }), {
    matches: [],
    visitedNodes: 0,
    testedEntries: 0,
  })

  const populated = createPointSpatialIndex([
    { key: 'origin', x: 0, y: 0, value: 'origin' },
  ])
  for (const query of [
    { point: { x: Number.NaN, y: 0 }, radius: 1 },
    { point: { x: 0, y: Number.POSITIVE_INFINITY }, radius: 1 },
    { point: { x: 0, y: 0 }, radius: Number.NaN },
    { point: { x: 0, y: 0 }, radius: Number.POSITIVE_INFINITY },
    { point: { x: 0, y: 0 }, radius: -1 },
  ]) {
    assert.equal(populated.nearest(query), null)
    assert.equal(populated.searchNearest(query).testedEntries, 0)
  }
})

test('non-finite records are omitted without changing valid source indices', () => {
  const points = createPointSpatialIndex([
    { key: 'nan', x: Number.NaN, y: 0, value: 'nan' },
    { key: 'valid', x: 2, y: 3, value: 'valid' },
    { key: 'infinity', x: 0, y: Number.NEGATIVE_INFINITY, value: 'infinity' },
  ])
  assert.equal(points.size, 1)
  assert.deepEqual(points.nearest({ point: { x: 2, y: 3 }, radius: 0 }), {
    key: 'valid',
    value: 'valid',
    sourceIndex: 1,
    point: { x: 2, y: 3 },
    distance: 0,
  })

  const segments = createSegmentSpatialIndex([
    { key: 'nan', x1: 0, y1: 0, x2: Number.NaN, y2: 0, value: 'nan' },
    { key: 'valid', x1: 0, y1: 0, x2: 4, y2: 0, value: 'valid' },
    {
      key: 'infinity',
      x1: Number.POSITIVE_INFINITY,
      y1: 0,
      x2: 1,
      y2: 1,
      value: 'infinity',
    },
  ])
  assert.equal(segments.size, 1)
  assert.equal(segments.nearest({ point: { x: 2, y: 1 }, radius: 1 })?.value, 'valid')
})

test('point range boundaries are explicit and zero-radius inclusive queries work', () => {
  const index = createPointSpatialIndex([
    { key: 'origin', x: 0, y: 0, value: 'origin' },
    { key: 'boundary', x: 3, y: 4, value: 'boundary' },
    { key: 'outside', x: 3, y: 4.000_000_001, value: 'outside' },
  ])
  assert.deepEqual(
    index.withinRadius({ point: { x: 0, y: 0 }, radius: 5 }).matches.map(({ value }) => value),
    ['origin', 'boundary'],
  )
  assert.deepEqual(
    index.withinRadius({
      point: { x: 0, y: 0 },
      radius: 5,
      boundary: 'exclusive',
    }).matches.map(({ value }) => value),
    ['origin'],
  )
  assert.equal(index.nearest({ point: { x: 0, y: 0 }, radius: 0 })?.value, 'origin')
  assert.equal(index.nearest({
    point: { x: 0, y: 0 },
    radius: 0,
    boundary: 'exclusive',
  }), null)
})

test('equal-distance ties support source order and lexical key authority', () => {
  const index = createPointSpatialIndex([
    { key: 'z-last-key', x: -1, y: 0, value: 'first-source' },
    { key: 'a-first-key', x: 1, y: 0, value: 'second-source' },
    { key: 'a-first-key', x: 0, y: 1, value: 'duplicate-key-later' },
  ])
  const query = { point: { x: 0, y: 0 }, radius: 1 }
  assert.equal(index.nearest(query)?.value, 'first-source')
  assert.equal(index.nearest({ ...query, tieBreak: 'source-order' })?.value, 'first-source')
  assert.equal(index.nearest({ ...query, tieBreak: 'key' })?.value, 'second-source')
  assert.equal(index.nearest({
    ...query,
    tieBreak: 'key',
    accept: ({ value }) => value !== 'second-source',
  })?.value, 'duplicate-key-later')
})

test('segment nearest returns a clamped projection and stable fraction', () => {
  const index = createSegmentSpatialIndex([
    { key: 'horizontal', x1: 0, y1: 0, x2: 10, y2: 0, value: 'horizontal' },
    { key: 'point', x1: 20, y1: 5, x2: 20, y2: 5, value: 'point' },
  ])
  assert.deepEqual(index.nearest({ point: { x: 2.5, y: 3 }, radius: 3 }), {
    key: 'horizontal',
    value: 'horizontal',
    sourceIndex: 0,
    point: { x: 2.5, y: 0 },
    distance: 3,
    fraction: 0.25,
  })
  assert.deepEqual(index.nearest({ point: { x: -2, y: 0 }, radius: 2 }), {
    key: 'horizontal',
    value: 'horizontal',
    sourceIndex: 0,
    point: { x: 0, y: 0 },
    distance: 2,
    fraction: 0,
  })
  assert.equal(index.nearest({
    point: { x: -2, y: 0 },
    radius: 2,
    boundary: 'exclusive',
  }), null)
  assert.deepEqual(index.nearest({ point: { x: 20, y: 5 }, radius: 0 }), {
    key: 'point',
    value: 'point',
    sourceIndex: 1,
    point: { x: 20, y: 5 },
    distance: 0,
    fraction: 0,
  })
})

test('segment ties and accept fallback remain deterministic across BVH branches', () => {
  const index = createSegmentSpatialIndex([
    { key: 'z', x1: -10, y1: -1, x2: 10, y2: -1, value: 'first' },
    { key: 'a', x1: -10, y1: 1, x2: 10, y2: 1, value: 'second' },
    ...Array.from({ length: 20 }, (_, index) => ({
      key: `far-${String(index).padStart(2, '0')}`,
      x1: 100 + index * 10,
      y1: 100,
      x2: 105 + index * 10,
      y2: 100,
      value: `far-${index}`,
    })),
  ])
  const query = { point: { x: 0, y: 0 }, radius: 2 }
  assert.equal(index.nearest(query)?.value, 'first')
  assert.equal(index.nearest({ ...query, tieBreak: 'key' })?.value, 'second')
  assert.equal(index.nearest({
    ...query,
    tieBreak: 'key',
    accept: ({ value }) => value !== 'second',
  })?.value, 'first')
})

test('range results preserve original source order rather than tree order', () => {
  const records = [
    { key: 'third-coordinate', x: 30, y: 0, value: 0 },
    { key: 'first-coordinate', x: 10, y: 0, value: 1 },
    { key: 'second-coordinate', x: 20, y: 0, value: 2 },
  ]
  const snapshot = structuredClone(records)
  const index = createPointSpatialIndex(records)
  assert.deepEqual(records, snapshot, 'building must not mutate the caller array')
  assert.deepEqual(
    index.withinRadius({ point: { x: 20, y: 0 }, radius: 20 }).matches
      .map(({ sourceIndex }) => sourceIndex),
    [0, 1, 2],
  )
})

test('10,000 sparse points are pruned to a bounded local search', () => {
  const index = createPointSpatialIndex(Array.from({ length: 10_000 }, (_, value) => ({
    key: `point-${String(value).padStart(5, '0')}`,
    x: value * 100,
    y: value % 2,
    value,
  })))
  const result = index.searchNearest({
    point: { x: 543_200.25, y: 0 },
    radius: 1,
  })
  assert.equal(result.match?.value, 5_432)
  assert.ok(result.visitedNodes < 100, `visited ${result.visitedNodes} nodes`)
  assert.ok(result.testedEntries < 32, `tested ${result.testedEntries} entries`)
})

test('10,000 sparse segments are pruned without changing exact boundary results', () => {
  const index = createSegmentSpatialIndex(Array.from({ length: 10_000 }, (_, value) => ({
    key: `segment-${String(value).padStart(5, '0')}`,
    x1: value * 100,
    y1: -10,
    x2: value * 100,
    y2: 10,
    value,
  })))
  const result = index.searchNearest({
    point: { x: 765_400.5, y: 7 },
    radius: 0.5,
  })
  assert.equal(result.match?.value, 7_654)
  assert.equal(result.match?.distance, 0.5)
  assert.ok(result.visitedNodes < 100, `visited ${result.visitedNodes} nodes`)
  assert.ok(result.testedEntries < 32, `tested ${result.testedEntries} entries`)
  assert.equal(index.nearest({
    point: { x: 765_400.5, y: 7 },
    radius: 0.5,
    boundary: 'exclusive',
  }), null)
})

test('point overflow fails closed while finite huge segments use normalized projection', () => {
  const points = createPointSpatialIndex([
    { key: 'extreme', x: Number.MAX_VALUE, y: Number.MAX_VALUE, value: 'extreme' },
  ])
  assert.equal(points.nearest({ point: { x: -Number.MAX_VALUE, y: 0 }, radius: 1 }), null)

  const segments = createSegmentSpatialIndex([
    {
      key: 'overflowing-direction',
      x1: -Number.MAX_VALUE,
      y1: 0,
      x2: Number.MAX_VALUE,
      y2: 0,
      value: 'overflowing-direction',
    },
  ])
  assert.equal(segments.size, 1)
  assert.deepEqual(segments.nearest({ point: { x: 0, y: 0 }, radius: 0 }), {
    key: 'overflowing-direction',
    value: 'overflowing-direction',
    sourceIndex: 0,
    point: { x: 0, y: 0 },
    distance: 0,
    fraction: 0.5,
  })
})

test('snap spatial indexes preserve vertex, midpoint, and edge results exactly', () => {
  const vertices: SnapVertex[] = [
    { id: 'z', x: -5, y: 0 },
    { id: 'a', x: 5, y: 0 },
    { id: 'excluded', x: 0, y: 0 },
  ]
  const segments: SnapSegment[] = [
    {
      id: 'z',
      startVertexId: 'z1',
      endVertexId: 'z2',
      x1: -10,
      y1: -4,
      x2: 10,
      y2: -4,
    },
    {
      id: 'a',
      startVertexId: 'a1',
      endVertexId: 'a2',
      x1: -10,
      y1: 4,
      x2: 10,
      y2: 4,
    },
  ]
  const spatialIndex = createSnapSpatialIndex(vertices, segments)
  const resolveBoth = (overrides: Partial<ResolveSnapTargetOptions>) => {
    const base: ResolveSnapTargetOptions = {
      point: { x: 0, y: 0 },
      scale: 1,
      settings: { ...DEFAULT_SNAP_SETTINGS, intersection: false, grid: false },
      vertices,
      segments,
      grid: { xValues: [], yValues: [] },
      ...overrides,
    }
    return [
      resolveSnapTarget(base),
      resolveSnapTarget({ ...base, spatialIndex }),
    ] as const
  }

  for (const overrides of [
    {
      settings: {
        ...DEFAULT_SNAP_SETTINGS,
        intersection: false,
        midpoint: false,
        horizontal: false,
        vertical: false,
        parallel: false,
        angle: false,
        edge: false,
        grid: false,
      },
      excludedVertexId: 'excluded',
      accept: (target: { sourceId?: string }) => target.sourceId !== 'a',
    },
    {
      vertices: [],
      settings: {
        ...DEFAULT_SNAP_SETTINGS,
        vertex: false,
        intersection: false,
        horizontal: false,
        vertical: false,
        parallel: false,
        angle: false,
        edge: false,
        grid: false,
      },
    },
    {
      point: { x: 0, y: 0 },
      vertices: [],
      settings: {
        ...DEFAULT_SNAP_SETTINGS,
        vertex: false,
        intersection: false,
        midpoint: false,
        horizontal: false,
        vertical: false,
        parallel: false,
        angle: false,
        grid: false,
      },
    },
    {
      point: { x: 1e300, y: 0 },
      scale: Number.MIN_VALUE,
      settings: {
        ...DEFAULT_SNAP_SETTINGS,
        intersection: false,
        midpoint: false,
        horizontal: false,
        vertical: false,
        parallel: false,
        angle: false,
        edge: false,
        grid: false,
      },
    },
  ]) {
    const [unindexed, indexed] = resolveBoth(overrides as Partial<ResolveSnapTargetOptions>)
    assert.deepEqual(indexed, unindexed)
  }
})

test('indexed snap queries do not iterate the full source arrays', () => {
  const vertices: SnapVertex[] = Array.from({ length: 10_000 }, (_, index) => ({
    id: `vertex-${index}`,
    x: index * 100,
    y: 0,
  }))
  const segments: SnapSegment[] = Array.from({ length: 10_000 }, (_, index) => ({
    id: `edge-${index}`,
    startVertexId: `start-${index}`,
    endVertexId: `end-${index}`,
    x1: index * 100,
    y1: -10,
    x2: index * 100,
    y2: 10,
  }))
  const spatialIndex = createSnapSpatialIndex(vertices, segments)
  const originalVertexIterator = vertices[Symbol.iterator]
  const originalSegmentIterator = segments[Symbol.iterator]
  vertices[Symbol.iterator] = function* forbiddenVertexScan() {
    yield* [] as SnapVertex[]
    throw new Error('vertex source array was fully scanned')
  }
  segments[Symbol.iterator] = function* forbiddenSegmentScan() {
    yield* [] as SnapSegment[]
    throw new Error('segment source array was fully scanned')
  }
  try {
    const base = {
      point: { x: 432_100.25, y: 0 },
      scale: 1,
      vertices,
      segments,
      spatialIndex,
      grid: { xValues: [], yValues: [] },
    }
    assert.equal(resolveSnapTarget({
      ...base,
      settings: {
        ...DEFAULT_SNAP_SETTINGS,
        intersection: false,
        midpoint: false,
        horizontal: false,
        vertical: false,
        parallel: false,
        angle: false,
        edge: false,
        grid: false,
      },
    })?.sourceId, 'vertex-4321')
    assert.equal(resolveSnapTarget({
      ...base,
      settings: {
        ...DEFAULT_SNAP_SETTINGS,
        vertex: false,
        intersection: false,
        horizontal: false,
        vertical: false,
        parallel: false,
        angle: false,
        edge: false,
        grid: false,
      },
    })?.sourceId, 'edge-4321')
    assert.equal(resolveSnapTarget({
      ...base,
      settings: {
        ...DEFAULT_SNAP_SETTINGS,
        vertex: false,
        intersection: false,
        midpoint: false,
        horizontal: false,
        vertical: false,
        parallel: false,
        angle: false,
        grid: false,
      },
    })?.sourceId, 'edge-4321')
  } finally {
    vertices[Symbol.iterator] = originalVertexIterator
    segments[Symbol.iterator] = originalSegmentIterator
  }
})

test('30,000 seeded snap queries are identical with and without the spatial index', () => {
  let state = 0x51a7_1a1d
  const random = () => {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0
    return state / 0x1_0000_0000
  }
  const vertices: SnapVertex[] = Array.from({ length: 128 }, (_, index) => ({
    id: index % 31 === 0 ? `duplicate-${index % 2}` : `vertex-${index}`,
    x: (random() - 0.5) * 2_000,
    y: (random() - 0.5) * 2_000,
  }))
  vertices.push(
    { id: 'nan-vertex', x: Number.NaN, y: 0 },
    { id: 'infinite-vertex', x: 0, y: Number.POSITIVE_INFINITY },
  )
  const segments: SnapSegment[] = Array.from({ length: 128 }, (_, index) => {
    const x1 = (random() - 0.5) * 2_000
    const y1 = (random() - 0.5) * 2_000
    const angle = random() * Math.PI * 2
    const length = 5 + random() * 200
    return {
      id: index % 29 === 0 ? `duplicate-edge-${index % 2}` : `edge-${index}`,
      startVertexId: `start-${index}`,
      endVertexId: `end-${index}`,
      x1,
      y1,
      x2: x1 + Math.cos(angle) * length,
      y2: y1 + Math.sin(angle) * length,
    }
  })
  segments.push(
    {
      id: 'zero-length',
      startVertexId: 'zero-start',
      endVertexId: 'zero-end',
      x1: 0,
      y1: 0,
      x2: 0,
      y2: 0,
    },
    {
      id: 'nan-edge',
      startVertexId: 'nan-start',
      endVertexId: 'nan-end',
      x1: Number.NaN,
      y1: 0,
      x2: 1,
      y2: 1,
    },
    {
      id: 'overflow-edge',
      startVertexId: 'overflow-start',
      endVertexId: 'overflow-end',
      x1: -Number.MAX_VALUE,
      y1: 0,
      x2: Number.MAX_VALUE,
      y2: 0,
    },
  )
  const spatialIndex = createSnapSpatialIndex(vertices, segments)
  const disabledSettings = {
    ...DEFAULT_SNAP_SETTINGS,
    vertex: false,
    intersection: false,
    midpoint: false,
    horizontal: false,
    vertical: false,
    parallel: false,
    angle: false,
    edge: false,
    grid: false,
  }
  const idHash = (value: string | undefined) => {
    let hash = 2_166_136_261
    for (const character of value ?? '') {
      hash = Math.imul(hash ^ character.codePointAt(0)!, 16_777_619) >>> 0
    }
    return hash
  }

  for (let sample = 0; sample < 30_000; sample += 1) {
    const kind = (['vertex', 'midpoint', 'edge'] as const)[sample % 3]
    const thresholdPx = sample % 19 === 0 ? 0 : random() * 15
    const scale = sample % 997 === 0
      ? Number.MIN_VALUE
      : sample % 991 === 0
        ? 1e200
        : 0.01 + random() * 100
    let point = {
      x: (random() - 0.5) * 2_400,
      y: (random() - 0.5) * 2_400,
    }
    if (sample % 17 === 0 && Number.isFinite(thresholdPx / scale)) {
      const modelThreshold = thresholdPx / scale
      if (kind === 'vertex') {
        const vertex = vertices[sample % 128]
        point = { x: vertex.x + modelThreshold, y: vertex.y }
      } else {
        const segment = segments[sample % 128]
        const midpoint = {
          x: segment.x1 + (segment.x2 - segment.x1) / 2,
          y: segment.y1 + (segment.y2 - segment.y1) / 2,
        }
        if (kind === 'midpoint') {
          point = { x: midpoint.x + modelThreshold, y: midpoint.y }
        } else {
          const dx = segment.x2 - segment.x1
          const dy = segment.y2 - segment.y1
          const length = Math.hypot(dx, dy)
          if (Number.isFinite(length) && length > 0) {
            point = {
              x: midpoint.x - dy / length * modelThreshold,
              y: midpoint.y + dx / length * modelThreshold,
            }
          }
        }
      }
    }
    const rejectedBucket = sample % 7
    const accept = sample % 5 === 0
      ? (target: { sourceId?: string }) => idHash(target.sourceId) % 7 !== rejectedBucket
      : undefined
    const settings = { ...disabledSettings, [kind]: true }
    const options: ResolveSnapTargetOptions = {
      point,
      scale,
      settings,
      vertices,
      segments,
      grid: { xValues: [], yValues: [] },
      thresholdsPx: { [kind]: thresholdPx },
      ...(sample % 4 === 0
        ? { excludedVertexId: vertices[(sample * 13) % 128].id }
        : {}),
      ...(accept ? { accept } : {}),
    }
    const unindexed = resolveSnapTarget(options)
    const indexed = resolveSnapTarget({ ...options, spatialIndex })
    assert.deepEqual(indexed, unindexed, `indexed authority diverged at sample ${sample}`)
  }
})
