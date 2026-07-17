const MAX_ANGULAR_SPAN_RADIANS = Math.PI * 2
const NUMERICAL_MARGIN_FACTOR = 128
const MIN_NORMALIZED_AXIS_LENGTH = Number.MIN_VALUE / Number.EPSILON

export type FoldPreviewContinuousPoint3 = Readonly<{
  x: number
  y: number
  z: number
}>

export type FoldPreviewContinuousIntervalAabb = Readonly<{
  minX: number
  minY: number
  minZ: number
  maxX: number
  maxY: number
  maxZ: number
  /** Largest perpendicular distance from an input vertex to the rotation axis. */
  maximumPerpendicularRadius: number
  /** Exact spherical displacement bound before the floating-point margin. */
  maximumChordDisplacement: number
  /** Scale-aware allowance included once in every coordinate direction. */
  numericalMargin: number
}>

/**
 * Conservatively bounds midpoint vertices rotated about one fixed world axis
 * over a symmetric angular interval.
 *
 * `angularSpanRadians` is the full endpoint-to-endpoint span, so each endpoint
 * is at most half that angle from the supplied midpoint pose. A vertex at
 * perpendicular radius `r` therefore stays inside a sphere of radius
 * `2 * r * sin(abs(span) / 4)` around its midpoint position. Expanding the
 * midpoint AABB by the largest such sphere is conservative for every vertex;
 * using one global maximum is intentionally looser, but cannot omit motion.
 *
 * The returned bounds also include a scale-aware floating-point margin for
 * axis normalization, radius evaluation, and bound arithmetic. Invalid,
 * numerically degenerate, or unrepresentable input fails closed with `null`.
 */
export function findFoldPreviewSingleAxisSweptAabb(
  midpointWorldVertices: readonly FoldPreviewContinuousPoint3[],
  axisStart: FoldPreviewContinuousPoint3,
  axisEnd: FoldPreviewContinuousPoint3,
  angularSpanRadians: number,
): FoldPreviewContinuousIntervalAabb | null {
  if (
    !Array.isArray(midpointWorldVertices)
    || midpointWorldVertices.length === 0
    || !finitePoint(axisStart)
    || !finitePoint(axisEnd)
    || !Number.isFinite(angularSpanRadians)
  ) return null

  const absoluteSpan = Math.abs(angularSpanRadians)
  if (absoluteSpan > MAX_ANGULAR_SPAN_RADIANS) return null

  const axisX = axisEnd.x - axisStart.x
  const axisY = axisEnd.y - axisStart.y
  const axisZ = axisEnd.z - axisStart.z
  const axisLength = Math.hypot(axisX, axisY, axisZ)
  if (!Number.isFinite(axisLength)) return null

  const axisCoordinateScale = Math.max(
    Math.abs(axisStart.x),
    Math.abs(axisStart.y),
    Math.abs(axisStart.z),
    Math.abs(axisEnd.x),
    Math.abs(axisEnd.y),
    Math.abs(axisEnd.z),
  )
  const axisResolution = axisCoordinateScale
    * Number.EPSILON
    * NUMERICAL_MARGIN_FACTOR
  if (
    !Number.isFinite(axisResolution)
    || axisLength <= Math.max(axisResolution, MIN_NORMALIZED_AXIS_LENGTH)
  ) return null

  const unitX = axisX / axisLength
  const unitY = axisY / axisLength
  const unitZ = axisZ / axisLength
  if (![unitX, unitY, unitZ].every(Number.isFinite)) return null

  let minX = Number.POSITIVE_INFINITY
  let minY = Number.POSITIVE_INFINITY
  let minZ = Number.POSITIVE_INFINITY
  let maxX = Number.NEGATIVE_INFINITY
  let maxY = Number.NEGATIVE_INFINITY
  let maxZ = Number.NEGATIVE_INFINITY
  let coordinateScale = Math.max(axisCoordinateScale, axisLength)
  let maximumPerpendicularRadius = 0

  for (const vertex of midpointWorldVertices) {
    if (!finitePoint(vertex)) return null
    const relativeX = vertex.x - axisStart.x
    const relativeY = vertex.y - axisStart.y
    const relativeZ = vertex.z - axisStart.z
    if (![relativeX, relativeY, relativeZ].every(Number.isFinite)) return null

    // |relative × unitAxis| is the perpendicular radius without subtracting
    // two nearly equal squared lengths.
    const crossX = relativeY * unitZ - relativeZ * unitY
    const crossY = relativeZ * unitX - relativeX * unitZ
    const crossZ = relativeX * unitY - relativeY * unitX
    const radius = Math.hypot(crossX, crossY, crossZ)
    if (!Number.isFinite(radius)) return null

    maximumPerpendicularRadius = Math.max(maximumPerpendicularRadius, radius)
    minX = Math.min(minX, vertex.x)
    minY = Math.min(minY, vertex.y)
    minZ = Math.min(minZ, vertex.z)
    maxX = Math.max(maxX, vertex.x)
    maxY = Math.max(maxY, vertex.y)
    maxZ = Math.max(maxZ, vertex.z)
    coordinateScale = Math.max(
      coordinateScale,
      Math.abs(vertex.x),
      Math.abs(vertex.y),
      Math.abs(vertex.z),
      Math.abs(relativeX),
      Math.abs(relativeY),
      Math.abs(relativeZ),
      radius,
    )
  }

  const chordFactor = 2 * Math.sin(absoluteSpan / 4)
  const maximumChordDisplacement = maximumPerpendicularRadius * chordFactor
  const numericalMargin = coordinateScale
    * Number.EPSILON
    * NUMERICAL_MARGIN_FACTOR
  const expansion = maximumChordDisplacement + numericalMargin
  if (
    !Number.isFinite(maximumChordDisplacement)
    || !Number.isFinite(numericalMargin)
    || !Number.isFinite(expansion)
  ) return null

  const result = {
    minX: minX - expansion,
    minY: minY - expansion,
    minZ: minZ - expansion,
    maxX: maxX + expansion,
    maxY: maxY + expansion,
    maxZ: maxZ + expansion,
    maximumPerpendicularRadius,
    maximumChordDisplacement,
    numericalMargin,
  }
  return Object.values(result).every(Number.isFinite)
    ? Object.freeze(result)
    : null
}

function finitePoint(value: unknown): value is FoldPreviewContinuousPoint3 {
  if (!value || typeof value !== 'object') return false
  const point = value as Partial<FoldPreviewContinuousPoint3>
  return Number.isFinite(point.x)
    && Number.isFinite(point.y)
    && Number.isFinite(point.z)
}
