export type FoldPreviewBinary64Point = Readonly<{
  x: number
  y: number
  z: number
}>

type Dyadic = Readonly<{
  coefficient: bigint
  exponent: number
}>

type ExactVector = Readonly<{
  x: Dyadic
  y: Dyadic
  z: Dyadic
}>

type ExactFraction = Readonly<{
  numerator: Dyadic
  denominator: Dyadic
}>

type ExactInterval = Readonly<{
  minimum: ExactFraction
  maximum: ExactFraction
}>

const FRACTION_BITS = 52
const EXPONENT_BIAS = 1023
const SUBNORMAL_EXPONENT = -1074
const binary64Buffer = new ArrayBuffer(8)
const binary64View = new DataView(binary64Buffer)

/**
 * Proves a positive-length transversal intersection of two ideal triangles.
 *
 * Every input coordinate is interpreted as the exact binary64 value stored in
 * the analyzed pose. The predicate uses BigInt dyadic arithmetic throughout:
 * it never widens a tolerance and never treats an unresolved sign as zero.
 * Returning false means only "not proved", so coplanar overlap, shared
 * feature-only contact, boundary-only contact, and malformed input cannot be
 * promoted to penetration by this fallback.
 */
export function provesFoldPreviewBinary64TransversalTriangleIntersection(
  first: readonly FoldPreviewBinary64Point[],
  second: readonly FoldPreviewBinary64Point[],
): boolean {
  try {
    const firstTriangle = exactTriangle(first)
    const secondTriangle = exactTriangle(second)
    if (!firstTriangle || !secondTriangle) return false

    const firstNormal = cross(
      subtract(firstTriangle[1], firstTriangle[0]),
      subtract(firstTriangle[2], firstTriangle[0]),
    )
    const secondNormal = cross(
      subtract(secondTriangle[1], secondTriangle[0]),
      subtract(secondTriangle[2], secondTriangle[0]),
    )
    if (isZeroVector(firstNormal) || isZeroVector(secondNormal)) return false

    const lineDirection = cross(firstNormal, secondNormal)
    if (isZeroVector(lineDirection)) return false

    const firstDistances = firstTriangle.map((point) =>
      dot(secondNormal, subtract(point, secondTriangle[0])))
    const secondDistances = secondTriangle.map((point) =>
      dot(firstNormal, subtract(point, firstTriangle[0])))
    const firstSection = strictTrianglePlaneSection(
      firstTriangle,
      firstDistances,
      lineDirection,
    )
    const secondSection = strictTrianglePlaneSection(
      secondTriangle,
      secondDistances,
      lineDirection,
    )
    if (!firstSection || !secondSection) return false

    const overlapMinimum = compareFractions(
      firstSection.minimum,
      secondSection.minimum,
    ) >= 0
      ? firstSection.minimum
      : secondSection.minimum
    const overlapMaximum = compareFractions(
      firstSection.maximum,
      secondSection.maximum,
    ) <= 0
      ? firstSection.maximum
      : secondSection.maximum
    return compareFractions(overlapMinimum, overlapMaximum) < 0
  } catch {
    return false
  }
}

function exactTriangle(
  value: readonly FoldPreviewBinary64Point[],
): readonly ExactVector[] | null {
  if (!Array.isArray(value) || value.length !== 3) return null
  const result: ExactVector[] = []
  for (let index = 0; index < 3; index += 1) {
    const point = value[index]
    if (!point) return null
    const rawX = point.x
    const rawY = point.y
    const rawZ = point.z
    if (
      !Number.isFinite(rawX)
      || !Number.isFinite(rawY)
      || !Number.isFinite(rawZ)
    ) return null
    const x = exactBinary64(rawX)
    const y = exactBinary64(rawY)
    const z = exactBinary64(rawZ)
    if (!x || !y || !z) return null
    result.push({ x, y, z })
  }
  return result
}

function exactBinary64(value: number): Dyadic | null {
  if (!Number.isFinite(value)) return null
  if (value === 0) return dyadic(0n, 0)
  binary64View.setFloat64(0, value, false)
  const high = binary64View.getUint32(0, false)
  const low = binary64View.getUint32(4, false)
  const negative = (high >>> 31) !== 0
  const exponentBits = (high >>> 20) & 0x7ff
  const fraction = (BigInt(high & 0x000f_ffff) << 32n) | BigInt(low)
  if (exponentBits === 0x7ff) return null
  const coefficient = exponentBits === 0
    ? fraction
    : (1n << BigInt(FRACTION_BITS)) | fraction
  const exponent = exponentBits === 0
    ? SUBNORMAL_EXPONENT
    : exponentBits - EXPONENT_BIAS - FRACTION_BITS
  return dyadic(negative ? -coefficient : coefficient, exponent)
}

function strictTrianglePlaneSection(
  triangle: readonly ExactVector[],
  distances: readonly Dyadic[],
  lineDirection: ExactVector,
): ExactInterval | null {
  if (triangle.length !== 3 || distances.length !== 3) return null
  const signs = distances.map(sign)
  if (
    !signs.some((value) => value > 0)
    || !signs.some((value) => value < 0)
  ) return null

  const intersections: ExactFraction[] = []
  for (let index = 0; index < 3; index += 1) {
    if (signs[index] !== 0) continue
    pushDistinctFraction(intersections, {
      numerator: dot(triangle[index], lineDirection),
      denominator: dyadic(1n, 0),
    })
  }
  for (let index = 0; index < 3; index += 1) {
    const next = (index + 1) % 3
    if (
      signs[index] === 0
      || signs[next] === 0
      || signs[index] === signs[next]
    ) continue
    const projected = projectedPlaneIntersection(
      triangle[index],
      triangle[next],
      distances[index],
      distances[next],
      lineDirection,
    )
    if (!projected) return null
    pushDistinctFraction(intersections, projected)
  }
  if (intersections.length !== 2) return null
  const order = compareFractions(intersections[0], intersections[1])
  if (order === 0) return null
  return order < 0
    ? { minimum: intersections[0], maximum: intersections[1] }
    : { minimum: intersections[1], maximum: intersections[0] }
}

function pushDistinctFraction(
  target: ExactFraction[],
  value: ExactFraction,
) {
  if (!target.some((candidate) => compareFractions(candidate, value) === 0)) {
    target.push(value)
  }
}

function projectedPlaneIntersection(
  start: ExactVector,
  end: ExactVector,
  startDistance: Dyadic,
  endDistance: Dyadic,
  lineDirection: ExactVector,
): ExactFraction | null {
  const denominator = subtractDyadic(startDistance, endDistance)
  if (sign(denominator) === 0) return null
  const startProjection = dot(start, lineDirection)
  const endProjection = dot(end, lineDirection)
  const numerator = subtractDyadic(
    multiplyDyadic(startDistance, endProjection),
    multiplyDyadic(endDistance, startProjection),
  )
  return fraction(numerator, denominator)
}

function fraction(
  numerator: Dyadic,
  denominator: Dyadic,
): ExactFraction | null {
  const denominatorSign = sign(denominator)
  if (denominatorSign === 0) return null
  return denominatorSign > 0
    ? { numerator, denominator }
    : {
        numerator: negateDyadic(numerator),
        denominator: negateDyadic(denominator),
      }
}

function compareFractions(first: ExactFraction, second: ExactFraction) {
  return sign(subtractDyadic(
    multiplyDyadic(first.numerator, second.denominator),
    multiplyDyadic(second.numerator, first.denominator),
  ))
}

function subtract(first: ExactVector, second: ExactVector): ExactVector {
  return {
    x: subtractDyadic(first.x, second.x),
    y: subtractDyadic(first.y, second.y),
    z: subtractDyadic(first.z, second.z),
  }
}

function cross(first: ExactVector, second: ExactVector): ExactVector {
  return {
    x: subtractDyadic(
      multiplyDyadic(first.y, second.z),
      multiplyDyadic(first.z, second.y),
    ),
    y: subtractDyadic(
      multiplyDyadic(first.z, second.x),
      multiplyDyadic(first.x, second.z),
    ),
    z: subtractDyadic(
      multiplyDyadic(first.x, second.y),
      multiplyDyadic(first.y, second.x),
    ),
  }
}

function dot(first: ExactVector, second: ExactVector) {
  return addDyadic(
    addDyadic(
      multiplyDyadic(first.x, second.x),
      multiplyDyadic(first.y, second.y),
    ),
    multiplyDyadic(first.z, second.z),
  )
}

function addDyadic(first: Dyadic, second: Dyadic): Dyadic {
  if (first.coefficient === 0n) return second
  if (second.coefficient === 0n) return first
  const exponent = Math.min(first.exponent, second.exponent)
  return dyadic(
    (first.coefficient << BigInt(first.exponent - exponent))
      + (second.coefficient << BigInt(second.exponent - exponent)),
    exponent,
  )
}

function subtractDyadic(first: Dyadic, second: Dyadic) {
  return addDyadic(first, negateDyadic(second))
}

function multiplyDyadic(first: Dyadic, second: Dyadic): Dyadic {
  return dyadic(
    first.coefficient * second.coefficient,
    first.exponent + second.exponent,
  )
}

function negateDyadic(value: Dyadic): Dyadic {
  return value.coefficient === 0n
    ? value
    : dyadic(-value.coefficient, value.exponent)
}

function dyadic(coefficient: bigint, exponent: number): Dyadic {
  return coefficient === 0n
    ? { coefficient: 0n, exponent: 0 }
    : { coefficient, exponent }
}

function sign(value: Dyadic) {
  return value.coefficient < 0n ? -1 : value.coefficient > 0n ? 1 : 0
}

function isZeroVector(value: ExactVector) {
  return value.x.coefficient === 0n
    && value.y.coefficient === 0n
    && value.z.coefficient === 0n
}
