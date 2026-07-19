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
import { useLocale } from '../lib/i18n.ts'

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

const CREASE_EXPORT_COPY = {
  ja: {
    eyebrow: '展開図の書き出し',
    title: '形式と情報損失を確認',
    close: '閉じる',
    description:
      '現在の編集リビジョンから展開図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。',
    format: '出力形式',
    optionDetails: {
      fold: '他の折り紙ソフトと交換しやすいJSON形式',
      svg: '印刷・作図ソフトで扱いやすい静的な線図',
      pdf: '実寸1:1・四辺10 mm余白の白黒ベクター印刷',
      dxf: 'AutoCAD 2007・mm・5意味レイヤーのCAD交換',
    },
    generating: 'データを検証・生成しています…',
    rebuild: '現在の編集内容から作り直す',
    retry: '同じ形式で再試行',
    metadata: {
      format: '形式',
      specification: '出力仕様',
      suggestedName: '保存名候補',
      size: 'サイズ',
      geometry: '形状',
      revision: '固定元',
      cuts: '切断線',
    },
    vertices: '頂点',
    edges: '辺',
    includes: '含む',
    excludes: '含まない',
    lines: '書き出す線',
    lineUnit: '本',
    assignmentLabels: {
      boundary: '外周',
      mountain: '山折り',
      valley: '谷折り',
      auxiliary: '補助線',
      cut: '切断線',
    },
    lossTitle: 'この形式に含まれない情報',
    acknowledge: '上記の情報が出力に含まれないことを確認しました',
    lossless: '現在の展開図について確認が必要な情報損失はありません。',
    cancel: 'キャンセル',
    processing: '処理中…',
    save: '保存先を選んで書き出す…',
    formatSummaries: {
      fold: 'FOLD 1.2・2D creasePattern・座標単位mm',
      svg: '静的直線SVG・1 SVG unit = 1 mm',
      pdf: '実寸1:1ベクター・図面範囲＋四辺10 mm余白',
      dxf: 'AC1021 text-form・UTF-8・mm・5意味レイヤー',
    },
  },
  en: {
    eyebrow: 'Export crease pattern',
    title: 'Review format and information loss',
    close: 'Close',
    description:
      'Generate a crease pattern from the current edit revision. Exporting does not change the project save state or history.',
    format: 'Export format',
    optionDetails: {
      fold: 'JSON for exchanging data with other origami software',
      svg: 'Static line art for printing and drawing software',
      pdf: 'Full-size 1:1 monochrome vector print with 10 mm margins',
      dxf: 'CAD exchange using AutoCAD 2007, mm, and five semantic layers',
    },
    generating: ' data is being validated and generated…',
    rebuild: 'Rebuild from the current edits',
    retry: 'Retry the same format',
    metadata: {
      format: 'Format',
      specification: 'Specification',
      suggestedName: 'Suggested file name',
      size: 'Size',
      geometry: 'Geometry',
      revision: 'Source',
      cuts: 'Cut lines',
    },
    vertices: 'vertices',
    edges: 'edges',
    includes: 'Included',
    excludes: 'Not included',
    lines: 'Lines to export',
    lineUnit: 'lines',
    assignmentLabels: {
      boundary: 'Boundary',
      mountain: 'Mountain folds',
      valley: 'Valley folds',
      auxiliary: 'Auxiliary lines',
      cut: 'Cut lines',
    },
    lossTitle: 'Information not included in this format',
    acknowledge: 'I understand that the information above is not included',
    lossless: 'No information loss requires confirmation for this crease pattern.',
    cancel: 'Cancel',
    processing: 'Processing…',
    save: 'Choose destination and export…',
    formatSummaries: {
      fold: 'FOLD 1.2 · 2D creasePattern · coordinates in mm',
      svg: 'Static line SVG · 1 SVG unit = 1 mm',
      pdf: 'Full-size 1:1 vector · drawing bounds + 10 mm margins',
      dxf: 'AC1021 text form · UTF-8 · mm · 5 semantic layers',
    },
  },
} as const

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
  const locale = useLocale()
  const copy = CREASE_EXPORT_COPY[locale]
  const numberLocale = locale === 'ja' ? 'ja-JP' : 'en-US'
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
            <span className="dialog-eyebrow">{copy.eyebrow}</span>
            <h2 id="crease-export-title">{copy.title}</h2>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="dialog-close"
            disabled={busy}
            onClick={onCancel}
            aria-label={copy.close}
          >
            ×
          </button>
        </header>

        <div className="crease-export-dialog-body">
          <p id="crease-export-description" className="dialog-note">
            {copy.description}
          </p>

          <label className="crease-export-format">
            <span>{copy.format}</span>
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
                  {option.label} — {copy.optionDetails[option.value]}
                </option>
              ))}
            </select>
          </label>

          {busy && !preview && (
            <p className="crease-export-loading" role="status">
              {creasePatternExportFormatLabel(format)}{copy.generating}
            </p>
          )}

          {error && (
            <div className="crease-export-error">
              <p className="dialog-error" role="alert">{error}</p>
              {!busy && (
                <button type="button" onClick={onRetry}>
                  {preview ? copy.rebuild : copy.retry}
                </button>
              )}
            </div>
          )}

          {preview && (
            <>
              <dl className="crease-export-metadata">
                <div>
                  <dt>{copy.metadata.format}</dt>
                  <dd>{creasePatternExportFormatLabel(preview.format)}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.specification}</dt>
                  <dd>{locale === 'ja'
                    ? preview.format_summary
                    : copy.formatSummaries[preview.format]}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.suggestedName}</dt>
                  <dd>{preview.suggested_file_name}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.size}</dt>
                  <dd>{formatCreasePatternExportBytes(preview.byte_count)}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.geometry}</dt>
                  <dd>
                    {preview.vertex_count.toLocaleString(numberLocale)}
                    {locale === 'ja' ? `${copy.vertices}・` : ` ${copy.vertices} · `}
                    {preview.edge_count.toLocaleString(numberLocale)}
                    {locale === 'ja' ? copy.edges : ` ${copy.edges}`}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.revision}</dt>
                  <dd>revision {preview.expected_revision.toLocaleString(numberLocale)}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.cuts}</dt>
                  <dd>{preview.has_cuts ? copy.includes : copy.excludes}</dd>
                </div>
              </dl>

              <section className="crease-export-assignments" aria-labelledby="crease-export-lines">
                <h3 id="crease-export-lines">{copy.lines}</h3>
                <ul>
                  {creasePatternExportAssignmentRows(preview.assignment_counts).map((row) => (
                    <li key={row.key}>
                      <span>{copy.assignmentLabels[row.key]}</span>
                      <strong>
                        {row.count.toLocaleString(numberLocale)}
                        {locale === 'ja'
                          ? copy.lineUnit
                          : row.count === 1
                            ? ' line'
                            : ` ${copy.lineUnit}`}
                      </strong>
                    </li>
                  ))}
                </ul>
              </section>

              <section
                className="crease-export-warnings"
                aria-labelledby="crease-export-loss-title"
              >
                <h3 id="crease-export-loss-title">{copy.lossTitle}</h3>
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
                      {copy.acknowledge}
                    </label>
                  </>
                ) : (
                  <p>{copy.lossless}</p>
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
            {copy.cancel}
          </button>
          <button
            type="button"
            className="primary"
            disabled={!canSave}
            onClick={() => onSave(warningsAcknowledged)}
          >
            {busy ? copy.processing : copy.save}
          </button>
        </footer>
      </section>
    </div>
  )
}
