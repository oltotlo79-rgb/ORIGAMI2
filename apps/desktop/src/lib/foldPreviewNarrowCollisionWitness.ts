export const MAX_FOLD_PREVIEW_WITNESS_POSITION_CANDIDATES = 16

const TRIANGLE_PRISM_VERTEX_COUNT = 6
const TRIANGLE_PRISM_SUPPORT_LIMIT = 4
// This independently derived witness must be more conservative than the
// authoritative classifier at its near-parallel boundary. Otherwise tiny
// normalization-rounding differences could turn an indeterminate pair into a
// false witness.
const PARALLEL_AXIS_TOLERANCE = 1e-10
const PRISM_DEGENERACY_TOLERANCE = 1e-10
const UNIT_VECTOR_TOLERANCE = 1e-9
const PRISM_RELATIVE_TOLERANCE = 1e-9

export type FoldPreviewWitnessPoint = Readonly<{
  x: number
  y: number
  z: number
}>

export type FoldPreviewWitnessFrame = Readonly<{
  xAxis: FoldPreviewWitnessPoint
  yAxis: FoldPreviewWitnessPoint
  zAxis: FoldPreviewWitnessPoint
}>

export type FoldPreviewTrianglePrismWitnessInput = Readonly<{
  /**
   * Vertices 0..2 are one triangular cap. Vertices 3..5 are the matching
   * opposite cap in the same order. These must be the exact ordered vertex
   * snapshots used by the authoritative narrow-phase SAT; callers must not
   * rebuild or reorder them independently.
   */
  firstVertices: readonly FoldPreviewWitnessPoint[]
  secondVertices: readonly FoldPreviewWitnessPoint[]
  /**
   * A verified right-handed world frame belonging to the first face.
   * It makes symmetric-axis tie breaking rigid-transform covariant.
   */
  firstFrame: FoldPreviewWitnessFrame
  numericalMargin: number
  /**
   * A definitive result from the authoritative SAT. Separated and
   * indeterminate pairs must never be submitted for witness derivation.
   */
  authoritativeGeometryClass: 'touching' | 'penetrating'
}>

export type FoldPreviewTrianglePrismWitness = Readonly<{
  algorithm: 'triangle_prism_sat_witness_v1'
  geometryClass: 'touching' | 'penetrating'
  numericalMargin: number
  normal: Readonly<{
    vector: FoldPreviewWitnessPoint
    convention: 'moves_second_away_from_first'
    uniqueness: 'unique' | 'one_of_multiple'
  }>
  /** Translation distance for this one prism pair to reach contact only. */
  escapeDistance: number
  /** Existing positive gap accepted only because it is within the margin. */
  toleratedGap: number
  firstSupport: readonly FoldPreviewWitnessPoint[]
  secondSupport: readonly FoldPreviewWitnessPoint[]
  positionRegion: Readonly<{
    kind: 'support_midpoint_hull_v1'
    /** The generators belong to the analyzed input pose, before the hint. */
    sourcePose: 'analyzed_input_pose'
    generators: readonly FoldPreviewWitnessPoint[]
  }>
  localSeparationHint: Readonly<{
    translation: FoldPreviewWitnessPoint
    distance: number
    scope: 'selected_triangle_prism_pair_only'
    autoApplicable: false
  }>
}>

type MutableVector = {
  x: number
  y: number
  z: number
}

type PreparedPrism = Readonly<{
  vertices: readonly FoldPreviewWitnessPoint[]
  faceAxes: readonly FoldPreviewWitnessPoint[]
  edgeDirections: readonly FoldPreviewWitnessPoint[]
  extrusionDirection: FoldPreviewWitnessPoint
}>

type Projection = Readonly<{
  min: number
  max: number
}>

type AxisEscape = Readonly<{
  normal: FoldPreviewWitnessPoint
  distance: number
  toleratedGap: number
  directionIsAmbiguous: boolean
}>

/**
 * Derives a bounded witness seed for one positive-thickness triangular-prism
 * pair. It does not prove a whole-face correction and must not be applied
 * automatically. Existing narrow-phase classification remains authoritative,
 * is required as input, and is checked against the locally derived class.
 * This helper deliberately accepts no separated or numerically indeterminate
 * class. A common rigid transform can therefore turn an exact parallel contact
 * into null when independently rounded vertices create a near-parallel axis;
 * this matches the existing fail-closed SAT policy.
 */
export function deriveFoldPreviewTrianglePrismWitness(
  input: FoldPreviewTrianglePrismWitnessInput,
): FoldPreviewTrianglePrismWitness | null {
  try {
    if (!isRecord(input)) return null
    const rawFirstVertices = input.firstVertices
    const rawSecondVertices = input.secondVertices
    const rawFirstFrame = input.firstFrame
    const rawNumericalMargin = input.numericalMargin
    const rawAuthoritativeGeometryClass =
      input.authoritativeGeometryClass
    if (
      typeof rawNumericalMargin !== 'number'
      || !Number.isFinite(rawNumericalMargin)
      || rawNumericalMargin < 0
      || (
        rawAuthoritativeGeometryClass !== 'touching'
        && rawAuthoritativeGeometryClass !== 'penetrating'
      )
    ) return null
    const numericalMargin = canonicalZero(rawNumericalMargin)

    const firstVertices = snapshotVertices(rawFirstVertices)
    const secondVertices = snapshotVertices(rawSecondVertices)
    const firstFrame = snapshotFrame(rawFirstFrame)
    if (!firstVertices || !secondVertices || !firstFrame) return null

    const first = preparePrism(firstVertices, numericalMargin)
    const second = preparePrism(secondVertices, numericalMargin)
    if (
      !first
      || !second
      || !validFrame(firstFrame, first.extrusionDirection)
    ) return null

    const axes = collectCanonicalAxes(first, second, firstFrame)
    if (!axes || axes.length === 0) return null

    let boundaryContact = false
    let best: AxisEscape | null = null
    let multipleMinimumDirections = false
    for (const axis of axes) {
      const firstProjection = project(first.vertices, axis)
      const secondProjection = project(second.vertices, axis)
      if (!firstProjection || !secondProjection) return null
      const positiveGap = secondProjection.min - firstProjection.max
      const negativeGap = firstProjection.min - secondProjection.max
      const gap = Math.max(positiveGap, negativeGap)
      if (!Number.isFinite(gap) || gap > numericalMargin) return null
      const overlap = Math.min(firstProjection.max, secondProjection.max)
        - Math.max(firstProjection.min, secondProjection.min)
      if (!Number.isFinite(overlap)) return null
      if (overlap <= numericalMargin) boundaryContact = true

      const candidate = escapeForAxis(
        axis,
        firstProjection,
        secondProjection,
        numericalMargin,
      )
      if (!candidate) return null
      if (!best) {
        best = candidate
        multipleMinimumDirections = candidate.directionIsAmbiguous
        continue
      }
      if (candidate.distance === 0 && best.distance > 0) {
        best = candidate
        multipleMinimumDirections = candidate.directionIsAmbiguous
        continue
      }
      if (best.distance === 0 && candidate.distance > 0) continue
      if (
        candidate.distance === 0
        && best.distance === 0
        && candidate.toleratedGap > best.toleratedGap
      ) {
        best = candidate
        multipleMinimumDirections = true
        continue
      }
      if (candidate.distance + numericalMargin < best.distance) {
        best = candidate
        multipleMinimumDirections = candidate.directionIsAmbiguous
        continue
      }
      if (best.distance + numericalMargin < candidate.distance) continue
      multipleMinimumDirections = true
    }
    if (!best) return null
    const derivedGeometryClass = boundaryContact
      ? 'touching'
      : 'penetrating'
    if (derivedGeometryClass !== rawAuthoritativeGeometryClass) return null

    const firstSupport = supportVertices(
      first.vertices,
      best.normal,
      'maximum',
      numericalMargin,
      firstFrame,
      first.vertices[0],
    )
    const secondSupport = supportVertices(
      second.vertices,
      best.normal,
      'minimum',
      numericalMargin,
      firstFrame,
      first.vertices[0],
    )
    if (
      !firstSupport
      || !secondSupport
      || firstSupport.length * secondSupport.length
        > MAX_FOLD_PREVIEW_WITNESS_POSITION_CANDIDATES
    ) return null

    const generators: FoldPreviewWitnessPoint[] = []
    for (const firstPoint of firstSupport) {
      for (const secondPoint of secondSupport) {
        const generator = midpoint(firstPoint, secondPoint)
        if (!generator) return null
        generators.push(freezePoint(generator))
      }
    }
    if (
      generators.length < 1
      || generators.length > MAX_FOLD_PREVIEW_WITNESS_POSITION_CANDIDATES
    ) return null

    const translation = scaled(best.normal, best.distance)
    if (!translation) return null
    const normalVector = freezePoint(best.normal)
    const escapeDistance = finiteNonNegative(best.distance)
    const toleratedGap = finiteNonNegative(best.toleratedGap)
    if (escapeDistance === null || toleratedGap === null) return null

    return Object.freeze({
      algorithm: 'triangle_prism_sat_witness_v1',
      geometryClass: rawAuthoritativeGeometryClass,
      numericalMargin,
      normal: Object.freeze({
        vector: normalVector,
        convention: 'moves_second_away_from_first',
        uniqueness: multipleMinimumDirections
          ? 'one_of_multiple'
          : 'unique',
      }),
      escapeDistance,
      toleratedGap,
      firstSupport,
      secondSupport,
      positionRegion: Object.freeze({
        kind: 'support_midpoint_hull_v1',
        sourcePose: 'analyzed_input_pose',
        generators: Object.freeze(generators),
      }),
      localSeparationHint: Object.freeze({
        translation: freezePoint(translation),
        distance: escapeDistance,
        scope: 'selected_triangle_prism_pair_only',
        autoApplicable: false,
      }),
    })
  } catch {
    return null
  }
}

function snapshotVertices(value: unknown) {
  if (
    !Array.isArray(value)
    || value.length !== TRIANGLE_PRISM_VERTEX_COUNT
  ) return null
  const vertices: FoldPreviewWitnessPoint[] = []
  for (let index = 0; index < TRIANGLE_PRISM_VERTEX_COUNT; index += 1) {
    const point = snapshotPoint(value[index])
    if (!point) return null
    vertices.push(point)
  }
  return Object.freeze(vertices)
}

function snapshotFrame(value: unknown): FoldPreviewWitnessFrame | null {
  if (!isRecord(value)) return null
  const rawXAxis = value.xAxis
  const rawYAxis = value.yAxis
  const rawZAxis = value.zAxis
  const xAxis = snapshotPoint(rawXAxis)
  const yAxis = snapshotPoint(rawYAxis)
  const zAxis = snapshotPoint(rawZAxis)
  return xAxis && yAxis && zAxis
    ? Object.freeze({ xAxis, yAxis, zAxis })
    : null
}

function snapshotPoint(value: unknown): FoldPreviewWitnessPoint | null {
  if (!isRecord(value)) return null
  const x = value.x
  const y = value.y
  const z = value.z
  return (
    typeof x === 'number'
    && Number.isFinite(x)
    && typeof y === 'number'
    && Number.isFinite(y)
    && typeof z === 'number'
    && Number.isFinite(z)
  )
    ? freezePoint({ x, y, z })
    : null
}

function preparePrism(
  vertices: readonly FoldPreviewWitnessPoint[],
  numericalMargin: number,
): PreparedPrism | null {
  const firstEdge = difference(vertices[1], vertices[0])
  const secondEdge = difference(vertices[2], vertices[1])
  const thirdEdge = difference(vertices[0], vertices[2])
  const diagonal = difference(vertices[2], vertices[0])
  const firstExtrusion = difference(vertices[3], vertices[0])
  const secondExtrusion = difference(vertices[4], vertices[1])
  const thirdExtrusion = difference(vertices[5], vertices[2])
  if (
    !firstEdge
    || !secondEdge
    || !thirdEdge
    || !diagonal
    || !firstExtrusion
    || !secondExtrusion
    || !thirdExtrusion
  ) return null

  const extrusionScale = maximumComponent(firstExtrusion)
  if (!Number.isFinite(extrusionScale) || extrusionScale <= 0) return null
  const extrusionTolerance = Math.max(
    numericalMargin,
    extrusionScale * PRISM_RELATIVE_TOLERANCE,
  )
  if (!Number.isFinite(extrusionTolerance)) return null
  if (
    !vectorsClose(
      firstExtrusion,
      secondExtrusion,
      extrusionTolerance,
    )
    || !vectorsClose(
      firstExtrusion,
      thirdExtrusion,
      extrusionTolerance,
    )
  ) return null

  const extrusionDirection = normalized(firstExtrusion)
  const firstDirection = normalized(firstEdge)
  const secondDirection = normalized(secondEdge)
  const thirdDirection = normalized(thirdEdge)
  const diagonalDirection = normalized(diagonal)
  if (
    !extrusionDirection
    || !firstDirection
    || !secondDirection
    || !thirdDirection
  ) return null
  if (!diagonalDirection) return null
  const baseSine = classifierLength(cross(
    firstDirection,
    diagonalDirection,
  ))
  if (
    baseSine === null
    || baseSine <= PRISM_DEGENERACY_TOLERANCE
  ) return null
  // Keep the same operand scale and operation order as the authoritative
  // prism builder; independent robust rescaling can change exact-zero parallel
  // decisions after world-space translation.
  const baseNormal = normalized(cross(firstEdge, diagonal))
  if (
    !baseNormal
    || Math.abs(dot(baseNormal, extrusionDirection))
      < 1 - UNIT_VECTOR_TOLERANCE
  ) return null

  const edgeDirections = Object.freeze([
    freezePoint(firstDirection),
    freezePoint(secondDirection),
    freezePoint(thirdDirection),
    freezePoint(extrusionDirection),
  ])
  const faceAxes: FoldPreviewWitnessPoint[] = [freezePoint(baseNormal)]
  for (const edge of edgeDirections.slice(0, 3)) {
    const sideAxis = normalized(cross(edge, firstExtrusion))
    if (!sideAxis) return null
    faceAxes.push(freezePoint(sideAxis))
  }
  return Object.freeze({
    vertices,
    faceAxes: Object.freeze(faceAxes),
    edgeDirections,
    extrusionDirection: freezePoint(extrusionDirection),
  })
}

function validFrame(
  frame: FoldPreviewWitnessFrame,
  extrusionDirection: FoldPreviewWitnessPoint,
) {
  const xAxis = normalized(frame.xAxis)
  const yAxis = normalized(frame.yAxis)
  const zAxis = normalized(frame.zAxis)
  if (!xAxis || !yAxis || !zAxis) return false
  if (
    !unitInput(frame.xAxis)
    || !unitInput(frame.yAxis)
    || !unitInput(frame.zAxis)
    || Math.abs(dot(xAxis, yAxis)) > UNIT_VECTOR_TOLERANCE
    || Math.abs(dot(xAxis, zAxis)) > UNIT_VECTOR_TOLERANCE
    || Math.abs(dot(yAxis, zAxis)) > UNIT_VECTOR_TOLERANCE
  ) return false
  const handedness = dot(cross(xAxis, yAxis), zAxis)
  return Number.isFinite(handedness)
    && handedness >= 1 - UNIT_VECTOR_TOLERANCE
    && Math.abs(dot(yAxis, extrusionDirection))
      >= 1 - UNIT_VECTOR_TOLERANCE
}

function collectCanonicalAxes(
  first: PreparedPrism,
  second: PreparedPrism,
  frame: FoldPreviewWitnessFrame,
) {
  const rawAxes: FoldPreviewWitnessPoint[] = [
    ...first.faceAxes,
    ...second.faceAxes,
  ]
  for (const firstEdge of first.edgeDirections) {
    for (const secondEdge of second.edgeDirections) {
      const candidate = cross(firstEdge, secondEdge)
      const candidateLength = classifierLength(candidate)
      if (candidateLength === null) return null
      if (candidateLength === 0) continue
      if (candidateLength <= PARALLEL_AXIS_TOLERANCE) return null
      const axis = normalized(candidate)
      if (!axis) return null
      rawAxes.push(freezePoint(axis))
    }
  }

  const axes: FoldPreviewWitnessPoint[] = []
  for (const rawAxis of rawAxes) {
    const oriented = orientToFrame(rawAxis, frame)
    if (!oriented) return null
    let duplicate = false
    for (const axis of axes) {
      const parallelLength = classifierLength(cross(axis, oriented))
      if (parallelLength === null) return null
      if (parallelLength === 0 && dot(axis, oriented) > 0) {
        duplicate = true
        break
      }
    }
    if (duplicate) continue
    axes.push(freezePoint(oriented))
  }
  axes.sort((firstAxis, secondAxis) =>
    compareAxesInFrame(firstAxis, secondAxis, frame))
  return Object.freeze(axes)
}

function orientToFrame(
  axis: FoldPreviewWitnessPoint,
  frame: FoldPreviewWitnessFrame,
) {
  for (const reference of [frame.yAxis, frame.xAxis, frame.zAxis]) {
    const component = dot(axis, reference)
    if (!Number.isFinite(component)) return null
    if (Math.abs(component) <= PARALLEL_AXIS_TOLERANCE) continue
    return component < 0 ? negated(axis) : copyVector(axis)
  }
  return null
}

function compareAxesInFrame(
  first: FoldPreviewWitnessPoint,
  second: FoldPreviewWitnessPoint,
  frame: FoldPreviewWitnessFrame,
) {
  for (const reference of [frame.yAxis, frame.xAxis, frame.zAxis]) {
    const difference = dot(second, reference) - dot(first, reference)
    if (difference !== 0) return difference
  }
  return 0
}

function escapeForAxis(
  axis: FoldPreviewWitnessPoint,
  first: Projection,
  second: Projection,
  margin: number,
): AxisEscape | null {
  const positiveGap = second.min - first.max
  const negativeGap = first.min - second.max
  if (positiveGap >= 0 || negativeGap >= 0) {
    const usePositive = positiveGap >= negativeGap
    const toleratedGap = usePositive ? positiveGap : negativeGap
    if (!Number.isFinite(toleratedGap) || toleratedGap > margin) return null
    return Object.freeze({
      normal: freezePoint(usePositive ? axis : negated(axis)),
      distance: 0,
      toleratedGap: canonicalZero(toleratedGap),
      directionIsAmbiguous: false,
    })
  }

  const positiveEscape = first.max - second.min
  const negativeEscape = second.max - first.min
  if (
    !Number.isFinite(positiveEscape)
    || !Number.isFinite(negativeEscape)
    || positiveEscape < 0
    || negativeEscape < 0
  ) return null
  const directionIsAmbiguous =
    Math.abs(positiveEscape - negativeEscape) <= margin
  const usePositive = directionIsAmbiguous
    || positiveEscape < negativeEscape
  return Object.freeze({
    normal: freezePoint(usePositive ? axis : negated(axis)),
    distance: canonicalZero(usePositive ? positiveEscape : negativeEscape),
    toleratedGap: 0,
    directionIsAmbiguous,
  })
}

function supportVertices(
  vertices: readonly FoldPreviewWitnessPoint[],
  axis: FoldPreviewWitnessPoint,
  side: 'minimum' | 'maximum',
  margin: number,
  frame: FoldPreviewWitnessFrame,
  orderingOrigin: FoldPreviewWitnessPoint,
) {
  const projection = project(vertices, axis)
  if (!projection) return null
  const boundary = side === 'maximum' ? projection.max : projection.min
  const support: FoldPreviewWitnessPoint[] = []
  for (const vertex of vertices) {
    const value = dot(vertex, axis)
    if (!Number.isFinite(value)) return null
    const distance = side === 'maximum'
      ? boundary - value
      : value - boundary
    if (!Number.isFinite(distance) || distance < 0) return null
    if (distance <= margin) support.push(freezePoint(vertex))
  }
  if (
    support.length < 1
    || support.length > TRIANGLE_PRISM_SUPPORT_LIMIT
  ) return null
  const keyedSupport: Array<Readonly<{
    point: FoldPreviewWitnessPoint
    key: readonly [number, number, number]
  }>> = []
  for (const point of support) {
    const relative = difference(point, orderingOrigin)
    if (!relative) return null
    const key = [
      dot(relative, frame.xAxis),
      dot(relative, frame.yAxis),
      dot(relative, frame.zAxis),
    ] as const
    if (!key.every(Number.isFinite)) return null
    keyedSupport.push(Object.freeze({ point, key: Object.freeze(key) }))
  }
  keyedSupport.sort((first, second) => {
    for (let index = 0; index < 3; index += 1) {
      const delta = first.key[index] - second.key[index]
      if (delta !== 0) return delta
    }
    return 0
  })
  return Object.freeze(keyedSupport.map(({ point }) => point))
}

function project(
  vertices: readonly FoldPreviewWitnessPoint[],
  axis: FoldPreviewWitnessPoint,
): Projection | null {
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const vertex of vertices) {
    const value = dot(vertex, axis)
    if (!Number.isFinite(value)) return null
    min = Math.min(min, value)
    max = Math.max(max, value)
  }
  return Number.isFinite(min) && Number.isFinite(max)
    ? Object.freeze({ min, max })
    : null
}

function snapshotVector(value: MutableVector): FoldPreviewWitnessPoint {
  return freezePoint({
    x: canonicalZero(value.x),
    y: canonicalZero(value.y),
    z: canonicalZero(value.z),
  })
}

function freezePoint(value: MutableVector): FoldPreviewWitnessPoint {
  return Object.freeze({
    x: canonicalZero(value.x),
    y: canonicalZero(value.y),
    z: canonicalZero(value.z),
  })
}

function copyVector(value: FoldPreviewWitnessPoint): MutableVector {
  return { x: value.x, y: value.y, z: value.z }
}

function difference(
  first: FoldPreviewWitnessPoint,
  second: FoldPreviewWitnessPoint,
): MutableVector | null {
  const result = {
    x: first.x - second.x,
    y: first.y - second.y,
    z: first.z - second.z,
  }
  return finiteVector(result) ? result : null
}

function cross(
  first: FoldPreviewWitnessPoint,
  second: FoldPreviewWitnessPoint,
): MutableVector {
  return {
    x: first.y * second.z - first.z * second.y,
    y: first.z * second.x - first.x * second.z,
    z: first.x * second.y - first.y * second.x,
  }
}

function dot(
  first: FoldPreviewWitnessPoint,
  second: FoldPreviewWitnessPoint,
) {
  return first.x * second.x
    + first.y * second.y
    + first.z * second.z
}

function negated(value: FoldPreviewWitnessPoint): MutableVector {
  return { x: -value.x, y: -value.y, z: -value.z }
}

function scaled(
  value: FoldPreviewWitnessPoint,
  scale: number,
): MutableVector | null {
  const result = {
    x: value.x * scale,
    y: value.y * scale,
    z: value.z * scale,
  }
  return finiteVector(result) ? result : null
}

function midpoint(
  first: FoldPreviewWitnessPoint,
  second: FoldPreviewWitnessPoint,
): MutableVector | null {
  const result = {
    x: first.x / 2 + second.x / 2,
    y: first.y / 2 + second.y / 2,
    z: first.z / 2 + second.z / 2,
  }
  return finiteVector(result) ? result : null
}

function normalized(value: FoldPreviewWitnessPoint) {
  const length = classifierLength(value)
  if (length === null || length <= 0) return null
  const inverseLength = 1 / length
  if (!Number.isFinite(inverseLength)) return null
  return snapshotVector({
    x: value.x * inverseLength,
    y: value.y * inverseLength,
    z: value.z * inverseLength,
  })
}

/** Matches Three.js Vector3.length(), used by the authoritative SAT path. */
function classifierLength(value: FoldPreviewWitnessPoint) {
  const length = Math.sqrt(
    value.x * value.x
    + value.y * value.y
    + value.z * value.z,
  )
  return Number.isFinite(length) ? length : null
}

function unitInput(value: FoldPreviewWitnessPoint) {
  const length = classifierLength(value)
  return length !== null && Math.abs(length - 1) <= UNIT_VECTOR_TOLERANCE
}

function vectorsClose(
  first: FoldPreviewWitnessPoint,
  second: FoldPreviewWitnessPoint,
  tolerance: number,
) {
  const differenceVector = difference(first, second)
  return differenceVector !== null
    && maximumComponent(differenceVector) <= tolerance
}

function maximumComponent(value: FoldPreviewWitnessPoint) {
  return Math.max(Math.abs(value.x), Math.abs(value.y), Math.abs(value.z))
}

function finiteVector(value: FoldPreviewWitnessPoint) {
  return Number.isFinite(value.x)
    && Number.isFinite(value.y)
    && Number.isFinite(value.z)
}

function finiteNonNegative(value: number) {
  return Number.isFinite(value) && value >= 0
    ? canonicalZero(value)
    : null
}

function canonicalZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}
