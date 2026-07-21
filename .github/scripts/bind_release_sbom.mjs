import { createHash } from 'node:crypto'
import { closeSync, fstatSync, ftruncateSync, fsyncSync, lstatSync, openSync, readFileSync, readSync, writeSync } from 'node:fs'
import { resolve } from 'node:path'
import { buildDependencyPolicy } from './dependency_policy.mjs'

const path = process.argv[2]
if (
  typeof path !== 'string'
  || path.length < 1
  || path.length > 4096
  || /[\u0000-\u001f\u007f*?\[\]]/u.test(path)
  || path.startsWith('-')
) throw new Error('invalid SBOM path')
const repositoryRoot = resolve(import.meta.dirname, '..', '..')
const pathStat = lstatSync(path)
if (!pathStat.isFile() || pathStat.isSymbolicLink() || pathStat.size < 2 || pathStat.size > 16_777_216) throw new Error('SBOM path is not a bounded regular file')
const sbomFd = openSync(path, 'r+')
let sbomFdOpen = true
process.on('exit', () => {
  if (sbomFdOpen) closeSync(sbomFd)
})
const openedStat = fstatSync(sbomFd)
if (!openedStat.isFile() || openedStat.dev !== pathStat.dev || openedStat.ino !== pathStat.ino) throw new Error('SBOM file identity changed before open')
const sourceBytes = readFileSync(sbomFd)
const sbom = JSON.parse(sourceBytes.toString('utf8'))
const version = process.env.VERSION
const platform = process.env.PLATFORM
const commit = process.env.RELEASE_COMMIT
const rustc = process.env.RUSTC_VERSION
const node = process.env.NODE_VERSION
const buildMode = process.env.BUILD_MODE
const targetTriple = process.env.TARGET_TRIPLE
const releaseRunId = process.env.RELEASE_RUN_ID
const releaseRunStartedAt = process.env.RELEASE_RUN_STARTED_AT
const sourceCommitAuthoredAt = process.env.SOURCE_COMMIT_AUTHORED_AT
const sourceCommitCommittedAt = process.env.SOURCE_COMMIT_COMMITTED_AT
const releaseTagCreatedAt = process.env.RELEASE_TAG_CREATED_AT || null
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
if (!/^rustc [0-9]+\.[0-9]+\.[0-9]+(?: \([^\u0000-\u001f\u007f()]{1,200}\))?$/u.test(rustc ?? '')) {
  throw new Error('invalid rustc version')
}
if (!/^v[0-9]+\.[0-9]+\.[0-9]+$/u.test(node ?? '')) throw new Error('invalid Node.js version')
if (!['signed-release', 'unsigned-dry-run'].includes(buildMode)) throw new Error('invalid build mode')
const expectedTarget = platform === 'windows-x64'
  ? 'x86_64-pc-windows-msvc'
  : 'aarch64-apple-darwin'
if (targetTriple !== expectedTarget) throw new Error('invalid build target triple')
const canonicalUtcMillis = (value, name) => {
  if (!/^20\d{2}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\dZ$/u.test(value ?? '')) {
    throw new Error(`invalid ${name}`)
  }
  const millis = Date.parse(value)
  if (!Number.isFinite(millis) || new Date(millis).toISOString().replace('.000Z', 'Z') !== value) throw new Error(`non-canonical ${name}`)
  return millis
}
const canonicalGitHubMillis = (value) => {
  if (!/^20\d{2}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])T(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d(?:\.\d{3})?Z$/u.test(value ?? '')) return null
  const millis = Date.parse(value)
  const canonical = value.includes('.') ? new Date(millis).toISOString() : new Date(millis).toISOString().replace('.000Z', 'Z')
  return Number.isFinite(millis) && canonical === value ? millis : null
}
if (!/^[1-9][0-9]*$/u.test(releaseRunId ?? '')) throw new Error('invalid release CI run ID')
const runStartMillis = canonicalUtcMillis(releaseRunStartedAt, 'release CI run start time')
const authoredMillis = canonicalUtcMillis(sourceCommitAuthoredAt, 'source commit author time')
const committedMillis = canonicalUtcMillis(sourceCommitCommittedAt, 'source commit committer time')
if ((buildMode === 'signed-release') !== (releaseTagCreatedAt !== null)) throw new Error('invalid release tag time')
const tagMillis = releaseTagCreatedAt === null ? null : canonicalUtcMillis(releaseTagCreatedAt, 'release tag time')
if (
  authoredMillis > runStartMillis + 300_000
  || committedMillis > runStartMillis + 300_000
  || runStartMillis - committedMillis > 30 * 86_400_000
  || (tagMillis !== null && (tagMillis < committedMillis || tagMillis > runStartMillis + 300_000))
  || ciChecks.artifacts.some(({ createdAt }) => {
    const artifactMillis = Date.parse(createdAt)
    return artifactMillis < committedMillis - 300_000 || artifactMillis > runStartMillis + 300_000
  })
) throw new Error('release chronology is inconsistent or replayed')
if (!Number.isSafeInteger(executedTestCount) || executedTestCount < 1 || executedTestCount > 100000) {
  throw new Error('invalid executed test count')
}
if (
  process.env.CI_CHECK_EVIDENCE_JSON !== JSON.stringify(ciChecks)
  || ciChecks.schema !== 'origami2.ci-check-evidence.v1'
  || ciChecks.sourceCommit !== commit
) throw new Error('CI check evidence is non-canonical or bound to another commit')
const reviewArtifact = ciChecks.rustsecReviewArtifact
const expectedArtifactNames = [
  `ORIGAMI2-macos-app-${ciChecks.workflowRunId}`,
  `ORIGAMI2-windows-nsis-${ciChecks.workflowRunId}`,
  'rustsec-warning-review',
  'sample-viewer-runtime-log',
]
if (
  !Array.isArray(ciChecks.artifacts)
  || ciChecks.artifacts.length !== expectedArtifactNames.length
  || ciChecks.artifacts.map(({ name }) => name).join('\n') !== expectedArtifactNames.join('\n')
  || ciChecks.artifacts.some((artifact) => (
    !/^[1-9][0-9]*$/u.test(artifact?.artifactId ?? '')
    || !/^sha256:[0-9a-f]{64}$/u.test(artifact?.digest ?? '')
    || !Number.isSafeInteger(artifact?.size) || artifact.size < 1 || artifact.size > 2_147_483_648
    || canonicalGitHubMillis(artifact?.createdAt) === null
    || canonicalGitHubMillis(artifact?.expiresAt) === null
    || canonicalGitHubMillis(artifact.expiresAt) - canonicalGitHubMillis(artifact.createdAt) < 6 * 86_400_000
    || canonicalGitHubMillis(artifact.expiresAt) - canonicalGitHubMillis(artifact.createdAt) > 8 * 86_400_000
  ))
) throw new Error('CI artifact inventory evidence is invalid')
const inventoryReview = ciChecks.artifacts.find(({ name }) => name === 'rustsec-warning-review')
if (
  !/^[1-9][0-9]*$/u.test(reviewArtifact?.artifactId ?? '')
  || reviewArtifact?.name !== 'rustsec-warning-review'
  || !/^sha256:[0-9a-f]{64}$/u.test(reviewArtifact?.digest ?? '')
  || reviewArtifact.digest !== `sha256:${reviewArtifact.archiveSha256}`
  || !/^[0-9a-f]{64}$/u.test(reviewArtifact.reportSha256 ?? '')
  || !Number.isSafeInteger(reviewArtifact.size) || reviewArtifact.size < 1 || reviewArtifact.size > 16_777_216
  || reviewArtifact.workflowRunId !== ciChecks.workflowRunId
  || reviewArtifact.runAttempt !== ciChecks.runAttempt
  || reviewArtifact.checkSuiteId !== ciChecks.checkSuiteId
  || inventoryReview?.artifactId !== reviewArtifact.artifactId
  || inventoryReview?.digest !== reviewArtifact.digest
  || inventoryReview?.size !== reviewArtifact.size
  || Date.parse(reviewArtifact.expiresAt) - Date.parse(reviewArtifact.createdAt) < 6 * 86_400_000
  || Date.parse(reviewArtifact.expiresAt) - Date.parse(reviewArtifact.createdAt) > 8 * 86_400_000
) throw new Error('RustSec review artifact evidence is invalid')

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
const dependencyPolicy = buildDependencyPolicy()
properties['origami2.release.evidence-json'] = JSON.stringify({
  schema: 'origami2.release-evidence.v1',
  sourceCommit: commit,
  ciRunId: releaseRunId,
  runStartedAt: releaseRunStartedAt,
  sourceCommitAuthoredAt,
  sourceCommitCommittedAt,
  releaseTagCreatedAt,
  executedTestCount,
  executedSuites: ['formal-release-contract'],
  ciChecks,
  rustsecWarningReview: dependencyPolicy.vulnerabilityAssessment.rustsecReviewReport,
})
sbom.metadata = {
  ...(sbom.metadata ?? {}),
  component: { type: 'application', name: 'ORIGAMI2', version },
  properties: Object.entries(properties).map(([name, value]) => ({ name, value })),
}
const finalStat = fstatSync(sbomFd)
if (finalStat.dev !== openedStat.dev || finalStat.ino !== openedStat.ino || finalStat.size !== openedStat.size) throw new Error('SBOM file changed during binding')
const currentBytes = Buffer.alloc(sourceBytes.length)
if (readSync(sbomFd, currentBytes, 0, currentBytes.length, 0) !== currentBytes.length || !currentBytes.equals(sourceBytes)) throw new Error('SBOM bytes changed during binding')
const output = Buffer.from(`${JSON.stringify(sbom)}\n`)
if (output.length > 16_777_216) throw new Error('bound SBOM exceeds size limit')
ftruncateSync(sbomFd, 0)
if (writeSync(sbomFd, output, 0, output.length, 0) !== output.length) throw new Error('bound SBOM write is partial')
fsyncSync(sbomFd)
closeSync(sbomFd)
sbomFdOpen = false
