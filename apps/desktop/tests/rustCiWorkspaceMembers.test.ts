import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const workflow = readFileSync('../../.github/workflows/ci.yml', 'utf8')
const rustStart = workflow.indexOf('\n  rust:')
const rustEnd = workflow.indexOf('\n  windows-bundle:', rustStart)
assert.ok(rustStart >= 0 && rustEnd > rustStart)
const rust = workflow.slice(rustStart, rustEnd)

test('the hosted matrix uses one drift-proof workspace command for every non-desktop core member', () => {
  const workspaceCommand = 'cargo test --workspace --exclude origami2-desktop --exclude ori-collision --locked --all-targets --no-fail-fast'
  assert.match(rust, /matrix:\s*\n\s+os: \[windows-latest, macos-latest\]/u)
  assert.equal(rust.split(workspaceCommand).length - 1, 1)
  assert.doesNotMatch(rust, /packages=\(/u)
  assert.doesNotMatch(rust, /cargo test -p "\$package"/u)
  assert.doesNotMatch(rust, /if \[ "\$RUNNER_OS" = "Windows" \]; then/u)
})

test('collision shares immutable fixtures before workspace core and split desktop tests', () => {
  assert.match(
    rust,
    /run_component ori-collision-fixture-shared-test-profile\s+\\\n\s*cargo test -p ori-collision --locked --all-targets --no-fail-fast/u,
  )
  assert.doesNotMatch(rust, /cargo nextest run -p ori-collision/u)
  assert.match(
    rust,
    /run_component origami2-desktop-process-isolated-test-profile\s+\\\n[ \t]*cargo nextest run -p origami2-desktop --locked --lib --no-fail-fast --test-threads=4 --ignore-default-filter -E 'all\(\)'[ \t]*\r?$/mu,
  )
  assert.doesNotMatch(rust, /cargo nextest run -p origami2-desktop[^\r\n]*--release/u)
  assertInOrder(rust, [
    'run_component ori-collision-fixture-shared-test-profile',
    'run_component workspace-core-excluding-collision-test-profile',
    'run_component origami2-desktop-process-isolated-test-profile',
    'run_component origami2-desktop-event-schema-test-profile',
  ])
})

function assertInOrder(source: string, values: string[]): void {
  let previous = -1
  for (const value of values) {
    const current = source.indexOf(value, previous + 1)
    assert.ok(current > previous, `expected ${value} after the preceding Rust component`)
    previous = current
  }
}
