import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..', '..')
const ledger = JSON.parse(readFileSync(resolve(root, '.github/rustsec-warning-ledger.json'), 'utf8'))
const fail = (message) => { throw new Error(`RustSec warning ledger: ${message}`) }
if (ledger.schema !== 'origami2.rustsec-warning-ledger.v1' || ledger.entries?.length !== 18) fail('invalid schema or entry count')
const ids = ledger.entries.map(({ id }) => id)
if (new Set(ids).size !== 18 || ids.join() !== [...ids].sort().join()) fail('IDs must be unique and sorted')
const today = process.env.RUSTSEC_POLICY_DATE ?? new Date().toISOString().slice(0, 10)
for (const entry of ledger.entries) {
  if (!/^RUSTSEC-\d{4}-\d{4}$/u.test(entry.id) || !/^\d{4}-\d{2}-\d{2}$/u.test(entry.expires)) fail(`invalid identity/date: ${entry.id}`)
  if (entry.expires < today) fail(`expired exception: ${entry.id} (${entry.expires})`)
  if (!entry.reason || !Array.isArray(entry.dependencyPath) || entry.dependencyPath.at(-1) !== `${entry.package}@${entry.version}`) fail(`incomplete entry: ${entry.id}`)
}

const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--format-version', '1'], { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }))
const identities = new Map(metadata.packages.map((pkg) => [`${pkg.name}@${pkg.version}`, pkg.id]))
const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, new Set(node.deps.map((dep) => dep.pkg))]))
for (const entry of ledger.entries) {
  for (let index = 0; index + 1 < entry.dependencyPath.length; index += 1) {
    const parent = identities.get(entry.dependencyPath[index])
    const child = identities.get(entry.dependencyPath[index + 1])
    if (!parent || !child || !nodes.get(parent)?.has(child)) fail(`dependency path changed: ${entry.id}`)
  }
}

if (process.argv[2]) {
  const report = JSON.parse(readFileSync(process.argv[2], 'utf8'))
  const warnings = Object.entries(report.warnings ?? {}).flatMap(([kind, values]) => (values ?? []).map((warning) => ({ kind, ...warning })))
  if (warnings.length !== ledger.entries.length) fail(`audit warning count changed: ${warnings.length}`)
  for (const entry of ledger.entries) {
    const warning = warnings.find((candidate) => candidate.advisory?.id === entry.id)
    const actual = warning && {
      package: warning.package?.name,
      version: warning.package?.version,
      kind: warning.kind,
      title: warning.advisory?.title,
      url: warning.advisory?.url,
    }
    for (const field of ['package', 'version', 'kind', 'title', 'url']) if (actual?.[field] !== entry[field]) fail(`advisory content changed: ${entry.id}.${field}`)
  }
}
process.stdout.write(`${ids.join('\n')}\n`)
