import { createHash } from 'node:crypto'
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, join, resolve } from 'node:path'
import { validateReleaseArchiveEntries } from './release_archive_contract.mjs'

const directory = resolve(process.argv[2])
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
if (
  process.env.RELEASE_MODE !== undefined
  && process.env.RELEASE_MODE !== 'dry-run'
  && process.env.REQUIRE_SIGNATURE !== 'true'
) throw new Error('publishable release mode requires platform signatures')
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
const lockDigest = (file) => createHash('sha256').update(readFileSync(file)).digest('hex')
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
  buildMode: process.env.REQUIRE_SIGNATURE === 'true'
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
}
const updateManifestBytes = readFileSync(join(directory, updateManifest), 'utf8')
const parsedUpdateManifest = JSON.parse(updateManifestBytes)
const expectedUpdateManifest = {
  schema: 'origami2.update-manifest.v1',
  version,
  platform,
  signaturePolicy: process.env.REQUIRE_SIGNATURE === 'true'
    ? 'platform-signed'
    : 'unsigned-dry-run',
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
