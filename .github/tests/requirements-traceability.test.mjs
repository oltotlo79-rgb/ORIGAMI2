import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import test from 'node:test'

const verifier = resolve(import.meta.dirname, '../scripts/verify_requirements_traceability.mjs')

test('requirements traceability binds every status to ancestral code and executable evidence', () => {
  const root = mkdtempSync(join(tmpdir(), 'origami2-traceability-'))
  try {
    mkdirSync(join(root, 'docs'))
    mkdirSync(join(root, 'src'))
    mkdirSync(join(root, 'tests'))
    writeFileSync(join(root, 'docs/status.md'), '| ID | 状態 | 根拠 |\n|---|---|---|\n| UI-001 | 実装済み | a |\n| SIM-010 | 部分実装 | b |\n')
    writeFileSync(join(root, 'docs/ids.txt'), 'SIM-010\nUI-001\n')
    writeFileSync(join(root, 'src/app.ts'), 'export function productionBoundary() {}\nexport function partialBoundary() {}\n')
    writeFileSync(join(root, 'tests/app.test.ts'), "test('production behavior', () => {})\ntest('partial present behavior', () => {})\n")
    execFileSync('git', ['init', '-q'], { cwd: root })
    execFileSync('git', ['config', 'core.autocrlf', 'false'], { cwd: root })
    execFileSync('git', ['config', 'user.email', 'fixture@example.invalid'], { cwd: root })
    execFileSync('git', ['config', 'user.name', 'Fixture'], { cwd: root })
    execFileSync('git', ['commit', '--allow-empty', '-qm', 'empty baseline'], { cwd: root })
    execFileSync('git', ['add', '.'], { cwd: root })
    execFileSync('git', ['commit', '-qm', 'fixture'], { cwd: root })
    const commit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
    const manifest = {
      schema: 'origami2.requirements-evidence.v1',
      requirements: [
        {
          id: 'UI-001', status: '実装済み', commits: [commit],
          evidence: [
            { kind: 'production-symbol', path: 'src/app.ts', selector: 'productionBoundary' },
            { kind: 'test', path: 'tests/app.test.ts', selector: 'production behavior' },
          ],
          limitations: [], missingAcceptance: [],
        },
        {
          id: 'SIM-010', status: '部分実装', commits: [commit],
          evidence: [
            { kind: 'production-symbol', path: 'src/app.ts', selector: 'partialBoundary' },
            { kind: 'contract', path: 'tests/app.test.ts', selector: 'partial present behavior' },
          ],
          limitations: ['general geometry remains unsupported'],
          missingAcceptance: ['native arbitrary schedule acceptance'],
        },
      ],
    }
    const manifestPath = join(root, 'docs/evidence.json')
    writeFileSync(manifestPath, JSON.stringify(manifest))
    const verify = (value = manifest) => {
      writeFileSync(manifestPath, `${JSON.stringify(value, null, 2)}\n`)
      return execFileSync('node', [verifier, 'docs/status.md', 'docs/evidence.json', 'docs/ids.txt'], {
        env: { ...process.env, REQUIREMENTS_EVIDENCE_ROOT: root }, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'],
      })
    }
    const invoke = (status = 'docs/status.md', evidence = 'docs/evidence.json', ids = 'docs/ids.txt', evidenceRoot = root) => execFileSync(
      'node', [verifier, status, evidence, ids],
      { env: { ...process.env, REQUIREMENTS_EVIDENCE_ROOT: evidenceRoot }, stdio: 'pipe' },
    )
    assert.equal(verify(), '2\n')
    assert.throws(() => verify({ ...manifest, requirements: manifest.requirements.slice(0, 1) }), /exact requirement set/u)
    const missingSelector = structuredClone(manifest)
    missingSelector.requirements[0].evidence[1].selector = 'invented test'
    assert.throws(() => verify(missingSelector), /selector is absent/u)
    const noBoundary = structuredClone(manifest)
    noBoundary.requirements[1].missingAcceptance = []
    assert.throws(() => verify(noBoundary), /lacks explicit present and missing boundaries/u)
    const noProduction = structuredClone(manifest)
    noProduction.requirements[0].evidence[0].kind = 'documentation'
    assert.throws(() => verify(noProduction), /lacks production and executable evidence/u)
    const foreignCommit = structuredClone(manifest)
    foreignCommit.requirements[0].commits = ['0'.repeat(40)]
    assert.throws(() => verify(foreignCommit), /not an ancestor/u)
    const traversal = structuredClone(manifest)
    traversal.requirements[0].evidence[0].path = '../outside.ts'
    assert.throws(() => verify(traversal), /escapes repository root/u)
    writeFileSync(manifestPath, '{"schema":"first","schema":"second","requirements":[]}\n')
    assert.throws(() => invoke(), /non-canonical or contains duplicate/u)
    const canonical = `${JSON.stringify(manifest, null, 2)}\n`
    writeFileSync(manifestPath, canonical.replace('"id": "UI-001",', '"id": "UI-001",\n      "id": "UI-001",'))
    assert.throws(() => invoke(), /non-canonical or contains duplicate/u)
    writeFileSync(manifestPath, canonical.replace('"kind": "production-symbol",', '"kind": "production-symbol",\n          "kind": "test",'))
    assert.throws(() => invoke(), /non-canonical or contains duplicate/u)
    verify()
    const invalidStatus = join(root, 'docs/status-invalid.md')
    writeFileSync(invalidStatus, '| ID | 状態 | 根拠 |\n|---|---|---|\n| UI-001 | 完了 | a |\n| SIM-010 | 部分実装 | b |\n')
    assert.throws(() => invoke('docs/status-invalid.md'), /invalid requirement status/u)
    const abbreviated = structuredClone(manifest)
    abbreviated.requirements[0].commits = [commit.slice(0, 12)]
    assert.throws(() => verify(abbreviated), /full SHA-1/u)
    const beforeSelector = structuredClone(manifest)
    execFileSync('git', ['show', 'HEAD^'], { cwd: root, stdio: 'ignore' })
    beforeSelector.requirements[0].commits = [execFileSync('git', ['rev-parse', 'HEAD^'], { cwd: root, encoding: 'utf8' }).trim()]
    assert.throws(() => verify(beforeSelector), /not bound to a listed commit/u)
    assert.throws(() => invoke('../status.md'), /path escapes repository root/u)
    assert.throws(() => invoke('docs/status.md', '../evidence.json'), /path escapes repository root/u)
    assert.throws(() => invoke('docs/status.md', 'docs/evidence.json', '../ids.txt'), /path escapes repository root/u)
    writeFileSync(join(root, 'docs/oversized-ids.txt'), 'X'.repeat(20_001))
    assert.throws(() => invoke('docs/status.md', 'docs/evidence.json', 'docs/oversized-ids.txt'), /file exceeds 20000 bytes/u)
    if (process.platform !== 'win32') {
      mkdirSync(join(root, 'linked-target'))
      writeFileSync(join(root, 'linked-target/evidence.ts'), 'productionBoundary\n')
      symlinkSync(join(root, 'linked-target'), join(root, 'linked-parent'))
      const linked = structuredClone(manifest)
      linked.requirements[0].evidence[0].path = 'linked-parent/evidence.ts'
      assert.throws(() => verify(linked), /symbolic link or junction/u)
      symlinkSync(join(root, 'docs/status.md'), join(root, 'docs/status-link.md'))
      symlinkSync(join(root, 'docs/evidence.json'), join(root, 'docs/evidence-link.json'))
      symlinkSync(join(root, 'docs/ids.txt'), join(root, 'docs/ids-link.txt'))
      assert.throws(() => invoke('docs/status-link.md'), /symbolic link or junction/u)
      assert.throws(() => invoke('docs/status.md', 'docs/evidence-link.json'), /symbolic link or junction/u)
      assert.throws(() => invoke('docs/status.md', 'docs/evidence.json', 'docs/ids-link.txt'), /symbolic link or junction/u)
    }
    verify()
    execFileSync('git', ['add', 'docs/evidence.json'], { cwd: root })
    execFileSync('git', ['commit', '-qm', 'add evidence manifest'], { cwd: root })
    const shallow = `${root}-shallow`
    execFileSync('git', ['clone', '--depth', '1', '-q', pathToFileURL(root).href, shallow])
    assert.throws(() => invoke('docs/status.md', 'docs/evidence.json', 'docs/ids.txt', shallow), /shallow repositories are unsupported/u)
    rmSync(shallow, { recursive: true, force: true })
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})
