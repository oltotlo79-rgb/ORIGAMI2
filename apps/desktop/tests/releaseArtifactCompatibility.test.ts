import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

import {
  selectReleaseArtifactPlan,
} from '../src/lib/releaseArtifactCompatibility.ts'

const version = '1.2.3'
const assets = [
  'ORIGAMI2-v1.2.3-windows-x64-setup.exe',
  'ORIGAMI2-v1.2.3-windows-x64-portable.zip',
  'ORIGAMI2-v1.2.3-windows-x64.cdx.json',
  'SHA256SUMS-windows-x64.txt',
  'ORIGAMI2-v1.2.3-macos-arm64-app.tar.gz',
  'ORIGAMI2-v1.2.3-macos-arm64.cdx.json',
  'SHA256SUMS-macos-arm64.txt',
]

test('selects exact signed-release assets without retaining download authority', () => {
  const windows = selectReleaseArtifactPlan(version, 'windows-x64', assets)
  const macos = selectReleaseArtifactPlan(version, 'macos-arm64', assets)
  assert.deepEqual(windows, {
    schema: 'origami2.release-artifact-compatibility.v1',
    version,
    platform: 'windows-x64',
    payloadName: 'ORIGAMI2-v1.2.3-windows-x64-setup.exe',
    supplementalPayloadName:
      'ORIGAMI2-v1.2.3-windows-x64-portable.zip',
    checksumManifestName: 'SHA256SUMS-windows-x64.txt',
    sbomName: 'ORIGAMI2-v1.2.3-windows-x64.cdx.json',
    signatureVerification: 'authenticode',
    provenanceAttestationRequired: true,
    userConfirmationRequired: true,
  })
  assert.equal(macos?.signatureVerification, 'apple_codesign')
  assert.equal(macos?.supplementalPayloadName, null)
  assert.doesNotMatch(JSON.stringify([windows, macos]), /https?:|download/iu)
  assert.equal(Object.isFrozen(windows), true)
  assert.equal(Object.isFrozen(macos), true)
})

test('rejects incomplete ambiguous and path-bearing release metadata', () => {
  for (const candidate of [
    assets.filter((name) => name !== 'SHA256SUMS-windows-x64.txt'),
    [...assets, assets[0]],
    [...assets, 'unexpected-debug-symbols.zip'],
    assets.map((name) => name.replace('v1.2.3', 'v1.2.4')),
    [...assets, '../payload.exe'],
    Array.from({ length: 33 }, (_, index) => `asset-${index}`),
    new Proxy([], {
      getPrototypeOf() {
        throw new Error('private path')
      },
    }),
  ]) {
    assert.equal(
      selectReleaseArtifactPlan(version, 'windows-x64', candidate),
      null,
    )
  }
})

test('formal release output and manual update UI remain contract-compatible', async () => {
  const workflow = await readFile(
    new URL('../../../.github/workflows/release.yml', import.meta.url),
    'utf8',
  )
  const packager = await readFile(
    new URL('../../../.github/scripts/package_formal_release.ps1', import.meta.url),
    'utf8',
  )
  const verifier = await readFile(
    new URL('../../../.github/scripts/verify_formal_release.mjs', import.meta.url),
    'utf8',
  )
  const updateClient = await readFile(
    new URL('../src/lib/githubReleaseUpdate.ts', import.meta.url),
    'utf8',
  )
  const updateControl = await readFile(
    new URL('../src/components/UpdateCheckControl.tsx', import.meta.url),
    'utf8',
  )

  assert.match(packager, /-setup\.exe/u)
  assert.match(packager, /-portable\.zip/u)
  assert.match(packager, /-app\.tar\.gz/u)
  assert.match(packager, /SHA256SUMS-\$env:PLATFORM\.txt/u)
  assert.match(verifier, /checksum manifest is incomplete/u)
  assert.match(workflow, /REQUIRE_SIGNATURE:.*true.*false/u)
  assert.match(workflow, /attest-build-provenance/u)
  assert.match(updateClient, /manual-only update client/u)
  assert.match(updateClient, /no scheduler,[\s\S]*downloader,[\s\S]*installer/u)
  assert.match(updateControl, /target="_blank"/u)
  assert.match(updateControl, /rel="noopener noreferrer"/u)
  assert.doesNotMatch(updateControl, /download=/u)
})
