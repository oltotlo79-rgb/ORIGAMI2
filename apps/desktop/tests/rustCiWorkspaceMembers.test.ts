import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const workflow = readFileSync('../../.github/workflows/ci.yml', 'utf8')
const expectedOrder = [
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
  'origami2-desktop',
]

test('Windows crate CI list exactly covers cargo metadata workspace members', () => {
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
  assert.deepEqual([...packages].sort(), [...workspaceNames].sort())
})

test('Windows crate CI order is pinned dependency-first and cannot silently drift', () => {
  assert.deepEqual(windowsPackages(), expectedOrder)
  const positions = new Map(expectedOrder.map((name, index) => [name, index]))
  for (const [before, after] of [
    ['ori-numeric', 'ori-domain'],
    ['ori-domain', 'ori-geometry'],
    ['ori-geometry', 'ori-topology'],
    ['ori-topology', 'ori-kinematics'],
    ['ori-kinematics', 'ori-collision'],
    ['ori-collision', 'ori-core'],
    ['ori-core', 'origami2-desktop'],
  ]) assert.ok(positions.get(before)! < positions.get(after)!, `${before} must precede ${after}`)
})

function windowsPackages(): string[] {
  const rustStart = workflow.indexOf('\n  rust:')
  const packagesStart = workflow.indexOf('            packages=(', rustStart)
  const packagesEnd = workflow.indexOf('            )', packagesStart)
  assert.ok(rustStart >= 0 && packagesStart > rustStart && packagesEnd > packagesStart)
  const block = workflow.slice(packagesStart, packagesEnd)
  return [...block.matchAll(/^\s{14}([a-z0-9-]+)$/gmu)].map((match) => match[1])
}
