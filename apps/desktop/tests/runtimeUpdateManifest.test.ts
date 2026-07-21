import assert from 'node:assert/strict'
import test from 'node:test'

import { authorizeRuntimeUpdate } from '../src/lib/runtimeUpdateManifest.ts'

const manifest = (overrides: Record<string, unknown> = {}) => JSON.stringify({
  schema: 'origami2.update-manifest.v1',
  version: '2.0.0',
  platform: 'windows-x64',
  signaturePolicy: 'platform-signed',
  assets: [
    { name: 'ORIGAMI2-v2.0.0-windows-x64-portable.zip', sha256: 'a'.repeat(64) },
    { name: 'ORIGAMI2-v2.0.0-windows-x64-setup.exe', sha256: 'b'.repeat(64) },
    { name: 'ORIGAMI2-v2.0.0-windows-x64.cdx.json', sha256: 'c'.repeat(64) },
  ],
  ...overrides,
})

test('mock metadata transport authorizes a newer signed checksummed platform release', async () => {
  let calls = 0
  const result = await authorizeRuntimeUpdate({
    async requestManifest() { calls += 1; return manifest() },
  }, '1.9.9', 'windows-x64')
  assert.equal(calls, 1)
  assert.equal(result.kind, 'authorized')
  if (result.kind === 'authorized') {
    assert.equal(result.authorization.signaturePolicy, 'platform-signed')
    assert.equal(result.authorization.assets.length, 3)
    assert.doesNotMatch(JSON.stringify(result), /https?:|download_url/iu)
  }
})

test('fails closed before download for offline rollback malformed and mismatched feeds', async () => {
  const check = (body: unknown, current = '1.9.9') => authorizeRuntimeUpdate({
    async requestManifest() { return body },
  }, current, 'windows-x64')
  assert.deepEqual(await check(manifest(), '2.0.0'), { kind: 'rejected', reason: 'rollback' })
  assert.deepEqual(await check(manifest(), '2.0.1'), { kind: 'rejected', reason: 'rollback' })
  for (const body of [
    '{',
    manifest({ platform: 'macos-arm64' }),
    manifest({ signaturePolicy: 'unsigned-dry-run' }),
    manifest({ assets: [{ name: '../payload.exe', sha256: 'a'.repeat(64) }] }),
    manifest({ assets: [
      { name: 'ORIGAMI2-v2.0.0-windows-x64-portable.zip', sha256: 'A'.repeat(64) },
      { name: 'ORIGAMI2-v2.0.0-windows-x64-setup.exe', sha256: 'b'.repeat(64) },
      { name: 'ORIGAMI2-v2.0.0-windows-x64.cdx.json', sha256: 'c'.repeat(64) },
    ] }),
    manifest({ unexpected: true }),
  ]) assert.deepEqual(await check(body), { kind: 'rejected', reason: 'malformed' })
  const offline = await authorizeRuntimeUpdate({
    async requestManifest() { throw new Error('offline') },
  }, '1.0.0', 'windows-x64')
  assert.deepEqual(offline, { kind: 'rejected', reason: 'offline' })
})
