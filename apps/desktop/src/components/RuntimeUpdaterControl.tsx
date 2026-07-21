import { useEffect, useId, useRef, useState } from 'react'
import { localeStore, selectLocalizedText, useLocale, type Locale, type LocaleStore } from '../lib/i18n.ts'

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
  const text = (ja: string, en: string) => selectLocalizedText(locale, { ja, en })
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
      <h3 id={titleId}>{text('アプリ更新', 'App update')}</h3>
      <p>{text('確認ではproject dataを送信しません。payloadは明示操作後にのみ取得し、署名とchecksumを検証します。', 'Checks never send project data. Payloads are fetched only after an explicit action and are verified by signature and checksum.')}</p>
      {candidate && <dl aria-label={text('更新内容', 'Update details')}>
        <dt>{text('バージョン', 'Version')}</dt><dd>{candidate.version}</dd>
        <dt>{text('プラットフォーム', 'Platform')}</dt><dd>{candidate.platform}</dd>
        <dt>{text('サイズ', 'Size')}</dt><dd>{formatBytes(candidate.byteLength)}</dd>
        <dt>{text('リリースノート', 'Release notes')}</dt><dd>{candidate.releaseNotes}</dd>
      </dl>}
      <p role="status" aria-live="polite">{statusText(state, locale)}</p>
      <div className="update-check-actions">
        {(state.kind === 'idle' || state.kind === 'cancelled' || state.kind === 'error') && <button type="button" onClick={() => void run('check')}>{text('更新を確認', 'Check for updates')}</button>}
        {state.kind === 'available' && <button type="button" onClick={() => void run('download')}>{text('ダウンロードして検証', 'Download and verify')}</button>}
        {state.kind === 'verified' && <button type="button" onClick={() => void apply()}>{text('再起動して適用', 'Restart and apply')}</button>}
        {(state.kind === 'checking' || state.kind === 'downloading') && <button type="button" onClick={cancel}>{text('キャンセル', 'Cancel')}</button>}
      </div>
    </section>
  )
}

function formatBytes(value: number) { return `${(value / 1024 / 1024).toFixed(1)} MB` }
function statusText(state: State, locale: Locale) {
  const localized = (ja: string, en: string) => selectLocalizedText(locale, { ja, en })
  const fixed: Record<Exclude<State['kind'], 'available' | 'error'>, string> = {
    disabled: '更新確認は無効です', recovering: '保留中の更新を確認しています', idle: '更新を手動で確認できます', checking: '更新を確認しています',
    downloading: 'ダウンロードして署名とchecksumを検証しています', verified: '検証済みです。明示的に再起動して適用できます',
    applying: '再起動と適用を準備しています', applied: '更新の適用を確認しました', cancelled: '操作をキャンセルしました',
  }
  if (state.kind === 'available') return localized('更新を利用できます。内容を確認してダウンロードしてください', 'An update is available. Review it before downloading.')
  if (state.kind === 'error') return localized(`更新を安全に停止しました: ${state.error}`, `Update stopped safely: ${state.error}`)
  const english: typeof fixed = { disabled: 'Update checks are disabled', recovering: 'Checking pending update', idle: 'Check for updates manually', checking: 'Checking for updates', downloading: 'Downloading and verifying signature and checksum', verified: 'Verified. Restart explicitly to apply', applying: 'Preparing restart and apply', applied: 'Update application confirmed', cancelled: 'Operation cancelled' }
  return locale === 'ja' ? fixed[state.kind] : english[state.kind]
}
