import assert from 'node:assert/strict'
import test from 'node:test'
import { createTauriRuntimeUpdaterController } from '../src/lib/tauriRuntimeUpdaterController.ts'

test('maps bounded native command DTOs through the production controller', async () => {
  const calls: string[] = []
  const controller = createTauriRuntimeUpdaterController(async (command) => {
    calls.push(command)
    if (command.endsWith('recover_pending')) return 'ready'
    if (command.endsWith('check')) return { version: '2.0.0', platform: 'windows-x64', byteLength: 42, releaseNotes: 'notes' }
    if (command.includes('download')) return 'verified'
    return 'applied'
  })
  assert.equal(await controller.recoverPending(), 'ready')
  const candidate = await controller.check(new AbortController().signal)
  assert.deepEqual(candidate, { version: '2.0.0', platform: 'windows-x64', byteLength: 42, releaseNotes: 'notes' })
  if (typeof candidate === 'string') throw new Error('candidate rejected')
  assert.equal(await controller.downloadAndVerify(candidate, new AbortController().signal), 'verified')
  assert.equal(await controller.restartAndApply(candidate), 'applied')
  assert.deepEqual(calls, ['runtime_update_recover_pending', 'runtime_update_check', 'runtime_update_download_verify_stage', 'runtime_update_apply'])
})

test('fails closed for offline malformed stale and cancelled native responses', async () => {
  const malformed = createTauriRuntimeUpdaterController(async () => ({ version: '../bad' }))
  assert.equal(await malformed.check(new AbortController().signal), 'malformed')
  const offline = createTauriRuntimeUpdaterController(async () => 'offline')
  assert.equal(await offline.check(new AbortController().signal), 'offline')
  const signature = createTauriRuntimeUpdaterController(async () => {
    throw new Error('signature')
  })
  assert.equal(
    await signature.downloadAndVerify(
      { version: '2.0.0', platform: 'windows-x64', byteLength: 42, releaseNotes: '' },
      new AbortController().signal,
    ),
    'signature',
  )
  let resolve!: (value: unknown) => void
  const calls: string[] = []
  const stale = createTauriRuntimeUpdaterController((command) => {
    calls.push(command)
    if (command === 'runtime_update_cancel') return Promise.resolve(null)
    return new Promise((done) => { resolve = done })
  })
  const abort = new AbortController()
  const pending = stale.check(abort.signal)
  abort.abort()
  resolve({ version: '2.0.0', platform: 'windows-x64', byteLength: 42, releaseNotes: '' })
  await assert.rejects(pending, /stale/u)
  assert.equal(calls.includes('runtime_update_cancel'), true)
})
