import { invoke } from '@tauri-apps/api/core'
import type {
  RuntimeUpdateUiCandidate,
  RuntimeUpdaterUiController,
  RuntimeUpdaterUiError,
} from '../components/RuntimeUpdaterControl.tsx'

const ERRORS = new Set<RuntimeUpdaterUiError>(['offline', 'rollback', 'signature', 'disk', 'malformed'])

export function createTauriRuntimeUpdaterController(
  invokeCommand: typeof invoke = invoke,
): RuntimeUpdaterUiController {
  let generation = 0
  const call = async (command: string, payload?: Record<string, unknown>) => {
    const requestGeneration = ++generation
    let value: unknown
    try {
      value = await invokeCommand<unknown>(command, payload)
    } catch (error) {
      if (isError(error)) value = error
      else if (error instanceof Error && isError(error.message)) {
        value = error.message
      } else throw error
    }
    if (requestGeneration !== generation) throw new Error('stale updater response')
    return value
  }
  const controller: RuntimeUpdaterUiController = {
    async recoverPending() {
      return parseStatus(await call('runtime_update_recover_pending'), ['ready'])
    },
    async check(signal) {
      const token = crypto.randomUUID()
      const cancel = () => { generation += 1; void invokeCommand('runtime_update_cancel', { token }).catch(() => undefined) }
      signal.addEventListener('abort', cancel, { once: true })
      try { return parseCandidate(await call('runtime_update_check', { token })) }
      finally { signal.removeEventListener('abort', cancel) }
    },
    async downloadAndVerify(candidate, signal) {
      const token = crypto.randomUUID()
      const cancel = () => { generation += 1; void invokeCommand('runtime_update_cancel', { token }).catch(() => undefined) }
      signal.addEventListener('abort', cancel, { once: true })
      try { return parseStatus(await call('runtime_update_download_verify_stage', { token, version: candidate.version, platform: candidate.platform }), ['verified']) }
      finally { signal.removeEventListener('abort', cancel) }
    },
    async restartAndApply(candidate) {
      return parseStatus(await call('runtime_update_apply', { version: candidate.version, platform: candidate.platform }), ['applied'])
    },
  }
  return Object.freeze(controller)
}

function parseCandidate(value: unknown): RuntimeUpdateUiCandidate | RuntimeUpdaterUiError {
  if (isError(value)) return value
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return 'malformed'
  const record = value as Record<string, unknown>
  if (Object.keys(record).sort().join(',') !== 'byteLength,platform,releaseNotes,version'
    || typeof record.version !== 'string' || !/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/u.test(record.version)
    || !['windows-x64', 'macos-arm64'].includes(String(record.platform))
    || typeof record.releaseNotes !== 'string' || record.releaseNotes.length > 100_000
    || !Number.isSafeInteger(record.byteLength) || Number(record.byteLength) <= 0 || Number(record.byteLength) > 1024 * 1024 * 1024) return 'malformed'
  return Object.freeze(record) as RuntimeUpdateUiCandidate
}

function parseStatus<const Success extends 'ready' | 'verified' | 'applied'>(
  value: unknown,
  successes: readonly Success[],
): Success | RuntimeUpdaterUiError {
  if (typeof value === 'string' && successes.includes(value as Success)) return value as Success
  return isError(value) ? value : 'malformed'
}
function isError(value: unknown): value is RuntimeUpdaterUiError {
  return typeof value === 'string' && ERRORS.has(value as RuntimeUpdaterUiError)
}

export const tauriRuntimeUpdaterController = createTauriRuntimeUpdaterController()
