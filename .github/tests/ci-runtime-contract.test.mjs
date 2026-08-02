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

test('CI fixes the optimized fail-closed Rust profile and process-isolated commands', () => {
  const rustJob = rustJobSource()
  assert.match(rustJob, /CARGO_PROFILE_TEST_OPT_LEVEL: "2"/u)
  assert.match(rustJob, /CARGO_PROFILE_TEST_DEBUG: "line-tables-only"/u)
  assert.match(rustJob, /CARGO_PROFILE_TEST_DEBUG_ASSERTIONS: "true"/u)
  assert.match(rustJob, /CARGO_PROFILE_TEST_OVERFLOW_CHECKS: "true"/u)
  assert.match(rustJob, /CARGO_PROFILE_DEV_DEBUG: "line-tables-only"/u)
  assert.match(rustJob, /CARGO_INCREMENTAL: "0"/u)
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

  const windowsStart = rustJob.indexOf('if [ "$RUNNER_OS" = "Windows" ]; then')
  const macosStart = rustJob.indexOf('\n          else', windowsStart)
  const summaryStart = rustJob.indexOf('\n          if [ "$test_status"', macosStart)
  assert.ok(windowsStart >= 0 && macosStart > windowsStart && summaryStart > macosStart)
  const windowsCommands = rustJob.slice(windowsStart, macosStart)
  const macosCommands = rustJob.slice(macosStart, summaryStart)
  assert.ok(windowsCommands.indexOf('ori-collision') < windowsCommands.indexOf('ori-numeric'))
  assert.match(
    windowsCommands,
    /cargo nextest run -p "\$package" --locked --all-targets --no-fail-fast --test-threads=4/u,
  )
  assert.match(
    windowsCommands,
    /cargo nextest run -p origami2-desktop --release --locked --lib --no-fail-fast --test-threads=4/u,
  )
  assert.match(
    macosCommands,
    /cargo nextest run -p ori-collision --locked --all-targets --no-fail-fast --test-threads=4/u,
  )
  assert.ok(
    macosCommands.indexOf('run_component ori-collision-process-isolated-debug')
      < macosCommands.indexOf('run_component workspace-core-excluding-collision-debug'),
  )
  assert.match(
    macosCommands,
    /cargo nextest run -p origami2-desktop --release --locked --lib --no-fail-fast --test-threads=4/u,
  )

  const serialFullSuiteCommands = [...rustJob.matchAll(/^\s*cargo test[^\r\n]*/gmu)]
    .map(([command]) => command)
    .filter((command) => command.includes('--no-fail-fast') && command.includes('--test-threads=1'))
  assert.deepEqual(serialFullSuiteCommands, [])
})

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
