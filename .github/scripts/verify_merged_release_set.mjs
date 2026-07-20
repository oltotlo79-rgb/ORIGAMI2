import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs'
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
if (process.env.RELEASE_COMMIT !== undefined) {
  const identities = [...platformFiles.keys()].map((platform) => {
    const sbom = JSON.parse(readFileSync(
      join(directory, `ORIGAMI2-v${version}-${platform}.cdx.json`),
      'utf8',
    ))
    const properties = new Map(
      sbom.metadata?.properties?.map(({ name, value }) => [name, value]) ?? [],
    )
    const identityJson = properties.get('origami2.build.identity-json')
    let canonicalIdentity
    try {
      canonicalIdentity = JSON.parse(identityJson)
    } catch {
      throw new Error('cross-platform canonical build input identity is invalid')
    }
    if (identityJson !== JSON.stringify(canonicalIdentity)) {
      throw new Error('cross-platform build input identity is non-canonical')
    }
    return {
      platform,
      cargoLock: properties.get('origami2.build.cargo-lock-sha256'),
      node: properties.get('origami2.build.node-version'),
      packageLock: properties.get('origami2.build.package-lock-sha256'),
      rustc: properties.get('origami2.build.rustc-version'),
      sourceCommit: properties.get('origami2.release.source-commit'),
      version: properties.get('origami2.release.version'),
      declaredPlatform: properties.get('origami2.release.platform'),
      canonicalIdentity,
    }
  })
  for (const identity of identities) {
    if (identity.declaredPlatform !== identity.platform) {
      throw new Error('cross-platform SBOM platform identity mismatch')
    }
    const expectedTargetTriple = identity.platform === 'windows-x64'
      ? 'x86_64-pc-windows-msvc'
      : 'aarch64-apple-darwin'
    if (
      identity.canonicalIdentity.platform !== identity.platform
      || identity.canonicalIdentity.targetTriple !== expectedTargetTriple
    ) throw new Error('cross-platform build target identity mismatch')
  }
  const withoutPlatform = ({
    platform: _platform,
    declaredPlatform: _declared,
    canonicalIdentity,
    ...identity
  }) => ({
    ...identity,
    canonicalIdentity: {
      ...canonicalIdentity,
      platform: undefined,
      targetTriple: undefined,
    },
  })
  if (JSON.stringify(withoutPlatform(identities[0])) !== JSON.stringify(withoutPlatform(identities[1]))) {
    throw new Error('cross-platform build input identity mismatch')
  }
}
console.log(`verified merged release set v${version}`)
