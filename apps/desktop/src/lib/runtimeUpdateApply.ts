import {
  isIssuedRuntimeStagedPayload,
  type RuntimeStagedPayload,
} from './runtimeUpdatePayload.ts'

export type RuntimeUpdatePendingJournal = Readonly<{
  schema: 'origami2.runtime-update-pending.v1'
  version: string
  platform: RuntimeStagedPayload['platform']
  assetName: string
  payloadSha256: string
  byteLength: number
}>
export type RuntimeUpdateApplyAdapter = Readonly<{
  readPending: () => Promise<unknown>
  writePending: (journal: RuntimeUpdatePendingJournal) => Promise<void>
  clearPending: () => Promise<void>
  flush: () => Promise<void>
  wasApplied: (payloadSha256: string) => Promise<boolean>
  markApplied: (payloadSha256: string) => Promise<void>
  handoffToPlatformInstaller: (assetName: string) => Promise<unknown>
  confirmPlatformSuccess: (handoff: unknown) => Promise<boolean>
  rollbackStagedPayload: (journal: RuntimeUpdatePendingJournal) => Promise<void>
}>
export type RuntimeUpdateApplyResult = Readonly<{
  kind: 'applied' | 'rejected'
  reason?: 'unauthorized' | 'replay' | 'journal' | 'handoff' | 'confirmation' | 'rollback'
}>

export async function applyStagedRuntimeUpdate(
  staged: RuntimeStagedPayload,
  adapter: RuntimeUpdateApplyAdapter,
): Promise<RuntimeUpdateApplyResult> {
  if (!isIssuedRuntimeStagedPayload(staged)) return rejected('unauthorized')
  try {
    if (await adapter.wasApplied(staged.payloadSha256)) return rejected('replay')
  } catch { return rejected('journal') }
  const journal = journalFrom(staged)
  try {
    if (await adapter.readPending() !== null) return rejected('journal')
    await adapter.writePending(journal)
    await adapter.flush()
  } catch { return rejected('journal') }
  try {
    let handoff: unknown
    try { handoff = await adapter.handoffToPlatformInstaller(staged.assetName) } catch {
      return await rollback(adapter, journal, 'handoff')
    }
    let confirmed = false
    try { confirmed = await adapter.confirmPlatformSuccess(handoff) } catch {
      return await rollback(adapter, journal, 'confirmation')
    }
    if (!confirmed) return await rollback(adapter, journal, 'confirmation')
    try {
      await adapter.markApplied(staged.payloadSha256)
      await adapter.clearPending()
      await adapter.flush()
    } catch { return await rollback(adapter, journal, 'journal') }
    return Object.freeze({ kind: 'applied' })
  } catch { return await rollback(adapter, journal, 'rollback') }
}

/** Called at startup before accepting another update or installer handoff. */
export async function recoverPendingRuntimeUpdate(
  adapter: RuntimeUpdateApplyAdapter,
): Promise<RuntimeUpdateApplyResult> {
  let value: unknown
  try { value = await adapter.readPending() } catch { return rejected('journal') }
  if (value === null) return Object.freeze({ kind: 'applied' })
  const journal = parseJournal(value)
  if (!journal) return rejected('journal')
  return rollback(adapter, journal, 'rollback')
}

function journalFrom(staged: RuntimeStagedPayload): RuntimeUpdatePendingJournal {
  return Object.freeze({
    schema: 'origami2.runtime-update-pending.v1', version: staged.version,
    platform: staged.platform, assetName: staged.assetName,
    payloadSha256: staged.payloadSha256, byteLength: staged.byteLength,
  })
}

function parseJournal(value: unknown): RuntimeUpdatePendingJournal | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
  const record = value as Record<string, unknown>
  if (Object.keys(record).sort().join(',') !== 'assetName,byteLength,payloadSha256,platform,schema,version'
    || record.schema !== 'origami2.runtime-update-pending.v1'
    || typeof record.version !== 'string'
    || !/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(record.version)
    || !['windows-x64', 'macos-arm64'].includes(String(record.platform))
    || typeof record.assetName !== 'string' || record.assetName.includes('/') || record.assetName.includes('\\')
    || typeof record.payloadSha256 !== 'string' || !/^[0-9a-f]{64}$/u.test(record.payloadSha256)
    || !Number.isSafeInteger(record.byteLength) || Number(record.byteLength) <= 0) return null
  return Object.freeze(record) as RuntimeUpdatePendingJournal
}

async function rollback(
  adapter: RuntimeUpdateApplyAdapter,
  journal: RuntimeUpdatePendingJournal,
  reason: Exclude<RuntimeUpdateApplyResult['reason'], undefined | 'unauthorized' | 'replay'>,
): Promise<RuntimeUpdateApplyResult> {
  try {
    await adapter.rollbackStagedPayload(journal)
    await adapter.clearPending()
    await adapter.flush()
    return rejected(reason)
  } catch { return rejected('rollback') }
}

function rejected(reason: Exclude<RuntimeUpdateApplyResult['reason'], undefined>): RuntimeUpdateApplyResult {
  return Object.freeze({ kind: 'rejected', reason })
}
