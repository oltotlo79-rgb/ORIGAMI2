import { createHash } from 'node:crypto'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'

const directory = resolve(process.argv[2])
const platform = process.env.RELEASE_PLATFORM
const version = process.env.RELEASE_VERSION
const prefix = `ORIGAMI2-v${version}-${platform}`
const payloads = platform === 'windows-x64'
  ? [`${prefix}-setup.exe`, `${prefix}-portable.zip`, `${prefix}.cdx.json`]
  : [`${prefix}-app.tar.gz`, `${prefix}.cdx.json`]
const checksum = `SHA256SUMS-${platform}.txt`
const expected = [...payloads, checksum].sort()
const actual = readdirSync(directory).sort()
if (actual.join('\n') !== expected.join('\n')) {
  throw new Error(`artifact set mismatch:\n${actual.join('\n')}`)
}
const lines = readFileSync(join(directory, checksum), 'utf8').trim().split(/\r?\n/u)
const checksums = new Map(lines.map((line) => {
  const match = /^([0-9a-f]{64})  ([^/\\]+)$/u.exec(line)
  if (!match) throw new Error(`invalid checksum line: ${line}`)
  return [match[2], match[1]]
}))
if (checksums.size !== payloads.length) throw new Error('checksum manifest is incomplete')
for (const name of payloads) {
  if (statSync(join(directory, name)).size === 0) throw new Error(`${name} is empty`)
  const digest = createHash('sha256').update(readFileSync(join(directory, name))).digest('hex')
  if (checksums.get(name) !== digest) throw new Error(`${name} checksum mismatch`)
}
const sbom = JSON.parse(readFileSync(join(directory, `${prefix}.cdx.json`), 'utf8'))
if (sbom.bomFormat !== 'CycloneDX' || !Array.isArray(sbom.components)) {
  throw new Error('CycloneDX SBOM contract failed')
}
if (process.env.REQUIRE_SIGNATURE === 'true') {
  const { execFileSync } = await import('node:child_process')
  if (platform === 'windows-x64') {
    for (const name of payloads.filter((item) => item.endsWith('.exe'))) {
      const command = `(Get-AuthenticodeSignature -LiteralPath '${join(directory, name).replaceAll("'", "''")}').Status`
      const status = execFileSync('pwsh', ['-NoProfile', '-Command', command], { encoding: 'utf8' }).trim()
      if (status !== 'Valid') throw new Error(`${name} Authenticode status is ${status}`)
    }
  } else {
    execFileSync('codesign', ['--verify', '--deep', '--strict',
      join('target', 'release', 'bundle', 'macos', 'ORIGAMI2.app')], { stdio: 'inherit' })
  }
}
console.log(`verified ${basename(directory)} ${platform} release artifacts`)
