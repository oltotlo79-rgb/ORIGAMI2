export type TrigonometricQuadraticCoefficients = Readonly<{
  a0: number
  aCos: number
  aSin: number
  aCos2: number
  aSin2: number
}>

export type TrigonometricQuadraticRootRejectionReason =
  | 'numeric'
  | 'ambiguous'
  | 'work_limit'

export type TrigonometricQuadraticRootResult =
  | Readonly<{
      kind: 'success'
      /** Every distinct real root in the closed interval [0, Math.PI]. */
      rootsRadians: readonly number[]
      evaluationCount: number
    }>
  | Readonly<{
      kind: 'rejected'
      reason: TrigonometricQuadraticRootRejectionReason
    }>

const MINIMUM_ANGLE = 0
const MAXIMUM_ANGLE = Math.PI
const MAXIMUM_EVALUATIONS = 640
const BISECTION_STEPS = 64
const ROOT_VALUE_TOLERANCE = 2e-12
const AMBIGUOUS_VALUE_TOLERANCE = 2e-9
const LINEAR_BOUNDARY_TOLERANCE = 1e-10
const DUPLICATE_ROOT_TOLERANCE = 1e-13
const MINIMUM_DISTINCT_ROOT_SEPARATION = 1e-8
const FINAL_RESIDUAL_TOLERANCE = 2e-9

type InternalResult =
  | Readonly<{ kind: 'success'; roots: readonly number[] }>
  | Readonly<{
      kind: 'rejected'
      reason: TrigonometricQuadraticRootRejectionReason
    }>

type WorkBudget = {
  evaluations: number
  exhausted: boolean
}

/**
 * Enumerates the roots of
 * `a0 + aCos*cos(theta) + aSin*sin(theta)
 *     + aCos2*cos(2*theta) + aSin2*sin(2*theta)`
 * on the closed interval `[0, Math.PI]`.
 *
 * The half-angle substitution is mapped onto the compact interval
 * `x = tan(theta / 2) / (1 + tan(theta / 2))`. The resulting quartic is
 * isolated recursively by all roots of its derivative. A classification too
 * close to a multiple/no-root boundary is rejected instead of guessed.
 */
export function enumerateTrigonometricQuadraticRoots(
  coefficients: TrigonometricQuadraticCoefficients,
): TrigonometricQuadraticRootResult {
  const normalized = normalizeTrigonometricCoefficients(coefficients)
  if (normalized.kind === 'rejected') return rejected(normalized.reason)
  if (normalized.kind === 'zero') return rejected('ambiguous')

  const polynomial = compactHalfAnglePolynomial(normalized.coefficients)
  if (!polynomial || polynomial.some((coefficient) => !Number.isFinite(coefficient))) {
    return rejected('numeric')
  }

  const budget: WorkBudget = { evaluations: 0, exhausted: false }
  const isolated = isolatePolynomialRoots(polynomial, budget)
  if (isolated.kind === 'rejected') return rejected(isolated.reason)

  const rootsRadians: number[] = []
  for (const compactRoot of isolated.roots) {
    if (
      !Number.isFinite(compactRoot)
      || compactRoot < 0
      || compactRoot > 1
    ) return rejected('numeric')
    const angle = 2 * Math.atan2(compactRoot, 1 - compactRoot)
    if (
      !Number.isFinite(angle)
      || angle < MINIMUM_ANGLE
      || angle > MAXIMUM_ANGLE
    ) return rejected('numeric')
    const residual = evaluateTrigonometric(normalized.coefficients, angle, budget)
    if (budget.exhausted) return rejected('work_limit')
    if (!Number.isFinite(residual) || Math.abs(residual) > FINAL_RESIDUAL_TOLERANCE) {
      return rejected('numeric')
    }
    rootsRadians.push(normalizeAngleEndpoint(angle))
  }

  const distinct = distinctSortedRoots(rootsRadians)
  if (distinct.kind === 'rejected') return rejected(distinct.reason)
  return Object.freeze({
    kind: 'success',
    rootsRadians: Object.freeze([...distinct.roots]),
    evaluationCount: budget.evaluations,
  })
}

function compactHalfAnglePolynomial(
  coefficients: TrigonometricQuadraticCoefficients,
) {
  const {
    a0,
    aCos,
    aSin,
    aCos2,
    aSin2,
  } = coefficients

  // First form P(t) after multiplying by (1 + t^2)^2, where
  // t = tan(theta / 2).
  const p0 = a0 + aCos + aCos2
  const p1 = 2 * aSin + 4 * aSin2
  const p2 = 2 * a0 - 6 * aCos2
  const p3 = 2 * aSin - 4 * aSin2
  const p4 = a0 - aCos + aCos2

  // Compact t in [0, +infinity] to x in [0, 1] with t = x / (1 - x)
  // and multiply by (1 - x)^4.
  return [
    p0,
    -4 * p0 + p1,
    6 * p0 - 3 * p1 + p2,
    -4 * p0 + 3 * p1 - 2 * p2 + p3,
    p0 - p1 + p2 - p3 + p4,
  ]
}

function isolatePolynomialRoots(
  input: readonly number[],
  budget: WorkBudget,
): InternalResult {
  const polynomial = normalizePolynomial(input)
  if (!polynomial) return internalRejected('numeric')
  const degree = polynomial.length - 1
  if (degree === 0) return internalSuccess([])
  if (degree === 1) return isolateLinearRoot(polynomial, budget)

  const derivative = polynomial
    .slice(1)
    .map((coefficient, index) => coefficient * (index + 1))
  const criticalResult = isolatePolynomialRoots(derivative, budget)
  if (criticalResult.kind === 'rejected') return criticalResult
  const criticalRoots = criticalResult.roots

  const partitions = distinctPartitionPoints([0, ...criticalRoots, 1])
  if (!partitions) return internalRejected('ambiguous')
  const values: number[] = []
  const roots: number[] = []
  for (const point of partitions) {
    const value = evaluatePolynomial(polynomial, point, budget)
    if (budget.exhausted) return internalRejected('work_limit')
    if (!Number.isFinite(value)) return internalRejected('numeric')
    values.push(value)
    const classification = classifyStationaryValue(value)
    if (classification === 'ambiguous') return internalRejected('ambiguous')
    if (classification === 'root') roots.push(point)
  }

  for (let index = 0; index + 1 < partitions.length; index += 1) {
    const left = partitions[index]
    const right = partitions[index + 1]
    const leftValue = values[index]
    const rightValue = values[index + 1]
    if (
      Math.abs(leftValue) <= ROOT_VALUE_TOLERANCE
      || Math.abs(rightValue) <= ROOT_VALUE_TOLERANCE
      || Math.sign(leftValue) === Math.sign(rightValue)
    ) continue
    const root = bisectSignChangingRoot(
      polynomial,
      left,
      right,
      leftValue,
      budget,
    )
    if (root.kind === 'rejected') return root
    roots.push(root.root)
  }

  const distinct = distinctSortedRoots(roots)
  return distinct.kind === 'rejected'
    ? internalRejected(distinct.reason)
    : internalSuccess(distinct.roots)
}

function isolateLinearRoot(
  polynomial: readonly number[],
  budget: WorkBudget,
): InternalResult {
  const root = -polynomial[0] / polynomial[1]
  if (!Number.isFinite(root)) return internalRejected('numeric')
  if (root < 0 || root > 1) {
    if (
      Math.abs(root) <= LINEAR_BOUNDARY_TOLERANCE
      || Math.abs(root - 1) <= LINEAR_BOUNDARY_TOLERANCE
    ) return internalRejected('ambiguous')
    return internalSuccess([])
  }
  const residual = evaluatePolynomial(polynomial, root, budget)
  if (budget.exhausted) return internalRejected('work_limit')
  if (!Number.isFinite(residual)) return internalRejected('numeric')
  if (Math.abs(residual) > ROOT_VALUE_TOLERANCE) return internalRejected('numeric')
  return internalSuccess([normalizeCompactEndpoint(root)])
}

function bisectSignChangingRoot(
  polynomial: readonly number[],
  leftInput: number,
  rightInput: number,
  leftValueInput: number,
  budget: WorkBudget,
): Readonly<{ kind: 'success'; root: number }> | Extract<
  InternalResult,
  { kind: 'rejected' }
> {
  let left = leftInput
  let right = rightInput
  let leftValue = leftValueInput
  for (let step = 0; step < BISECTION_STEPS; step += 1) {
    const midpoint = left / 2 + right / 2
    if (midpoint === left || midpoint === right) break
    const midpointValue = evaluatePolynomial(polynomial, midpoint, budget)
    if (budget.exhausted) return internalRejected('work_limit')
    if (!Number.isFinite(midpointValue)) return internalRejected('numeric')
    if (midpointValue === 0) {
      left = midpoint
      right = midpoint
      break
    }
    if (Math.sign(midpointValue) === Math.sign(leftValue)) {
      left = midpoint
      leftValue = midpointValue
    } else {
      right = midpoint
    }
  }
  const root = left / 2 + right / 2
  return Number.isFinite(root)
    ? Object.freeze({ kind: 'success', root: normalizeCompactEndpoint(root) })
    : internalRejected('numeric')
}

function evaluatePolynomial(
  polynomial: readonly number[],
  x: number,
  budget: WorkBudget,
) {
  if (!consumeEvaluation(budget)) return Number.NaN
  let value = 0
  for (let index = polynomial.length - 1; index >= 0; index -= 1) {
    value = value * x + polynomial[index]
  }
  return value
}

function evaluateTrigonometric(
  coefficients: TrigonometricQuadraticCoefficients,
  angle: number,
  budget: WorkBudget,
) {
  if (!consumeEvaluation(budget)) return Number.NaN
  return coefficients.a0
    + coefficients.aCos * Math.cos(angle)
    + coefficients.aSin * Math.sin(angle)
    + coefficients.aCos2 * Math.cos(2 * angle)
    + coefficients.aSin2 * Math.sin(2 * angle)
}

function consumeEvaluation(budget: WorkBudget) {
  budget.evaluations += 1
  if (budget.evaluations > MAXIMUM_EVALUATIONS) {
    budget.exhausted = true
    return false
  }
  return true
}

function classifyStationaryValue(value: number) {
  const magnitude = Math.abs(value)
  if (magnitude <= ROOT_VALUE_TOLERANCE) return 'root' as const
  if (magnitude <= AMBIGUOUS_VALUE_TOLERANCE) return 'ambiguous' as const
  return 'non_root' as const
}

function normalizeTrigonometricCoefficients(
  value: unknown,
):
  | Readonly<{
      kind: 'success'
      coefficients: TrigonometricQuadraticCoefficients
    }>
  | Readonly<{ kind: 'zero' }>
  | Extract<InternalResult, { kind: 'rejected' }> {
  if (!value || typeof value !== 'object') return internalRejected('numeric')
  const input = value as Partial<TrigonometricQuadraticCoefficients>
  const values = [
    input.a0,
    input.aCos,
    input.aSin,
    input.aCos2,
    input.aSin2,
  ]
  if (!values.every(Number.isFinite)) return internalRejected('numeric')
  const scale = Math.max(...values.map((coefficient) => Math.abs(coefficient as number)))
  if (!Number.isFinite(scale)) return internalRejected('numeric')
  if (scale === 0) return Object.freeze({ kind: 'zero' })
  const coefficients = Object.freeze({
    a0: (input.a0 as number) / scale,
    aCos: (input.aCos as number) / scale,
    aSin: (input.aSin as number) / scale,
    aCos2: (input.aCos2 as number) / scale,
    aSin2: (input.aSin2 as number) / scale,
  })
  return Object.values(coefficients).every(Number.isFinite)
    ? Object.freeze({ kind: 'success', coefficients })
    : internalRejected('numeric')
}

function normalizePolynomial(input: readonly number[]) {
  if (input.length === 0 || input.some((coefficient) => !Number.isFinite(coefficient))) {
    return null
  }
  const coefficients = [...input]
  while (coefficients.length > 1 && coefficients.at(-1) === 0) coefficients.pop()
  const scale = Math.max(...coefficients.map(Math.abs))
  if (!Number.isFinite(scale) || scale === 0) return null
  const normalized = coefficients.map((coefficient) => coefficient / scale)
  return normalized.every(Number.isFinite) ? normalized : null
}

function distinctPartitionPoints(points: readonly number[]) {
  const sorted = [...points].sort((first, second) => first - second)
  const distinct: number[] = []
  for (const point of sorted) {
    if (!Number.isFinite(point) || point < 0 || point > 1) return null
    const previous = distinct.at(-1)
    if (previous === undefined || point !== previous) distinct.push(point)
  }
  return distinct
}

function distinctSortedRoots(
  roots: readonly number[],
):
  | Readonly<{ kind: 'success'; roots: readonly number[] }>
  | Extract<InternalResult, { kind: 'rejected' }> {
  const sorted = [...roots].sort((first, second) => first - second)
  const distinct: number[] = []
  for (const root of sorted) {
    if (!Number.isFinite(root)) return internalRejected('numeric')
    const previous = distinct.at(-1)
    if (previous === undefined) {
      distinct.push(root)
      continue
    }
    const separation = root - previous
    if (separation <= DUPLICATE_ROOT_TOLERANCE) continue
    if (separation < MINIMUM_DISTINCT_ROOT_SEPARATION) {
      return internalRejected('ambiguous')
    }
    distinct.push(root)
  }
  return Object.freeze({
    kind: 'success',
    roots: Object.freeze(distinct),
  })
}

function normalizeCompactEndpoint(value: number) {
  if (Math.abs(value) <= Number.EPSILON * 8) return 0
  if (Math.abs(value - 1) <= Number.EPSILON * 8) return 1
  return value
}

function normalizeAngleEndpoint(value: number) {
  if (Math.abs(value) <= Number.EPSILON * 16) return 0
  if (Math.abs(value - Math.PI) <= Number.EPSILON * 16) return Math.PI
  return value
}

function internalSuccess(roots: readonly number[]): InternalResult {
  return Object.freeze({
    kind: 'success',
    roots: Object.freeze([...roots]),
  })
}

function internalRejected(
  reason: TrigonometricQuadraticRootRejectionReason,
): Extract<InternalResult, { kind: 'rejected' }> {
  return Object.freeze({ kind: 'rejected', reason })
}

function rejected(
  reason: TrigonometricQuadraticRootRejectionReason,
): TrigonometricQuadraticRootResult {
  return Object.freeze({ kind: 'rejected', reason })
}
