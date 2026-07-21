import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import test from 'node:test'

import { applyStagedRuntimeUpdate, recoverPendingRuntimeUpdate } from '../src/lib/runtimeUpdateApply.ts'
import { parseRuntimeUpdateManifest } from '../src/lib/runtimeUpdateManifest.ts'
import { stageAuthorizedRuntimePayload, type RuntimeStagedPayload } from '../src/lib/runtimeUpdatePayload.ts'

const bytes = new TextEncoder().encode('installer')
const hash = createHash('sha256').update(bytes).digest('hex')
const name = 'ORIGAMI2-v3.0.0-windows-x64-setup.exe'

async function stagedFixture(): Promise<RuntimeStagedPayload> {
  const authorization = parseRuntimeUpdateManifest(JSON.stringify({
    schema: 'origami2.update-manifest.v1', version: '3.0.0', platform: 'windows-x64', signaturePolicy: 'platform-signed',
    assets: [
      { name: 'ORIGAMI2-v3.0.0-windows-x64-portable.zip', sha256: 'a'.repeat(64) },
      { name, sha256: hash }, { name: 'ORIGAMI2-v3.0.0-windows-x64.cdx.json', sha256: 'c'.repeat(64) },
    ],
  }), 'windows-x64')
  if (!authorization) throw new Error('authorization fixture failed')
  const result = await stageAuthorizedRuntimePayload(authorization, name, {
    transport: { async requestPayload() { return (async function * () { yield bytes })() } },
    signatureVerifier: { async verifyPlatformSignature() { return true } },
    staging: { async begin() { return { async write() {}, async commit() {}, async rollback() {} } } },
  })
  if (result.kind !== 'staged') throw new Error('staging fixture failed')
  return result
}

function adapterFixture(options: { confirm?: boolean; throwAt?: string; pending?: unknown; applied?: boolean } = {}) {
  const events: string[] = []
  let pending: unknown = options.pending ?? null
  let applied = options.applied ?? false
  const step = async (name: string) => { events.push(name); if (options.throwAt === name) throw new Error(name) }
  return { events, adapter: {
    async readPending() { await step('read'); return pending },
    async writePending(value: unknown) { await step('writePending'); pending = value },
    async clearPending() { await step('clear'); pending = null },
    async flush() { await step('flush') },
    async wasApplied() { await step('wasApplied'); return applied },
    async markApplied() { await step('markApplied'); applied = true },
    async handoffToPlatformInstaller() { await step('handoff'); return 'opaque-handoff' },
    async confirmPlatformSuccess() { await step('confirm'); return options.confirm ?? true },
    async rollbackStagedPayload() { await step('rollback') },
  } }
}

test('flushes pending journal before installer handoff and records confirmed success', async () => {
  const target = adapterFixture()
  assert.deepEqual(await applyStagedRuntimeUpdate(await stagedFixture(), target.adapter), { kind: 'applied' })
  assert.deepEqual(target.events, ['wasApplied', 'read', 'writePending', 'flush', 'handoff', 'confirm', 'markApplied', 'clear', 'flush'])
})

test('rejects replay and rolls back handoff confirmation and disk failures', async () => {
  const staged = await stagedFixture()
  const replay = adapterFixture({ applied: true })
  assert.deepEqual(await applyStagedRuntimeUpdate(staged, replay.adapter), { kind: 'rejected', reason: 'replay' })
  assert.doesNotMatch(replay.events.join(','), /handoff/u)
  for (const options of [{ confirm: false }, { throwAt: 'handoff' }, { throwAt: 'confirm' }, { throwAt: 'markApplied' }]) {
    const target = adapterFixture(options)
    const result = await applyStagedRuntimeUpdate(staged, target.adapter)
    assert.equal(result.kind, 'rejected')
    assert.match(target.events.join(','), /rollback,clear,flush$/u)
  }
  assert.deepEqual(await applyStagedRuntimeUpdate({ ...staged }, adapterFixture().adapter), { kind: 'rejected', reason: 'unauthorized' })
})

test('startup recovery rolls back a valid pending journal before another apply', async () => {
  const staged = await stagedFixture()
  const pending = {
    schema: 'origami2.runtime-update-pending.v1', version: staged.version, platform: staged.platform,
    assetName: staged.assetName, payloadSha256: staged.payloadSha256, byteLength: staged.byteLength,
  }
  const target = adapterFixture({ pending })
  assert.deepEqual(await recoverPendingRuntimeUpdate(target.adapter), { kind: 'rejected', reason: 'rollback' })
  assert.deepEqual(target.events, ['read', 'rollback', 'clear', 'flush'])
  const malformed = adapterFixture({ pending: { schema: 'evil' } })
  assert.deepEqual(await recoverPendingRuntimeUpdate(malformed.adapter), { kind: 'rejected', reason: 'journal' })
})
