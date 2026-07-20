import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'

const root = resolve(import.meta.dirname, '..', '..')

test('release workflow keeps publication permissions out of build jobs', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const validator = readFileSync(join(root, '.github/scripts/validate_formal_release.mjs'), 'utf8')
  assert.match(workflow, /options: \[dry-run, prerelease, stable, promote\]/u)
  assert.match(validator, /'git', \['verify-tag', tag\]/u)
  assert.match(workflow, /permissions:\s*\n\s+contents: read/u)
  assert.match(workflow, /attest-build-provenance@43d14bc2/u)
  assert.match(workflow, /gh release edit "\$RELEASE_TAG".*--prerelease=false --latest/u)
  assert.doesNotMatch(workflow, /pull_request:/u)
})

test('CI and formal release share the strict macOS bundle contract', () => {
  const ciWorkflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const releaseWorkflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const verifier = readFileSync(join(root, '.github/scripts/verify_macos_bundle.sh'), 'utf8')
  assert.match(ciWorkflow, /\.\/\.github\/scripts\/verify_macos_bundle\.sh/u)
  assert.match(ciWorkflow, /ORIGAMI2-macos-app\.tar\.gz/u)
  assert.match(releaseWorkflow, /verify_macos_bundle\.sh target\/release\/bundle\/macos\/ORIGAMI2\.app/u)
  assert.match(verifier, /CFBundleIdentifier/u)
  assert.match(verifier, /CFBundleShortVersionString/u)
  assert.match(verifier, /\[\[ -x "\$bundle\/Contents\/MacOS\/\$executable_name" \]\]/u)
  assert.match(verifier, /c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f/u)
})

test('CI and formal release share the strict Windows bundle contract', () => {
  const ciWorkflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')
  const releaseWorkflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const verifier = readFileSync(join(root, 'scripts/verify_windows_bundle.ps1'), 'utf8')
  assert.match(ciWorkflow, /verify_windows_bundle\.ps1[\s\S]*-ExpectedSignatureStatus NotSigned/u)
  assert.match(releaseWorkflow, /verify_windows_bundle\.ps1[\s\S]*-ExpectedVersion \$env:RELEASE_VERSION/u)
  assert.match(releaseWorkflow, /SIGNATURE_STATUS:.*Valid.*NotSigned/u)
  assert.match(verifier, /GetVersionInfo/u)
  assert.match(verifier, /Get-AuthenticodeSignature/u)
  assert.match(verifier, /Embedded Windows executable/u)
  assert.match(verifier, /NotoSansJP-Variable\.ttf/u)
  assert.match(verifier, /NotoSansJP-OFL\.txt/u)
})

test('dry-run validates without a tag or GitHub mutation', () => {
  const output = execFileSync('node', ['.github/scripts/validate_formal_release.mjs'], {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, REQUESTED_MODE: 'dry-run', REQUESTED_TAG: '' },
  })
  assert.match(output, /mode=dry-run/u)
})

test('local artifact verifier accepts checksummed CycloneDX fixtures', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-release-contract-'))
  try {
    const prefix = 'ORIGAMI2-v0.1.0-windows-x64'
    const payloads = {
      [`${prefix}-setup.exe`]: 'installer',
      [`${prefix}-portable.zip`]: 'portable',
      [`${prefix}.cdx.json`]: JSON.stringify({ bomFormat: 'CycloneDX', components: [] }),
    }
    const checksums = []
    for (const [name, value] of Object.entries(payloads)) {
      writeFileSync(join(directory, name), value)
      checksums.push(`${createHash('sha256').update(value).digest('hex')}  ${name}`)
    }
    writeFileSync(
      join(directory, 'SHA256SUMS-windows-x64.txt'),
      `${checksums.sort((left, right) => left.slice(66).localeCompare(right.slice(66))).join('\n')}\n`,
    )
    execFileSync('node', ['.github/scripts/verify_formal_release.mjs', directory], {
      cwd: root,
      env: {
        ...process.env,
        RELEASE_PLATFORM: 'windows-x64',
        RELEASE_VERSION: '0.1.0',
        REQUIRE_SIGNATURE: 'false',
      },
    })
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('local artifact verifier rejects non-canonical checksum manifests', () => {
  const directory = mkdtempSync(join(tmpdir(), 'origami2-release-contract-'))
  try {
    const prefix = 'ORIGAMI2-v0.1.0-windows-x64'
    const payloads = {
      [`${prefix}-setup.exe`]: 'installer',
      [`${prefix}-portable.zip`]: 'portable',
      [`${prefix}.cdx.json`]: JSON.stringify({ bomFormat: 'CycloneDX', components: [] }),
    }
    for (const [name, value] of Object.entries(payloads)) {
      writeFileSync(join(directory, name), value)
    }
    const entries = Object.entries(payloads).map(([name, value]) =>
      `${createHash('sha256').update(value).digest('hex')}  ${name}`,
    )
    const manifest = join(directory, 'SHA256SUMS-windows-x64.txt')
    const verify = () => execFileSync(
      'node',
      ['.github/scripts/verify_formal_release.mjs', directory],
      {
        cwd: root,
        stdio: 'pipe',
        env: {
          ...process.env,
          RELEASE_PLATFORM: 'windows-x64',
          RELEASE_VERSION: '0.1.0',
          REQUIRE_SIGNATURE: 'false',
        },
      },
    )

    writeFileSync(manifest, `${entries.reverse().join('\n')}\n`)
    assert.throws(verify, /non-canonical/u)

    writeFileSync(manifest, `${entries.slice(0, 2).sort().join('\n')}\n`)
    assert.throws(verify, /incomplete/u)

    writeFileSync(manifest, `${[...entries, entries[0]].sort().join('\n')}\n`)
    assert.throws(verify, /non-canonical/u)
  } finally {
    rmSync(directory, { recursive: true, force: true })
  }
})

test('local artifact verifier rejects unbounded platform and version input', () => {
  const verify = (platform, version) => () => execFileSync(
    'node',
    ['.github/scripts/verify_formal_release.mjs', root],
    {
      cwd: root,
      stdio: 'pipe',
      env: {
        ...process.env,
        RELEASE_PLATFORM: platform,
        RELEASE_VERSION: version,
        REQUIRE_SIGNATURE: 'false',
      },
    },
  )
  assert.throws(verify('linux-x64', '0.1.0'), /unsupported release platform/u)
  assert.throws(verify('windows-x64', '../0.1.0'), /invalid release version/u)
  assert.throws(verify('macos-arm64', '01.0.0'), /invalid release version/u)
})

test('local artifact verifier requires explicit signature policy and verifies packaged payloads', () => {
  const verifier = readFileSync(
    join(root, '.github/scripts/verify_formal_release.mjs'),
    'utf8',
  )
  assert.match(verifier, /origami2-desktop\.exe/u)
  assert.match(verifier, /origami2-macos-signature-/u)
  assert.doesNotMatch(
    verifier,
    /target['"], ['"]release['"], ['"]bundle['"], ['"]macos/u,
  )
  assert.throws(
    () => execFileSync(
      'node',
      ['.github/scripts/verify_formal_release.mjs', root],
      {
        cwd: root,
        stdio: 'pipe',
        env: {
          ...process.env,
          RELEASE_PLATFORM: 'windows-x64',
          RELEASE_VERSION: '0.1.0',
          REQUIRE_SIGNATURE: 'yes',
        },
      },
    ),
    /REQUIRE_SIGNATURE must be exactly true or false/u,
  )
})
