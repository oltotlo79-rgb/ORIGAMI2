import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useRef,
  useState,
} from 'react'

import {
  CREASE_PATTERN_EXPORT_FORMATS,
  creasePatternExportAssignmentRows,
  creasePatternExportFormatLabel,
  formatCreasePatternExportBytes,
  isCreasePatternExportFormat,
  type CreasePatternExportFormat,
  type CreasePatternExportPreview,
} from '../lib/creaseExport.ts'

type CreaseExportDialogProps = Readonly<{
  format: CreasePatternExportFormat
  preview: CreasePatternExportPreview | null
  busy: boolean
  error: string | null
  notice: string | null
  onFormatChange: (format: CreasePatternExportFormat) => void
  onRetry: () => void
  onSave: (warningsAcknowledged: boolean) => void
  onCancel: () => void
}>

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export function CreaseExportDialog({
  format,
  preview,
  busy,
  error,
  notice,
  onFormatChange,
  onRetry,
  onSave,
  onCancel,
}: CreaseExportDialogProps) {
  const [warningsAcknowledged, setWarningsAcknowledged] = useState(false)
  const dialogRef = useRef<HTMLElement>(null)
  const formatRef = useRef<HTMLSelectElement>(null)
  const closeRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    setWarningsAcknowledged(preview?.warnings.length === 0)
  }, [preview])

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      if (busy) {
        dialogRef.current?.focus()
      } else if (preview) {
        formatRef.current?.focus()
      } else {
        closeRef.current?.focus()
      }
    })
    return () => cancelAnimationFrame(frame)
  }, [busy, preview])

  useEffect(() => {
    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current
      const target = event.target
      if (!dialog || !(target instanceof Node) || dialog.contains(target)) return
      if (busy) {
        dialog.focus()
      } else if (preview) {
        formatRef.current?.focus()
      } else {
        closeRef.current?.focus()
      }
    }
    document.addEventListener('focusin', handleFocusIn, true)
    return () => document.removeEventListener('focusin', handleFocusIn, true)
  }, [busy, preview])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.isComposing || busy) return
      event.preventDefault()
      event.stopPropagation()
      onCancel()
    }
    document.addEventListener('keydown', handleKeyDown, true)
    return () => document.removeEventListener('keydown', handleKeyDown, true)
  }, [busy, onCancel])

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

  const warningsConfirmed = preview !== null
    && (preview.warnings.length === 0 || warningsAcknowledged)
  const canSave = Boolean(preview) && !busy && warningsConfirmed

  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        className="crease-export-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="crease-export-title"
        aria-describedby="crease-export-description"
        aria-busy={busy}
        tabIndex={-1}
        onKeyDown={trapFocus}
      >
        <header>
          <div>
            <span className="dialog-eyebrow">展開図の書き出し</span>
            <h2 id="crease-export-title">形式と情報損失を確認</h2>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="dialog-close"
            disabled={busy}
            onClick={onCancel}
            aria-label="閉じる"
          >
            ×
          </button>
        </header>

        <div className="crease-export-dialog-body">
          <p id="crease-export-description" className="dialog-note">
            現在の編集リビジョンから展開図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。
          </p>

          <label className="crease-export-format">
            <span>出力形式</span>
            <select
              ref={formatRef}
              value={format}
              disabled={busy}
              onChange={(event) => {
                const next = event.currentTarget.value
                if (isCreasePatternExportFormat(next)) onFormatChange(next)
              }}
            >
              {CREASE_PATTERN_EXPORT_FORMATS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label} — {option.detail}
                </option>
              ))}
            </select>
          </label>

          {busy && !preview && (
            <p className="crease-export-loading" role="status">
              {creasePatternExportFormatLabel(format)}データを検証・生成しています…
            </p>
          )}

          {error && (
            <div className="crease-export-error">
              <p className="dialog-error" role="alert">{error}</p>
              {!busy && (
                <button type="button" onClick={onRetry}>
                  {preview ? '現在の編集内容から作り直す' : '同じ形式で再試行'}
                </button>
              )}
            </div>
          )}

          {preview && (
            <>
              <dl className="crease-export-metadata">
                <div>
                  <dt>形式</dt>
                  <dd>{creasePatternExportFormatLabel(preview.format)}</dd>
                </div>
                <div>
                  <dt>出力仕様</dt>
                  <dd>{preview.format_summary}</dd>
                </div>
                <div>
                  <dt>保存名候補</dt>
                  <dd>{preview.suggested_file_name}</dd>
                </div>
                <div>
                  <dt>サイズ</dt>
                  <dd>{formatCreasePatternExportBytes(preview.byte_count)}</dd>
                </div>
                <div>
                  <dt>形状</dt>
                  <dd>
                    {preview.vertex_count.toLocaleString('ja-JP')}頂点・
                    {preview.edge_count.toLocaleString('ja-JP')}辺
                  </dd>
                </div>
                <div>
                  <dt>固定元</dt>
                  <dd>revision {preview.expected_revision.toLocaleString('ja-JP')}</dd>
                </div>
                <div>
                  <dt>切断線</dt>
                  <dd>{preview.has_cuts ? '含む' : '含まない'}</dd>
                </div>
              </dl>

              <section className="crease-export-assignments" aria-labelledby="crease-export-lines">
                <h3 id="crease-export-lines">書き出す線</h3>
                <ul>
                  {creasePatternExportAssignmentRows(preview.assignment_counts).map((row) => (
                    <li key={row.key}>
                      <span>{row.label}</span>
                      <strong>{row.count.toLocaleString('ja-JP')}本</strong>
                    </li>
                  ))}
                </ul>
              </section>

              <section
                className="crease-export-warnings"
                aria-labelledby="crease-export-loss-title"
              >
                <h3 id="crease-export-loss-title">この形式に含まれない情報</h3>
                {preview.warnings.length > 0 ? (
                  <>
                    <ul>
                      {preview.warnings.map((warning, index) => (
                        <li key={`${index}:${warning}`}>{warning}</li>
                      ))}
                    </ul>
                    <label>
                      <input
                        type="checkbox"
                        checked={warningsAcknowledged}
                        disabled={busy}
                        onChange={(event) => setWarningsAcknowledged(event.currentTarget.checked)}
                      />
                      上記の情報が出力に含まれないことを確認しました
                    </label>
                  </>
                ) : (
                  <p>現在の展開図について確認が必要な情報損失はありません。</p>
                )}
              </section>
            </>
          )}

          <p
            className="crease-export-notice"
            role="status"
            aria-live="polite"
          >
            {notice ?? '\u00a0'}
          </p>
        </div>

        <footer>
          <button type="button" disabled={busy} onClick={onCancel}>
            キャンセル
          </button>
          <button
            type="button"
            className="primary"
            disabled={!canSave}
            onClick={() => onSave(warningsAcknowledged)}
          >
            {busy ? '処理中…' : '保存先を選んで書き出す…'}
          </button>
        </footer>
      </section>
    </div>
  )
}
