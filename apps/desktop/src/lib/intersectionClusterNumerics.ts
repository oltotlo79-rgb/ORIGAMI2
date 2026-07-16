export type ClusterPointLike = Readonly<{
  x: number
  y: number
}>

export type ClusterSegmentLike = Readonly<{
  x1: number
  y1: number
  x2: number
  y2: number
}>

// Keep this fixed-operation roundoff envelope aligned with ori-core's
// intersection-cluster predicate. It scales only with the two local
// determinant products, so translating a drawing cannot create a larger
// world-space snap tolerance.
const CLUSTER_INTERSECTION_ROUNDOFF_FACTOR = 16
const CLUSTER_INTERSECTION_MAX_ULPS = 4
const MIN_NORMAL = 2 ** -1022

export function clusterPointLiesOnSegment(
  point: ClusterPointLike,
  segment: ClusterSegmentLike,
) {
  if (
    !isFinitePoint(point)
    || ![
      segment.x1,
      segment.y1,
      segment.x2,
      segment.y2,
    ].every(Number.isFinite)
    || (segment.x1 === segment.x2 && segment.y1 === segment.y2)
  ) return false
  if (
    (point.x === segment.x1 && point.y === segment.y1)
    || (point.x === segment.x2 && point.y === segment.y2)
  ) return true
  if (
    point.x < Math.min(segment.x1, segment.x2)
    || point.x > Math.max(segment.x1, segment.x2)
    || point.y < Math.min(segment.y1, segment.y2)
    || point.y > Math.max(segment.y1, segment.y2)
  ) return false

  const directionX = segment.x2 - segment.x1
  const directionY = segment.y2 - segment.y1
  const offsetX = point.x - segment.x1
  const offsetY = point.y - segment.y1
  const firstProduct = directionX * offsetY
  const secondProduct = directionY * offsetX
  const determinant = firstProduct - secondProduct
  const productMagnitude = Math.abs(firstProduct) + Math.abs(secondProduct)
  if (![
    directionX,
    directionY,
    offsetX,
    offsetY,
    firstProduct,
    secondProduct,
    determinant,
    productMagnitude,
  ].every(Number.isFinite)) return false
  const errorBound = CLUSTER_INTERSECTION_ROUNDOFF_FACTOR
    * Number.EPSILON
    * productMagnitude
  return Number.isFinite(errorBound) && Math.abs(determinant) <= errorBound
}

// Used only to recognize an indeterminate near-cluster and block a two-edge
// fallback. Passing this test never makes an edge a cluster member; membership
// still requires `clusterPointLiesOnSegment`, matching the Rust authority.
export function clusterIntersectionPointsAreClose(
  first: ClusterPointLike,
  second: ClusterPointLike,
  firstSegment: ClusterSegmentLike,
  secondSegment: ClusterSegmentLike,
) {
  if (!isFinitePoint(first) || !isFinitePoint(second)) return false
  if (first.x === second.x && first.y === second.y) return true
  const segments = [firstSegment, secondSegment] as const
  const xTolerance = coordinateUncertainty(first.x, second.x, segments, 'x')
  const yTolerance = coordinateUncertainty(first.y, second.y, segments, 'y')
  return xTolerance !== null
    && yTolerance !== null
    && Math.abs(first.x - second.x) <= xTolerance
    && Math.abs(first.y - second.y) <= yTolerance
}

export function clusterIntersectionPointSearchRadius(
  point: ClusterPointLike,
  firstSegment: ClusterSegmentLike,
  secondSegment: ClusterSegmentLike,
) {
  if (!isFinitePoint(point)) return null
  const segments = [firstSegment, secondSegment] as const
  const xUncertainty = coordinateUncertainty(
    point.x,
    point.x,
    segments,
    'x',
  )
  const yUncertainty = coordinateUncertainty(
    point.y,
    point.y,
    segments,
    'y',
  )
  if (xUncertainty === null || yUncertainty === null) return null

  // A candidate can sit across an exponent boundary and therefore have an
  // ULP twice the seed point's ULP. Search twice the seed uncertainty, but
  // never farther than either source segment can extend on that axis.
  const xSpan = Math.max(
    Math.abs(firstSegment.x2 - firstSegment.x1),
    Math.abs(secondSegment.x2 - secondSegment.x1),
  )
  const ySpan = Math.max(
    Math.abs(firstSegment.y2 - firstSegment.y1),
    Math.abs(secondSegment.y2 - secondSegment.y1),
  )
  if (![xSpan, ySpan].every(Number.isFinite)) return null
  const x = Math.min(xSpan, xUncertainty * 2)
  const y = Math.min(ySpan, yUncertainty * 2)
  return Number.isFinite(x) && Number.isFinite(y) && x >= 0 && y >= 0
    ? { x, y }
    : null
}

function coordinateUncertainty(
  first: number,
  second: number,
  segments: readonly [ClusterSegmentLike, ClusterSegmentLike],
  axis: 'x' | 'y',
) {
  const span = Math.max(...segments.map((segment) => axis === 'x'
    ? Math.abs(segment.x2 - segment.x1)
    : Math.abs(segment.y2 - segment.y1)))
  const condition = intersectionCondition(segments)
  if (!Number.isFinite(span) || condition === null) return null
  const local = span * Number.EPSILON
    * CLUSTER_INTERSECTION_ROUNDOFF_FACTOR * condition
  const ulps = Math.max(unitInLastPlace(first), unitInLastPlace(second))
    * CLUSTER_INTERSECTION_MAX_ULPS
  const tolerance = Math.max(local, ulps)
  return Number.isFinite(tolerance) ? tolerance : null
}

function intersectionCondition(
  [first, second]: readonly [ClusterSegmentLike, ClusterSegmentLike],
) {
  const firstX = first.x2 - first.x1
  const firstY = first.y2 - first.y1
  const secondX = second.x2 - second.x1
  const secondY = second.y2 - second.y1
  const firstProduct = firstX * secondY
  const secondProduct = firstY * secondX
  const denominator = firstProduct - secondProduct
  const productMagnitude = Math.abs(firstProduct) + Math.abs(secondProduct)
  if (![
    firstX,
    firstY,
    secondX,
    secondY,
    firstProduct,
    secondProduct,
    denominator,
    productMagnitude,
  ].every(Number.isFinite)) return null
  if (denominator === 0) return 1
  const condition = productMagnitude / Math.abs(denominator)
  return Number.isFinite(condition) ? Math.max(1, condition) : null
}

function unitInLastPlace(value: number) {
  const magnitude = Math.abs(value)
  if (magnitude === 0 || magnitude < MIN_NORMAL) return Number.MIN_VALUE
  const ulp = 2 ** (Math.floor(Math.log2(magnitude)) - 52)
  return Number.isFinite(ulp) && ulp > 0 ? ulp : Number.MIN_VALUE
}

function isFinitePoint(point: ClusterPointLike) {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
}
