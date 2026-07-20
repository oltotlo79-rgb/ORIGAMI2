import {
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'

import {
  prepareDiagnosticsSharePreview,
  saveDiagnosticsSharePreview,
  type DiagnosticsSharePreview,
} from '../lib/diagnosticsShare.ts'
import { useLocale } from '../lib/i18n.ts'
import { parseProofScopeDiagnosticsJson } from '../lib/proofScopePresentation.ts'

type DiagnosticsNotice =
  | 'selected'
  | 'save_canceled'
  | 'saved'
  | 'save_failed'

type DiagnosticsDialogState =
  | Readonly<{ kind: 'loading'; requestId: number }>
  | Readonly<{ kind: 'load_error'; requestId: number }>
  | Readonly<{
      kind: 'ready'
      requestId: number
      preview: DiagnosticsSharePreview
      saving: boolean
      notice: DiagnosticsNotice | null
    }>

type DiagnosticsDialogProps = Readonly<{
  open: boolean
  onClose: () => void
  proofScopeDiagnosticsJson?: string | null
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'textarea:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

const DIAGNOSTICS_COPY = {
  ja: {
    eyebrow: '問題報告の準備',
    title: '診断情報を確認',
    close: '閉じる',
    disclosure: [
      '作品名、作品形状、ファイル内容、ローカルパス、ID、座標、時刻、アプリ版、OS、CPU、GPU情報は含みません。',
      'この情報は自動送信されません。下に表示されたJSONと保存されるJSONは同一です。',
      '保存後、内容を確認したうえで利用者自身がGitHub Issuesへ添付してください。',
    ],
    loading: '診断情報を準備しています…',
    loadError:
      '診断情報を準備できませんでした。アプリを再起動して、もう一度お試しください。',
    retry: '再試行',
    jsonLabel: '共有前に確認する診断JSON',
    proofScopeLabel: '証明範囲JSON（手動コピー専用）',
    proofScopeDisclosure: '全体判定certificateと局所summaryのmodel・version・件数・理由だけを含みます。座標、作品ID、UUID、時刻は含みません。',
    selectProofScope: '証明範囲JSONをすべて選択',
    selectAll: '内容をすべて選択',
    saving: '保存中…',
    save: 'JSONファイルとして保存…',
    notices: {
      selected: '内容をすべて選択しました。Ctrl/Cmd+Cでコピーできます。',
      save_canceled: '保存をキャンセルしました。',
      saved: '診断JSONを保存しました。',
      save_failed:
        '診断JSONを保存できませんでした。保存先を確認して、もう一度お試しください。',
    },
  },
  en: {
    eyebrow: 'Prepare a problem report',
    title: 'Review diagnostics',
    close: 'Close',
    disclosure: [
      'The report does not include the work name, work geometry, file contents, local paths, IDs, coordinates, timestamps, app version, OS, CPU, or GPU information.',
      'This information is never sent automatically. The JSON shown below is identical to the JSON that will be saved.',
      'After saving and reviewing it, attach the file to GitHub Issues yourself.',
    ],
    loading: 'Preparing diagnostics…',
    loadError:
      'Diagnostics could not be prepared. Restart the app and try again.',
    retry: 'Retry',
    jsonLabel: 'Diagnostics JSON to review before sharing',
    proofScopeLabel: 'Proof coverage JSON (manual copy only)',
    proofScopeDisclosure: 'Contains only certificate models, versions, counts, and allowlisted reasons. It excludes coordinates, project IDs, UUIDs, and timestamps.',
    selectProofScope: 'Select all proof coverage JSON',
    selectAll: 'Select all contents',
    saving: 'Saving…',
    save: 'Save as JSON file…',
    notices: {
      selected: 'All contents are selected. Press Ctrl/Cmd+C to copy.',
      save_canceled: 'Save was canceled.',
      saved: 'Diagnostics JSON was saved.',
      save_failed:
        'Diagnostics JSON could not be saved. Check the destination and try again.',
    },
  },
} as const

export function DiagnosticsDialog({
  open,
  onClose,
  proofScopeDiagnosticsJson = null,
}: DiagnosticsDialogProps) {
  const locale = useLocale()
  const copy = DIAGNOSTICS_COPY[locale]
  const [state, setState] = useState<DiagnosticsDialogState>({
    kind: 'loading',
    requestId: 0,
  })
  const requestSequenceRef = useRef(0)
  const dialogRef = useRef<HTMLElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)
  const jsonRef = useRef<HTMLTextAreaElement>(null)
  const proofScopeJsonRef = useRef<HTMLTextAreaElement>(null)
  const safeProofScopeJson = useMemo(
    () => parseProofScopeDiagnosticsJson(proofScopeDiagnosticsJson),
    [proofScopeDiagnosticsJson],
  )
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
      notice: 'selected',
    })
  }

  const selectProofScope = () => {
    if (state.kind !== 'ready' || safeProofScopeJson === null) return
    proofScopeJsonRef.current?.focus()
    proofScopeJsonRef.current?.select()
    setState({ ...state, notice: 'selected' })
  }

  const savePreview = async () => {
    if (state.kind !== 'ready' || state.saving || savingRef.current) return
    savingRef.current = true
    const { requestId, preview } = state
    setState({
      ...state,
      saving: true,
      notice: null,
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
          ? 'save_canceled'
          : 'saved',
      })
    } catch {
      if (requestId !== requestSequenceRef.current) return
      setState({
        kind: 'ready',
        requestId,
        preview,
        saving: false,
        notice: 'save_failed',
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
            <span className="dialog-eyebrow">{copy.eyebrow}</span>
            <h2 id="diagnostics-dialog-title">{copy.title}</h2>
          </div>
          <button
            ref={closeButtonRef}
            type="button"
            className="dialog-close"
            disabled={saving}
            onClick={closeDialog}
            aria-label={copy.close}
          >
            ×
          </button>
        </header>

        <div className="diagnostics-dialog-body">
          <div
            id="diagnostics-dialog-description"
            className="diagnostics-disclosure"
          >
            {copy.disclosure.map((paragraph) => (
              <p key={paragraph}>{paragraph}</p>
            ))}
          </div>

          {state.kind === 'loading' && (
            <p className="diagnostics-loading" role="status">
              {copy.loading}
            </p>
          )}

          {state.kind === 'load_error' && (
            <>
              <p className="dialog-error" role="alert">
                {copy.loadError}
              </p>
              <footer>
                <button type="button" onClick={loadPreview}>{copy.retry}</button>
                <button type="button" onClick={closeDialog}>{copy.close}</button>
              </footer>
            </>
          )}

          {state.kind === 'ready' && (
            <>
              <label className="diagnostics-json-label">
                <span>
                  {copy.jsonLabel} ({formatBytes(state.preview.byte_length)})
                </span>
                <textarea
                  ref={jsonRef}
                  className="diagnostics-json"
                  readOnly
                  value={state.preview.json}
                  aria-label={copy.jsonLabel}
                  wrap="off"
                  spellCheck={false}
                />
              </label>
              {safeProofScopeJson !== null && (
                <section className="diagnostics-proof-scope">
                  <p>{copy.proofScopeDisclosure}</p>
                  <label className="diagnostics-json-label">
                    <span>{copy.proofScopeLabel}</span>
                    <textarea
                      ref={proofScopeJsonRef}
                      className="diagnostics-json"
                      readOnly
                      value={safeProofScopeJson}
                      aria-label={copy.proofScopeLabel}
                      wrap="off"
                      spellCheck={false}
                    />
                  </label>
                  <button
                    type="button"
                    disabled={state.saving}
                    onClick={selectProofScope}
                  >
                    {copy.selectProofScope}
                  </button>
                </section>
              )}
              <p
                className={state.notice === 'save_failed'
                  ? 'dialog-error'
                  : 'diagnostics-notice'}
                role={state.notice === 'save_failed' ? 'alert' : 'status'}
                aria-live={state.notice === 'save_failed' ? 'assertive' : 'polite'}
              >
                {state.notice === null
                  ? '\u00a0'
                  : copy.notices[state.notice]}
              </p>
              <footer>
                <button type="button" disabled={state.saving} onClick={selectAll}>
                  {copy.selectAll}
                </button>
                <button
                  type="button"
                  className="primary"
                  disabled={state.saving}
                  onClick={() => void savePreview()}
                >
                  {state.saving ? copy.saving : copy.save}
                </button>
                <button type="button" disabled={state.saving} onClick={closeDialog}>
                  {copy.close}
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
