import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import test from 'node:test'

import { parseRuntimeUpdateManifest } from '../src/lib/runtimeUpdateManifest.ts'
import { stageAuthorizedRuntimePayload } from '../src/lib/runtimeUpdatePayload.ts'

const payload = new TextEncoder().encode('signed update payload')
const name = 'ORIGAMI2-v2.0.0-windows-x64-setup.exe'
const hash = createHash('sha256').update(payload).digest('hex')
const authorization = parseRuntimeUpdateManifest(JSON.stringify({
  schema: 'origami2.update-manifest.v1', version: '2.0.0', platform: 'windows-x64',
  signaturePolicy: 'platform-signed',
  assets: [
    { name: 'ORIGAMI2-v2.0.0-windows-x64-portable.zip', sha256: 'a'.repeat(64) },
    { name, sha256: hash },
    { name: 'ORIGAMI2-v2.0.0-windows-x64.cdx.json', sha256: 'c'.repeat(64) },
  ],
}), 'windows-x64')
if (!authorization) throw new Error('fixture authorization failed')

function fixture(overrides: Record<string, unknown> = {}) {
  const events: string[] = []
  const defaultStaging = { async begin() { return {
    async write() {}, async commit() {}, async rollback() {},
  } } }
  const rawStaging = (overrides.staging ?? defaultStaging) as typeof defaultStaging
  const dependencies = {
    transport: { async requestPayload() { return (async function * () { yield payload.slice(0, 5); yield payload.slice(5) })() } },
    signatureVerifier: { async verifyPlatformSignature() { events.push('signature'); return true } },
    ...overrides,
    staging: { async begin(_assetName: string) {
      events.push('begin')
      const transaction = await rawStaging.begin()
      return {
        async write(chunk: Uint8Array) { events.push('write'); await transaction.write(chunk) },
        async commit() { events.push('commit'); await transaction.commit() },
        async rollback() { events.push('rollback'); await transaction.rollback() },
      }
    } },
  }
  return { dependencies, events }
}

test('streams an authorized payload then verifies hash and signature before atomic commit', async () => {
  const target = fixture()
  const result = await stageAuthorizedRuntimePayload(authorization, name, target.dependencies)
  assert.deepEqual(result, {
    kind: 'staged', version: '2.0.0', platform: 'windows-x64', assetName: name,
    payloadSha256: hash, byteLength: payload.byteLength,
  })
  assert.deepEqual(target.events, ['begin', 'write', 'write', 'signature', 'commit'])
})

test('fails closed and rolls back every interrupted or invalid payload', async () => {
  const cases: Array<[string, Record<string, unknown>, string]> = [
    ['network', { transport: { async requestPayload() { throw new Error('offline') } } }, 'network'],
    ['cut', { transport: { async requestPayload() { return (async function * () { yield payload.slice(0, 3); throw new Error('cut') })() } } }, 'network'],
    ['oversize', { maxPayloadBytes: 3 }, 'oversize'],
    ['hash', { transport: { async requestPayload() { return (async function * () { yield new Uint8Array([1]) })() } } }, 'hash_mismatch'],
    ['signature', { signatureVerifier: { async verifyPlatformSignature() { return false } } }, 'signature_mismatch'],
    ['disk', { staging: { async begin() { return { async write() { throw new Error('disk') }, async commit() {}, async rollback() {} } } } }, 'storage'],
    ['commit', { staging: { async begin() { return { async write() {}, async commit() { throw new Error('disk') }, async rollback() {} } } } }, 'storage'],
  ]
  for (const [, overrides, reason] of cases) {
    const target = fixture(overrides)
    const result = await stageAuthorizedRuntimePayload(authorization, name, target.dependencies)
    assert.deepEqual(result, { kind: 'rejected', reason })
    assert.equal(target.events.at(-1), 'rollback')
  }
  const forged = { ...authorization }
  assert.deepEqual(await stageAuthorizedRuntimePayload(forged, name, fixture().dependencies), { kind: 'rejected', reason: 'unauthorized' })
})
