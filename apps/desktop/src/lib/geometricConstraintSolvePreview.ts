import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import {
  GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID,
} from './geometricConstraints.ts'

const MAX_CHANGED_VERTICES = 256
const MAX_SOLVER_CONSTRAINTS = 1_024
const MAX_SOLVER_EQUATIONS = MAX_SOLVER_CONSTRAINTS * 2
const MAX_SOLVER_DEGREES_OF_FREEDOM = MAX_CHANGED_VERTICES * 2

export type GeometricConstraintSolvePreview = Readonly<{
  token: string
  revision: number
  iterations: number
  maximumResidual: number
  rank: number
  degreesOfFreedom: number
  equationCount: number
  conditionEstimate: number
  systemClassification:
    | 'under_constrained'
    | 'over_constrained'
    | 'well_constrained'
  changedVertices: readonly Readonly<{
    vertexId: string
    x: number
    y: number
  }>[]
  exactSatisfaction?: Readonly<{
    modelId:
      typeof GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID
    constraintCount: number
    equationCount: number
    authorizesProjectMutation: false
    replayableAcrossRuntimes: false
  }>
}>

export function normalizeGeometricConstraintSolvePreview(
  value: unknown,
): GeometricConstraintSolvePreview | null {
  const source = snapshotDataRecord(value)
  if (!source) return null
  const requiredKeys = [
    'token',
    'revision',
    'iterations',
    'maximumResidual',
    'rank',
    'degreesOfFreedom',
    'equationCount',
    'conditionEstimate',
    'systemClassification',
    'changedVertices',
  ] as const
  if (
    Object.keys(source).some((key) =>
      key !== 'exactSatisfaction'
      && !requiredKeys.includes(key as typeof requiredKeys[number]))
    || requiredKeys.some((key) => !Object.hasOwn(source, key))
    || !isCanonicalNonNilUuid(source.token)
    || !isBoundedInteger(source.revision, 0, Number.MAX_SAFE_INTEGER)
    || !isBoundedInteger(source.iterations, 0, 32)
    || !isFiniteNonnegative(source.maximumResidual)
    || !isBoundedInteger(source.rank, 0, MAX_SOLVER_EQUATIONS)
    || !isBoundedInteger(
      source.degreesOfFreedom,
      0,
      MAX_SOLVER_DEGREES_OF_FREEDOM,
    )
    || !isBoundedInteger(source.equationCount, 1, MAX_SOLVER_EQUATIONS)
    || source.rank > source.equationCount
    || !isFiniteNonnegative(source.conditionEstimate)
    || (
      source.systemClassification !== 'under_constrained'
      && source.systemClassification !== 'over_constrained'
      && source.systemClassification !== 'well_constrained'
    )
  ) return null
  const expectedClassification = source.degreesOfFreedom > 0
    ? 'under_constrained'
    : source.equationCount > source.rank
      ? 'over_constrained'
      : 'well_constrained'
  if (source.systemClassification !== expectedClassification) return null

  const rawVertices = snapshotDataArray(
    source.changedVertices,
    MAX_CHANGED_VERTICES,
  )
  if (!rawVertices) return null
  const seen = new Set<string>()
  const changedVertices: Array<{ vertexId: string; x: number; y: number }> = []
  for (const rawVertex of rawVertices) {
    const vertex = exactDataRecord(rawVertex, ['vertexId', 'x', 'y'] as const)
    if (
      !vertex
      || !isCanonicalNonNilUuid(vertex.vertexId)
      || seen.has(vertex.vertexId)
      || typeof vertex.x !== 'number'
      || !Number.isFinite(vertex.x)
      || typeof vertex.y !== 'number'
      || !Number.isFinite(vertex.y)
    ) return null
    seen.add(vertex.vertexId)
    changedVertices.push({
      vertexId: vertex.vertexId,
      x: vertex.x,
      y: vertex.y,
    })
  }

  let exactSatisfaction:
    GeometricConstraintSolvePreview['exactSatisfaction'] | undefined
  if (Object.hasOwn(source, 'exactSatisfaction')) {
    const exact = exactDataRecord(source.exactSatisfaction, [
      'modelId',
      'constraintCount',
      'equationCount',
      'authorizesProjectMutation',
      'replayableAcrossRuntimes',
    ] as const)
    if (
      !exact
      || exact.modelId
        !== GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID
      || !isBoundedInteger(
        exact.constraintCount,
        1,
        MAX_SOLVER_CONSTRAINTS,
      )
      || exact.equationCount !== source.equationCount
      || exact.constraintCount > source.equationCount
      || exact.authorizesProjectMutation !== false
      || exact.replayableAcrossRuntimes !== false
    ) return null
    exactSatisfaction = {
      modelId: exact.modelId,
      constraintCount: exact.constraintCount,
      equationCount: exact.equationCount,
      authorizesProjectMutation: false,
      replayableAcrossRuntimes: false,
    }
  }

  return {
    token: source.token,
    revision: source.revision,
    iterations: source.iterations,
    maximumResidual: source.maximumResidual,
    rank: source.rank,
    degreesOfFreedom: source.degreesOfFreedom,
    equationCount: source.equationCount,
    conditionEstimate: source.conditionEstimate,
    systemClassification: source.systemClassification,
    changedVertices,
    ...(exactSatisfaction ? { exactSatisfaction } : {}),
  }
}

function isFiniteNonnegative(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
}

function isBoundedInteger(
  value: unknown,
  minimum: number,
  maximum: number,
): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && value >= minimum
    && value <= maximum
}

function exactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expectedKeys: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  const record = snapshotDataRecord(value)
  if (!record) return null
  const actualKeys = Object.keys(record)
  return actualKeys.length === expectedKeys.length
    && expectedKeys.every((key) => Object.hasOwn(record, key))
    ? record as Readonly<Record<Keys[number], unknown>>
    : null
}

function snapshotDataRecord(value: unknown): Record<string, unknown> | null {
  try {
    if (
      value === null
      || typeof value !== 'object'
      || Array.isArray(value)
    ) return null
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return null
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const snapshot = Object.create(null) as Record<string, unknown>
    for (const key of Reflect.ownKeys(descriptors)) {
      if (typeof key !== 'string') return null
      const descriptor = descriptors[key]
      if (!descriptor || !('value' in descriptor) || !descriptor.enumerable) {
        return null
      }
      snapshot[key] = descriptor.value
    }
    return snapshot
  } catch {
    return null
  }
}

function snapshotDataArray(
  value: unknown,
  maximumLength: number,
): readonly unknown[] | null {
  try {
    if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
      return null
    }
    const descriptors = Object.getOwnPropertyDescriptors(value)
    const lengthDescriptor = Reflect.getOwnPropertyDescriptor(value, 'length')
    const length = lengthDescriptor && 'value' in lengthDescriptor
      ? lengthDescriptor.value
      : null
    if (
      typeof length !== 'number'
      || !Number.isSafeInteger(length)
      || length < 0
      || length > maximumLength
    ) return null
    const keys = Reflect.ownKeys(descriptors)
    if (
      keys.length !== length + 1
      || keys.some((key) =>
        typeof key !== 'string'
        || (
          key !== 'length'
          && (
            !/^(?:0|[1-9][0-9]*)$/u.test(key)
            || Number(key) >= length
          )
        ))
    ) return null
    const snapshot: unknown[] = []
    for (let index = 0; index < length; index += 1) {
      const descriptor = descriptors[String(index)]
      if (!descriptor || !('value' in descriptor) || !descriptor.enumerable) {
        return null
      }
      snapshot.push(descriptor.value)
    }
    return snapshot
  } catch {
    return null
  }
}
