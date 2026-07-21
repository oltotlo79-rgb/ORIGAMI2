import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  authorizeRuntimeUpdate,
  parseRuntimeUpdateManifest,
} from '../../apps/desktop/src/lib/runtimeUpdateManifest.ts'
import { stageAuthorizedRuntimePayload } from '../../apps/desktop/src/lib/runtimeUpdatePayload.ts'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')
const version = '2.0.0'
const directory = mkdtempSync(join(tmpdir(), 'origami2-updater-contract-'))
try {
  for (const platform of ['windows-x64', 'macos-arm64']) {
    const prefix = `ORIGAMI2-v${version}-${platform}`
    const names = platform === 'windows-x64'
      ? [`${prefix}-portable.zip`, `${prefix}-setup.exe`, `${prefix}.cdx.json`]
      : [`${prefix}-app.tar.gz`, `${prefix}.cdx.json`]
    for (const name of names) writeFileSync(join(directory, name), `fixture:${name}`)
    const generated = spawnSync(process.execPath, [join(root, '.github/scripts/write_update_manifest.mjs'), directory], {
      cwd: root, encoding: 'utf8', env: { ...process.env, PLATFORM: platform, VERSION: version, SIGNATURE_POLICY: 'platform-signed' },
    })
    if (generated.status !== 0) throw new Error(generated.stderr || 'manifest generator failed')
    const body = readFileSync(join(directory, `${prefix}.update.json`), 'utf8')
    const parsed = parseRuntimeUpdateManifest(body, platform)
    if (!parsed || parsed.assets.length !== names.length) throw new Error('runtime parser rejected formal manifest')
    for (const asset of parsed.assets) {
      const expected = createHash('sha256').update(readFileSync(join(directory, asset.name))).digest('hex')
      if (asset.sha256 !== expected) throw new Error('runtime parser checksum binding mismatch')
    }
    const stable = await authorizeRuntimeUpdate({ async requestManifest() { return body } }, '1.9.9', platform)
    if (stable.kind !== 'authorized') throw new Error('stable upgrade was not authorized')
    const tamperedManifest = { ...JSON.parse(body), assets: JSON.parse(body).assets.map((asset, index) => index === 0 ? { ...asset, sha256: '0'.repeat(64) } : asset) }
    const tampered = await authorizeRuntimeUpdate({ async requestManifest() { return JSON.stringify(tamperedManifest) } }, '1.9.9', platform)
    if (tampered.kind !== 'authorized') throw new Error('tamper fixture setup failed')
    const tamperedAsset = tampered.authorization.assets[0]
    const tamperedResult = await stageAuthorizedRuntimePayload(tampered.authorization, tamperedAsset.name, {
      transport: { async requestPayload() { return (async function * () { yield readFileSync(join(directory, tamperedAsset.name)) })() } },
      signatureVerifier: { async verifyPlatformSignature() { return true } },
      staging: { async begin() { return { async write() {}, async commit() {}, async rollback() {} } } },
    })
    if (tamperedResult.kind !== 'rejected' || tamperedResult.reason !== 'hash_mismatch') throw new Error('tampered payload was not rejected')
    for (const installed of ['2.0.0', '2.0.1']) {
      const rollback = await authorizeRuntimeUpdate({ async requestManifest() { return body } }, installed, platform)
      if (rollback.kind !== 'rejected' || rollback.reason !== 'rollback') throw new Error('rollback was not rejected')
    }
    const manifest = JSON.parse(body)
    const rejected = [
      { ...manifest, version: '2.0.0-rc.1' },
      { ...manifest, signaturePolicy: 'unsigned-dry-run' },
      { ...manifest, assets: manifest.assets.slice(1) },
      { ...manifest, platform: platform === 'windows-x64' ? 'macos-arm64' : 'windows-x64' },
    ]
    for (const candidate of rejected) {
      if (parseRuntimeUpdateManifest(JSON.stringify(candidate), platform) !== null) {
        throw new Error('runtime parser accepted a tampered release manifest')
      }
    }
  }
  console.log('formal release manifests satisfy the runtime updater contract')
} finally {
  rmSync(directory, { recursive: true, force: true })
}
