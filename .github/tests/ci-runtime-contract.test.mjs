import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const root = resolve(here, '../..')
const workflowPath = join(root, '.github/workflows/ci.yml')

function rustJobSource() {
  const workflow = readFileSync(workflowPath, 'utf8')
  return workflow.slice(workflow.indexOf('\n  rust:'), workflow.indexOf('\n  windows-bundle:'))
}

function frontendJobSource() {
  const workflow = readFileSync(workflowPath, 'utf8')
  return workflow.slice(workflow.indexOf('\n  frontend:'), workflow.indexOf('\n  slicer-acceptance:'))
}

test('CI keeps exactly the three reviewed workflows', () => {
  assert.deepEqual(
    readdirSync(join(root, '.github/workflows'))
      .filter((name) => /\.ya?ml$/u.test(name))
      .sort(),
    ['ci.yml', 'release-windows.yml', 'release.yml'],
  )
  assert.doesNotMatch(readFileSync(workflowPath, 'utf8'), /paths-ignore:/u)
})

test('CI fixes one optimized fail-closed profile with fixture sharing and bounded isolation', () => {
  const rustJob = rustJobSource()
  assert.match(rustJob, /CARGO_PROFILE_TEST_OPT_LEVEL: "2"/u)
  assert.match(rustJob, /CARGO_PROFILE_TEST_DEBUG: "line-tables-only"/u)
  assert.match(rustJob, /CARGO_PROFILE_TEST_DEBUG_ASSERTIONS: "true"/u)
  assert.match(rustJob, /CARGO_PROFILE_TEST_OVERFLOW_CHECKS: "true"/u)
  assert.match(rustJob, /CARGO_PROFILE_DEV_DEBUG: "line-tables-only"/u)
  assert.match(rustJob, /CARGO_INCREMENTAL: "0"/u)
  assert.match(rustJob, /RUST_TEST_THREADS: "4"/u)
  assert.doesNotMatch(rustJob, /CARGO_PROFILE_DEV_OPT_LEVEL/u)

  const staticRuntime = rustJob.indexOf('Link the Windows Rust test harness to the static MSVC runtime')
  const rustCache = rustJob.indexOf('uses: Swatinem/rust-cache@')
  assert.ok(staticRuntime >= 0 && rustCache > staticRuntime)
  assert.match(rustJob, /key: test-opt2-line-tables-v2/u)

  const installAction = 'taiki-e/install-action@67729d5c413db75907f0ad1e39bb04b9c868ff60'
  assert.equal(rustJob.split(installAction).length - 1, 1)
  assert.match(
    rustJob,
    /tool: nextest@0\.9\.140\s+checksum: true\s+fallback: none/u,
  )

  const componentsStart = rustJob.indexOf('run_component ori-collision-fixture-shared-test-profile')
  const summaryStart = rustJob.indexOf('\n          if [ "$test_status"', componentsStart)
  assert.ok(componentsStart >= 0 && summaryStart > componentsStart)
  const commands = rustJob.slice(componentsStart, summaryStart)
  assert.doesNotMatch(commands, /if \[ "\$RUNNER_OS" = "Windows" \]; then/u)
  assert.match(
    commands,
    /cargo test -p ori-collision --locked --all-targets --no-fail-fast/u,
  )
  assert.doesNotMatch(commands, /cargo nextest run -p ori-collision/u)
  assert.match(
    commands,
    /cargo test --workspace --exclude origami2-desktop --exclude ori-collision --locked --all-targets --no-fail-fast/u,
  )
  assert.match(
    commands,
    /^[ \t]*cargo nextest run -p origami2-desktop --locked --lib --no-fail-fast --test-threads=4 --ignore-default-filter -E 'all\(\)'[ \t]*$/mu,
  )
  assert.doesNotMatch(commands, /cargo nextest run -p origami2-desktop[^\r\n]*--release/u)
  assertInOrder(commands, [
    'run_component ori-collision-fixture-shared-test-profile',
    'run_component workspace-core-excluding-collision-test-profile',
    'run_component origami2-desktop-process-isolated-test-profile',
    'run_component origami2-desktop-event-schema-test-profile',
  ])

  const serialFullSuiteCommands = [...rustJob.matchAll(/^\s*cargo test[^\r\n]*/gmu)]
    .map(([command]) => command)
    .filter((command) => command.includes('--no-fail-fast') && command.includes('--test-threads=1'))
  assert.deepEqual(serialFullSuiteCommands, [])
})

function assertInOrder(source, values) {
  let previous = -1
  for (const value of values) {
    const current = source.indexOf(value, previous + 1)
    assert.ok(current > previous, `expected ${value} after the preceding Rust component`)
    previous = current
  }
}

test('frontend builds once before production audits and retains Blender and lint gates', () => {
  const frontendJob = frontendJobSource()
  const build = 'npm run build'
  const csp = 'verify_desktop_bundle_csp.mjs dist'
  const security = 'verify_production_security_contract.mjs dist'
  const diagnostics = 'verify_diagnostics_privacy.mjs'
  const blender = 'npm run test:blender'
  const lint = 'npm run lint'

  assert.equal(frontendJob.split(build).length - 1, 1)
  for (const command of [csp, security, diagnostics, blender, lint]) {
    assert.equal(frontendJob.split(command).length - 1, 1, command)
  }
  assert.ok(frontendJob.indexOf(build) < frontendJob.indexOf(csp))
  assert.ok(frontendJob.indexOf(build) < frontendJob.indexOf(security))
  assert.ok(frontendJob.indexOf(csp) < frontendJob.indexOf(security))
  assert.ok(frontendJob.indexOf(security) < frontendJob.indexOf(diagnostics))
  assert.ok(frontendJob.indexOf(diagnostics) < frontendJob.indexOf(blender))
  assert.ok(frontendJob.indexOf(blender) < frontendJob.indexOf(lint))
})

test('generic target browser failure emits one bounded diagnostic annotation', () => {
  const frontendJob = frontendJobSource()
  assert.match(
    frontendJob,
    /npm run test:generic-target-browser 2>&1 \| tee "\$RUNNER_TEMP\/generic-target-browser\.log"/u,
  )
  assert.match(frontendJob, /tail -n 40 "\$RUNNER_TEMP\/generic-target-browser\.log"/u)
  assert.match(frontendJob, /cut -c1-4000/u)
  assertInOrder(frontendJob, [
    'set -o pipefail',
    'npm run test:generic-target-browser 2>&1 | tee "$RUNNER_TEMP/generic-target-browser.log"',
    'message="${message//\'%\'/\'%25\'}"',
    'message="${message//$\'\\r\'/\'%0D\'}"',
    'message="${message//$\'\\n\'/\'%0A\'}"',
    'message="${message:0:4000}"',
    '::error title=Generic target browser E2E failed::',
  ])
  assert.equal(
    frontendJob.split('::error title=Generic target browser E2E failed::').length - 1,
    1,
  )
  assert.doesNotMatch(frontendJob, /test:generic-target-browser[^\r\n]*retry/iu)
})

test('CI preserves required release checks and evidence artifacts', () => {
  const workflow = readFileSync(workflowPath, 'utf8')
  const jobs = workflow.slice(workflow.indexOf('\njobs:\n'))
  assert.deepEqual(
    [...jobs.matchAll(/^  ([a-z][a-z0-9-]+):\s*$/gmu)].map(([, name]) => name),
    [
      'dependency-advisory-audit',
      'frontend',
      'slicer-acceptance',
      'rust',
      'windows-bundle',
      'macos-bundle',
    ],
  )
  assert.match(jobs, /matrix:\s*\r?\n\s+os: \[windows-latest, macos-latest\]/u)

  const requiredChecks = [
    'dependency-advisory-audit',
    'frontend',
    'macos-bundle',
    'rust (macos-latest)',
    'rust (windows-latest)',
    'slicer-acceptance',
    'windows-bundle',
  ]
  const verifier = readFileSync(join(root, '.github/scripts/verify_release_ci.mjs'), 'utf8')
  for (const check of requiredChecks) {
    assert.match(verifier, new RegExp(`'${check.replace(/[()]/gu, '\\$&')}'`, 'u'))
  }
  for (const artifact of [
    'rustsec-warning-review',
    'sample-viewer-runtime-log',
    'ORIGAMI2-windows-nsis-${{ github.run_id }}',
    'ORIGAMI2-macos-app-${{ github.run_id }}',
  ]) {
    assert.equal(workflow.split(`name: ${artifact}`).length - 1, 1, artifact)
  }
})
