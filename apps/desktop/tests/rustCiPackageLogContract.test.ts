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
  assert.match(workflow, /failed_package="\$\(cat cargo-test-failed-package\.txt\)"/u)
  assert.match(workflow, /tail -n 30 cargo-test\.log/u)
  assert.match(workflow, /failure_summary="package=\$failed_package final-30-lines: \$failure_summary"/u)
  assert.match(workflow, /title=Rust test log tail \(\$failed_package\)::package=\$failed_package final-30-lines:/u)
})

test('both hosted OSes run the same fixture-sharing and process-isolated Rust components', () => {
  const rustStart = workflow.indexOf('\n  rust:')
  const componentsStart = workflow.indexOf('          run_component ori-collision-fixture-shared-test-profile', rustStart)
  const componentsEnd = workflow.indexOf('\n          if [ "$test_status"', componentsStart)
  assert.ok(rustStart >= 0 && componentsStart > rustStart && componentsEnd > componentsStart)
  const components = workflow.slice(componentsStart, componentsEnd)
  assert.match(components, /run_component ori-collision-fixture-shared-test-profile\s+\\\n\s*cargo test -p ori-collision --locked --all-targets --no-fail-fast/u)
  assert.match(components, /run_component workspace-core-excluding-collision-test-profile\s+\\\n\s*cargo test --workspace --exclude origami2-desktop --exclude ori-collision --locked --all-targets --no-fail-fast/u)
  assert.match(components, /run_component origami2-desktop-process-isolated-test-profile\s+\\\n[ \t]*cargo nextest run -p origami2-desktop --locked --lib --no-fail-fast --test-threads=4 --ignore-default-filter -E 'all\(\)'[ \t]*\r?$/mu)
  assert.match(components, /run_component origami2-desktop-event-schema-test-profile\s+\\\n\s*cargo test -p origami2-desktop --locked --test event_schema_corpus --no-fail-fast/u)
  assertInOrder(components, [
    'run_component ori-collision-fixture-shared-test-profile',
    'run_component workspace-core-excluding-collision-test-profile',
    'run_component origami2-desktop-process-isolated-test-profile',
    'run_component origami2-desktop-event-schema-test-profile',
  ])
})

test('the full process-isolated desktop library gate runs exactly once', () => {
  assert.equal(
    workflow.match(/^[ \t]*cargo nextest run -p origami2-desktop --locked --lib --no-fail-fast --test-threads=4 --ignore-default-filter -E 'all\(\)'[ \t]*$/gmu)?.length,
    1,
  )
})

function assertInOrder(source: string, values: string[]): void {
  let previous = -1
  for (const value of values) {
    const current = source.indexOf(value, previous + 1)
    assert.ok(current > previous, `expected ${value} after the previous CI component`)
    previous = current
  }
}
