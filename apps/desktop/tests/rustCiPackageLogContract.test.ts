import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const workflow = readFileSync('../../.github/workflows/ci.yml', 'utf8')

test('each Rust CI component has stable begin end and numeric status markers', () => {
  assert.match(workflow, /ORIGAMI2_CARGO_PACKAGE_BEGIN::%s/u)
  assert.match(workflow, /ORIGAMI2_CARGO_PACKAGE_END::%s::status=%s/u)
  assert.match(workflow, /run_component\(\) \{/u)
  assert.match(workflow, /"\$component" "\$component_status"/u)
  assert.match(workflow, /printf '%s\\n' "\$component" > cargo-test-failed-package\.txt/u)
  const begin = workflow.indexOf('ORIGAMI2_CARGO_PACKAGE_BEGIN::%s')
  const command = workflow.indexOf('"$@" 2>&1 | tee -a cargo-test.log', begin)
  const end = workflow.indexOf('ORIGAMI2_CARGO_PACKAGE_END::%s::status=%s', command)
  assert.ok(begin >= 0 && command > begin && end > command)
})

test('failure annotations retain the first failed component and include the final 30 lines', () => {
  assert.match(workflow, /if \[ "\$component_status" -ne 0 \] && \[ "\$test_status" -eq 0 \]; then/u)
  assert.match(workflow, /test_status="\$component_status"/u)
  assert.match(workflow, /last_component_status="\$component_status"/u)
  assert.match(workflow, /failed_package="\$\(cat cargo-test-failed-package\.txt\)"/u)
  assert.match(workflow, /tail -n 30 cargo-test\.log/u)
  assert.match(workflow, /failure_summary="package=\$failed_package final-30-lines: \$failure_summary"/u)
  assert.match(workflow, /title=Rust test log tail \(\$failed_package\)::package=\$failed_package final-30-lines:/u)
})

test('macOS isolates process-global suites while preserving every Rust component', () => {
  const macosStart = workflow.indexOf('          else\n            run_component workspace-core-excluding-collision-debug')
  const macosEnd = workflow.indexOf('\n          fi\n          if [ "$test_status"', macosStart)
  assert.ok(macosStart >= 0 && macosEnd > macosStart)
  const macos = workflow.slice(macosStart, macosEnd)
  assert.match(macos, /run_component workspace-core-excluding-collision-debug\s+\\\n\s*cargo test --workspace --exclude origami2-desktop --exclude ori-collision --locked --all-targets --no-fail-fast/u)
  assert.match(macos, /run_component ori-collision-serial-debug\s+\\\n\s*cargo test -p ori-collision --locked --all-targets --no-fail-fast -- --test-threads=1/u)
  assert.match(macos, /run_component origami2-desktop-release-lib\s+\\\n\s*cargo test -p origami2-desktop --release --locked --lib --no-fail-fast -- --test-threads=1/u)
  assert.match(macos, /run_component origami2-desktop-event-schema-debug\s+\\\n\s*cargo test -p origami2-desktop --locked --test event_schema_corpus --no-fail-fast/u)
  assertInOrder(macos, [
    'run_component workspace-core-excluding-collision-debug',
    'run_component ori-collision-serial-debug',
    'run_component origami2-desktop-release-lib',
    'run_component origami2-desktop-event-schema-debug',
  ])
})

test('dedicated debug lifecycle selectors remain exact after the split Rust components', () => {
  const selectors = [
    'even_cycle_exact_schedules_are_admitted_by_strict_dyadic_read',
    'concave_boundary_strict_dyadic_read_fails_closed_without_mutation_authority',
    'cut_boundary_strict_dyadic_read_fails_closed_without_mutation_authority',
    'hole_boundary_strict_dyadic_read_fails_closed_without_mutation_authority',
    'open_cut_seam_strict_dyadic_preflight_is_unsupported_no_op',
    'nonfinite_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'degenerate_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'missing_boundary_vertex_strict_dyadic_preflight_is_unsupported_no_op',
    'duplicate_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'self_intersecting_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'zero_length_boundary_strict_dyadic_preflight_is_unsupported_no_op',
    'missing_pose_capability_strict_dyadic_read_returns_unsupported_dto',
    'tree_pose_capability_rejects_incomplete_target_without_mutation',
    'four_hinge_tree_level_three_proof_applies_and_persists_atomically',
    'five_hinge_tree_level_three_proof_applies_and_persists_atomically',
    'six_hinge_tree_level_three_proof_applies_and_persists_atomically',
    'seven_hinge_generic_grid_proof_applies_and_persists_atomically',
    'bounded_multi_block_opposite_bifolds_preview_apply_and_reopen_history',
    'balloon_six_sector_straight_line_cycle_previews_applies_and_round_trips_history',
    'coupled_cactus_previews_fail_closed_without_continuous_authority',
    'theta_positive_thickness_preview_fails_closed_without_continuous_authority',
  ]
  for (const selector of selectors) {
    assert.ok(workflow.includes(
      `cargo test --locked -p origami2-desktop --lib stacked_fold_read::tests::${selector} -- --exact --test-threads=1`,
    ), `missing dedicated debug selector: ${selector}`)
  }
})

function assertInOrder(source: string, values: string[]): void {
  let previous = -1
  for (const value of values) {
    const current = source.indexOf(value, previous + 1)
    assert.ok(current > previous, `expected ${value} after the previous CI component`)
    previous = current
  }
}
