import type {
  GeometricConstraintSemanticMusV1,
} from './geometricConstraintSemanticMus.ts'
import {
  MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
  MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK,
} from './geometricConstraintSemanticMus.ts'

type CertifiedSemanticMus = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

export type GeometricConstraintSemanticMusCertifiedViewModel = Readonly<{
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
    return buildCertifiedViewModel(result)
  } catch {
    return null
  }
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
  })
}

function isPositiveSafeInteger(value: number): boolean {
  return isNonNegativeSafeInteger(value) && value > 0
}

function isNonNegativeSafeInteger(value: number): boolean {
  return Number.isSafeInteger(value) && !Object.is(value, -0) && value >= 0
}
