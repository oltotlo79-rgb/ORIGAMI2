import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const directory = resolve(process.argv[2])
const version = process.env.RELEASE_VERSION
if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(version ?? '')) {
  throw new Error('invalid merged release version')
}

const platformFiles = new Map([
  ['windows-x64', [
    `ORIGAMI2-v${version}-windows-x64-portable.zip`,
    `ORIGAMI2-v${version}-windows-x64-setup.exe`,
    `ORIGAMI2-v${version}-windows-x64.cdx.json`,
    `ORIGAMI2-v${version}-windows-x64.update.json`,
    'SHA256SUMS-windows-x64.txt',
  ]],
  ['macos-arm64', [
    `ORIGAMI2-v${version}-macos-arm64-app.tar.gz`,
    `ORIGAMI2-v${version}-macos-arm64.cdx.json`,
    `ORIGAMI2-v${version}-macos-arm64.update.json`,
    'SHA256SUMS-macos-arm64.txt',
  ]],
])
const expected = [...platformFiles.values()].flat().sort()
const actual = readdirSync(directory).sort()
if (actual.join('\n') !== expected.join('\n')) {
  throw new Error(`merged release asset set mismatch:\n${actual.join('\n')}`)
}

const verifier = resolve(import.meta.dirname, 'verify_formal_release.mjs')
for (const [platform, names] of platformFiles) {
  const staging = mkdtempSync(join(tmpdir(), `origami2-${platform}-verify-`))
  try {
    for (const name of names) copyFileSync(join(directory, name), join(staging, name))
    execFileSync(process.execPath, [verifier, staging], {
      stdio: 'inherit',
      env: {
        ...process.env,
        RELEASE_PLATFORM: platform,
        RELEASE_VERSION: version,
        REQUIRE_SIGNATURE: 'false',
      },
    })
  } finally {
    rmSync(staging, { recursive: true, force: true })
  }
}
console.log(`verified merged release set v${version}`)
