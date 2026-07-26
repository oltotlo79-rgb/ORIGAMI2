export const MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS = 16
export const MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK = 20_000_000
export const GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID =
  'geometric_constraint_current_runtime_semantic_mus_v1' as const

export type GeometricConstraintSemanticMusUnknownReasonV1 =
  | 'direct_oracle_incomplete'
  | 'deletion_witness_limit_exceeded'
  | 'deletion_witness_work_limit_exceeded'
  | 'deletion_witness_unavailable'
  | 'cancelled'
  | 'deadline_reached'

export type GeometricConstraintSemanticMusV1 =
  | Readonly<{
      status: 'certified'
      model_id:
        typeof GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID
      constraint_ids: readonly string[]
      constraint_count: number
      direct_oracle_calls: number
      deletion_witness_checks: number
      deletion_witness_work: number
      current_assignment_witness_count: number
      axis_exactification_witness_count: number
      single_constraint_constructive_witness_count: number
      pair_constraint_constructive_witness_count: number
      pair_constraint_algebraic_witness_count: number
      length_constraint_constructive_witness_count: number
      authorizes_project_mutation: false
      replayable_across_runtimes: false
    }>
  | Readonly<{
      status: 'unknown'
      model_id:
        typeof GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID
      reason: GeometricConstraintSemanticMusUnknownReasonV1
      direct_core_constraint_ids: readonly string[]
      direct_oracle_calls: number
      deletion_witness_checks: number
      certified_deletion_witnesses: number
      deletion_witness_work: number
      max_deletion_witness_checks:
        typeof MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS
      max_deletion_witness_work:
        typeof MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK
      authorizes_project_mutation: false
      replayable_across_runtimes: false
    }>

type CertifiedSemanticMusV1 = Extract<
  GeometricConstraintSemanticMusV1,
  { status: 'certified' }
>

const certifiedSemanticMusValues = new WeakSet<object>()

/**
 * Checks the exact identity issued by this module's strict parser.
 *
 * WeakSet membership does not inspect the candidate, so accessor and Proxy
 * traps cannot run before the presentation boundary rejects an unissued
 * in-process value.
 */
export function isParsedCertifiedGeometricConstraintSemanticMus(
  value: unknown,
): value is CertifiedSemanticMusV1 {
  return value !== null
    && typeof value === 'object'
    && certifiedSemanticMusValues.has(value)
}

type BoundedDirectMus =
  | Readonly<{
      status: 'proven_unsatisfiable'
      constraint_ids: readonly string[]
      oracle_calls: number
    }>
  | Readonly<{
      status: 'unknown'
      reason:
        | 'constraint_limit_exceeded'
        | 'oracle_incomplete'
        | 'cancelled'
        | 'deadline_reached'
      oracle_calls: number
      max_constraints: number
    }>

type DirectConflictResult = Readonly<{
  bounded_direct_mus: BoundedDirectMus
  conflicts: readonly Readonly<{ constraint_ids: readonly string[] }>[]
}>

type StrictParserPrimitives = Readonly<{
  snapshotDataRecord(value: unknown): Record<string, unknown> | null
  hasExactKeys(
    record: Readonly<Record<string, unknown>>,
    expected: readonly string[],
  ): boolean
  parseSortedUniqueUuidArray(
    value: unknown,
    maximum: number,
  ): readonly string[] | null
  isBoundedDirectMusOracleCalls(
    value: unknown,
    allowZero: boolean,
  ): value is number
}>

export function parseGeometricConstraintSemanticMus(
  value: unknown,
  directResult: DirectConflictResult,
  parser: StrictParserPrimitives,
): GeometricConstraintSemanticMusV1 | null {
  const record = parser.snapshotDataRecord(value)
  if (!record || typeof record.status !== 'string') return null
  if (record.status === 'certified') {
    return parseCertified(record, directResult, parser)
  }
  if (record.status === 'unknown') {
    return parseUnknown(record, directResult, parser)
  }
  return null
}

function parseCertified(
  record: Record<string, unknown>,
  directResult: DirectConflictResult,
  parser: StrictParserPrimitives,
): GeometricConstraintSemanticMusV1 | null {
  if (
    !parser.hasExactKeys(record, [
      'status',
      'model_id',
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
      'authorizes_project_mutation',
      'replayable_across_runtimes',
    ])
    || record.model_id
      !== GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID
  ) return null
  const constraintIds = parser.parseSortedUniqueUuidArray(
    record.constraint_ids,
    MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
  )
  if (
    !constraintIds
    || constraintIds.length === 0
    || !isCount(
      record.constraint_count,
      MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
      false,
    )
    || record.constraint_count !== constraintIds.length
    || !parser.isBoundedDirectMusOracleCalls(
      record.direct_oracle_calls,
      false,
    )
    || !isCount(
      record.deletion_witness_checks,
      MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
      false,
    )
    || record.deletion_witness_checks !== constraintIds.length
    || !isCount(
      record.deletion_witness_work,
      MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK,
      true,
    )
    || !isCount(record.current_assignment_witness_count, constraintIds.length, true)
    || !isCount(record.axis_exactification_witness_count, constraintIds.length, true)
    || !isCount(
      record.single_constraint_constructive_witness_count,
      constraintIds.length,
      true,
    )
    || !isCount(
      record.pair_constraint_constructive_witness_count,
      constraintIds.length,
      true,
    )
    || !isCount(
      record.pair_constraint_algebraic_witness_count,
      constraintIds.length,
      true,
    )
    || !isCount(
      record.length_constraint_constructive_witness_count,
      constraintIds.length,
      true,
    )
    || record.current_assignment_witness_count
      + record.axis_exactification_witness_count
      + record.single_constraint_constructive_witness_count
      + record.pair_constraint_constructive_witness_count
      + record.pair_constraint_algebraic_witness_count
      + record.length_constraint_constructive_witness_count
      !== constraintIds.length
    || record.authorizes_project_mutation !== false
    || record.replayable_across_runtimes !== false
    || directResult.bounded_direct_mus.status !== 'proven_unsatisfiable'
    || record.direct_oracle_calls !== directResult.bounded_direct_mus.oracle_calls
    || !sameStrings(
      constraintIds,
      directResult.bounded_direct_mus.constraint_ids,
    )
    || !matchesOuterConflict(constraintIds, directResult.conflicts)
  ) return null
  const result: CertifiedSemanticMusV1 = Object.freeze({
    status: 'certified',
    model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
    constraint_ids: constraintIds,
    constraint_count: record.constraint_count,
    direct_oracle_calls: record.direct_oracle_calls,
    deletion_witness_checks: record.deletion_witness_checks,
    deletion_witness_work: record.deletion_witness_work,
    current_assignment_witness_count:
      record.current_assignment_witness_count,
    axis_exactification_witness_count:
      record.axis_exactification_witness_count,
    single_constraint_constructive_witness_count:
      record.single_constraint_constructive_witness_count,
    pair_constraint_constructive_witness_count:
      record.pair_constraint_constructive_witness_count,
    pair_constraint_algebraic_witness_count:
      record.pair_constraint_algebraic_witness_count,
    length_constraint_constructive_witness_count:
      record.length_constraint_constructive_witness_count,
    authorizes_project_mutation: false,
    replayable_across_runtimes: false,
  })
  certifiedSemanticMusValues.add(result)
  return result
}

function parseUnknown(
  record: Record<string, unknown>,
  directResult: DirectConflictResult,
  parser: StrictParserPrimitives,
): GeometricConstraintSemanticMusV1 | null {
  if (
    !parser.hasExactKeys(record, [
      'status',
      'model_id',
      'reason',
      'direct_core_constraint_ids',
      'direct_oracle_calls',
      'deletion_witness_checks',
      'certified_deletion_witnesses',
      'deletion_witness_work',
      'max_deletion_witness_checks',
      'max_deletion_witness_work',
      'authorizes_project_mutation',
      'replayable_across_runtimes',
    ])
    || record.model_id
      !== GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID
    || !isUnknownReason(record.reason)
  ) return null
  const directCoreIds = parser.parseSortedUniqueUuidArray(
    record.direct_core_constraint_ids,
    MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
  )
  if (
    !directCoreIds
    || !parser.isBoundedDirectMusOracleCalls(record.direct_oracle_calls, true)
    || !isCount(
      record.deletion_witness_checks,
      MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
      true,
    )
    || record.deletion_witness_checks > directCoreIds.length
    || !isCount(
      record.certified_deletion_witnesses,
      record.deletion_witness_checks,
      true,
    )
    || !isCount(
      record.deletion_witness_work,
      MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK,
      true,
    )
    || record.max_deletion_witness_checks
      !== MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS
    || record.max_deletion_witness_work
      !== MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK
    || record.authorizes_project_mutation !== false
    || record.replayable_across_runtimes !== false
    || !unknownPhaseIsReachable(
      record.reason,
      directCoreIds,
      record.direct_oracle_calls,
      record.deletion_witness_checks,
      record.certified_deletion_witnesses,
      record.deletion_witness_work,
    )
    || !unknownMatchesBoundedDirect(
      record.reason,
      directCoreIds,
      record.direct_oracle_calls,
      directResult.bounded_direct_mus,
    )
    || (
      directCoreIds.length > 0
      && !matchesOuterConflict(directCoreIds, directResult.conflicts)
    )
  ) return null
  return Object.freeze({
    status: 'unknown',
    model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
    reason: record.reason,
    direct_core_constraint_ids: directCoreIds,
    direct_oracle_calls: record.direct_oracle_calls,
    deletion_witness_checks: record.deletion_witness_checks,
    certified_deletion_witnesses: record.certified_deletion_witnesses,
    deletion_witness_work: record.deletion_witness_work,
    max_deletion_witness_checks:
      MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS,
    max_deletion_witness_work:
      MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK,
    authorizes_project_mutation: false,
    replayable_across_runtimes: false,
  })
}

function isUnknownReason(
  value: unknown,
): value is GeometricConstraintSemanticMusUnknownReasonV1 {
  return value === 'direct_oracle_incomplete'
    || value === 'deletion_witness_limit_exceeded'
    || value === 'deletion_witness_work_limit_exceeded'
    || value === 'deletion_witness_unavailable'
    || value === 'cancelled'
    || value === 'deadline_reached'
}

function isCount(value: unknown, maximum: number, allowZero: boolean): value is number {
  return typeof value === 'number'
    && Number.isSafeInteger(value)
    && !Object.is(value, -0)
    && value >= (allowZero ? 0 : 1)
    && value <= maximum
}

function unknownPhaseIsReachable(
  reason: GeometricConstraintSemanticMusUnknownReasonV1,
  ids: readonly string[],
  calls: number,
  checks: number,
  certified: number,
  work: number,
): boolean {
  if (ids.length === 0) {
    return checks === 0 && certified === 0 && work === 0
      && (
        reason === 'direct_oracle_incomplete'
        || reason === 'cancelled'
        || reason === 'deadline_reached'
      )
  }
  if (calls === 0 || reason === 'direct_oracle_incomplete') return false
  if (reason === 'deletion_witness_limit_exceeded') {
    return checks === 0 && certified === 0 && work === 0
  }
  if (reason === 'deletion_witness_unavailable') {
    return checks > certified && checks > 0 && work > 0
  }
  if (reason === 'deletion_witness_work_limit_exceeded') {
    return checks === 0
      ? certified === 0 && work === 0
      : certified < checks
  }
  return true
}

function unknownMatchesBoundedDirect(
  reason: GeometricConstraintSemanticMusUnknownReasonV1,
  ids: readonly string[],
  calls: number,
  bounded: BoundedDirectMus,
): boolean {
  if (ids.length > 0) {
    return bounded.status === 'proven_unsatisfiable'
      && bounded.oracle_calls === calls
      && sameStrings(bounded.constraint_ids, ids)
  }
  if (bounded.status !== 'unknown' || bounded.oracle_calls !== calls) return false
  if (
    bounded.reason === 'constraint_limit_exceeded'
    || bounded.reason === 'oracle_incomplete'
  ) return reason === 'direct_oracle_incomplete'
  return bounded.reason === reason
}

function matchesOuterConflict(
  ids: readonly string[],
  conflicts: readonly Readonly<{ constraint_ids: readonly string[] }>[],
): boolean {
  return conflicts.some((conflict) => sameStrings(ids, conflict.constraint_ids))
}

function sameStrings(first: readonly string[], second: readonly string[]): boolean {
  return first.length === second.length
    && first.every((value, index) => value === second[index])
}
