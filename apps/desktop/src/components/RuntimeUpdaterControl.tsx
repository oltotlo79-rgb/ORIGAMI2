import { useEffect, useRef, useState } from 'react'

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
  | { kind: 'recovering' | 'idle' | 'checking' | 'downloading' | 'verified' | 'applying' | 'applied' | 'cancelled' }
  | { kind: 'available'; candidate: RuntimeUpdateUiCandidate }
  | { kind: 'error'; error: RuntimeUpdaterUiError }

export function RuntimeUpdaterControl({ controller }: Readonly<{ controller: RuntimeUpdaterUiController }>) {
  const [state, setState] = useState<State>({ kind: 'recovering' })
  const [candidate, setCandidate] = useState<RuntimeUpdateUiCandidate | null>(null)
  const abortRef = useRef<AbortController | null>(null)
  const operationRef = useRef(0)

  useEffect(() => {
    const operation = ++operationRef.current
    void controller.recoverPending().then((result) => {
      if (operation !== operationRef.current) return
      setState(result === 'ready' ? { kind: 'idle' } : { kind: 'error', error: result })
    }).catch(() => {
      if (operation === operationRef.current) setState({ kind: 'error', error: 'disk' })
    })
    return () => { operationRef.current += 1; abortRef.current?.abort() }
  }, [controller])

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
    <section className="runtime-updater-control" aria-labelledby="runtime-update-title" aria-busy={busy}>
      <h3 id="runtime-update-title">アプリ更新</h3>
      {candidate && <dl aria-label="更新内容">
        <dt>バージョン</dt><dd>{candidate.version}</dd>
        <dt>プラットフォーム</dt><dd>{candidate.platform}</dd>
        <dt>サイズ</dt><dd>{formatBytes(candidate.byteLength)}</dd>
        <dt>リリースノート</dt><dd>{candidate.releaseNotes}</dd>
      </dl>}
      <p role="status" aria-live="polite">{statusText(state)}</p>
      <div className="update-check-actions">
        {(state.kind === 'idle' || state.kind === 'cancelled' || state.kind === 'error') && <button type="button" onClick={() => void run('check')}>更新を確認</button>}
        {state.kind === 'available' && <button type="button" onClick={() => void run('download')}>ダウンロードして検証</button>}
        {state.kind === 'verified' && <button type="button" onClick={() => void apply()}>再起動して適用</button>}
        {(state.kind === 'checking' || state.kind === 'downloading') && <button type="button" onClick={cancel}>キャンセル</button>}
      </div>
    </section>
  )
}

function formatBytes(value: number) { return `${(value / 1024 / 1024).toFixed(1)} MB` }
function statusText(state: State) {
  const fixed: Record<Exclude<State['kind'], 'available' | 'error'>, string> = {
    recovering: '保留中の更新を確認しています', idle: '更新を手動で確認できます', checking: '更新を確認しています',
    downloading: 'ダウンロードして署名とchecksumを検証しています', verified: '検証済みです。明示的に再起動して適用できます',
    applying: '再起動と適用を準備しています', applied: '更新の適用を確認しました', cancelled: '操作をキャンセルしました',
  }
  if (state.kind === 'available') return '更新を利用できます。内容を確認してダウンロードしてください'
  if (state.kind === 'error') return `更新を安全に停止しました: ${state.error}`
  return fixed[state.kind]
}
