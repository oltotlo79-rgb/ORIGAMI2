import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { buildDependencyPolicy } from './dependency_policy.mjs'

const path = process.argv[2]
const repositoryRoot = resolve(import.meta.dirname, '..', '..')
const sbom = JSON.parse(readFileSync(path, 'utf8'))
const version = process.env.VERSION
const platform = process.env.PLATFORM
const commit = process.env.RELEASE_COMMIT
const rustc = process.env.RUSTC_VERSION
const node = process.env.NODE_VERSION
const buildMode = process.env.BUILD_MODE
const targetTriple = process.env.TARGET_TRIPLE
const releaseRunId = process.env.RELEASE_RUN_ID
const executedTestCount = Number(process.env.EXECUTED_TEST_COUNT)
let ciChecks
try {
  ciChecks = JSON.parse(process.env.CI_CHECK_EVIDENCE_JSON)
} catch {
  throw new Error('invalid CI check evidence JSON')
}
if (sbom.bomFormat !== 'CycloneDX' || !Array.isArray(sbom.components)) {
  throw new Error('invalid CycloneDX source SBOM')
}
if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(version ?? '')) {
  throw new Error('invalid SBOM release version')
}
if (!['windows-x64', 'macos-arm64'].includes(platform)) throw new Error('invalid SBOM platform')
if (!/^[0-9a-f]{40}$/u.test(commit ?? '')) throw new Error('invalid SBOM source commit')
if (!/^rustc [0-9]+\.[0-9]+\.[0-9]+/u.test(rustc ?? '')) throw new Error('invalid rustc version')
if (!/^v[0-9]+\.[0-9]+\.[0-9]+/u.test(node ?? '')) throw new Error('invalid Node.js version')
if (!['signed-release', 'unsigned-dry-run'].includes(buildMode)) throw new Error('invalid build mode')
const expectedTarget = platform === 'windows-x64'
  ? 'x86_64-pc-windows-msvc'
  : 'aarch64-apple-darwin'
if (targetTriple !== expectedTarget) throw new Error('invalid build target triple')
if (!/^[1-9][0-9]*$/u.test(releaseRunId ?? '')) throw new Error('invalid release CI run ID')
if (!Number.isSafeInteger(executedTestCount) || executedTestCount < 1 || executedTestCount > 100000) {
  throw new Error('invalid executed test count')
}
if (
  process.env.CI_CHECK_EVIDENCE_JSON !== JSON.stringify(ciChecks)
  || ciChecks.schema !== 'origami2.ci-check-evidence.v1'
  || ciChecks.sourceCommit !== commit
) throw new Error('CI check evidence is non-canonical or bound to another commit')

for (const key of ['bom-ref', 'purl']) {
  const values = sbom.components.map((component) => component?.[key]).filter(Boolean)
  if (new Set(values).size !== values.length) throw new Error(`duplicate CycloneDX ${key}`)
}
const digest = (file) => createHash('sha256').update(readFileSync(resolve(repositoryRoot, file))).digest('hex')
const properties = {
  'origami2.build.cargo-lock-sha256': digest('Cargo.lock'),
  'origami2.build.node-version': node,
  'origami2.build.package-lock-sha256': digest('apps/desktop/package-lock.json'),
  'origami2.build.rustc-version': rustc,
  'origami2.release.platform': platform,
  'origami2.release.source-commit': commit,
  'origami2.release.version': version,
}
properties['origami2.build.identity-json'] = JSON.stringify({
  schema: 'origami2.build-identity.v1',
  sourceCommit: commit,
  version,
  platform,
  cargoLockSha256: properties['origami2.build.cargo-lock-sha256'],
  packageLockSha256: properties['origami2.build.package-lock-sha256'],
  rustcVersion: rustc,
  nodeVersion: node,
  buildMode,
  targetTriple,
})
properties['origami2.dependency.policy-json'] = JSON.stringify(buildDependencyPolicy())
properties['origami2.release.evidence-json'] = JSON.stringify({
  schema: 'origami2.release-evidence.v1',
  sourceCommit: commit,
  ciRunId: releaseRunId,
  executedTestCount,
  executedSuites: ['formal-release-contract'],
  ciChecks,
})
sbom.metadata = {
  ...(sbom.metadata ?? {}),
  component: { type: 'application', name: 'ORIGAMI2', version },
  properties: Object.entries(properties).map(([name, value]) => ({ name, value })),
}
writeFileSync(path, `${JSON.stringify(sbom)}\n`, 'utf8')
