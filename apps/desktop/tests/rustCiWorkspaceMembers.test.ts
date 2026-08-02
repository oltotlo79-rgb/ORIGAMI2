import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const workflow = readFileSync('../../.github/workflows/ci.yml', 'utf8')
const expectedWindowsCoreOrder = [
  'ori-numeric',
  'ori-domain',
  'ori-geometry',
  'ori-topology',
  'ori-kinematics',
  'ori-foldability',
  'ori-collision',
  'ori-core',
  'ori-formats',
  'ori-instructions',
]
const desktopPackage = 'origami2-desktop'

test('Windows debug core list exactly covers cargo metadata workspace members excluding split desktop', () => {
  const packages = windowsPackages()
  const metadata = JSON.parse(execFileSync('cargo', [
    'metadata', '--no-deps', '--format-version', '1',
  ], { cwd: '../..', encoding: 'utf8' })) as {
    packages: Array<{ id: string, name: string }>
    workspace_members: string[]
  }
  const workspaceNames = metadata.workspace_members.map((id) => {
    const found = metadata.packages.find((pkg) => pkg.id === id)
    assert.ok(found, `metadata member ${id} must resolve to a package`)
    return found.name
  })
  assert.equal(new Set(packages).size, packages.length, 'Windows package list has duplicates')
  assert.equal(new Set(workspaceNames).size, workspaceNames.length, 'cargo metadata has duplicate members')
  assert.ok(workspaceNames.includes(desktopPackage), 'desktop remains a workspace member')
  assert.ok(!packages.includes(desktopPackage), 'desktop is split out of the Windows debug all-targets loop')
  assert.deepEqual(
    [...packages].sort(),
    workspaceNames.filter((name) => name !== desktopPackage).sort(),
  )
})

test('Windows core debug order and desktop split cannot silently drift', () => {
  assert.deepEqual(windowsPackages(), expectedWindowsCoreOrder)
  const positions = new Map(expectedWindowsCoreOrder.map((name, index) => [name, index]))
  for (const [before, after] of [
    ['ori-numeric', 'ori-domain'],
    ['ori-domain', 'ori-geometry'],
    ['ori-geometry', 'ori-topology'],
    ['ori-topology', 'ori-kinematics'],
    ['ori-kinematics', 'ori-collision'],
    ['ori-collision', 'ori-core'],
  ]) assert.ok(positions.get(before)! < positions.get(after)!, `${before} must precede ${after}`)

  const windowsStart = workflow.indexOf('          if [ "$RUNNER_OS" = "Windows" ]; then')
  const windowsEnd = workflow.indexOf('\n          else', windowsStart)
  assert.ok(windowsStart >= 0 && windowsEnd > windowsStart)
  const windows = workflow.slice(windowsStart, windowsEnd)
  assert.match(windows, /if \[ "\$package" = "ori-collision" \]; then\s+[\s\S]*?run_component "\$package"\s+\\\n\s*cargo test -p "\$package" --locked --all-targets --no-fail-fast -- --test-threads=1\s+else/u)
  assert.match(windows, /run_component "\$package"\s+\\\n\s*cargo test -p "\$package" --locked --all-targets --no-fail-fast/u)
  assert.match(windows, /if \[ "\$last_component_status" -ne 0 \]; then\s+core_failed=1\s+break\s+fi/u)
  assert.match(windows, /if \[ "\$core_failed" -eq 0 \]; then\s+run_component origami2-desktop-release-lib\s+\\\n\s*cargo test -p origami2-desktop --release --locked --lib --no-fail-fast -- --test-threads=1\s+run_component origami2-desktop-event-schema-debug\s+\\\n\s*cargo test -p origami2-desktop --locked --test event_schema_corpus --no-fail-fast\s+fi/u)
})

function windowsPackages(): string[] {
  const rustStart = workflow.indexOf('\n  rust:')
  const packagesStart = workflow.indexOf('            packages=(', rustStart)
  const packagesEnd = workflow.indexOf('            )', packagesStart)
  assert.ok(rustStart >= 0 && packagesStart > rustStart && packagesEnd > packagesStart)
  const block = workflow.slice(packagesStart, packagesEnd)
  return [...block.matchAll(/^\s{14}([a-z0-9-]+)$/gmu)].map((match) => match[1])
}
