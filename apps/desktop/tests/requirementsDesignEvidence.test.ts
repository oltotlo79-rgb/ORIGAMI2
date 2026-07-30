import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { readDesktopRustUnitTestSources } from './testRustSource.ts'

const status = readFileSync('../../docs/requirements-status.md', 'utf8')
const progress = readFileSync('../../docs/progress.md', 'utf8')
const evidenceManifest = JSON.parse(
  readFileSync('../../docs/requirements-evidence.v1.json', 'utf8'),
)
const evidence = readFileSync('../../docs/requirements-design-evidence-2026-07-21.md', 'utf8')
const editor = readFileSync('../../crates/ori-core/src/editor.rs', 'utf8')
const history = readFileSync('../../crates/ori-core/src/editor/history_persistence.rs', 'utf8')
const constraints = readFileSync('../../crates/ori-core/src/constraints.rs', 'utf8')
const semanticMus = readFileSync('../../crates/ori-core/src/constraint_semantic_mus.rs', 'utf8')
const native = readFileSync('src-tauri/src/lib.rs', 'utf8')
const nativeTests = readDesktopRustUnitTestSources()
const client = readFileSync('src/lib/coreClient.ts', 'utf8')
const panel = readFileSync('src/components/InstructionTimelinePanel.tsx', 'utf8')

const CURRENT_SEMANTIC_MUS_MODEL_ID
  = 'geometric_constraint_deterministic_binary64_semantic_mus_v4'
const CURRENT_SEMANTIC_INVENTORY_HEADING
  = '## 2026-07-30 EDT-009 semantic MUS 現行正本訂正（v4・24/24）'
const EDT_009_LIMITATION
  = 'Semantic MUS v4 covers all 24 wire variants only inside bounded, shape-specific proof families. The two newest families accept only common-center-star cores: exact two-hop Parallel plus 90 degrees with two unit terminals, and Parallel plus 45 degrees with one canonical unit terminal. Nonexact, nonunit, longer, nonstar, and generic variants fail closed; certificates never authorize mutation.'
const EDT_009_MISSING_ACCEPTANCE
  = 'Complete SAT/UNSAT and general semantic MUS discovery for arbitrary combinations of all 11 constraint kinds, including arbitrary-length parallel components and generic or non-star angle topologies.'

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

test('EDT-009 retains its wire tags and tracks twenty-four sound proof families', () => {
  const enumBody = constraints.match(
    /pub enum DirectConstraintConflictKindV1 \{(?<body>[\s\S]*?)\n\}/u,
  )?.groups?.body
  assert.ok(enumBody)
  const enumVariants = [
    ...enumBody.matchAll(/^    (?<name>[A-Z][A-Za-z0-9]+) \{/gmu),
  ]
    .map((match) => match.groups?.name)
    .filter((name): name is string => name !== undefined)
  assert.equal(enumVariants.length, 24)
  assert.equal(new Set(enumVariants).size, 24)

  const statusRow = status.match(/^\| EDT-009 \| 部分実装 \|.*$/mu)?.[0]
  assert.ok(statusRow)
  const allowlist = [
    'InconsistentLengthRatioGraphBetweenFixedLengths',
    'DifferentFixedLengths',
    'DifferentFixedAngles',
    'HorizontalAndVertical',
    'EqualLengthWithDifferentFixedLengths',
    'LengthRatioWithIncompatibleFixedLengths',
    'DifferentLengthRatios',
    'DifferentFixedLengthsInEqualLengthComponent',
    'PerpendicularOrientationsInParallelComponent',
    'ParallelWithPerpendicularOrientations',
    'SameOrientationWithFixedNonParallelAngle',
    'PerpendicularOrientationsWithFixedNonRightAngle',
    'DifferentRotationalSymmetryAnglesWithFixedRadius',
    'NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius',
    'RotationalSymmetryWithCollinearRadius',
    'MirrorSymmetryWithPointOnAxisAndFixedSeparation',
    'PositiveFixedLengthInBoundedZeroLengthClosure',
    'ZeroLengthClosureReachesNondegenerateProvider',
    'EqualLengthWithNonUnitRatioAndFixedLength',
    'NonReciprocalLengthRatiosWithFixedLength',
    'NonUnitLengthRatioCycleWithFixedLength',
    'InconsistentLengthRatioGraphWithFixedLength',
    'NonParallelFixedAngleInParallelComponent',
    'ParallelWithFixedNonParallelAngle',
  ]
  const documentedVariants = [
    ...statusRow.matchAll(/`(?<name>[A-Z][A-Za-z0-9]+)`/gu),
  ]
    .map((match) => match.groups?.name)
    .filter((name): name is string => name !== undefined)
    .filter((name) => enumVariants.includes(name))
    .filter((name, index, names) => names.indexOf(name) === index)
  assert.deepEqual(documentedVariants, allowlist)
  assert.deepEqual(
    enumVariants.filter((name) => !allowlist.includes(name)),
    [],
  )
  assert.match(statusRow, /legacy 21 variantをwire互換/u)
  assert.match(statusRow, /sound semantic family/u)
  for (const document of [status, progress, evidence]) {
    assert.equal(
      document.indexOf(CURRENT_SEMANTIC_INVENTORY_HEADING),
      document.indexOf('\n') + 2,
    )
    const currentSectionStart = document.indexOf(
      CURRENT_SEMANTIC_INVENTORY_HEADING,
    )
    const nextSectionStart = document.indexOf(
      '\n## ',
      currentSectionStart + CURRENT_SEMANTIC_INVENTORY_HEADING.length,
    )
    const currentSection = document.slice(
      currentSectionStart,
      nextSectionStart === -1 ? undefined : nextSectionStart,
    )
    assert.match(currentSection, /24\/24 sound semantic proof family/u)
    assert.ok(currentSection.includes(CURRENT_SEMANTIC_MUS_MODEL_ID))
    assert.match(
      currentSection,
      /23番目の `NonParallelFixedAngleInParallelComponent`[\s\S]*canonical 5-ID cause[\s\S]*common-center star[\s\S]*5件すべての単独削除/u,
    )
    assert.match(
      currentSection,
      /24番目の `ParallelWithFixedNonParallelAngle`[\s\S]*canonical 3-ID cause[\s\S]*common-center star[\s\S]*3件すべての単独削除witness/u,
    )
    assert.match(
      currentSection,
      /one-sidedなgeneric angle[\s\S]*nonstar topology[\s\S]*generic angleの既存4-ID direct boundary/u,
    )
    assert.match(
      currentSection,
      /部分実装[\s\S]*実装済み85 \/ 部分実装2 \/ 未着手0[\s\S]*数式・幾何制約は85%[\s\S]*全体は81\.96%（表示82\.0%）/u,
    )
  }
  assert.match(statusRow, /canonical 5-IDのunit-terminal two-hop core/u)
  assert.match(statusRow, /common-center star/u)
  assert.match(statusRow, /3-hop以上または任意長のparallel pathはsolver-required `Unknown`/u)
  assert.ok(semanticMus.includes(`"${CURRENT_SEMANTIC_MUS_MODEL_ID}"`))
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
  assert.deepEqual(edtEvidence.limitations, [EDT_009_LIMITATION])
  assert.deepEqual(
    edtEvidence.missingAcceptance,
    [EDT_009_MISSING_ACCEPTANCE],
  )
  const requiredUnitTwoHopEvidence = [
    [
      'production-symbol',
      'crates/ori-core/src/constraints/unit_two_hop_parallel.rs',
      'pub(super) fn conflict_v1(',
    ],
    [
      'test',
      'crates/ori-core/src/constraints_unit_two_hop_parallel_tests.rs',
      'fn exact_unit_two_hop_subset_emits_five_canonical_causes_and_is_direct_minimal()',
    ],
    [
      'test',
      'crates/ori-core/src/constraint_semantic_mus_tests/unit_two_hop_parallel_phase.rs',
      'fn all_five_deletions_have_independent_production_exact_residual_witnesses()',
    ],
    [
      'test',
      'apps/desktop/src-tauri/src/geometric_constraint_analysis/semantic_mus_certified_tests.rs',
      'fn unit_terminal_two_hop_parallel_counter_crosses_the_native_dto_exactly_five_times()',
    ],
  ] as const
  for (const [kind, path, selector] of requiredUnitTwoHopEvidence) {
    assert.ok(
      edtEvidence.evidence.some(
        (item: { kind: string, path: string, selector: string }) =>
          item.kind === kind
          && item.path === path
          && item.selector === selector,
      ),
      `missing EDT-009 evidence ${path} :: ${selector}`,
    )
  }
  const requiredCurrentInventoryEvidence = [
    [
      'production-symbol',
      'crates/ori-core/src/constraint_semantic_mus.rs',
      'pub const GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1',
    ],
    [
      'production-symbol',
      'crates/ori-core/src/constraints/unit_terminal_two_hop_parallel_angle.rs',
      'pub(super) fn conflict_v1(',
    ],
    [
      'test',
      'crates/ori-core/src/constraint_semantic_mus_tests/unit_terminal_two_hop_parallel_angle_phase.rs',
      'fn exact_five_id_core_recertifies_all_deletions_and_publishes_only_the_new_counter()',
    ],
    [
      'production-symbol',
      'crates/ori-core/src/constraints/unit_parallel_fixed_angle.rs',
      'fn is_proven_exact_forty_five_single_unit_parallel_angle_shape_v1(',
    ],
    [
      'test',
      'crates/ori-core/src/constraint_semantic_mus_tests/unit_parallel_fixed_angle_phase.rs',
      'fn exact_three_id_core_recertifies_every_deletion_in_the_dedicated_phase()',
    ],
  ] as const
  for (const [kind, path, selector] of requiredCurrentInventoryEvidence) {
    assert.ok(
      edtEvidence.evidence.some(
        (item: { kind: string, path: string, selector: string }) =>
          item.kind === kind
          && item.path === path
          && item.selector === selector,
      ),
      `missing EDT-009 current inventory evidence ${path} :: ${selector}`,
    )
  }
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraint_semantic_mus_tests/direct_family_inventory.rs'
      && item.selector === 'fn public_semantic_pipeline_hard_inventory_is_twenty_four_of_twenty_four()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { selector: string }) =>
      item.selector === 'fn horizontal_and_vertical_require_an_exact_noncollapse_witness()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraint_exactification/pair_constructive/cardinal_rotation.rs'
      && item.selector === 'pub(super) fn construct_cardinal_rotation_pair_candidate_v1(',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraint_exactification/pair_constructive_cardinal_rotation_tests.rs'
      && item.selector === 'fn cardinal_rotation_and_either_fixed_radius_cover_the_full_finite_length_range()',
  ))
  assert.ok(edtEvidence.commits.includes(
    '27b26585cc33618061d4c3c51987c96eeead8982',
  ))
  assert.ok(edtEvidence.commits.includes(
    'b5ca7e2c4ac0bba124a96e5c04922f34153fcddd',
  ))
  assert.ok(edtEvidence.commits.includes(
    '77033836b063b9d62dbc06f46dea2babbaf6f50b',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints_same_orientation_angle_tests.rs'
      && item.selector === 'fn witness_is_canonical_storage_invariant_and_irredundant()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints_perpendicular_angle_tests.rs'
      && item.selector === 'fn witness_is_canonical_storage_invariant_and_irredundant()',
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
    (item: { path: string, selector: string }) =>
      item.path === 'apps/desktop/src-tauri/src/tests/desktop_suite_09_geometric_constraint_worker_gates.rs'
      && item.selector === 'fn geometric_constraint_worker_cancel_is_bound_to_exact_request_generation()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'apps/desktop/src-tauri/src/tests/desktop_suite_09_geometric_constraint_worker_gates.rs'
      && item.selector === 'fn geometric_constraint_pre_cancel_ledger_is_bounded_and_evicts_oldest_only()',
  ))
  assert.ok(edtEvidence.evidence.some(
    (item: { path: string, selector: string }) =>
      item.path === 'crates/ori-core/src/constraints/directed_ratio_closure.rs'
      && item.selector === 'pub(super) fn conflict_with_limits_and_observer(',
  ))
  assert.match(nativeTests, /fn geometric_constraint_worker_cancel_is_bound_to_exact_request_generation\(\)/u)
  assert.match(nativeTests, /fn geometric_constraint_gate_consumes_exact_cancel_before_acquire_once\(\)/u)
  assert.match(nativeTests, /fn geometric_constraint_gate_retains_queued_cancel_while_another_generation_is_active\(\)/u)
  assert.match(nativeTests, /fn geometric_constraint_pre_cancel_ledger_is_bounded_and_evicts_oldest_only\(\)/u)
})
