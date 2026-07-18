import {
  type KeyboardEvent as ReactKeyboardEvent,
  useEffect,
  useRef,
  useState,
} from 'react'

import {
  formatInstructionExportBytes,
  INSTRUCTION_EXPORT_FORMATS,
  instructionExportFormatLabel,
  instructionExportPhaseLabel,
  isInstructionExportFormat,
  type InstructionExportFormat,
  type InstructionExportPhase,
  type InstructionExportPreview,
} from '../lib/instructionExport.ts'

type InstructionExportDialogProps = Readonly<{
  format: InstructionExportFormat
  preview: InstructionExportPreview | null
  busy: boolean
  generationActive: boolean
  phase: InstructionExportPhase
  error: string | null
  notice: string | null
  onFormatChange: (format: InstructionExportFormat) => void
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

export function InstructionExportDialog({
  format,
  preview,
  busy,
  generationActive,
  phase,
  error,
  notice,
  onFormatChange,
  onRetry,
  onSave,
  onCancel,
}: InstructionExportDialogProps) {
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
        if (generationActive) {
          closeRef.current?.focus()
        } else {
          dialogRef.current?.focus()
        }
      } else if (preview) {
        formatRef.current?.focus()
      } else {
        closeRef.current?.focus()
      }
    })
    return () => cancelAnimationFrame(frame)
  }, [busy, generationActive, preview])

  useEffect(() => {
    const handleFocusIn = (event: FocusEvent) => {
      const dialog = dialogRef.current
      const target = event.target
      if (!dialog || !(target instanceof Node) || dialog.contains(target)) return
      if (busy) {
        if (generationActive) {
          closeRef.current?.focus()
        } else {
          dialog.focus()
        }
      } else if (preview) {
        formatRef.current?.focus()
      } else {
        closeRef.current?.focus()
      }
    }
    document.addEventListener('focusin', handleFocusIn, true)
    return () => document.removeEventListener('focusin', handleFocusIn, true)
  }, [busy, generationActive, preview])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== 'Escape'
        || event.isComposing
        || (busy && !generationActive)
      ) return
      event.preventDefault()
      event.stopPropagation()
      onCancel()
    }
    document.addEventListener('keydown', handleKeyDown, true)
    return () => document.removeEventListener('keydown', handleKeyDown, true)
  }, [busy, generationActive, onCancel])

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
        aria-labelledby="instruction-export-title"
        aria-describedby="instruction-export-description"
        aria-busy={busy}
        tabIndex={-1}
        onKeyDown={trapFocus}
      >
        <header>
          <div>
            <span className="dialog-eyebrow">折り図の書き出し</span>
            <h2 id="instruction-export-title">形式と出力内容を確認</h2>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="dialog-close"
            disabled={busy && !generationActive}
            onClick={onCancel}
            aria-label="閉じる"
          >
            ×
          </button>
        </header>

        <div className="crease-export-dialog-body">
          <p id="instruction-export-description" className="dialog-note">
            現在の編集リビジョンから折り図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。
          </p>

          <label className="crease-export-format">
            <span>出力形式</span>
            <select
              ref={formatRef}
              value={format}
              disabled={busy}
              onChange={(event) => {
                const next = event.currentTarget.value
                if (isInstructionExportFormat(next)) onFormatChange(next)
              }}
            >
              {INSTRUCTION_EXPORT_FORMATS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label} — {option.detail}
                </option>
              ))}
            </select>
          </label>

          {busy && !preview && (
            <p className="crease-export-loading" role="status">
              {instructionExportFormatLabel(format)}: {instructionExportPhaseLabel(phase)}…
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
                  <dd>{instructionExportFormatLabel(preview.format)}</dd>
                </div>
                <div>
                  <dt>出力仕様</dt>
                  <dd>{preview.format_summary}</dd>
                </div>
                <div>
                  <dt>出力プロファイル</dt>
                  <dd>{preview.profile}</dd>
                </div>
                <div>
                  <dt>投影プロファイル</dt>
                  <dd>{preview.projection_profile}</dd>
                </div>
                <div>
                  <dt>保存名候補</dt>
                  <dd>{preview.suggested_file_name}</dd>
                </div>
                <div>
                  <dt>サイズ</dt>
                  <dd>{formatInstructionExportBytes(preview.byte_count)}</dd>
                </div>
                <div>
                  <dt>折り手順</dt>
                  <dd>{preview.step_count.toLocaleString('ja-JP')}手順</dd>
                </div>
                <div>
                  <dt>ページ</dt>
                  <dd>{preview.page_count.toLocaleString('ja-JP')}ページ</dd>
                </div>
                <div>
                  <dt>注意事項</dt>
                  <dd>{preview.caution_count.toLocaleString('ja-JP')}件</dd>
                </div>
                <div>
                  <dt>固定元</dt>
                  <dd>revision {preview.expected_revision.toLocaleString('ja-JP')}</dd>
                </div>
              </dl>

              <section
                className="crease-export-warnings"
                aria-labelledby="instruction-export-warnings-title"
              >
                <h3 id="instruction-export-warnings-title">出力前の確認事項</h3>
                {preview.warnings.length > 0 ? (
                  <>
                    <ul>
                      {preview.warnings.map((warning) => (
                        <li key={warning.category}>{warning.message_ja}</li>
                      ))}
                    </ul>
                    <label>
                      <input
                        type="checkbox"
                        checked={warningsAcknowledged}
                        disabled={busy}
                        onChange={(event) => setWarningsAcknowledged(event.currentTarget.checked)}
                      />
                      上記の注意事項を確認しました
                    </label>
                  </>
                ) : (
                  <p>この折り図について追加確認が必要な注意事項はありません。</p>
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
          <button
            type="button"
            disabled={busy && !generationActive}
            onClick={onCancel}
          >
            {generationActive ? '生成を中止' : 'キャンセル'}
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
