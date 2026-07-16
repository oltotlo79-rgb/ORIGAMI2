export type SpatialPoint = Readonly<{
  x: number
  y: number
}>

export type SpatialPointRecord<Value> = Readonly<{
  key: string
  x: number
  y: number
  value: Value
}>

export type SpatialSegmentRecord<Value> = Readonly<{
  key: string
  x1: number
  y1: number
  x2: number
  y2: number
  value: Value
}>

export type SpatialBoundary = 'inclusive' | 'exclusive'
export type SpatialTieBreak = 'source-order' | 'key'

export type SpatialQuery = Readonly<{
  point: SpatialPoint
  radius: number
  boundary?: SpatialBoundary
}>

export type PointSpatialMatch<Value> = Readonly<{
  key: string
  value: Value
  sourceIndex: number
  point: SpatialPoint
  distance: number
}>

export type SegmentSpatialMatch<Value> = PointSpatialMatch<Value> & Readonly<{
  fraction: number
}>

export type SpatialNearestQuery<Match> = SpatialQuery & Readonly<{
  tieBreak?: SpatialTieBreak
  accept?: (match: Match) => boolean
}>

export type SpatialSearchResult<Match> = Readonly<{
  match: Match | null
  visitedNodes: number
  testedEntries: number
}>

export type SpatialRangeResult<Match> = Readonly<{
  matches: readonly Match[]
  visitedNodes: number
  testedEntries: number
}>

export type PointSpatialIndex<Value> = Readonly<{
  size: number
  nearest: (query: SpatialNearestQuery<PointSpatialMatch<Value>>) => PointSpatialMatch<Value> | null
  searchNearest: (
    query: SpatialNearestQuery<PointSpatialMatch<Value>>,
  ) => SpatialSearchResult<PointSpatialMatch<Value>>
  withinRadius: (query: SpatialQuery) => SpatialRangeResult<PointSpatialMatch<Value>>
}>

export type SegmentSpatialIndex<Value> = Readonly<{
  size: number
  nearest: (
    query: SpatialNearestQuery<SegmentSpatialMatch<Value>>,
  ) => SegmentSpatialMatch<Value> | null
  searchNearest: (
    query: SpatialNearestQuery<SegmentSpatialMatch<Value>>,
  ) => SpatialSearchResult<SegmentSpatialMatch<Value>>
  withinRadius: (query: SpatialQuery) => SpatialRangeResult<SegmentSpatialMatch<Value>>
}>

type Bounds = Readonly<{
  minX: number
  minY: number
  maxX: number
  maxY: number
}>

type IndexedEntry<Value, Geometry> = Readonly<{
  key: string
  value: Value
  sourceIndex: number
  bounds: Bounds
  centerX: number
  centerY: number
  geometry: Geometry
}>

type SpatialNode<Value, Geometry> = Readonly<{
  bounds: Bounds
  minimumSourceIndex: number
  left: SpatialNode<Value, Geometry> | null
  right: SpatialNode<Value, Geometry> | null
  entries: readonly IndexedEntry<Value, Geometry>[] | null
}>

type PointGeometry = Readonly<{
  x: number
  y: number
}>

type SegmentGeometry = Readonly<{
  x1: number
  y1: number
  x2: number
  y2: number
}>

type MutableSearchStats = {
  visitedNodes: number
  testedEntries: number
}

const LEAF_SIZE = 8

export function createPointSpatialIndex<Value>(
  records: readonly SpatialPointRecord<Value>[],
): PointSpatialIndex<Value> {
  const entries = records.flatMap((record, sourceIndex): IndexedEntry<Value, PointGeometry>[] => {
    if (
      typeof record.key !== 'string'
      || !Number.isFinite(record.x)
      || !Number.isFinite(record.y)
    ) return []
    const bounds = pointBounds(record.x, record.y)
    return [{
      key: record.key,
      value: record.value,
      sourceIndex,
      bounds,
      centerX: record.x,
      centerY: record.y,
      geometry: { x: record.x, y: record.y },
    }]
  })
  const root = buildSpatialTree(entries)
  const searchNearest = (
    query: SpatialNearestQuery<PointSpatialMatch<Value>>,
  ) => searchNearestEntry(root, query, pointMatch)
  return Object.freeze({
    size: entries.length,
    nearest: (query) => searchNearest(query).match,
    searchNearest,
    withinRadius: (query) => searchWithinRadius(root, query, pointMatch),
  })
}

export function createSegmentSpatialIndex<Value>(
  records: readonly SpatialSegmentRecord<Value>[],
): SegmentSpatialIndex<Value> {
  const entries = records.flatMap((record, sourceIndex): IndexedEntry<Value, SegmentGeometry>[] => {
    if (
      typeof record.key !== 'string'
      || ![
        record.x1,
        record.y1,
        record.x2,
        record.y2,
      ].every(Number.isFinite)
    ) return []
    const bounds: Bounds = {
      minX: Math.min(record.x1, record.x2),
      minY: Math.min(record.y1, record.y2),
      maxX: Math.max(record.x1, record.x2),
      maxY: Math.max(record.y1, record.y2),
    }
    return [{
      key: record.key,
      value: record.value,
      sourceIndex,
      bounds,
      centerX: stableAverage(bounds.minX, bounds.maxX),
      centerY: stableAverage(bounds.minY, bounds.maxY),
      geometry: {
        x1: record.x1,
        y1: record.y1,
        x2: record.x2,
        y2: record.y2,
      },
    }]
  })
  const root = buildSpatialTree(entries)
  const searchNearest = (
    query: SpatialNearestQuery<SegmentSpatialMatch<Value>>,
  ) => searchNearestEntry(root, query, segmentMatch)
  return Object.freeze({
    size: entries.length,
    nearest: (query) => searchNearest(query).match,
    searchNearest,
    withinRadius: (query) => searchWithinRadius(root, query, segmentMatch),
  })
}

function searchNearestEntry<Value, Geometry, Match extends PointSpatialMatch<Value>>(
  root: SpatialNode<Value, Geometry> | null,
  query: SpatialNearestQuery<Match>,
  createMatch: (entry: IndexedEntry<Value, Geometry>, point: SpatialPoint) => Match | null,
): SpatialSearchResult<Match> {
  const normalized = normalizeQuery(query)
  if (!root || !normalized) return emptySearchResult()

  const stats: MutableSearchStats = { visitedNodes: 0, testedEntries: 0 }
  let best: Match | null = null
  const visit = (node: SpatialNode<Value, Geometry>) => {
    const lowerDistance = pointBoundsDistance(normalized.point, node.bounds)
    if (!distanceIsWithin(lowerDistance, normalized.radius, normalized.boundary)) return
    if (best && lowerDistance > best.distance) return
    stats.visitedNodes += 1

    if (node.entries) {
      for (const entry of node.entries) {
        stats.testedEntries += 1
        const match = createMatch(entry, normalized.point)
        if (
          !match
          || !distanceIsWithin(match.distance, normalized.radius, normalized.boundary)
          || (query.accept && !query.accept(match))
          || !matchOutranks(match, best, query.tieBreak ?? 'source-order')
        ) continue
        best = match
      }
      return
    }

    const children = [node.left, node.right]
      .filter((child): child is SpatialNode<Value, Geometry> => child !== null)
      .map((child) => ({
        child,
        lowerDistance: pointBoundsDistance(normalized.point, child.bounds),
      }))
      .sort((first, second) => first.lowerDistance - second.lowerDistance
        || first.child.minimumSourceIndex - second.child.minimumSourceIndex)
    for (const { child } of children) visit(child)
  }
  visit(root)
  return Object.freeze({ match: best, ...stats })
}

function searchWithinRadius<Value, Geometry, Match extends PointSpatialMatch<Value>>(
  root: SpatialNode<Value, Geometry> | null,
  query: SpatialQuery,
  createMatch: (entry: IndexedEntry<Value, Geometry>, point: SpatialPoint) => Match | null,
): SpatialRangeResult<Match> {
  const normalized = normalizeQuery(query)
  if (!root || !normalized) return emptyRangeResult()

  const stats: MutableSearchStats = { visitedNodes: 0, testedEntries: 0 }
  const matches: Match[] = []
  const visit = (node: SpatialNode<Value, Geometry>) => {
    const lowerDistance = pointBoundsDistance(normalized.point, node.bounds)
    if (!distanceIsWithin(lowerDistance, normalized.radius, normalized.boundary)) return
    stats.visitedNodes += 1
    if (node.entries) {
      for (const entry of node.entries) {
        stats.testedEntries += 1
        const match = createMatch(entry, normalized.point)
        if (
          match
          && distanceIsWithin(match.distance, normalized.radius, normalized.boundary)
        ) matches.push(match)
      }
      return
    }
    if (node.left) visit(node.left)
    if (node.right) visit(node.right)
  }
  visit(root)
  matches.sort((first, second) => first.sourceIndex - second.sourceIndex)
  return Object.freeze({ matches: Object.freeze(matches), ...stats })
}

function pointMatch<Value>(
  entry: IndexedEntry<Value, PointGeometry>,
  point: SpatialPoint,
): PointSpatialMatch<Value> | null {
  const distance = Math.hypot(entry.geometry.x - point.x, entry.geometry.y - point.y)
  if (!Number.isFinite(distance)) return null
  return Object.freeze({
    key: entry.key,
    value: entry.value,
    sourceIndex: entry.sourceIndex,
    point: Object.freeze({ x: entry.geometry.x, y: entry.geometry.y }),
    distance,
  })
}

function segmentMatch<Value>(
  entry: IndexedEntry<Value, SegmentGeometry>,
  point: SpatialPoint,
): SegmentSpatialMatch<Value> | null {
  const { x1, y1, x2, y2 } = entry.geometry
  const dx = x2 - x1
  const dy = y2 - y1
  const lengthSquared = dx * dx + dy * dy
  const fraction = Number.isFinite(dx)
    && Number.isFinite(dy)
    && Number.isFinite(lengthSquared)
    && (lengthSquared > 0 || (dx === 0 && dy === 0))
    ? lengthSquared === 0
      ? 0
      : clampSegmentFraction(
          ((point.x - x1) * dx + (point.y - y1) * dy) / lengthSquared,
        )
    : normalizedSegmentFraction(point, entry.geometry)
  if (fraction === null) return null
  const projection = {
    x: stableConvexCombination(x1, x2, fraction),
    y: stableConvexCombination(y1, y2, fraction),
  }
  const distance = Math.hypot(point.x - projection.x, point.y - projection.y)
  if (
    !Number.isFinite(fraction)
    || !Number.isFinite(projection.x)
    || !Number.isFinite(projection.y)
    || !Number.isFinite(distance)
  ) return null
  return Object.freeze({
    key: entry.key,
    value: entry.value,
    sourceIndex: entry.sourceIndex,
    point: Object.freeze(projection),
    distance,
    fraction,
  })
}

function normalizedSegmentFraction(
  point: SpatialPoint,
  segment: SegmentGeometry,
) {
  const scale = Math.max(
    Math.abs(point.x),
    Math.abs(point.y),
    Math.abs(segment.x1),
    Math.abs(segment.y1),
    Math.abs(segment.x2),
    Math.abs(segment.y2),
  )
  if (!Number.isFinite(scale) || scale === 0) return scale === 0 ? 0 : null
  const x1 = segment.x1 / scale
  const y1 = segment.y1 / scale
  const dx = segment.x2 / scale - x1
  const dy = segment.y2 / scale - y1
  const offsetX = point.x / scale - x1
  const offsetY = point.y / scale - y1
  const lengthSquared = dx * dx + dy * dy
  if (
    ![x1, y1, dx, dy, offsetX, offsetY, lengthSquared].every(Number.isFinite)
    || lengthSquared <= 0
  ) return null
  return clampSegmentFraction((offsetX * dx + offsetY * dy) / lengthSquared)
}

function clampSegmentFraction(fraction: number) {
  return Number.isNaN(fraction) ? null : Math.max(0, Math.min(1, fraction))
}

function buildSpatialTree<Value, Geometry>(
  entries: readonly IndexedEntry<Value, Geometry>[],
): SpatialNode<Value, Geometry> | null {
  if (entries.length === 0) return null
  const bounds = unionBounds(entries)
  const minimumSourceIndex = entries.reduce(
    (minimum, entry) => Math.min(minimum, entry.sourceIndex),
    Number.POSITIVE_INFINITY,
  )
  if (entries.length <= LEAF_SIZE) {
    return {
      bounds,
      minimumSourceIndex,
      left: null,
      right: null,
      entries: [...entries].sort((first, second) => first.sourceIndex - second.sourceIndex),
    }
  }

  const width = bounds.maxX - bounds.minX
  const height = bounds.maxY - bounds.minY
  const useX = !Number.isFinite(height) || (Number.isFinite(width) && width >= height)
  const sorted = [...entries].sort((first, second) => {
    const primary = useX
      ? compareNumbers(first.centerX, second.centerX)
      : compareNumbers(first.centerY, second.centerY)
    const secondary = useX
      ? compareNumbers(first.centerY, second.centerY)
      : compareNumbers(first.centerX, second.centerX)
    return primary || secondary || compareKeys(first.key, second.key)
      || first.sourceIndex - second.sourceIndex
  })
  const middle = Math.floor(sorted.length / 2)
  return {
    bounds,
    minimumSourceIndex,
    left: buildSpatialTree(sorted.slice(0, middle)),
    right: buildSpatialTree(sorted.slice(middle)),
    entries: null,
  }
}

function unionBounds<Value, Geometry>(
  entries: readonly IndexedEntry<Value, Geometry>[],
): Bounds {
  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  for (const entry of entries) {
    minX = Math.min(minX, entry.bounds.minX)
    minY = Math.min(minY, entry.bounds.minY)
    maxX = Math.max(maxX, entry.bounds.maxX)
    maxY = Math.max(maxY, entry.bounds.maxY)
  }
  return { minX, minY, maxX, maxY }
}

function pointBounds(x: number, y: number): Bounds {
  return { minX: x, minY: y, maxX: x, maxY: y }
}

function pointBoundsDistance(point: SpatialPoint, bounds: Bounds) {
  const dx = point.x < bounds.minX
    ? bounds.minX - point.x
    : point.x > bounds.maxX
      ? point.x - bounds.maxX
      : 0
  const dy = point.y < bounds.minY
    ? bounds.minY - point.y
    : point.y > bounds.maxY
      ? point.y - bounds.maxY
      : 0
  return Math.hypot(dx, dy)
}

function normalizeQuery(query: SpatialQuery) {
  const boundary = query.boundary ?? 'inclusive'
  if (
    !Number.isFinite(query.point.x)
    || !Number.isFinite(query.point.y)
    || !Number.isFinite(query.radius)
    || query.radius < 0
    || (boundary !== 'inclusive' && boundary !== 'exclusive')
  ) return null
  return { point: query.point, radius: query.radius, boundary }
}

function distanceIsWithin(
  distance: number,
  radius: number,
  boundary: SpatialBoundary,
) {
  return Number.isFinite(distance)
    && (boundary === 'inclusive' ? distance <= radius : distance < radius)
}

function matchOutranks<Value, Match extends PointSpatialMatch<Value>>(
  candidate: Match,
  best: Match | null,
  tieBreak: SpatialTieBreak,
) {
  if (!best || candidate.distance < best.distance) return true
  if (candidate.distance > best.distance) return false
  if (tieBreak === 'key') {
    const keyOrder = compareKeys(candidate.key, best.key)
    if (keyOrder !== 0) return keyOrder < 0
  }
  return candidate.sourceIndex < best.sourceIndex
}

function compareNumbers(first: number, second: number) {
  return first < second ? -1 : first > second ? 1 : 0
}

function compareKeys(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}

function stableAverage(first: number, second: number) {
  if (first === second) return first
  const sameSign = (first < 0 || Object.is(first, -0))
    === (second < 0 || Object.is(second, -0))
  return sameSign ? first + (second - first) / 2 : first / 2 + second / 2
}

function stableConvexCombination(start: number, end: number, fraction: number) {
  if (fraction === 0) return start
  if (fraction === 1) return end
  const sameSign = (start < 0 || Object.is(start, -0))
    === (end < 0 || Object.is(end, -0))
  return sameSign
    ? start + (end - start) * fraction
    : start * (1 - fraction) + end * fraction
}

function emptySearchResult<Match>(): SpatialSearchResult<Match> {
  return Object.freeze({ match: null, visitedNodes: 0, testedEntries: 0 })
}

function emptyRangeResult<Match>(): SpatialRangeResult<Match> {
  return Object.freeze({ matches: Object.freeze([]), visitedNodes: 0, testedEntries: 0 })
}
