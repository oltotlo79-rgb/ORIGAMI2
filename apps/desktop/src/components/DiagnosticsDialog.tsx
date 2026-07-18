import {
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'

import {
  prepareDiagnosticsSharePreview,
  saveDiagnosticsSharePreview,
  type DiagnosticsSharePreview,
} from '../lib/diagnosticsShare.ts'

type DiagnosticsDialogState =
  | Readonly<{ kind: 'loading'; requestId: number }>
  | Readonly<{ kind: 'load_error'; requestId: number }>
  | Readonly<{
      kind: 'ready'
      requestId: number
      preview: DiagnosticsSharePreview
      saving: boolean
      notice: string | null
      saveError: boolean
    }>

type DiagnosticsDialogProps = Readonly<{
  open: boolean
  onClose: () => void
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'textarea:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export function DiagnosticsDialog({
  open,
  onClose,
}: DiagnosticsDialogProps) {
  const [state, setState] = useState<DiagnosticsDialogState>({
    kind: 'loading',
    requestId: 0,
  })
  const requestSequenceRef = useRef(0)
  const dialogRef = useRef<HTMLElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)
  const jsonRef = useRef<HTMLTextAreaElement>(null)
  const savingRef = useRef(false)
  const saving = state.kind === 'ready' && state.saving
  savingRef.current = saving

  const loadPreview = useCallback(() => {
    const requestId = ++requestSequenceRef.current
    setState({ kind: 'loading', requestId })
    void prepareDiagnosticsSharePreview().then((preview) => {
      if (requestId !== requestSequenceRef.current) return
      setState({
        kind: 'ready',
        requestId,
        preview,
        saving: false,
        notice: null,
        saveError: false,
      })
    }).catch(() => {
      if (requestId !== requestSequenceRef.current) return
      setState({ kind: 'load_error', requestId })
    })
  }, [])

  const closeDialog = useCallback(() => {
    if (savingRef.current) return
    requestSequenceRef.current += 1
    onClose()
  }, [onClose])

  useEffect(() => {
    if (!open) {
      requestSequenceRef.current += 1
      return
    }
    loadPreview()
  }, [loadPreview, open])

  useEffect(() => {
    if (!open) return
    const frame = requestAnimationFrame(() => closeButtonRef.current?.focus())
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      closeDialog()
    }
    document.addEventListener('keydown', handleKeyDown, true)
    return () => {
      cancelAnimationFrame(frame)
      document.removeEventListener('keydown', handleKeyDown, true)
    }
  }, [closeDialog, open])

  const trapFocus = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key !== 'Tab') return
    const dialog = dialogRef.current
    if (!dialog) return
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
    ).filter((element) => !element.hasAttribute('inert'))
    if (focusable.length === 0) {
      event.preventDefault()
      dialog.focus()
      return
    }
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    const active = document.activeElement
    if (event.shiftKey && (active === first || !dialog.contains(active))) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
      event.preventDefault()
      first.focus()
    }
  }

  const selectAll = () => {
    if (state.kind !== 'ready') return
    jsonRef.current?.focus()
    jsonRef.current?.select()
    setState({
      ...state,
      notice: '内容をすべて選択しました。Ctrl/Cmd+Cでコピーできます。',
      saveError: false,
    })
  }

  const savePreview = async () => {
    if (state.kind !== 'ready' || state.saving) return
    const { requestId, preview } = state
    setState({
      ...state,
      saving: true,
      notice: null,
      saveError: false,
    })
    try {
      const result = await saveDiagnosticsSharePreview(preview)
      if (requestId !== requestSequenceRef.current) return
      setState({
        kind: 'ready',
        requestId,
        preview,
        saving: false,
        notice: result.canceled
          ? '保存をキャンセルしました。'
          : '診断JSONを保存しました。',
        saveError: false,
      })
    } catch {
      if (requestId !== requestSequenceRef.current) return
      setState({
        kind: 'ready',
        requestId,
        preview,
        saving: false,
        notice: '診断JSONを保存できませんでした。保存先を確認して、もう一度お試しください。',
        saveError: true,
      })
    }
  }

  if (!open) return null

  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        className="diagnostics-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="diagnostics-dialog-title"
        aria-describedby="diagnostics-dialog-description"
        aria-busy={state.kind === 'loading' || saving}
        tabIndex={-1}
        onKeyDown={trapFocus}
      >
        <header>
          <div>
            <span className="dialog-eyebrow">問題報告の準備</span>
            <h2 id="diagnostics-dialog-title">診断情報を確認</h2>
          </div>
          <button
            ref={closeButtonRef}
            type="button"
            className="dialog-close"
            disabled={saving}
            onClick={closeDialog}
            aria-label="閉じる"
          >
            ×
          </button>
        </header>

        <div className="diagnostics-dialog-body">
          <div
            id="diagnostics-dialog-description"
            className="diagnostics-disclosure"
          >
            <p>
              作品名、作品形状、ファイル内容、ローカルパス、ID、座標、時刻、
              アプリ版、OS、CPU、GPU情報は含みません。
            </p>
            <p>
              この情報は自動送信されません。下に表示されたJSONと保存されるJSONは同一です。
            </p>
            <p>
              保存後、内容を確認したうえで利用者自身がGitHub Issuesへ添付してください。
            </p>
          </div>

          {state.kind === 'loading' && (
            <p className="diagnostics-loading" role="status">
              診断情報を準備しています…
            </p>
          )}

          {state.kind === 'load_error' && (
            <>
              <p className="dialog-error" role="alert">
                診断情報を準備できませんでした。アプリを再起動して、もう一度お試しください。
              </p>
              <footer>
                <button type="button" onClick={loadPreview}>再試行</button>
                <button type="button" onClick={closeDialog}>閉じる</button>
              </footer>
            </>
          )}

          {state.kind === 'ready' && (
            <>
              <label className="diagnostics-json-label">
                <span>
                  共有前に確認する診断JSON（{formatBytes(state.preview.byte_length)}）
                </span>
                <textarea
                  ref={jsonRef}
                  className="diagnostics-json"
                  readOnly
                  value={state.preview.json}
                  aria-label="共有前に確認する診断JSON"
                  wrap="off"
                  spellCheck={false}
                />
              </label>
              <p
                className={state.saveError ? 'dialog-error' : 'diagnostics-notice'}
                role={state.saveError ? 'alert' : 'status'}
                aria-live={state.saveError ? 'assertive' : 'polite'}
              >
                {state.notice ?? '\u00a0'}
              </p>
              <footer>
                <button type="button" disabled={state.saving} onClick={selectAll}>
                  内容をすべて選択
                </button>
                <button
                  type="button"
                  className="primary"
                  disabled={state.saving}
                  onClick={() => void savePreview()}
                >
                  {state.saving ? '保存中…' : 'JSONファイルとして保存…'}
                </button>
                <button type="button" disabled={state.saving} onClick={closeDialog}>
                  閉じる
                </button>
              </footer>
            </>
          )}
        </div>
      </section>
    </div>
  )
}

function formatBytes(bytes: number) {
  if (bytes < 1_000) return `${bytes} B`
  return `${(bytes / 1_000).toFixed(1)} KB`
}
