export const INTEGER_POLAR_POLYGON_MODEL_ID_V1 =
  'ori_integer_polar_polygon_half_plane_cross_radius_xy_v1'

const MAX_POINTS = 16
const MAX_ABSOLUTE_COORDINATE = 100_000

type IntegerPoint = readonly [number, number]
type RankedPoint = Readonly<{
  point: [number, number]
  vectorX: number
  vectorY: number
  radiusSquared: number
}>

/**
 * Returns a canonical cyclic ordering without a runtime transcendental call.
 *
 * At the public bounds, an axis numerator is at most 3,200,000 and every
 * cross/radius intermediate is at most 20,480,000,000,000 (< 2^53). The
 * explicit safe-integer checks below keep a future bound change fail-closed.
 */
export function canonicalizeIntegerPolarPolygonV1(
  points: readonly IntegerPoint[],
  clockwise: boolean,
): Array<[number, number]> | null {
  if (
    typeof clockwise !== 'boolean'
    || points.length < 3
    || points.length > MAX_POINTS
    || points.some(([x, y]) =>
      !Number.isSafeInteger(x)
      || !Number.isSafeInteger(y)
      || Math.abs(x) > MAX_ABSOLUTE_COORDINATE
      || Math.abs(y) > MAX_ABSOLUTE_COORDINATE)
  ) return null
  const keys = new Set(points.map(([x, y]) => `${x},${y}`))
  if (keys.size !== points.length) return null

  const centreX = points.reduce((sum, [x]) => sum + x, 0)
  const centreY = points.reduce((sum, [, y]) => sum + y, 0)
  if (!Number.isSafeInteger(centreX) || !Number.isSafeInteger(centreY)) {
    return null
  }
  const ranked: RankedPoint[] = []
  for (const [x, y] of points) {
    const vectorX = x * points.length - centreX
    const vectorY = y * points.length - centreY
    const radiusSquared = vectorX * vectorX + vectorY * vectorY
    if (
      !Number.isSafeInteger(vectorX)
      || !Number.isSafeInteger(vectorY)
      || !Number.isSafeInteger(radiusSquared)
    ) return null
    ranked.push({ point: [x, y], vectorX, vectorY, radiusSquared })
  }
  for (let first = 0; first < ranked.length; first += 1) {
    for (let second = first + 1; second < ranked.length; second += 1) {
      const cross = ranked[first]!.vectorX * ranked[second]!.vectorY
        - ranked[first]!.vectorY * ranked[second]!.vectorX
      if (!Number.isSafeInteger(cross)) return null
    }
  }

  ranked.sort((left, right) => {
    const order = compareCounterclockwise(left, right)
    return clockwise ? -order : order
  })
  const start = ranked.reduce((best, candidate, index) =>
    comparePoint(candidate.point, ranked[best]!.point) < 0 ? index : best, 0)
  return [...ranked.slice(start), ...ranked.slice(0, start)]
    .map(({ point }) => point)
}

function compareCounterclockwise(left: RankedPoint, right: RankedPoint) {
  const leftHalf = polarHalf(left.vectorX, left.vectorY)
  const rightHalf = polarHalf(right.vectorX, right.vectorY)
  if (leftHalf !== rightHalf) return leftHalf - rightHalf
  const cross = left.vectorX * right.vectorY - left.vectorY * right.vectorX
  if (cross !== 0) return cross > 0 ? -1 : 1
  if (left.radiusSquared !== right.radiusSquared) {
    return left.radiusSquared < right.radiusSquared ? -1 : 1
  }
  return comparePoint(left.point, right.point)
}

function polarHalf(x: number, y: number) {
  return y > 0 || (y === 0 && x >= 0) ? 0 : 1
}

function comparePoint(left: IntegerPoint, right: IntegerPoint) {
  if (left[0] !== right[0]) return left[0] < right[0] ? -1 : 1
  if (left[1] !== right[1]) return left[1] < right[1] ? -1 : 1
  return 0
}
