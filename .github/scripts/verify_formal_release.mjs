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
const updateManifestBytes = readFileSync(join(directory, updateManifest), 'utf8')
const parsedUpdateManifest = JSON.parse(updateManifestBytes)
const expectedUpdateManifest = {
  schema: 'origami2.update-manifest.v1',
  version,
  platform,
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
        const command = `(Get-AuthenticodeSignature -LiteralPath '${executable.replaceAll("'", "''")}').Status`
        const status = execFileSync('pwsh', ['-NoProfile', '-Command', command], { encoding: 'utf8' }).trim()
        if (status !== 'Valid') throw new Error(`${basename(executable)} Authenticode status is ${status}`)
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
    } finally {
      rmSync(extracted, { recursive: true, force: true })
    }
  }
}
console.log(`verified ${basename(directory)} ${platform} release artifacts`)
