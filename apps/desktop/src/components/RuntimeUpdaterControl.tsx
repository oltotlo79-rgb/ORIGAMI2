import { useEffect, useId, useRef, useState } from 'react'
import { formatLocalizedText, localeStore, selectLocalizedText, useLocale, type Locale, type LocaleStore } from '../lib/i18n.ts'
import { RUNTIME_UPDATER_CONTROL_TEXT } from '../lib/runtimeUpdaterControlText.ts'

export type RuntimeUpdateUiCandidate = Readonly<{
  version: string
  releaseNotes: string
  platform: 'windows-x64' | 'macos-arm64'
  byteLength: number
}>
export type RuntimeUpdaterUiError = 'offline' | 'rollback' | 'signature' | 'disk' | 'malformed'
export type RuntimeUpdaterUiController = Readonly<{
  recoverPending: () => Promise<'ready' | RuntimeUpdaterUiError>
  check: (signal: AbortSignal) => Promise<RuntimeUpdateUiCandidate | RuntimeUpdaterUiError>
  downloadAndVerify: (candidate: RuntimeUpdateUiCandidate, signal: AbortSignal) => Promise<'verified' | RuntimeUpdaterUiError>
  restartAndApply: (candidate: RuntimeUpdateUiCandidate) => Promise<'applied' | RuntimeUpdaterUiError>
}>

type State =
  | { kind: 'disabled' | 'recovering' | 'idle' | 'checking' | 'downloading' | 'verified' | 'applying' | 'applied' | 'cancelled' }
  | { kind: 'available'; candidate: RuntimeUpdateUiCandidate }
  | { kind: 'error'; error: RuntimeUpdaterUiError }

export function RuntimeUpdaterControl({ controller, enabled = true, localeStore: localeStore_ = localeStore }: Readonly<{ controller: RuntimeUpdaterUiController; enabled?: boolean; localeStore?: LocaleStore }>) {
  const locale = useLocale(localeStore_)
  const titleId = useId()
  const text = (key: keyof typeof RUNTIME_UPDATER_CONTROL_TEXT) => selectLocalizedText(locale, RUNTIME_UPDATER_CONTROL_TEXT[key])
  const [state, setState] = useState<State>({ kind: 'recovering' })
  const [candidate, setCandidate] = useState<RuntimeUpdateUiCandidate | null>(null)
  const abortRef = useRef<AbortController | null>(null)
  const operationRef = useRef(0)

  useEffect(() => {
    if (!enabled) { operationRef.current += 1; abortRef.current?.abort(); setState({ kind: 'disabled' }); return }
    const operation = ++operationRef.current
    void controller.recoverPending().then((result) => {
      if (operation !== operationRef.current) return
      setState(result === 'ready' ? { kind: 'idle' } : { kind: 'error', error: result })
    }).catch(() => {
      if (operation === operationRef.current) setState({ kind: 'error', error: 'disk' })
    })
    return () => { operationRef.current += 1; abortRef.current?.abort() }
  }, [controller, enabled])

  const run = async (kind: 'check' | 'download') => {
    abortRef.current?.abort()
    const abort = new AbortController()
    abortRef.current = abort
    const operation = ++operationRef.current
    setState({ kind: kind === 'check' ? 'checking' : 'downloading' })
    try {
      const result = kind === 'check'
        ? await controller.check(abort.signal)
        : await controller.downloadAndVerify(candidate as RuntimeUpdateUiCandidate, abort.signal)
      if (operation !== operationRef.current || abort.signal.aborted) return
      if (typeof result === 'object') { setCandidate(result); setState({ kind: 'available', candidate: result }) }
      else if (result === 'verified') setState({ kind: 'verified' })
      else setState({ kind: 'error', error: result })
    } catch { if (operation === operationRef.current) setState({ kind: 'error', error: 'offline' }) }
  }
  const cancel = () => { operationRef.current += 1; abortRef.current?.abort(); abortRef.current = null; setState({ kind: 'cancelled' }) }
  const apply = async () => {
    if (!candidate) return
    const operation = ++operationRef.current
    setState({ kind: 'applying' })
    try {
      const result = await controller.restartAndApply(candidate)
      if (operation === operationRef.current) setState(result === 'applied' ? { kind: 'applied' } : { kind: 'error', error: result })
    } catch { if (operation === operationRef.current) setState({ kind: 'error', error: 'disk' }) }
  }
  const busy = ['recovering', 'checking', 'downloading', 'applying'].includes(state.kind)
  return (
    <section className="runtime-updater-control" aria-labelledby={titleId} aria-busy={busy}>
      <h3 id={titleId}>{text('title')}</h3>
      <p>{text('privacyDescription')}</p>
      {candidate && <dl aria-label={text('detailsAriaLabel')}>
        <dt>{text('version')}</dt><dd>{candidate.version}</dd>
        <dt>{text('platform')}</dt><dd>{candidate.platform}</dd>
        <dt>{text('size')}</dt><dd>{formatBytes(candidate.byteLength, locale)}</dd>
        <dt>{text('releaseNotes')}</dt><dd>{candidate.releaseNotes}</dd>
      </dl>}
      <p role="status" aria-live="polite">{statusText(state, locale)}</p>
      <div className="update-check-actions">
        {(state.kind === 'idle' || state.kind === 'cancelled' || state.kind === 'error') && <button type="button" onClick={() => void run('check')}>{text('checkForUpdates')}</button>}
        {state.kind === 'available' && <button type="button" onClick={() => void run('download')}>{text('downloadAndVerify')}</button>}
        {state.kind === 'verified' && <button type="button" onClick={() => void apply()}>{text('restartAndApply')}</button>}
        {(state.kind === 'checking' || state.kind === 'downloading') && <button type="button" onClick={cancel}>{text('cancel')}</button>}
      </div>
    </section>
  )
}

function formatBytes(value: number, locale: Locale) {
  return formatLocalizedText(locale, RUNTIME_UPDATER_CONTROL_TEXT.sizeMegabytes, {
    size: (value / 1024 / 1024).toFixed(1),
  })
}
function statusText(state: State, locale: Locale) {
  const fixed = {
    disabled: RUNTIME_UPDATER_CONTROL_TEXT.statusDisabled,
    recovering: RUNTIME_UPDATER_CONTROL_TEXT.statusRecovering,
    idle: RUNTIME_UPDATER_CONTROL_TEXT.statusIdle,
    checking: RUNTIME_UPDATER_CONTROL_TEXT.statusChecking,
    downloading: RUNTIME_UPDATER_CONTROL_TEXT.statusDownloading,
    verified: RUNTIME_UPDATER_CONTROL_TEXT.statusVerified,
    applying: RUNTIME_UPDATER_CONTROL_TEXT.statusApplying,
    applied: RUNTIME_UPDATER_CONTROL_TEXT.statusApplied,
    cancelled: RUNTIME_UPDATER_CONTROL_TEXT.statusCancelled,
  }
  if (state.kind === 'available') return selectLocalizedText(locale, RUNTIME_UPDATER_CONTROL_TEXT.statusAvailable)
  if (state.kind === 'error') {
    return formatLocalizedText(locale, RUNTIME_UPDATER_CONTROL_TEXT.statusError, {
      error: state.error,
    })
  }
  return selectLocalizedText(locale, fixed[state.kind])
}
