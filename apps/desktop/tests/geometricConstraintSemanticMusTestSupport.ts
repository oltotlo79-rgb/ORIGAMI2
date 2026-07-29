import {
  GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
} from '../src/lib/geometricConstraints.ts'
import {
  DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
} from '../src/lib/deterministicTranscendentalModel.ts'

export const uuid = (index: number) =>
  `00000000-0000-4000-8000-${index.toString(16).padStart(12, '0')}`
export const IDS = Array.from({ length: 18 }, (_, index) => uuid(index + 1))
export const CORE = [IDS[0]!, IDS[1]!]
export const BINDING = Object.freeze({
  project_instance_id: uuid(100),
  project_id: uuid(101),
  revision: 7,
})

export function certified(
  overrides: Readonly<Record<string, unknown>> = {},
): Record<string, unknown> {
  return {
    status: 'certified',
    model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    constraint_ids: CORE,
    constraint_count: 2,
    direct_oracle_calls: 7,
    deletion_witness_checks: 2,
    deletion_witness_work: 100,
    current_assignment_witness_count: 1,
    axis_exactification_witness_count: 0,
    single_constraint_constructive_witness_count: 1,
    pair_constraint_constructive_witness_count: 0,
    pair_constraint_algebraic_witness_count: 0,
    length_constraint_constructive_witness_count: 0,
    zero_length_closure_constructive_witness_count: 0,
    anchored_mirror_residual_only_witness_count: 0,
    unit_parallel_fixed_angle_residual_only_witness_count: 0,
    unit_terminal_two_hop_parallel_angle_residual_only_witness_count: 0,
    unit_two_hop_parallel_residual_only_witness_count: 0,
    authorizes_project_mutation: false,
    replayable_across_runtimes: true,
    ...overrides,
  }
}

export function unknown(
  overrides: Readonly<Record<string, unknown>>,
): Record<string, unknown> {
  return {
    status: 'unknown',
    model_id: GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID,
    transcendental_model_id: DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
    reason: 'cancelled',
    direct_core_constraint_ids: CORE,
    direct_oracle_calls: 7,
    deletion_witness_checks: 0,
    certified_deletion_witnesses: 0,
    deletion_witness_work: 0,
    max_deletion_witness_checks: 16,
    max_deletion_witness_work: 20_000_000,
    authorizes_project_mutation: false,
    replayable_across_runtimes: false,
    ...overrides,
  }
}

export function provenDirect(
  constraintIds: readonly string[] = CORE,
  oracleCalls = 7,
) {
  return {
    status: 'proven_unsatisfiable',
    constraint_ids: constraintIds,
    oracle_calls: oracleCalls,
  }
}

export function unknownDirect(reason: string, oracleCalls: number) {
  return {
    status: 'unknown',
    reason,
    oracle_calls: oracleCalls,
    max_constraints: 16,
  }
}

export function directResult(boundedDirectMus: unknown) {
  return {
    status: 'direct_conflict',
    conflicts: [{
      conflict: { kind: 'different_fixed_lengths', edge: uuid(200) },
      constraint_ids: CORE,
    }],
    bounded_direct_mus: boundedDirectMus,
  }
}

export function envelope(semanticMus: unknown, boundedDirectMus: unknown) {
  return {
    ...BINDING,
    result: directResult(boundedDirectMus),
    semantic_mus: semanticMus,
  }
}
