import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const status = readFileSync('../../docs/requirements-status.md', 'utf8')
const progress = readFileSync('../../docs/progress.md', 'utf8')
const evidenceManifest = JSON.parse(
  readFileSync('../../docs/requirements-evidence.v1.json', 'utf8'),
)
const evidence = readFileSync('../../docs/requirements-design-evidence-2026-07-21.md', 'utf8')
const editor = readFileSync('../../crates/ori-core/src/editor.rs', 'utf8')
const history = readFileSync('../../crates/ori-core/src/editor/history_persistence.rs', 'utf8')
const constraints = readFileSync('../../crates/ori-core/src/constraints.rs', 'utf8')
const native = readFileSync('src-tauri/src/lib.rs', 'utf8')
const nativeTests = readFileSync('src-tauri/src/tests.rs', 'utf8')
const client = readFileSync('src/lib/coreClient.ts', 'utf8')
const panel = readFileSync('src/components/InstructionTimelinePanel.tsx', 'utf8')

test('the authoritative MUST table has two explicit partial boundaries and no unstarted row', () => {
  const rows = [...status.matchAll(/^\| ([A-Z]{2,3}-\d{3}) \| (実装済み|部分実装|未着手) \|/gmu)]
  assert.equal(rows.length, 87)
  assert.equal(new Set(rows.map((row) => row[1])).size, 87)
  assert.equal(rows.filter((row) => row[2] === '実装済み').length, 85)
  assert.deepEqual(
    rows.filter((row) => row[2] === '部分実装').map((row) => row[1]),
    ['EDT-009', 'SIM-010'],
  )
  assert.equal(rows.filter((row) => row[2] === '未着手').length, 0)
})

test('INS-007 design evidence is connected to every production boundary', () => {
  assert.match(status, /^\| INS-007 \| 実装済み \|.*分割.*結合.*永続履歴/mu)
  assert.match(evidence, /INS-007設計証拠の補完/u)
  assert.match(evidence, /RewriteInstructionTimelineSplitMerge/u)
  assert.match(editor, /RewriteInstructionTimelineSplitMerge/u)
  assert.match(editor, /is_one_instruction_split_or_merge/u)
  assert.match(history, /RewriteInstructionTimelineSplitMerge/u)
  assert.match(native, /fn split_instruction_step/u)
  assert.match(native, /fn merge_adjacent_instruction_steps/u)
  assert.match(client, /split_instruction_step/u)
  assert.match(client, /merge_adjacent_instruction_steps/u)
  assert.match(panel, /splitInstructionStep/u)
  assert.match(panel, /mergeAdjacentInstructionSteps/u)
})

test('the evidence audit does not promote the remaining SIM-010 proof boundary', () => {
  assert.match(evidence, /初版MUST全体が完成したとは扱わない/u)
  assert.match(evidence, /SIM-010の未証明範囲を完成へ昇格させる証拠には使用しない/u)
})

test('EDT-009 retains twenty-one legacy tags and tracks fifteen sound proof families', () => {
  const enumBody = constraints.match(
    /pub enum DirectConstraintConflictKindV1 \{(?<body>[\s\S]*?)\n\}/u,
  )?.groups?.body
  assert.ok(enumBody)
  const enumVariants = [
    ...enumBody.matchAll(/^    (?<name>[A-Z][A-Za-z0-9]+) \{/gmu),
  ]
    .map((match) => match.groups?.name)
    .filter((name): name is string => name !== undefined)
  assert.equal(enumVariants.length, 23)
  assert.equal(new Set(enumVariants).size, 23)

  const statusRow = status.match(/^\| EDT-009 \| 部分実装 \|.*$/mu)?.[0]
  assert.ok(statusRow)
  const allowlist = [
    'DifferentFixedLengths',
    'HorizontalAndVertical',
    'EqualLengthWithDifferentFixedLengths',
    'LengthRatioWithIncompatibleFixedLengths',
    'DifferentLengthRatios',
    'DifferentFixedLengthsInEqualLengthComponent',
    'ParallelWithPerpendicularOrientations',
    'SameOrientationWithFixedNonParallelAngle',
    'PerpendicularOrientationsWithFixedNonRightAngle',
    'PositiveFixedLengthInBoundedZeroLengthClosure',
    'ZeroLengthClosureReachesNondegenerateProvider',
    'EqualLengthWithNonUnitRatioAndFixedLength',
    'NonReciprocalLengthRatiosWithFixedLength',
    'NonUnitLengthRatioCycleWithFixedLength',
    'InconsistentLengthRatioGraphWithFixedLength',
  ]
  const documentedVariants = [
    ...statusRow.matchAll(/`(?<name>[A-Z][A-Za-z0-9]+)`/gu),
  ]
    .map((match) => match.groups?.name)
    .filter((name): name is string => name !== undefined)
    .filter((name) => enumVariants.includes(name))
    .filter((name, index, names) => names.indexOf(name) === index)
  assert.deepEqual(documentedVariants, allowlist)
  assert.equal(
    enumVariants.filter((name) => !allowlist.includes(name)).length,
    8,
  )
  assert.match(statusRow, /legacy 21 variantをwire互換/u)
  assert.match(statusRow, /sound allowlist/u)
  assert.match(statusRow, /残る8 variant/u)
  assert.match(statusRow, /`Unknown`へfail-closed/u)
  assert.match(statusRow, /全11種の一般充足可能性、完全な一般矛盾原因、一般最小不能部分集合は未完成/u)

  assert.match(
    status,
    /2026-07-26 EDT-009異比率追補:[^\n]+sound allowlistは7種[^\n]+本項がallowlist数の現行正本/u,
  )
  assert.match(
    progress,
    /2026-07-26 EDT-009異比率追補:[^\n]+sound allowlistは7種、fail-closedは14種/u,
  )
  assert.match(
    status,
    /2026-07-26 EDT-009非退化provider閉包追補:[^\n]+合計23 variant[^\n]+sound familyは9種[^\n]+本項がvariant数・sound family数の現行正本/u,
  )
  assert.match(
    progress,
    /2026-07-26 EDT-009非退化provider閉包追補:[^\n]+合計23 variantのうちsound familyは9種、legacy fail-closedは14種/u,
  )
  assert.match(
    status,
    /2026-07-26 EDT-009比率実残差閉包追補:[^\n]+sound familyは12種、legacy fail-closedは11種[^\n]+本項が一般比率グラフ実装前のvariant数・sound family数の現行正本であった/u,
  )
  assert.match(
    progress,
    /2026-07-26 EDT-009比率実残差閉包追補:[^\n]+sound familyは12種、legacy fail-closedは11種/u,
  )
  assert.match(
    status,
    /2026-07-26 EDT-009一般有向比率グラフ追補:[^\n]+sound familyは13種、legacy fail-closedは10種[^\n]+本項がvariant数・sound family数の現行正本/u,
  )
  assert.match(
    progress,
    /2026-07-26 EDT-009一般有向比率グラフ追補:[^\n]+sound familyは13種、legacy fail-closedは10種/u,
  )
  assert.match(
    status,
    /2026-07-26 EDT-009同軸固定角追補:[^\n]+sound familyは14種、legacy fail-closedは9種[^\n]+本項が直交固定角実装前のvariant数・sound family数の現行正本であった/u,
  )
  assert.match(
    progress,
    /2026-07-26 EDT-009同軸固定角追補:[^\n]+sound familyは14種、legacy fail-closedは9種/u,
  )
  assert.match(
    status,
    /2026-07-26 EDT-009直交固定角追補:[^\n]+sound familyは15種、legacy fail-closedは8種[^\n]+本項がvariant数・sound family数の現行正本/u,
  )
  assert.match(
    progress,
    /2026-07-26 EDT-009直交固定角追補:[^\n]+sound familyは15種、legacy fail-closedは8種/u,
  )
  assert.match(progress, /\*\*81\.96%（表示82\.0%）\*\*/u)
  assert.match(status, /\*\*実装済み85 \/ 部分実装2 \/ 未着手0\*\*/u)

  const edtEvidence = evidenceManifest.requirements.find(
    (requirement: { id: string }) => requirement.id === 'EDT-009',
  )
  assert.ok(edtEvidence)
  assert.deepEqual(edtEvidence.limitations, [
    'only fifteen of the twenty-three wire-compatible DirectConstraintConflictKindV1 variants are sound under the actual binary64 residuals; the eight retained legacy variants fail closed to Unknown',
  ])
  assert.deepEqual(edtEvidence.missingAcceptance, [
    'complete sound satisfiability and unsatisfiability decisions plus semantic minimal unsatisfiable subsets across all eleven constraint kinds beyond the bounded exact-zero and directed length-ratio proof families',
  ])
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn horizontal_and_vertical_require_an_exact_noncollapse_witness()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn partially_checked_fixed_angle_and_ratio_kinds_return_unknown()',
  ))
  assert.ok(edtEvidence.commits.includes(
    '27b26585cc33618061d4c3c51987c96eeead8982',
  ))
  assert.ok(edtEvidence.commits.includes(
    'b5ca7e2c4ac0bba124a96e5c04922f34153fcddd',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints_same_orientation_angle_tests.rs'
      && item.selector === 'fn zero_cross_classification_uses_the_shared_binary64_residual()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints_perpendicular_angle_tests.rs'
      && item.selector === 'fn perpendicular_actual_classes_use_only_shared_binary64_operations()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn fixed_lengths_and_ratio_share_the_solver_binary64_residual()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn incompatible_fixed_lengths_and_ratio_are_rejected_before_numerical_tolerance()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn different_ratios_need_a_fixed_denominator_and_incompatible_binary64_products()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn different_ratios_with_fixed_denominator_can_share_an_underflowed_zero_numerator()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints/bounded_zero_closure.rs'
      && item.selector === 'pub(super) fn conflict_with_limits_and_observer(',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn bounded_zero_length_closure_crosses_equal_length_and_ratio_without_solver_assumptions()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn bounded_zero_length_closure_core_is_canonical_and_cardinality_smallest_for_the_oracle()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'reverse_zero_length_ratio_underflow_is_solver_required',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn bounded_zero_length_closure_uses_each_binary64_proven_provider_terminal()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn bounded_zero_length_closure_resource_and_observer_stops_are_fail_closed()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'apps/desktop/src-tauri/src/tests.rs'
      && item.selector === 'fn geometric_constraint_worker_cancel_is_bound_to_exact_request_generation()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'apps/desktop/src-tauri/src/tests.rs'
      && item.selector === 'fn geometric_constraint_gate_consumes_exact_cancel_before_acquire_once()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'apps/desktop/src-tauri/src/tests.rs'
      && item.selector === 'fn geometric_constraint_gate_retains_queued_cancel_while_another_generation_is_active()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'apps/desktop/src-tauri/src/tests.rs'
      && item.selector === 'fn geometric_constraint_pre_cancel_ledger_is_bounded_and_evicts_oldest_only()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints/directed_ratio_closure.rs'
      && item.selector === 'pub(super) fn conflict_with_limits_and_observer(',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints_general_ratio_graph_tests.rs'
      && item.selector === 'fn every_four_cycle_root_and_both_directions_are_canonical_and_irredundant()',
  ))
  assert.match(nativeTests, /fn geometric_constraint_worker_cancel_is_bound_to_exact_request_generation\(\)/u)
  assert.match(nativeTests, /fn geometric_constraint_gate_consumes_exact_cancel_before_acquire_once\(\)/u)
  assert.match(nativeTests, /fn geometric_constraint_gate_retains_queued_cancel_while_another_generation_is_active\(\)/u)
  assert.match(nativeTests, /fn geometric_constraint_pre_cancel_ledger_is_bounded_and_evicts_oldest_only\(\)/u)
})
