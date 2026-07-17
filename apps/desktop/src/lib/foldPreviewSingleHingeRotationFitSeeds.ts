import {
  enumerateTrigonometricQuadraticRoots,
  type TrigonometricQuadraticCoefficients,
} from './trigonometricQuadraticRoots.ts'

export const FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS_VERSION =
  'single_hinge_rotation_fit_seeds_v1'
export const MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_POINTS = 100_000
export const MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS = 6

const MAX_ID_LENGTH = 512
const UNIT_VECTOR_TOLERANCE = 1e-9
const IMPROVEMENT_TOLERANCE_FACTOR = 4_096
const HARMONIC_ROUNDOFF_FACTOR = 4_096
const HARMONIC_ROOT_SHIFT_FACTOR = 128
const CANDIDATE_ANGLE_TOLERANCE_RADIANS = 1e-13

type Point = Readonly<{ x: number; y: number; z: number }>

export type FoldPreviewSingleHingeRotationFitInput = Readonly<{
  axis: Readonly<{
    point: Point
    direction: Point
  }>
  childRotationSign: 1 | -1
  blockingAngleDegrees: number
  maximumAngleDeltaDegrees: number
  translation: Point
  movingPoints: readonly Readonly<{
    id: string
    position: Point
  }>[]
}>

export type FoldPreviewSingleHingeRotationFitSeed = Readonly<{
  rank: number
  source:
    | 'least_squares_stationary'
    | 'angle_domain_minimum'
    | 'angle_domain_maximum'
  angleDegrees: number
  signedDeltaDegrees: number
  signedRotationRadians: number
  residualSquared: number
  residualRms: number
  improvementSquared: number
  improvementRatio: number
}>

export type FoldPreviewSingleHingeRotationFitSeeds = Readonly<{
  version: typeof FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS_VERSION
  kind: 'unverified_single_hinge_rotation_fit_seeds'
  blockingAngleDegrees: number
  maximumAngleDeltaDegrees: number
  angleDomain: Readonly<{
    minimumDegrees: number
    maximumDegrees: number
  }>
  translation: Point
  pointCount: number
  baselineResidualSquared: number
  baselineResidualRms: number
  evaluatedCandidateCount: number
  seeds: readonly FoldPreviewSingleHingeRotationFitSeed[]
  analysis: Readonly<{
    method: 'bounded_finite_rotation_least_squares_v1'
    objective: 'moving_material_points_match_common_translation'
    maximumPointCount:
      typeof MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_POINTS
    maximumSeedCount:
      typeof MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS
  }>
  safety: Readonly<{
    modelIdentityBound: false
    collisionConstraintsRevalidated: false
    legalCorrectionPoseGenerated: false
    staticCandidateRevalidated: false
    continuousCandidatePathCertified: false
    autoApplicable: false
  }>
}>

type SnapshotInput = Readonly<{
  axisPoint: Point
  axisDirection: Point
  childRotationSign: 1 | -1
  blockingAngleDegrees: number
  maximumAngleDeltaDegrees: number
  translation: Point
  movingPoints: readonly Readonly<{
    id: string
    position: Point
  }>[]
}>

type PreparedPoint = Readonly<{
  id: string
  radial: Point
  tangent: Point
}>

type CandidateSource = FoldPreviewSingleHingeRotationFitSeed['source']

type CandidateRotation = {
  radians: number
  source: CandidateSource
}

type RotationDomain = Readonly<{
  minimumRadians: number
  maximumRadians: number
  atAngleMinimumRadians: number
  atAngleMaximumRadians: number
}>

/**
 * Fits one finite rotation about a supplied world-space axis to a common
 * translation over detached moving material points.
 *
 * The output is only a bounded heuristic seed list. The axis is not bound to a
 * model here, and no collision, legal-pose, or continuous-path claim is made.
 */
export function deriveFoldPreviewSingleHingeRotationFitSeeds(
  input: FoldPreviewSingleHingeRotationFitInput,
): FoldPreviewSingleHingeRotationFitSeeds | null {
  try {
    const snapshot = snapshotInput(input)
    if (!snapshot) return null
    const translationLength = Math.hypot(
      snapshot.translation.x,
      snapshot.translation.y,
      snapshot.translation.z,
    )
    if (!Number.isFinite(translationLength) || translationLength <= 0) {
      return null
    }

    const coefficientCosAccumulator = finiteAccumulator()
    const coefficientSinAccumulator = finiteAccumulator()
    const coefficientCos2Accumulator = finiteAccumulator()
    const coefficientSin2Accumulator = finiteAccumulator()
    const harmonicScaleAccumulator = finiteAccumulator()
    const preparedPoints: PreparedPoint[] = []
    for (const movingPoint of snapshot.movingPoints) {
      const offset = subtract(movingPoint.position, snapshot.axisPoint)
      if (!offset) return null
      const axialDistance = dot(snapshot.axisDirection, offset)
      if (!Number.isFinite(axialDistance)) return null
      const radial = subtract(
        offset,
        scale(snapshot.axisDirection, axialDistance),
      )
      const tangent = cross(snapshot.axisDirection, offset)
      if (!radial || !tangent) return null
      const radialSquared = dot(radial, radial)
      const tangentSquared = dot(tangent, tangent)
      const radialTangent = dot(radial, tangent)
      const translationRadial = dot(snapshot.translation, radial)
      const translationTangent = dot(snapshot.translation, tangent)
      const coefficientCosTerm =
        -2 * (radialSquared + translationRadial)
      const coefficientSinTerm =
        -2 * (radialTangent + translationTangent)
      const coefficientCos2Term =
        (radialSquared - tangentSquared) / 2
      const harmonicScaleTerm = radialSquared + tangentSquared
      if (
        !Number.isFinite(radialSquared)
        || radialSquared < 0
        || !Number.isFinite(tangentSquared)
        || tangentSquared < 0
        || !Number.isFinite(radialTangent)
        || !Number.isFinite(translationRadial)
        || !Number.isFinite(translationTangent)
        || !Number.isFinite(coefficientCosTerm)
        || !Number.isFinite(coefficientSinTerm)
        || !Number.isFinite(coefficientCos2Term)
        || !Number.isFinite(harmonicScaleTerm)
        || !coefficientCosAccumulator.add(coefficientCosTerm)
        || !coefficientSinAccumulator.add(coefficientSinTerm)
        || !coefficientCos2Accumulator.add(coefficientCos2Term)
        || !coefficientSin2Accumulator.add(radialTangent)
        || !harmonicScaleAccumulator.add(harmonicScaleTerm)
      ) return null
      preparedPoints.push(Object.freeze({
        id: movingPoint.id,
        radial,
        tangent,
      }))
    }

    const coefficientCos = coefficientCosAccumulator.value()
    const coefficientSin = coefficientSinAccumulator.value()
    const coefficientCos2 = coefficientCos2Accumulator.value()
    const coefficientSin2 = coefficientSin2Accumulator.value()
    const harmonicScale = harmonicScaleAccumulator.value()
    if (
      !Number.isFinite(coefficientCos)
      || !Number.isFinite(coefficientSin)
      || !Number.isFinite(coefficientCos2)
      || !Number.isFinite(coefficientSin2)
      || !Number.isFinite(harmonicScale)
      || harmonicScale < 0
    ) return null
    const harmonicRoundoffTolerance =
      harmonicScale * (Number.EPSILON * HARMONIC_ROUNDOFF_FACTOR)
    const secondHarmonicMagnitude = Math.hypot(
      coefficientCos2,
      coefficientSin2,
    )
    const firstHarmonicMagnitude = Math.hypot(
      coefficientCos,
      coefficientSin,
    )
    if (
      !Number.isFinite(harmonicRoundoffTolerance)
      || !Number.isFinite(secondHarmonicMagnitude)
      || !Number.isFinite(firstHarmonicMagnitude)
    ) return null
    const rootShiftTolerance = firstHarmonicMagnitude
      * (Number.EPSILON * HARMONIC_ROOT_SHIFT_FACTOR)
    if (!Number.isFinite(rootShiftTolerance)) return null
    const canonicalizeSecondHarmonic =
      secondHarmonicMagnitude === 0
      || (
        secondHarmonicMagnitude <= harmonicRoundoffTolerance
        && secondHarmonicMagnitude <= rootShiftTolerance
      )
    if (
      canonicalizeSecondHarmonic
      && firstHarmonicMagnitude <= harmonicRoundoffTolerance
    ) return null

    const angleDomain = angleDomainFor(snapshot)
    if (!angleDomain) return null
    const rotationDomain = rotationDomainFor(snapshot, angleDomain)
    if (!rotationDomain) return null
    const candidates: CandidateRotation[] = []
    addCandidate(
      candidates,
      rotationDomain.atAngleMinimumRadians,
      'angle_domain_minimum',
    )
    addCandidate(
      candidates,
      rotationDomain.atAngleMaximumRadians,
      'angle_domain_maximum',
    )
    const stationaryRoots = stationaryRotationRoots(
      {
        a0: 0,
        aCos: coefficientSin,
        aSin: -coefficientCos,
        // For an exact unit axis radial and tangent are orthogonal with equal
        // length. Canonicalize only their bounded floating-point residue so
        // the lower-degree analytic root case remains numerically stable.
        aCos2: canonicalizeSecondHarmonic ? 0 : 2 * coefficientSin2,
        aSin2: canonicalizeSecondHarmonic ? 0 : -2 * coefficientCos2,
      },
      rotationDomain,
    )
    if (!stationaryRoots) return null
    for (const stationary of stationaryRoots) {
      addCandidate(
        candidates,
        stationary,
        'least_squares_stationary',
      )
    }
    candidates.sort((first, second) => first.radians - second.radians)
    if (
      candidates.length === 0
      || candidates.length > MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS
    ) return null

    const baselineResidualSquared = residualSquared(
      preparedPoints,
      snapshot.translation,
      0,
    )
    if (
      baselineResidualSquared === null
      || baselineResidualSquared <= 0
    ) return null
    const baselineResidualRms = Math.sqrt(
      baselineResidualSquared / preparedPoints.length,
    )
    if (!Number.isFinite(baselineResidualRms)) return null
    const seeds: Omit<FoldPreviewSingleHingeRotationFitSeed, 'rank'>[] = []
    for (const candidate of candidates) {
      const residual = residualSquared(
        preparedPoints,
        snapshot.translation,
        candidate.radians,
      )
      if (residual === null) return null
      const improvement = baselineResidualSquared - residual
      const improvementTolerance = Math.max(
        baselineResidualSquared,
        residual,
      ) * Number.EPSILON * IMPROVEMENT_TOLERANCE_FACTOR
      if (
        !Number.isFinite(improvement)
        || !Number.isFinite(improvementTolerance)
        || improvement <= improvementTolerance
      ) continue
      const signedDeltaDegrees = snapshot.childRotationSign
        * candidate.radians * 180 / Math.PI
      const rawAngle = snapshot.blockingAngleDegrees + signedDeltaDegrees
      if (
        !Number.isFinite(signedDeltaDegrees)
        || signedDeltaDegrees === 0
        || !Number.isFinite(rawAngle)
        || rawAngle < angleDomain.minimumDegrees - 1e-12
        || rawAngle > angleDomain.maximumDegrees + 1e-12
      ) return null
      const angleDegrees = canonicalZero(Math.min(
        angleDomain.maximumDegrees,
        Math.max(angleDomain.minimumDegrees, rawAngle),
      ))
      const improvementRatio = improvement / baselineResidualSquared
      const residualRms = Math.sqrt(residual / preparedPoints.length)
      if (
        !Number.isFinite(improvementRatio)
        || improvementRatio <= 0
        || improvementRatio > 1
        || !Number.isFinite(residualRms)
      ) return null
      seeds.push(Object.freeze({
        source: candidate.source,
        angleDegrees,
        signedDeltaDegrees: canonicalZero(
          angleDegrees - snapshot.blockingAngleDegrees,
        ),
        signedRotationRadians: canonicalZero(candidate.radians),
        residualSquared: residual,
        residualRms,
        improvementSquared: improvement,
        improvementRatio,
      }))
    }
    seeds.sort((first, second) =>
      first.residualSquared - second.residualSquared
      || Math.abs(first.signedDeltaDegrees)
        - Math.abs(second.signedDeltaDegrees)
      || first.angleDegrees - second.angleDegrees)
    if (
      seeds.length === 0
      || seeds.length > MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS
    ) return null
    const ranked = seeds.map((seed, index) => Object.freeze({
      rank: index + 1,
      ...seed,
    }))

    return Object.freeze({
      version: FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS_VERSION,
      kind: 'unverified_single_hinge_rotation_fit_seeds',
      blockingAngleDegrees: snapshot.blockingAngleDegrees,
      maximumAngleDeltaDegrees: snapshot.maximumAngleDeltaDegrees,
      angleDomain,
      translation: snapshot.translation,
      pointCount: preparedPoints.length,
      baselineResidualSquared,
      baselineResidualRms,
      evaluatedCandidateCount: candidates.length,
      seeds: Object.freeze(ranked),
      analysis: Object.freeze({
        method: 'bounded_finite_rotation_least_squares_v1',
        objective: 'moving_material_points_match_common_translation',
        maximumPointCount:
          MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_POINTS,
        maximumSeedCount:
          MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_SEEDS,
      }),
      safety: Object.freeze({
        modelIdentityBound: false,
        collisionConstraintsRevalidated: false,
        legalCorrectionPoseGenerated: false,
        staticCandidateRevalidated: false,
        continuousCandidatePathCertified: false,
        autoApplicable: false,
      }),
    })
  } catch {
    return null
  }
}

function snapshotInput(value: unknown): SnapshotInput | null {
  if (!isRecord(value)) return null
  const rawAxis = value.axis
  const childRotationSign = value.childRotationSign
  const blockingAngleDegrees = value.blockingAngleDegrees
  const maximumAngleDeltaDegrees = value.maximumAngleDeltaDegrees
  const rawTranslation = value.translation
  const rawMovingPoints = value.movingPoints
  if (
    !isRecord(rawAxis)
    || (childRotationSign !== 1 && childRotationSign !== -1)
    || !validAngle(blockingAngleDegrees)
    || !validPositiveAngleDelta(maximumAngleDeltaDegrees)
    || !Array.isArray(rawMovingPoints)
  ) return null
  const rawAxisPoint = rawAxis.point
  const rawAxisDirection = rawAxis.direction
  const axisPoint = snapshotPoint(rawAxisPoint)
  const axisDirection = snapshotUnitPoint(rawAxisDirection)
  const translation = snapshotPoint(rawTranslation)
  if (!axisPoint || !axisDirection || !translation) return null
  const pointCount = rawMovingPoints.length
  if (
    !Number.isSafeInteger(pointCount)
    || pointCount <= 0
    || pointCount
      > MAX_FOLD_PREVIEW_SINGLE_HINGE_ROTATION_FIT_POINTS
  ) return null
  const ids = new Set<string>()
  const movingPoints: SnapshotInput['movingPoints'][number][] = []
  for (let index = 0; index < pointCount; index += 1) {
    const rawPoint = rawMovingPoints[index]
    if (!isRecord(rawPoint)) return null
    const id = rawPoint.id
    const rawPosition = rawPoint.position
    const position = snapshotPoint(rawPosition)
    if (!validId(id) || ids.has(id) || !position) return null
    ids.add(id)
    movingPoints.push(Object.freeze({ id, position }))
  }
  movingPoints.sort((first, second) =>
    first.id < second.id ? -1 : first.id > second.id ? 1 : 0)
  return Object.freeze({
    axisPoint,
    axisDirection,
    childRotationSign,
    blockingAngleDegrees: canonicalZero(blockingAngleDegrees),
    maximumAngleDeltaDegrees,
    translation,
    movingPoints: Object.freeze(movingPoints),
  })
}

function angleDomainFor(snapshot: SnapshotInput) {
  const minimumDegrees = Math.max(
    0,
    snapshot.blockingAngleDegrees - snapshot.maximumAngleDeltaDegrees,
  )
  const maximumDegrees = Math.min(
    180,
    snapshot.blockingAngleDegrees + snapshot.maximumAngleDeltaDegrees,
  )
  return Number.isFinite(minimumDegrees)
    && Number.isFinite(maximumDegrees)
    && minimumDegrees <= snapshot.blockingAngleDegrees
    && maximumDegrees >= snapshot.blockingAngleDegrees
    && maximumDegrees > minimumDegrees
    ? Object.freeze({ minimumDegrees, maximumDegrees })
    : null
}

function rotationDomainFor(
  snapshot: SnapshotInput,
  angleDomain: Readonly<{
    minimumDegrees: number
    maximumDegrees: number
  }>,
): RotationDomain | null {
  const first = snapshot.childRotationSign
    * (angleDomain.minimumDegrees - snapshot.blockingAngleDegrees)
    * Math.PI / 180
  const second = snapshot.childRotationSign
    * (angleDomain.maximumDegrees - snapshot.blockingAngleDegrees)
    * Math.PI / 180
  if (!Number.isFinite(first) || !Number.isFinite(second)) return null
  return Object.freeze({
    minimumRadians: Math.min(first, second),
    maximumRadians: Math.max(first, second),
    atAngleMinimumRadians: first,
    atAngleMaximumRadians: second,
  })
}

function stationaryRotationRoots(
  coefficients: TrigonometricQuadraticCoefficients,
  rotationDomain: RotationDomain,
): readonly number[] | null {
  if (
    coefficients.a0 === 0
    && coefficients.aCos2 === 0
    && coefficients.aSin2 === 0
  ) return firstHarmonicStationaryRoots(coefficients, rotationDomain)

  const roots: number[] = []
  if (rotationDomain.maximumRadians > 0) {
    const positive = enumerateTrigonometricQuadraticRoots(coefficients)
    if (positive.kind === 'rejected') return null
    const minimum = Math.max(0, rotationDomain.minimumRadians)
    for (const root of positive.rootsRadians) {
      if (
        root < minimum - CANDIDATE_ANGLE_TOLERANCE_RADIANS
        || root
          > rotationDomain.maximumRadians
            + CANDIDATE_ANGLE_TOLERANCE_RADIANS
      ) continue
      roots.push(Math.min(
        rotationDomain.maximumRadians,
        Math.max(minimum, root),
      ))
    }
  }
  if (rotationDomain.minimumRadians < 0) {
    const negative = enumerateTrigonometricQuadraticRoots({
      a0: coefficients.a0,
      aCos: coefficients.aCos,
      aSin: -coefficients.aSin,
      aCos2: coefficients.aCos2,
      aSin2: -coefficients.aSin2,
    })
    if (negative.kind === 'rejected') return null
    const maximum = Math.min(0, rotationDomain.maximumRadians)
    for (const mirroredRoot of negative.rootsRadians) {
      const root = -mirroredRoot
      if (
        root
          < rotationDomain.minimumRadians
            - CANDIDATE_ANGLE_TOLERANCE_RADIANS
        || root > maximum + CANDIDATE_ANGLE_TOLERANCE_RADIANS
      ) continue
      roots.push(Math.min(
        maximum,
        Math.max(rotationDomain.minimumRadians, root),
      ))
    }
  }
  roots.sort((first, second) => first - second)
  const distinct: number[] = []
  for (const root of roots) {
    const previous = distinct.at(-1)
    if (
      previous === undefined
      || root - previous > CANDIDATE_ANGLE_TOLERANCE_RADIANS
    ) distinct.push(canonicalZero(root))
  }
  return Object.freeze(distinct)
}

function firstHarmonicStationaryRoots(
  coefficients: TrigonometricQuadraticCoefficients,
  rotationDomain: RotationDomain,
): readonly number[] | null {
  const amplitude = Math.hypot(coefficients.aCos, coefficients.aSin)
  if (!Number.isFinite(amplitude) || amplitude <= 0) return null
  const phase = Math.atan2(coefficients.aCos, -coefficients.aSin)
  if (!Number.isFinite(phase)) return null
  const roots: number[] = []
  for (let period = -2; period <= 2; period += 1) {
    const root = phase + period * Math.PI
    if (!Number.isFinite(root)) return null
    if (
      root
        < rotationDomain.minimumRadians
          - CANDIDATE_ANGLE_TOLERANCE_RADIANS
      || root
        > rotationDomain.maximumRadians
          + CANDIDATE_ANGLE_TOLERANCE_RADIANS
    ) continue
    roots.push(canonicalZero(Math.min(
      rotationDomain.maximumRadians,
      Math.max(rotationDomain.minimumRadians, root),
    )))
  }
  roots.sort((first, second) => first - second)
  const distinct: number[] = []
  for (const root of roots) {
    const previous = distinct.at(-1)
    if (
      previous === undefined
      || root - previous > CANDIDATE_ANGLE_TOLERANCE_RADIANS
    ) distinct.push(root)
  }
  return Object.freeze(distinct)
}

function addCandidate(
  candidates: CandidateRotation[],
  radians: number,
  source: CandidateSource,
) {
  if (!Number.isFinite(radians)) return
  const existing = candidates.find((candidate) =>
    Math.abs(candidate.radians - radians)
      <= CANDIDATE_ANGLE_TOLERANCE_RADIANS)
  if (existing) {
    if (source === 'least_squares_stationary') existing.source = source
    return
  }
  candidates.push({ radians: canonicalZero(radians), source })
}

function residualSquared(
  points: readonly PreparedPoint[],
  translation: Point,
  radians: number,
): number | null {
  const cosine = Math.cos(radians)
  const sine = Math.sin(radians)
  if (!Number.isFinite(cosine) || !Number.isFinite(sine)) return null
  const cosineOffset = cosine - 1
  if (!Number.isFinite(cosineOffset)) return null
  const accumulator = finiteAccumulator()
  for (const point of points) {
    const errorX = point.radial.x * cosineOffset
      + point.tangent.x * sine - translation.x
    const errorY = point.radial.y * cosineOffset
      + point.tangent.y * sine - translation.y
    const errorZ = point.radial.z * cosineOffset
      + point.tangent.z * sine - translation.z
    const squared = errorX * errorX + errorY * errorY + errorZ * errorZ
    if (
      !Number.isFinite(errorX)
      || !Number.isFinite(errorY)
      || !Number.isFinite(errorZ)
      || !Number.isFinite(squared)
      || squared < 0
      || !accumulator.add(squared)
    ) return null
  }
  const result = accumulator.value()
  return Number.isFinite(result) && result >= 0 ? result : null
}

function finiteAccumulator() {
  let sum = 0
  let correction = 0
  let valid = true
  return {
    add(value: number) {
      if (!valid || !Number.isFinite(value)) {
        valid = false
        return false
      }
      const adjusted = value - correction
      const next = sum + adjusted
      correction = (next - sum) - adjusted
      sum = next
      if (!Number.isFinite(sum) || !Number.isFinite(correction)) {
        valid = false
      }
      return valid
    },
    value() {
      return valid && Number.isFinite(sum) ? canonicalZero(sum) : Number.NaN
    },
  }
}

function snapshotUnitPoint(value: unknown): Point | null {
  const point = snapshotPoint(value)
  if (!point) return null
  const length = Math.hypot(point.x, point.y, point.z)
  if (
    !Number.isFinite(length)
    || length <= 0
    || Math.abs(length - 1) > UNIT_VECTOR_TOLERANCE
  ) return null
  const inverseLength = 1 / length
  if (!Number.isFinite(inverseLength)) return null
  const normalized = finitePoint({
    x: point.x * inverseLength,
    y: point.y * inverseLength,
    z: point.z * inverseLength,
  })
  if (!normalized) return null
  const normalizedLength = Math.hypot(
    normalized.x,
    normalized.y,
    normalized.z,
  )
  return Number.isFinite(normalizedLength)
    && Math.abs(normalizedLength - 1) <= UNIT_VECTOR_TOLERANCE
    ? normalized
    : null
}

function snapshotPoint(value: unknown): Point | null {
  if (!isRecord(value)) return null
  const x = value.x
  const y = value.y
  const z = value.z
  return typeof x === 'number'
    && Number.isFinite(x)
    && typeof y === 'number'
    && Number.isFinite(y)
    && typeof z === 'number'
    && Number.isFinite(z)
    ? Object.freeze({
        x: canonicalZero(x),
        y: canonicalZero(y),
        z: canonicalZero(z),
      })
    : null
}

function subtract(first: Point, second: Point): Point | null {
  return finitePoint({
    x: first.x - second.x,
    y: first.y - second.y,
    z: first.z - second.z,
  })
}

function scale(value: Point, factor: number): Point {
  return {
    x: value.x * factor,
    y: value.y * factor,
    z: value.z * factor,
  }
}

function cross(first: Point, second: Point): Point | null {
  return finitePoint({
    x: first.y * second.z - first.z * second.y,
    y: first.z * second.x - first.x * second.z,
    z: first.x * second.y - first.y * second.x,
  })
}

function finitePoint(value: { x: number; y: number; z: number }): Point | null {
  return Number.isFinite(value.x)
    && Number.isFinite(value.y)
    && Number.isFinite(value.z)
    ? Object.freeze({
        x: canonicalZero(value.x),
        y: canonicalZero(value.y),
        z: canonicalZero(value.z),
      })
    : null
}

function dot(first: Point, second: Point) {
  return first.x * second.x + first.y * second.y + first.z * second.z
}

function validId(value: unknown): value is string {
  return typeof value === 'string'
    && value.length > 0
    && value.length <= MAX_ID_LENGTH
}

function validAngle(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value >= 0
    && value <= 180
}

function validPositiveAngleDelta(value: unknown): value is number {
  return typeof value === 'number'
    && Number.isFinite(value)
    && value > 0
    && value <= 180
}

function canonicalZero(value: number) {
  return Object.is(value, -0) ? 0 : value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
}
