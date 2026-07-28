import type {
  GeometricConstraintSemanticMusV1,
} from './geometricConstraintSemanticMus.ts'
import {
  GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
  isParsedCertifiedGeometricConstraintSemanticMus,
  MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
  MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK,
} from './geometricConstraintSemanticMus.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from './deterministicTranscendentalModel.ts'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'

type CertifiedSemanticMus = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

const CERTIFIED_SEMANTIC_MUS_KEYS = [
  'status',
  'model_id',
  'transcendental_model_id',
  'constraint_ids',
  'constraint_count',
  'direct_oracle_calls',
  'deletion_witness_checks',
  'deletion_witness_work',
  'current_assignment_witness_count',
  'axis_exactification_witness_count',
  'single_constraint_constructive_witness_count',
  'pair_constraint_constructive_witness_count',
  'pair_constraint_algebraic_witness_count',
  'length_constraint_constructive_witness_count',
  'zero_length_closure_constructive_witness_count',
  'authorizes_project_mutation',
  'replayable_across_runtimes',
] as const

export type GeometricConstraintSemanticMusCertifiedViewModel = Readonly<{
  transcendentalModelId:
    typeof DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
  constraintIds: readonly string[]
  constraintCount: number
  directOracleCalls: number
  deletionWitnessChecks: number
  deletionWitnessWork: number
  currentAssignmentWitnessCount: number
  axisExactificationWitnessCount: number
  singleConstraintConstructiveWitnessCount: number
  pairConstraintConstructiveWitnessCount: number
  pairConstraintAlgebraicWitnessCount: number
  lengthConstraintConstructiveWitnessCount: number
  zeroLengthClosureConstructiveWitnessCount: number
  replayableAcrossRuntimes: boolean
}>

/**
 * Revalidates the proof-count invariant immediately before presentation.
 *
 * The strict wire parser already enforces this invariant. Keeping the display
 * projection fail-closed prevents a forged in-process typed value from
 * overstating how many deletion witnesses were actually certified.
 */
export function buildGeometricConstraintSemanticMusCertifiedViewModel(
  result: CertifiedSemanticMus,
): GeometricConstraintSemanticMusCertifiedViewModel | null {
  try {
    const snapshot = snapshotCertifiedSemanticMus(result)
    return snapshot === null ? null : buildCertifiedViewModel(snapshot)
  } catch {
    return null
  }
}

function snapshotCertifiedSemanticMus(
  value: unknown,
): CertifiedSemanticMus | null {
  // This identity check is deliberately first: WeakSet membership cannot run
  // a getter or Proxy trap. Only a deeply frozen value issued by the strict
  // wire parser reaches descriptor snapshotting below.
  if (!isParsedCertifiedGeometricConstraintSemanticMus(value)) return null
  const record = snapshotExactDataRecord(value, CERTIFIED_SEMANTIC_MUS_KEYS)
  if (
    !record
    || record.status !== 'certified'
    || record.model_id
      !== GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID
    || record.transcendental_model_id
      !== DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    || record.authorizes_project_mutation !== false
    || typeof record.replayable_across_runtimes !== 'boolean'
  ) return null
  const constraintIds = snapshotConstraintIds(record.constraint_ids)
  if (!constraintIds) return null
  return {
    status: 'certified',
    model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    constraint_ids: constraintIds,
    constraint_count: record.constraint_count as number,
    direct_oracle_calls: record.direct_oracle_calls as number,
    deletion_witness_checks: record.deletion_witness_checks as number,
    deletion_witness_work: record.deletion_witness_work as number,
    current_assignment_witness_count:
      record.current_assignment_witness_count as number,
    axis_exactification_witness_count:
      record.axis_exactification_witness_count as number,
    single_constraint_constructive_witness_count:
      record.single_constraint_constructive_witness_count as number,
    pair_constraint_constructive_witness_count:
      record.pair_constraint_constructive_witness_count as number,
    pair_constraint_algebraic_witness_count:
      record.pair_constraint_algebraic_witness_count as number,
    length_constraint_constructive_witness_count:
      record.length_constraint_constructive_witness_count as number,
    zero_length_closure_constructive_witness_count:
      record.zero_length_closure_constructive_witness_count as number,
    authorizes_project_mutation: false,
    replayable_across_runtimes:
      record.replayable_across_runtimes as boolean,
  }
}

function snapshotExactDataRecord<const Keys extends readonly string[]>(
  value: unknown,
  expected: Keys,
): Readonly<Record<Keys[number], unknown>> | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return null
  }
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return null
  const descriptors = Object.getOwnPropertyDescriptors(value)
  const keys = Reflect.ownKeys(descriptors)
  if (
    keys.length !== expected.length
    || keys.some((key) => typeof key !== 'string')
  ) return null
  const snapshot = Object.create(null) as Record<string, unknown>
  for (const key of expected) {
    const descriptor = descriptors[key]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
    ) return null
    snapshot[key] = descriptor.value
  }
  return snapshot as Readonly<Record<Keys[number], unknown>>
}

function snapshotConstraintIds(value: unknown): readonly string[] | null {
  if (!Array.isArray(value)) return null
  const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as
    Record<PropertyKey, PropertyDescriptor>
  const keys = Reflect.ownKeys(descriptors)
  const lengthDescriptor = descriptors.length
  if (
    keys.some((key) => typeof key !== 'string')
    || !lengthDescriptor
    || !('value' in lengthDescriptor)
    || lengthDescriptor.enumerable
    || !Number.isSafeInteger(lengthDescriptor.value)
    || lengthDescriptor.value < 1
    || lengthDescriptor.value
      > MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS
    || keys.length !== lengthDescriptor.value + 1
  ) return null
  const result: string[] = []
  for (let index = 0; index < lengthDescriptor.value; index += 1) {
    const descriptor = descriptors[String(index)]
    if (
      !descriptor
      || !('value' in descriptor)
      || !descriptor.enumerable
      || !isCanonicalNonNilUuid(descriptor.value)
      || (
        index > 0
        && result[index - 1]! >= descriptor.value
      )
    ) return null
    result.push(descriptor.value)
  }
  return Object.freeze(result)
}

function buildCertifiedViewModel(
  result: CertifiedSemanticMus,
): GeometricConstraintSemanticMusCertifiedViewModel | null {
  const methodCounts = [
    result.current_assignment_witness_count,
    result.axis_exactification_witness_count,
    result.single_constraint_constructive_witness_count,
    result.pair_constraint_constructive_witness_count,
    result.pair_constraint_algebraic_witness_count,
    result.length_constraint_constructive_witness_count,
    result.zero_length_closure_constructive_witness_count,
  ]
  if (
    !isPositiveSafeInteger(result.constraint_count)
    || result.constraint_count
      > MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS
    || result.constraint_ids.length !== result.constraint_count
    || result.deletion_witness_checks !== result.constraint_count
    || !isPositiveSafeInteger(result.direct_oracle_calls)
    || !isNonNegativeSafeInteger(result.deletion_witness_work)
    || result.deletion_witness_work
      > MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK
    || methodCounts.some((count) =>
      !isNonNegativeSafeInteger(count) || count > result.constraint_count
    )
    || methodCounts.reduce((sum, count) => sum + count, 0)
      !== result.constraint_count
  ) return null

  return Object.freeze({
    transcendentalModelId: result.transcendental_model_id,
    constraintIds: Object.freeze([...result.constraint_ids]),
    constraintCount: result.constraint_count,
    directOracleCalls: result.direct_oracle_calls,
    deletionWitnessChecks: result.deletion_witness_checks,
    deletionWitnessWork: result.deletion_witness_work,
    currentAssignmentWitnessCount:
      result.current_assignment_witness_count,
    axisExactificationWitnessCount:
      result.axis_exactification_witness_count,
    singleConstraintConstructiveWitnessCount:
      result.single_constraint_constructive_witness_count,
    pairConstraintConstructiveWitnessCount:
      result.pair_constraint_constructive_witness_count,
    pairConstraintAlgebraicWitnessCount:
      result.pair_constraint_algebraic_witness_count,
    lengthConstraintConstructiveWitnessCount:
      result.length_constraint_constructive_witness_count,
    zeroLengthClosureConstructiveWitnessCount:
      result.zero_length_closure_constructive_witness_count,
    replayableAcrossRuntimes: result.replayable_across_runtimes,
  })
}

function isPositiveSafeInteger(value: number): boolean {
  return isNonNegativeSafeInteger(value) && value > 0
}

function isNonNegativeSafeInteger(value: number): boolean {
  return Number.isSafeInteger(value) && !Object.is(value, -0) && value >= 0
}
