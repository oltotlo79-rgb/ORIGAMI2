import { createHash } from 'node:crypto'
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, join, resolve } from 'node:path'
import { validateReleaseArchiveEntries } from './release_archive_contract.mjs'
import { buildDependencyPolicy } from './dependency_policy.mjs'

const directory = resolve(process.argv[2])
const repositoryRoot = resolve(import.meta.dirname, '..', '..')
const platform = process.env.RELEASE_PLATFORM
const version = process.env.RELEASE_VERSION
if (!['windows-x64', 'macos-arm64'].includes(platform)) {
  throw new Error(`unsupported release platform: ${platform ?? '(missing)'}`)
}
if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(version ?? '')) {
  throw new Error(`invalid release version: ${version ?? '(missing)'}`)
}
if (!['true', 'false'].includes(process.env.REQUIRE_SIGNATURE)) {
  throw new Error('REQUIRE_SIGNATURE must be exactly true or false')
}
const expectedSignaturePolicy = process.env.EXPECTED_SIGNATURE_POLICY
  ?? (process.env.REQUIRE_SIGNATURE === 'true' ? 'platform-signed' : 'unsigned-dry-run')
if (!['platform-signed', 'unsigned-dry-run'].includes(expectedSignaturePolicy)) {
  throw new Error('EXPECTED_SIGNATURE_POLICY is invalid')
}
if (process.env.RELEASE_MODE !== undefined) {
  if (!['dry-run', 'prerelease', 'stable'].includes(process.env.RELEASE_MODE)) {
    throw new Error('RELEASE_MODE is invalid for artifact verification')
  }
  const dryRun = process.env.RELEASE_MODE === 'dry-run'
  if (
    process.env.REQUIRE_SIGNATURE !== (dryRun ? 'false' : 'true')
    || expectedSignaturePolicy !== (dryRun ? 'unsigned-dry-run' : 'platform-signed')
  ) throw new Error('release mode and signature policy are inconsistent')
}
const prefix = `ORIGAMI2-v${version}-${platform}`
const payloads = platform === 'windows-x64'
  ? [`${prefix}-setup.exe`, `${prefix}-portable.zip`, `${prefix}.cdx.json`]
  : [`${prefix}-app.tar.gz`, `${prefix}.cdx.json`]
const updateManifest = `${prefix}.update.json`
const releaseFiles = [...payloads, updateManifest]
const checksum = `SHA256SUMS-${platform}.txt`
const expected = [...releaseFiles, checksum].sort()
const actual = readdirSync(directory).sort()
if (actual.join('\n') !== expected.join('\n')) {
  throw new Error(`artifact set mismatch:\n${actual.join('\n')}`)
}
const lines = readFileSync(join(directory, checksum), 'utf8').trim().split(/\r?\n/u)
const entries = lines.map((line) => {
  const match = /^([0-9a-f]{64})  ([^/\\]+)$/u.exec(line)
  if (!match) throw new Error(`invalid checksum line: ${line}`)
  return [match[2], match[1]]
})
const manifestNames = entries.map(([name]) => name)
if (
  manifestNames.length !== releaseFiles.length
  || manifestNames.join('\n') !== [...releaseFiles].sort().join('\n')
) {
  throw new Error('checksum manifest is incomplete or non-canonical')
}
const checksums = new Map(entries)
for (const name of releaseFiles) {
  if (statSync(join(directory, name)).size === 0) throw new Error(`${name} is empty`)
  const digest = createHash('sha256').update(readFileSync(join(directory, name))).digest('hex')
  if (checksums.get(name) !== digest) throw new Error(`${name} checksum mismatch`)
}
const sbom = JSON.parse(readFileSync(join(directory, `${prefix}.cdx.json`), 'utf8'))
if (sbom.bomFormat !== 'CycloneDX' || !Array.isArray(sbom.components)) {
  throw new Error('CycloneDX SBOM contract failed')
}
const sbomProperties = sbom.metadata?.properties
if (process.env.RELEASE_COMMIT !== undefined && !Array.isArray(sbomProperties)) {
  throw new Error('CycloneDX SBOM properties are missing')
}
if (Array.isArray(sbomProperties)) {
const propertyMap = new Map(sbomProperties.map((property) => [property?.name, property?.value]))
if (propertyMap.size !== sbomProperties.length) throw new Error('CycloneDX SBOM properties are duplicated')
const lockDigest = (file) => createHash('sha256').update(readFileSync(resolve(repositoryRoot, file))).digest('hex')
const expectedSbomProperties = new Map([
  ['origami2.build.cargo-lock-sha256', lockDigest('Cargo.lock')],
  ['origami2.build.package-lock-sha256', lockDigest('apps/desktop/package-lock.json')],
  ['origami2.release.platform', platform],
  ['origami2.release.source-commit', process.env.RELEASE_COMMIT],
  ['origami2.release.version', version],
])
for (const [name, value] of expectedSbomProperties) {
  if (typeof value !== 'string' || propertyMap.get(name) !== value) {
    throw new Error(`CycloneDX SBOM property mismatch: ${name}`)
  }
}
if (
  sbom.metadata?.component?.type !== 'application'
  || sbom.metadata?.component?.name !== 'ORIGAMI2'
  || sbom.metadata?.component?.version !== version
  || !/^rustc [0-9]+\.[0-9]+\.[0-9]+/u.test(propertyMap.get('origami2.build.rustc-version') ?? '')
  || !/^v[0-9]+\.[0-9]+\.[0-9]+/u.test(propertyMap.get('origami2.build.node-version') ?? '')
) throw new Error('CycloneDX SBOM build identity mismatch')
const expectedBuildIdentity = {
  schema: 'origami2.build-identity.v1',
  sourceCommit: process.env.RELEASE_COMMIT,
  version,
  platform,
  cargoLockSha256: expectedSbomProperties.get('origami2.build.cargo-lock-sha256'),
  packageLockSha256: expectedSbomProperties.get('origami2.build.package-lock-sha256'),
  rustcVersion: propertyMap.get('origami2.build.rustc-version'),
  nodeVersion: propertyMap.get('origami2.build.node-version'),
  buildMode: expectedSignaturePolicy === 'platform-signed'
    ? 'signed-release'
    : 'unsigned-dry-run',
  targetTriple: platform === 'windows-x64'
    ? 'x86_64-pc-windows-msvc'
    : 'aarch64-apple-darwin',
}
const buildIdentityJson = propertyMap.get('origami2.build.identity-json')
try {
  JSON.parse(buildIdentityJson)
} catch {
  throw new Error('CycloneDX SBOM build identity JSON is invalid')
}
if (buildIdentityJson !== JSON.stringify(expectedBuildIdentity)) {
  throw new Error('CycloneDX SBOM canonical build input identity mismatch')
}
if (
  propertyMap.get('origami2.dependency.policy-json')
  !== JSON.stringify(buildDependencyPolicy())
) throw new Error('CycloneDX SBOM dependency policy binding mismatch')
const evidenceJson = propertyMap.get('origami2.release.evidence-json')
let releaseEvidence
try {
  releaseEvidence = JSON.parse(evidenceJson)
} catch {
  throw new Error('CycloneDX SBOM release evidence JSON is invalid')
}
if (
  JSON.stringify(releaseEvidence.rustsecWarningReview)
  !== JSON.stringify(buildDependencyPolicy().vulnerabilityAssessment.rustsecReviewReport)
) throw new Error('CycloneDX SBOM RustSec review evidence mismatch')
if (
  evidenceJson !== JSON.stringify(releaseEvidence)
  || releaseEvidence.schema !== 'origami2.release-evidence.v1'
  || releaseEvidence.sourceCommit !== process.env.RELEASE_COMMIT
  || !/^[1-9][0-9]*$/u.test(releaseEvidence.ciRunId ?? '')
  || !Number.isSafeInteger(releaseEvidence.executedTestCount)
  || releaseEvidence.executedTestCount < 1
  || releaseEvidence.executedTestCount > 100000
  || JSON.stringify(releaseEvidence.executedSuites) !== '["formal-release-contract"]'
  || releaseEvidence.ciChecks?.schema !== 'origami2.ci-check-evidence.v1'
  || releaseEvidence.ciChecks?.sourceCommit !== process.env.RELEASE_COMMIT
  || !/^[1-9][0-9]*$/u.test(releaseEvidence.ciChecks?.workflowRunId ?? '')
  || !Number.isSafeInteger(releaseEvidence.ciChecks?.runAttempt)
  || releaseEvidence.ciChecks.runAttempt < 1
  || !/^[1-9][0-9]*$/u.test(releaseEvidence.ciChecks?.checkSuiteId ?? '')
  || releaseEvidence.ciChecks?.workflow !== '.github/workflows/ci.yml'
  || releaseEvidence.ciChecks?.rustsecReviewArtifact?.name !== 'rustsec-warning-review'
  || releaseEvidence.ciChecks?.rustsecReviewArtifact?.digest
    !== `sha256:${releaseEvidence.ciChecks?.rustsecReviewArtifact?.archiveSha256}`
  || releaseEvidence.ciChecks?.rustsecReviewArtifact?.workflowRunId
    !== releaseEvidence.ciChecks?.workflowRunId
  || releaseEvidence.ciChecks?.rustsecReviewArtifact?.runAttempt
    !== releaseEvidence.ciChecks?.runAttempt
  || releaseEvidence.ciChecks?.rustsecReviewArtifact?.checkSuiteId
    !== releaseEvidence.ciChecks?.checkSuiteId
  || !Array.isArray(releaseEvidence.ciChecks?.checks)
  || releaseEvidence.ciChecks.checks.length < 1
  || releaseEvidence.ciChecks.checks.length > 100
  || releaseEvidence.ciChecks.checks.some((check) => (
    typeof check?.name !== 'string'
    || check.name.length < 1
    || check.name.length > 200
    || JSON.stringify(check) !== JSON.stringify({ name: check.name, conclusion: 'success' })
  ))
  || new Set(releaseEvidence.ciChecks.checks.map((check) => check.name)).size
    !== releaseEvidence.ciChecks.checks.length
  || JSON.stringify(releaseEvidence.ciChecks.checks)
    !== JSON.stringify([...releaseEvidence.ciChecks.checks].sort((left, right) =>
      left.name.localeCompare(right.name)))
  || JSON.stringify(releaseEvidence.ciChecks) !== JSON.stringify({
    schema: releaseEvidence.ciChecks.schema,
    sourceCommit: releaseEvidence.ciChecks.sourceCommit,
    workflow: releaseEvidence.ciChecks.workflow,
    workflowRunId: releaseEvidence.ciChecks.workflowRunId,
    runAttempt: releaseEvidence.ciChecks.runAttempt,
    checkSuiteId: releaseEvidence.ciChecks.checkSuiteId,
    checks: releaseEvidence.ciChecks.checks,
    rustsecReviewArtifact: releaseEvidence.ciChecks.rustsecReviewArtifact,
  })
  || (process.env.RELEASE_RUN_ID !== undefined && releaseEvidence.ciRunId !== process.env.RELEASE_RUN_ID)
  || (process.env.EXECUTED_TEST_COUNT !== undefined
    && releaseEvidence.executedTestCount !== Number(process.env.EXECUTED_TEST_COUNT))
) throw new Error('CycloneDX SBOM canonical release evidence mismatch')
}
const updateManifestBytes = readFileSync(join(directory, updateManifest), 'utf8')
const parsedUpdateManifest = JSON.parse(updateManifestBytes)
const expectedUpdateManifest = {
  schema: 'origami2.update-manifest.v1',
  version,
  platform,
  signaturePolicy: expectedSignaturePolicy,
  assets: [...payloads].sort().map((name) => ({
    name,
    sha256: checksums.get(name),
  })),
}
if (
  updateManifestBytes !== `${JSON.stringify(expectedUpdateManifest)}\n`
) {
  throw new Error('update manifest is non-canonical or digest binding failed')
}
if (process.env.REQUIRE_SIGNATURE === 'true') {
  const { execFileSync } = await import('node:child_process')
  if (platform === 'windows-x64') {
    const extracted = mkdtempSync(join(tmpdir(), 'origami2-portable-signature-'))
    try {
      const entryOutput = execFileSync('pwsh', [
        '-NoProfile',
        '-Command',
        'Add-Type -AssemblyName System.IO.Compression.FileSystem; $archive = [IO.Compression.ZipFile]::OpenRead($args[0]); try { $archive.Entries.FullName } finally { $archive.Dispose() }',
        join(directory, `${prefix}-portable.zip`),
      ], { encoding: 'utf8' })
      validateReleaseArchiveEntries(
        platform,
        entryOutput.split(/\r?\n/u).filter(Boolean),
      )
      execFileSync('pwsh', [
        '-NoProfile',
        '-Command',
        'Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1]',
        join(directory, `${prefix}-portable.zip`),
        extracted,
      ])
      const signedExecutables = [
        join(directory, `${prefix}-setup.exe`),
        join(extracted, 'origami2-desktop.exe'),
      ]
      if (!statSync(signedExecutables[1]).isFile()) {
        throw new Error('portable archive executable contract failed')
      }
      for (const executable of signedExecutables) {
        const escaped = executable.replaceAll("'", "''")
        const command = `$s = Get-AuthenticodeSignature -LiteralPath '${escaped}'; if ($s.Status -ne 'Valid' -or $null -eq $s.SignerCertificate -or $null -eq $s.TimeStamperCertificate) { throw 'signature, chain, or RFC 3161 timestamp failed' }; $s.Status`
        const status = execFileSync('pwsh', ['-NoProfile', '-Command', command], { encoding: 'utf8' }).trim()
        if (status !== 'Valid') throw new Error(`${basename(executable)} Authenticode status is ${status}`)
        execFileSync('signtool', ['verify', '/pa', '/all', executable], { stdio: 'inherit' })
      }
    } finally {
      rmSync(extracted, { recursive: true, force: true })
    }
  } else {
    const extracted = mkdtempSync(join(tmpdir(), 'origami2-macos-signature-'))
    try {
      const entryOutput = execFileSync(
        'tar',
        ['-tzf', join(directory, `${prefix}-app.tar.gz`)],
        { encoding: 'utf8' },
      )
      validateReleaseArchiveEntries(
        platform,
        entryOutput.split(/\r?\n/u).filter(Boolean),
      )
      execFileSync('tar', [
        '-xzf',
        join(directory, `${prefix}-app.tar.gz`),
        '-C',
        extracted,
      ])
      const app = join(extracted, 'ORIGAMI2.app')
      if (!statSync(app).isDirectory()) {
        throw new Error('macOS archive application contract failed')
      }
      execFileSync('codesign', ['--verify', '--deep', '--strict', app], { stdio: 'inherit' })
      execFileSync('xcrun', ['stapler', 'validate', app], { stdio: 'inherit' })
      execFileSync('spctl', ['--assess', '--type', 'execute', '--verbose=4', app], {
        stdio: 'inherit',
      })
    } finally {
      rmSync(extracted, { recursive: true, force: true })
    }
  }
}
console.log(`verified ${basename(directory)} ${platform} release artifacts`)
