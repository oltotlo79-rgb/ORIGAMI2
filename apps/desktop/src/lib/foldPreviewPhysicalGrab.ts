import {
  enumerateTrigonometricQuadraticRoots,
  type TrigonometricQuadraticRootResult,
} from './trigonometricQuadraticRoots.ts'

export const FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING = 'physical_grab_v2' as const

export type FoldPreviewPhysicalGrabPoint = Readonly<{
  x: number
  y: number
  z: number
}>

export type FoldPreviewPhysicalGrabRay = Readonly<{
  origin: FoldPreviewPhysicalGrabPoint
  /** Must be a unit vector. Both bounds are world distances along this ray. */
  direction: FoldPreviewPhysicalGrabPoint
  minimumDistance: number
  /** May be positive infinity, matching Three.js Raycaster's default. */
  maximumDistance: number
}>

export type FoldPreviewPhysicalGrabPrepareInput = Readonly<{
  contextKey: string
  axisStart: FoldPreviewPhysicalGrabPoint
  axisEnd: FoldPreviewPhysicalGrabPoint
  movingRotationSign: 1 | -1
  appliedAngleDegrees: number
  /** The independently recovered grab position in the canonical zero-angle pose. */
  grabRestWorldPoint: FoldPreviewPhysicalGrabPoint
  /** The surface hit in the currently displayed applied-angle pose. */
  grabWorldPoint: FoldPreviewPhysicalGrabPoint
  startRay: FoldPreviewPhysicalGrabRay
  minimumOrbitRadius: number
}>

export type FoldPreviewPhysicalGrabSession = Readonly<{
  mapping: typeof FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
  contextKey: string
  movingRotationSign: 1 | -1
  appliedAngleDegrees: number
  axisOrigin: FoldPreviewPhysicalGrabPoint
  axisUnit: FoldPreviewPhysicalGrabPoint
  orbitCenter: FoldPreviewPhysicalGrabPoint
  restRadialUnit: FoldPreviewPhysicalGrabPoint
  positiveTangentUnit: FoldPreviewPhysicalGrabPoint
  orbitRadius: number
  rayMinimumDistance: number
  rayMaximumDistance: number
  minimumOrbitRadius: number
}>

export type FoldPreviewPhysicalGrabPrepareReason =
  | 'invalid_input'
  | 'degenerate_axis'
  | 'grab_on_axis'
  | 'pose_mismatch'
  | 'start_ray_miss'
  | 'unresolvable_start'
  | 'numeric'

export type FoldPreviewPhysicalGrabPrepareResult =
  | Readonly<{
      kind: 'ready'
      session: FoldPreviewPhysicalGrabSession
    }>
  | Readonly<{
      kind: 'rejected'
      reason: FoldPreviewPhysicalGrabPrepareReason
    }>

export type FoldPreviewPhysicalGrabResolveInput = Readonly<{
  contextKey: string
  referenceAngleDegrees: number
  ray: FoldPreviewPhysicalGrabRay
}>

export type FoldPreviewPhysicalGrabResolveReason =
  | 'invalid_session'
  | 'invalid_input'
  | 'stale_context'
  | 'work_limit'
  | 'no_visible_candidate'
  | 'ambiguous_projection'
  | 'unstable_projection'
  | 'target_too_far'
  | 'branch_jump'
  | 'numeric'

export type FoldPreviewPhysicalGrabTarget = Readonly<{
  kind: 'unverified_target'
  mapping: typeof FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
  contextKey: string
  angleDegrees: number
  rawAngleDegrees: number
  endpoint: null | 'zero' | 'one_eighty'
  missDistance: number
  orbitWorldPoint: FoldPreviewPhysicalGrabPoint
  evaluationCount: number
  rootEvaluationCount: number
  stationaryCandidateCount: number
  boundaryCandidateCount: number
  equivalentCandidateCount: number
}>

export type FoldPreviewPhysicalGrabResolveResult =
  | FoldPreviewPhysicalGrabTarget
  | Readonly<{
      kind: 'rejected'
      reason: FoldPreviewPhysicalGrabResolveReason
    }>

const MIN_ANGLE = 0
const MAX_ANGLE = Math.PI
const MAX_EVALUATIONS = 256
const MAXIMUM_BRANCH_JUMP = Math.PI / 4
const REFERENCE_TIE_TOLERANCE = Math.PI / 720
const CANDIDATE_ANGLE_TOLERANCE = 1e-6
const START_ANGLE_TOLERANCE = Math.PI / 720
const START_RAY_RELATIVE_TOLERANCE = 1e-4
const POSE_SYNC_RELATIVE_TOLERANCE = 1e-7
const MAXIMUM_MISS_RATIO = 2
const SCORE_RELATIVE_TOLERANCE = 1e-10
const SCORE_BEST_RELATIVE_TOLERANCE = 1e-8
const MINIMUM_NORMALIZED_SENSITIVITY = 1e-7
const SENSITIVITY_STEP = Math.PI / 180
const UNIT_TOLERANCE = 1e-10

type MutablePoint = { x: number; y: number; z: number }
type ValidatedRay = Readonly<{
  origin: FoldPreviewPhysicalGrabPoint
  direction: FoldPreviewPhysicalGrabPoint
  minimumDistance: number
  maximumDistance: number
}>
type Candidate = Readonly<{ angle: number; score: number; depth: number }>
type EvaluatedPoint = Readonly<{
  score: number
  depth: number
}>
type CandidateAngleResult =
  | Readonly<{
      kind: 'success'
      stationaryAngles: readonly number[]
      boundaryAngles: readonly number[]
      rootEvaluationCount: number
    }>
  | Readonly<{
      kind: 'rejected'
      reason: FoldPreviewPhysicalGrabResolveReason
    }>

export function prepareFoldPreviewPhysicalGrab(
  input: FoldPreviewPhysicalGrabPrepareInput,
): FoldPreviewPhysicalGrabPrepareResult {
  if (
    !input
    || typeof input !== 'object'
    || !validContextKey(input.contextKey)
    || !finitePoint(input.axisStart)
    || !finitePoint(input.axisEnd)
    || !finitePoint(input.grabRestWorldPoint)
    || !finitePoint(input.grabWorldPoint)
    || (input.movingRotationSign !== 1 && input.movingRotationSign !== -1)
    || !validAngleDegrees(input.appliedAngleDegrees)
    || !validRay(input.startRay)
    || !Number.isFinite(input.minimumOrbitRadius)
    || input.minimumOrbitRadius <= 0
  ) return prepareRejected('invalid_input')

  const axisVector = subtract(input.axisEnd, input.axisStart)
  const axisUnit = normalized(axisVector)
  if (!axisUnit) return prepareRejected('degenerate_axis')
  const fromAxisOrigin = subtract(input.grabRestWorldPoint, input.axisStart)
  const axialDistance = dot(fromAxisOrigin, axisUnit)
  if (!Number.isFinite(axialDistance)) return prepareRejected('numeric')
  const orbitCenter = add(input.axisStart, scaled(axisUnit, axialDistance))
  const restRadial = subtract(input.grabRestWorldPoint, orbitCenter)
  const orbitRadius = length(restRadial)
  if (
    !Number.isFinite(orbitRadius)
    || !Number.isFinite(orbitRadius * orbitRadius)
  ) return prepareRejected('numeric')
  if (orbitRadius < input.minimumOrbitRadius) return prepareRejected('grab_on_axis')
  const restRadialUnit = scaled(restRadial, 1 / orbitRadius)
  const positiveTangentUnit = scaled(
    cross(axisUnit, restRadialUnit),
    input.movingRotationSign,
  )
  if (
    !finitePoint(orbitCenter)
    || !unitVector(restRadialUnit)
    || !unitVector(positiveTangentUnit)
  ) return prepareRejected('numeric')

  const session = freezeSession({
    mapping: FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
    contextKey: input.contextKey,
    movingRotationSign: input.movingRotationSign,
    appliedAngleDegrees: input.appliedAngleDegrees,
    axisOrigin: input.axisStart,
    axisUnit,
    orbitCenter,
    restRadialUnit,
    positiveTangentUnit,
    orbitRadius,
    rayMinimumDistance: input.startRay.minimumDistance,
    rayMaximumDistance: input.startRay.maximumDistance,
    minimumOrbitRadius: input.minimumOrbitRadius,
  })
  if (!validSession(session)) return prepareRejected('numeric')

  const expectedGrabWorldPoint = orbitPoint(
    session,
    degreesToRadians(input.appliedAngleDegrees),
  )
  const poseScale = Math.max(
    orbitRadius,
    input.minimumOrbitRadius,
    length(input.axisStart),
    length(input.axisEnd),
    length(input.grabRestWorldPoint),
    length(input.grabWorldPoint),
  )
  const relativePoseTolerance = Math.max(
    orbitRadius * POSE_SYNC_RELATIVE_TOLERANCE,
    input.minimumOrbitRadius * POSE_SYNC_RELATIVE_TOLERANCE,
  )
  const numericPoseFloor = Number.EPSILON * poseScale * 256
  if (
    !Number.isFinite(relativePoseTolerance)
    || !Number.isFinite(numericPoseFloor)
    || numericPoseFloor > relativePoseTolerance
  ) return prepareRejected('numeric')
  const poseSyncTolerance = Math.max(relativePoseTolerance, numericPoseFloor)
  if (
    !expectedGrabWorldPoint
    || distance(expectedGrabWorldPoint, input.grabWorldPoint) > poseSyncTolerance
  ) return prepareRejected('pose_mismatch')

  const startRay = copyValidatedRay(input.startRay)
  if (!startRay) return prepareRejected('invalid_input')
  const startHit = pointToRayDistance(
    input.grabWorldPoint,
    startRay,
  )
  const startRayTolerance = Math.max(
    orbitRadius * START_RAY_RELATIVE_TOLERANCE,
    input.minimumOrbitRadius * START_RAY_RELATIVE_TOLERANCE,
    Number.EPSILON * 256,
  )
  if (
    !startHit
    || startHit.distance > startRayTolerance
  ) return prepareRejected('start_ray_miss')

  const startResolution = resolvePreparedSession(session, {
    contextKey: input.contextKey,
    referenceAngleDegrees: input.appliedAngleDegrees,
    ray: input.startRay,
  })
  if (
    startResolution.kind !== 'unverified_target'
    || Math.abs(
      degreesToRadians(startResolution.rawAngleDegrees)
      - degreesToRadians(input.appliedAngleDegrees)
    ) > START_ANGLE_TOLERANCE
  ) return prepareRejected('unresolvable_start')

  return Object.freeze({ kind: 'ready', session })
}

export function resolveFoldPreviewPhysicalGrabTarget(
  session: FoldPreviewPhysicalGrabSession,
  input: FoldPreviewPhysicalGrabResolveInput,
): FoldPreviewPhysicalGrabResolveResult {
  if (!validSession(session)) return resolveRejected('invalid_session')
  return resolvePreparedSession(session, input)
}

function resolvePreparedSession(
  session: FoldPreviewPhysicalGrabSession,
  input: FoldPreviewPhysicalGrabResolveInput,
): FoldPreviewPhysicalGrabResolveResult {
  if (
    !input
    || typeof input !== 'object'
    || !validContextKey(input.contextKey)
    || !validAngleDegrees(input.referenceAngleDegrees)
    || !validRay(input.ray)
    || input.ray.minimumDistance !== session.rayMinimumDistance
    || input.ray.maximumDistance !== session.rayMaximumDistance
  ) return resolveRejected('invalid_input')
  if (input.contextKey !== session.contextKey) return resolveRejected('stale_context')
  const ray = copyValidatedRay(input.ray)
  if (!ray) return resolveRejected('invalid_input')
  const referenceAngle = degreesToRadians(input.referenceAngleDegrees)

  let evaluationCount = 0
  let workLimitReached = false
  const cache = new Map<number, EvaluatedPoint>()
  const evaluate = (angle: number): EvaluatedPoint => {
    const boundedAngle = Math.min(MAX_ANGLE, Math.max(MIN_ANGLE, angle))
    const cached = cache.get(boundedAngle)
    if (cached) return cached
    evaluationCount += 1
    if (evaluationCount > MAX_EVALUATIONS) {
      workLimitReached = true
      return INVALID_EVALUATION
    }
    const point = orbitPoint(session, boundedAngle)
    if (!point) return cacheEvaluation(cache, boundedAngle, INVALID_EVALUATION)
    const delta = subtract(point, ray.origin)
    const depth = dot(delta, ray.direction)
    if (
      !Number.isFinite(depth)
      || depth < ray.minimumDistance
      || depth > ray.maximumDistance
    ) return cacheEvaluation(cache, boundedAngle, INVALID_EVALUATION)
    const perpendicular = subtract(delta, scaled(ray.direction, depth))
    const score = lengthSquared(perpendicular)
    const result = Number.isFinite(score) && score >= 0
      ? Object.freeze({ score, depth })
      : INVALID_EVALUATION
    return cacheEvaluation(cache, boundedAngle, result)
  }

  const candidateAngles = enumerateCandidateAngles(session, ray)
  if (candidateAngles.kind === 'rejected') {
    return resolveRejected(candidateAngles.reason)
  }
  const candidates: Candidate[] = []
  addCandidate(candidates, 0, evaluate(0))
  addCandidate(candidates, MAX_ANGLE, evaluate(MAX_ANGLE))
  addCandidate(candidates, referenceAngle, evaluate(referenceAngle))
  for (const angle of candidateAngles.stationaryAngles) {
    addCandidate(candidates, angle, evaluate(angle))
  }
  for (const angle of candidateAngles.boundaryAngles) {
    addCandidate(candidates, angle, evaluate(angle))
  }
  if (workLimitReached) return resolveRejected('work_limit')

  const uniqueCandidates = deduplicateCandidates(candidates)
  if (uniqueCandidates.length === 0) return resolveRejected('no_visible_candidate')
  let bestScore = Number.POSITIVE_INFINITY
  for (const candidate of uniqueCandidates) {
    bestScore = Math.min(bestScore, candidate.score)
  }
  if (!Number.isFinite(bestScore)) return resolveRejected('no_visible_candidate')
  const scoreTolerance = Math.max(
    session.orbitRadius * session.orbitRadius * SCORE_RELATIVE_TOLERANCE,
    bestScore * SCORE_BEST_RELATIVE_TOLERANCE,
    Number.EPSILON * Number.EPSILON * 256,
  )
  const equivalent = uniqueCandidates
    .filter((candidate) => candidate.score <= bestScore + scoreTolerance)
    .sort((first, second) => {
      const firstDistance = Math.abs(first.angle - referenceAngle)
      const secondDistance = Math.abs(second.angle - referenceAngle)
      return firstDistance - secondDistance || first.angle - second.angle
    })
  if (equivalent.length === 0) return resolveRejected('numeric')
  if (equivalent.length > 1) {
    const firstDistance = Math.abs(equivalent[0].angle - referenceAngle)
    const secondDistance = Math.abs(equivalent[1].angle - referenceAngle)
    if (
      !Number.isFinite(firstDistance)
      || !Number.isFinite(secondDistance)
      || secondDistance - firstDistance <= REFERENCE_TIE_TOLERANCE
    ) return resolveRejected('ambiguous_projection')
  }
  const selected = equivalent[0]
  if (
    Math.abs(selected.angle - referenceAngle) > MAXIMUM_BRANCH_JUMP
  ) return resolveRejected('branch_jump')
  const missDistance = Math.sqrt(selected.score)
  if (
    !Number.isFinite(missDistance)
    || missDistance > session.orbitRadius * MAXIMUM_MISS_RATIO
  ) return resolveRejected('target_too_far')

  const sensitivity = projectionSensitivity(
    selected,
    evaluate,
    session.orbitRadius,
  )
  if (workLimitReached) return resolveRejected('work_limit')
  if (!sensitivity) return resolveRejected('unstable_projection')

  const rawAngleDegrees = radiansToDegrees(selected.angle)
  const angleDegrees = quantizeAngle(rawAngleDegrees)
  if (
    !validAngleDegrees(rawAngleDegrees)
    || !validAngleDegrees(angleDegrees)
  ) return resolveRejected('numeric')
  const resolvedPoint = orbitPoint(session, degreesToRadians(angleDegrees))
  if (!resolvedPoint) return resolveRejected('numeric')
  const resolvedHit = pointToRayDistance(resolvedPoint, ray)
  if (!resolvedHit) return resolveRejected('no_visible_candidate')
  if (
    resolvedHit.distance > session.orbitRadius * MAXIMUM_MISS_RATIO
  ) return resolveRejected('target_too_far')
  return Object.freeze({
    kind: 'unverified_target',
    mapping: FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING,
    contextKey: session.contextKey,
    angleDegrees,
    rawAngleDegrees,
    endpoint: angleDegrees === 0
      ? 'zero'
      : angleDegrees === 180
        ? 'one_eighty'
        : null,
    missDistance: resolvedHit.distance,
    orbitWorldPoint: freezePoint(resolvedPoint),
    evaluationCount,
    rootEvaluationCount: candidateAngles.rootEvaluationCount,
    stationaryCandidateCount: candidateAngles.stationaryAngles.length,
    boundaryCandidateCount: candidateAngles.boundaryAngles.length,
    equivalentCandidateCount: equivalent.length,
  })
}

function enumerateCandidateAngles(
  session: FoldPreviewPhysicalGrabSession,
  ray: ValidatedRay,
): CandidateAngleResult {
  const centerOffset = subtract(session.orbitCenter, ray.origin)
  const restOffset = scaled(session.restRadialUnit, session.orbitRadius)
  const tangentOffset = scaled(
    session.positiveTangentUnit,
    session.orbitRadius,
  )
  const depthCenter = dot(centerOffset, ray.direction)
  const depthCos = dot(restOffset, ray.direction)
  const depthSin = dot(tangentOffset, ray.direction)
  const depthAmplitude = Math.hypot(depthCos, depthSin)
  if (
    !Number.isFinite(depthCenter)
    || !Number.isFinite(depthCos)
    || !Number.isFinite(depthSin)
    || !Number.isFinite(depthAmplitude)
  ) return candidateAnglesRejected('numeric')
  if (
    depthCenter + depthAmplitude < ray.minimumDistance
    || depthCenter - depthAmplitude > ray.maximumDistance
  ) return candidateAnglesRejected('no_visible_candidate')

  const centerPerpendicular = perpendicularToRay(centerOffset, ray.direction)
  const restPerpendicular = perpendicularToRay(restOffset, ray.direction)
  const tangentPerpendicular = perpendicularToRay(tangentOffset, ray.direction)
  if (
    !finitePoint(centerPerpendicular)
    || !finitePoint(restPerpendicular)
    || !finitePoint(tangentPerpendicular)
  ) return candidateAnglesRejected('numeric')

  const scoreCos = 2 * dot(centerPerpendicular, restPerpendicular)
  const scoreSin = 2 * dot(centerPerpendicular, tangentPerpendicular)
  const scoreCos2 = (
    lengthSquared(restPerpendicular)
    - lengthSquared(tangentPerpendicular)
  ) / 2
  const scoreSin2 = dot(restPerpendicular, tangentPerpendicular)
  const stationary = enumerateTrigonometricQuadraticRoots({
    a0: 0,
    aCos: scoreSin,
    aSin: -scoreCos,
    aCos2: 2 * scoreSin2,
    aSin2: -2 * scoreCos2,
  })
  if (stationary.kind === 'rejected') {
    return candidateAnglesRejected(rootRejectionReason(stationary))
  }

  let rootEvaluationCount = stationary.evaluationCount
  const boundaryAngles: number[] = []
  const boundaries = Number.isFinite(ray.maximumDistance)
    ? [ray.minimumDistance, ray.maximumDistance]
    : [ray.minimumDistance]
  for (const boundary of boundaries) {
    const coefficients = {
      a0: depthCenter - boundary,
      aCos: depthCos,
      aSin: depthSin,
      aCos2: 0,
      aSin2: 0,
    }
    if (Object.values(coefficients).every((coefficient) => coefficient === 0)) {
      continue
    }
    const result = enumerateTrigonometricQuadraticRoots(coefficients)
    if (result.kind === 'rejected') {
      return candidateAnglesRejected(rootRejectionReason(result))
    }
    rootEvaluationCount += result.evaluationCount
    if (!Number.isSafeInteger(rootEvaluationCount)) {
      return candidateAnglesRejected('numeric')
    }
    boundaryAngles.push(...result.rootsRadians)
  }

  return Object.freeze({
    kind: 'success',
    stationaryAngles: Object.freeze([...stationary.rootsRadians]),
    boundaryAngles: Object.freeze(boundaryAngles),
    rootEvaluationCount,
  })
}

function perpendicularToRay(
  value: FoldPreviewPhysicalGrabPoint,
  direction: FoldPreviewPhysicalGrabPoint,
) {
  return subtract(value, scaled(direction, dot(value, direction)))
}

function rootRejectionReason(
  result: Extract<TrigonometricQuadraticRootResult, { kind: 'rejected' }>,
): FoldPreviewPhysicalGrabResolveReason {
  return result.reason === 'work_limit'
    ? 'work_limit'
    : result.reason === 'ambiguous'
      ? 'ambiguous_projection'
      : 'numeric'
}

function candidateAnglesRejected(
  reason: FoldPreviewPhysicalGrabResolveReason,
): CandidateAngleResult {
  return Object.freeze({ kind: 'rejected', reason })
}

function projectionSensitivity(
  selected: Candidate,
  evaluate: (angle: number) => EvaluatedPoint,
  orbitRadius: number,
) {
  const neighbors: EvaluatedPoint[] = []
  if (selected.angle > 0) {
    neighbors.push(evaluate(Math.max(0, selected.angle - SENSITIVITY_STEP)))
  }
  if (selected.angle < MAX_ANGLE) {
    neighbors.push(evaluate(Math.min(MAX_ANGLE, selected.angle + SENSITIVITY_STEP)))
  }
  if (neighbors.length === 0 || neighbors.some((neighbor) => !Number.isFinite(neighbor.score))) {
    return false
  }
  const minimumIncrease = Math.min(
    ...neighbors.map((neighbor) => neighbor.score - selected.score),
  )
  const normalizedIncrease = minimumIncrease / (orbitRadius * orbitRadius)
  return Number.isFinite(normalizedIncrease)
    && normalizedIncrease >= MINIMUM_NORMALIZED_SENSITIVITY
}

function addCandidate(
  candidates: Candidate[],
  angle: number,
  evaluation: EvaluatedPoint,
) {
  if (
    !Number.isFinite(angle)
    || angle < MIN_ANGLE
    || angle > MAX_ANGLE
    || !Number.isFinite(evaluation.score)
    || !Number.isFinite(evaluation.depth)
  ) return
  candidates.push(Object.freeze({
    angle,
    score: evaluation.score,
    depth: evaluation.depth,
  }))
}

function deduplicateCandidates(candidates: readonly Candidate[]) {
  const sorted = [...candidates].sort((first, second) =>
    first.angle - second.angle || first.score - second.score)
  const result: Candidate[] = []
  for (const candidate of sorted) {
    const previous = result.at(-1)
    if (
      previous
      && Math.abs(previous.angle - candidate.angle) <= CANDIDATE_ANGLE_TOLERANCE
    ) {
      if (candidate.score < previous.score) result[result.length - 1] = candidate
      continue
    }
    result.push(candidate)
  }
  return result
}

function orbitPoint(
  session: FoldPreviewPhysicalGrabSession,
  angle: number,
): MutablePoint | null {
  if (!Number.isFinite(angle) || angle < MIN_ANGLE || angle > MAX_ANGLE) return null
  const cosine = Math.cos(angle)
  const sine = Math.sin(angle)
  const radial = add(
    scaled(session.restRadialUnit, cosine),
    scaled(session.positiveTangentUnit, sine),
  )
  const point = add(session.orbitCenter, scaled(radial, session.orbitRadius))
  return finitePoint(point) ? point : null
}

function copyValidatedRay(ray: FoldPreviewPhysicalGrabRay) {
  if (!validRay(ray)) return null
  return {
    origin: copyPoint(ray.origin),
    direction: copyPoint(ray.direction),
    minimumDistance: ray.minimumDistance,
    maximumDistance: ray.maximumDistance,
  }
}

function pointToRayDistance(
  point: FoldPreviewPhysicalGrabPoint,
  ray: Readonly<{
    origin: FoldPreviewPhysicalGrabPoint
    direction: FoldPreviewPhysicalGrabPoint
    minimumDistance: number
    maximumDistance: number
  }>,
) {
  const delta = subtract(point, ray.origin)
  const depth = dot(delta, ray.direction)
  if (
    !Number.isFinite(depth)
    || depth < ray.minimumDistance
    || depth > ray.maximumDistance
  ) return null
  const perpendicular = subtract(delta, scaled(ray.direction, depth))
  const distanceValue = length(perpendicular)
  return Number.isFinite(distanceValue)
    ? { depth, distance: distanceValue }
    : null
}

function validSession(value: unknown): value is FoldPreviewPhysicalGrabSession {
  if (!value || typeof value !== 'object' || !Object.isFrozen(value)) return false
  const session = value as Partial<FoldPreviewPhysicalGrabSession>
  if (
    session.mapping !== FOLD_PREVIEW_PHYSICAL_GRAB_MAPPING
    || !validContextKey(session.contextKey)
    || (session.movingRotationSign !== 1 && session.movingRotationSign !== -1)
    || !validAngleDegrees(session.appliedAngleDegrees)
    || !finiteFrozenPoint(session.axisOrigin)
    || !finiteFrozenPoint(session.axisUnit)
    || !finiteFrozenPoint(session.orbitCenter)
    || !finiteFrozenPoint(session.restRadialUnit)
    || !finiteFrozenPoint(session.positiveTangentUnit)
    || !unitVector(session.axisUnit)
    || !unitVector(session.restRadialUnit)
    || !unitVector(session.positiveTangentUnit)
    || !Number.isFinite(session.orbitRadius)
    || !Number.isFinite((session.orbitRadius as number) * (session.orbitRadius as number))
    || !Number.isFinite(session.minimumOrbitRadius)
    || (session.orbitRadius as number) < (session.minimumOrbitRadius as number)
    || (session.minimumOrbitRadius as number) <= 0
    || !Number.isFinite(session.rayMinimumDistance)
    || !validMaximumDistance(session.rayMaximumDistance)
    || (session.rayMinimumDistance as number) < 0
    || (session.rayMaximumDistance as number)
      <= (session.rayMinimumDistance as number)
  ) return false
  const axis = session.axisUnit
  const rest = session.restRadialUnit
  const tangent = session.positiveTangentUnit
  const expectedTangent = scaled(
    cross(axis, rest),
    session.movingRotationSign,
  )
  const centerOffset = subtract(session.orbitCenter, session.axisOrigin)
  const centerOffAxis = length(cross(centerOffset, axis))
  const centerScale = Math.max(1, length(centerOffset))
  return Number.isFinite(centerOffAxis)
    && centerOffAxis <= centerScale * UNIT_TOLERANCE
    && Math.abs(dot(axis, rest)) <= UNIT_TOLERANCE
    && Math.abs(dot(axis, tangent)) <= UNIT_TOLERANCE
    && Math.abs(dot(rest, tangent)) <= UNIT_TOLERANCE
    && distance(expectedTangent, tangent) <= UNIT_TOLERANCE
}

function freezeSession(
  session: FoldPreviewPhysicalGrabSession,
): FoldPreviewPhysicalGrabSession {
  return Object.freeze({
    ...session,
    axisOrigin: freezePoint(session.axisOrigin),
    axisUnit: freezePoint(session.axisUnit),
    orbitCenter: freezePoint(session.orbitCenter),
    restRadialUnit: freezePoint(session.restRadialUnit),
    positiveTangentUnit: freezePoint(session.positiveTangentUnit),
  })
}

function validRay(value: unknown): value is FoldPreviewPhysicalGrabRay {
  if (!value || typeof value !== 'object') return false
  const ray = value as Partial<FoldPreviewPhysicalGrabRay>
  return finitePoint(ray.origin)
    && finitePoint(ray.direction)
    && unitVector(ray.direction)
    && Number.isFinite(ray.minimumDistance)
    && validMaximumDistance(ray.maximumDistance)
    && (ray.minimumDistance as number) >= 0
    && (ray.maximumDistance as number) > (ray.minimumDistance as number)
}

function validMaximumDistance(value: unknown): value is number {
  return value === Number.POSITIVE_INFINITY || Number.isFinite(value)
}

function validContextKey(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0
}

function validAngleDegrees(value: unknown): value is number {
  return Number.isFinite(value)
    && (value as number) >= 0
    && (value as number) <= 180
}

function finitePoint(value: unknown): value is FoldPreviewPhysicalGrabPoint {
  if (!value || typeof value !== 'object') return false
  const point = value as Partial<FoldPreviewPhysicalGrabPoint>
  return Number.isFinite(point.x)
    && Number.isFinite(point.y)
    && Number.isFinite(point.z)
}

function finiteFrozenPoint(value: unknown): value is FoldPreviewPhysicalGrabPoint {
  return finitePoint(value) && Object.isFrozen(value)
}

function unitVector(value: FoldPreviewPhysicalGrabPoint) {
  const vectorLength = length(value)
  return Number.isFinite(vectorLength)
    && Math.abs(vectorLength - 1) <= UNIT_TOLERANCE
}

function normalized(value: FoldPreviewPhysicalGrabPoint): MutablePoint | null {
  const vectorLength = length(value)
  if (!Number.isFinite(vectorLength) || vectorLength <= 0) return null
  const result = scaled(value, 1 / vectorLength)
  return finitePoint(result) ? result : null
}

function dot(
  first: FoldPreviewPhysicalGrabPoint,
  second: FoldPreviewPhysicalGrabPoint,
) {
  return first.x * second.x + first.y * second.y + first.z * second.z
}

function cross(
  first: FoldPreviewPhysicalGrabPoint,
  second: FoldPreviewPhysicalGrabPoint,
): MutablePoint {
  return {
    x: first.y * second.z - first.z * second.y,
    y: first.z * second.x - first.x * second.z,
    z: first.x * second.y - first.y * second.x,
  }
}

function add(
  first: FoldPreviewPhysicalGrabPoint,
  second: FoldPreviewPhysicalGrabPoint,
): MutablePoint {
  return {
    x: first.x + second.x,
    y: first.y + second.y,
    z: first.z + second.z,
  }
}

function subtract(
  first: FoldPreviewPhysicalGrabPoint,
  second: FoldPreviewPhysicalGrabPoint,
): MutablePoint {
  return {
    x: first.x - second.x,
    y: first.y - second.y,
    z: first.z - second.z,
  }
}

function scaled(
  value: FoldPreviewPhysicalGrabPoint,
  scale: number,
): MutablePoint {
  return {
    x: value.x * scale,
    y: value.y * scale,
    z: value.z * scale,
  }
}

function length(value: FoldPreviewPhysicalGrabPoint) {
  return Math.hypot(value.x, value.y, value.z)
}

function lengthSquared(value: FoldPreviewPhysicalGrabPoint) {
  return value.x * value.x + value.y * value.y + value.z * value.z
}

function distance(
  first: FoldPreviewPhysicalGrabPoint,
  second: FoldPreviewPhysicalGrabPoint,
) {
  return length(subtract(first, second))
}

function copyPoint(value: FoldPreviewPhysicalGrabPoint): MutablePoint {
  return { x: value.x, y: value.y, z: value.z }
}

function freezePoint(
  value: FoldPreviewPhysicalGrabPoint,
): FoldPreviewPhysicalGrabPoint {
  return Object.freeze(copyPoint(value))
}

function cacheEvaluation(
  cache: Map<number, EvaluatedPoint>,
  angle: number,
  value: EvaluatedPoint,
) {
  cache.set(angle, value)
  return value
}

function quantizeAngle(value: number) {
  const bounded = Math.min(180, Math.max(0, value))
  const rounded = Math.round(bounded * 10) / 10
  return Object.is(rounded, -0) ? 0 : rounded
}

function degreesToRadians(value: number) {
  return value * Math.PI / 180
}

function radiansToDegrees(value: number) {
  return value * 180 / Math.PI
}

function prepareRejected(
  reason: FoldPreviewPhysicalGrabPrepareReason,
): FoldPreviewPhysicalGrabPrepareResult {
  return Object.freeze({ kind: 'rejected', reason })
}

function resolveRejected(
  reason: FoldPreviewPhysicalGrabResolveReason,
): FoldPreviewPhysicalGrabResolveResult {
  return Object.freeze({ kind: 'rejected', reason })
}

const INVALID_EVALUATION: EvaluatedPoint = Object.freeze({
  score: Number.POSITIVE_INFINITY,
  depth: Number.NaN,
})
