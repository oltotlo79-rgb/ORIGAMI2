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
  instructionExportWarningMessage,
  isInstructionExportFormat,
  type InstructionExportFormat,
  type InstructionExportPhase,
  type InstructionExportPreview,
} from '../lib/instructionExport.ts'
import { useLocale } from '../lib/i18n.ts'

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

const INSTRUCTION_EXPORT_COPY = {
  ja: {
    eyebrow: '折り図の書き出し',
    title: '形式と出力内容を確認',
    close: '閉じる',
    description:
      '現在の編集リビジョンから折り図を生成します。書き出してもプロジェクトの保存状態や履歴は変わりません。',
    format: '出力形式',
    optionDetails: {
      pdf: '固定アイソメトリック視点の折り図を、複数ページのPDFにまとめます',
      svg_zip: '手順ごとのベクターSVG画像を、1つのZIPにまとめます',
    },
    rebuild: '現在の編集内容から作り直す',
    retry: '同じ形式で再試行',
    metadata: {
      format: '形式',
      specification: '出力仕様',
      profile: '出力プロファイル',
      projection: '投影プロファイル',
      suggestedName: '保存名候補',
      size: 'サイズ',
      steps: '折り手順',
      pages: 'ページ',
      cautions: '注意事項',
      revision: '固定元',
    },
    stepUnit: '手順',
    pageUnit: 'ページ',
    cautionUnit: '件',
    warningTitle: '出力前の確認事項',
    acknowledge: '上記の注意事項を確認しました',
    warningFree: 'この折り図について追加確認が必要な注意事項はありません。',
    stop: '生成を中止',
    cancel: 'キャンセル',
    processing: '処理中…',
    save: '保存先を選んで書き出す…',
    summaries: {
      pdf: 'PDF 1.7・A4縦・固定アイソメトリック投影・複数ページ',
      svg_zip: 'SVGページ画像・固定アイソメトリック投影・ZIPアーカイブ',
    },
  },
  en: {
    eyebrow: 'Export instructions',
    title: 'Review format and output',
    close: 'Close',
    description:
      'Generate instructions from the current edit revision. Exporting does not change the project save state or history.',
    format: 'Export format',
    optionDetails: {
      pdf: 'Combine fixed-isometric diagrams and hand/regrip guide labels into a multi-page PDF',
      svg_zip: 'Package one vector SVG page with fold directions, focus points, and hand positions into a ZIP',
    },
    rebuild: 'Rebuild from the current edits',
    retry: 'Retry the same format',
    metadata: {
      format: 'Format',
      specification: 'Specification',
      profile: 'Export profile',
      projection: 'Projection profile',
      suggestedName: 'Suggested file name',
      size: 'Size',
      steps: 'Instruction steps',
      pages: 'Pages',
      cautions: 'Notices',
      revision: 'Source',
    },
    stepUnit: 'steps',
    pageUnit: 'pages',
    cautionUnit: 'notices',
    warningTitle: 'Review before export',
    acknowledge: 'I have reviewed the notices above',
    warningFree: 'No additional notices require review for these instructions.',
    stop: 'Stop generation',
    cancel: 'Cancel',
    processing: 'Processing…',
    save: 'Choose destination and export…',
    summaries: {
      pdf: 'PDF 1.7 · A4 portrait · fixed isometric projection · multiple pages',
      svg_zip: 'SVG page images · fixed isometric projection · ZIP archive',
    },
  },
} as const

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
  const locale = useLocale()
  const copy = INSTRUCTION_EXPORT_COPY[locale]
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
            <span className="dialog-eyebrow">{copy.eyebrow}</span>
            <h2 id="instruction-export-title">{copy.title}</h2>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="dialog-close"
            disabled={busy && !generationActive}
            onClick={onCancel}
            aria-label={copy.close}
          >
            ×
          </button>
        </header>

        <div className="crease-export-dialog-body">
          <p id="instruction-export-description" className="dialog-note">
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
                if (isInstructionExportFormat(next)) onFormatChange(next)
              }}
            >
              {INSTRUCTION_EXPORT_FORMATS.map((option) => (
                <option key={option.value} value={option.value}>
                  {instructionExportFormatLabel(option.value, locale)}
                  {' — '}
                  {copy.optionDetails[option.value]}
                </option>
              ))}
            </select>
          </label>

          {busy && !preview && (
            <p className="crease-export-loading" role="status">
              {instructionExportFormatLabel(format, locale)}:{' '}
              {instructionExportPhaseLabel(phase, locale)}…
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
                  <dd>{instructionExportFormatLabel(preview.format, locale)}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.specification}</dt>
                  <dd>{locale === 'ja'
                    ? preview.format_summary
                    : copy.summaries[preview.format]}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.profile}</dt>
                  <dd>{preview.profile}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.projection}</dt>
                  <dd>{preview.projection_profile}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.suggestedName}</dt>
                  <dd>{preview.suggested_file_name}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.size}</dt>
                  <dd>{formatInstructionExportBytes(preview.byte_count, locale)}</dd>
                </div>
                <div>
                  <dt>{copy.metadata.steps}</dt>
                  <dd>
                    {preview.step_count.toLocaleString(numberLocale)}
                    {localizedCountUnit(locale, preview.step_count, copy.stepUnit, 'step')}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.pages}</dt>
                  <dd>
                    {preview.page_count.toLocaleString(numberLocale)}
                    {localizedCountUnit(locale, preview.page_count, copy.pageUnit, 'page')}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.cautions}</dt>
                  <dd>
                    {preview.caution_count.toLocaleString(numberLocale)}
                    {localizedCountUnit(
                      locale,
                      preview.caution_count,
                      copy.cautionUnit,
                      'notice',
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{copy.metadata.revision}</dt>
                  <dd>revision {preview.expected_revision.toLocaleString(numberLocale)}</dd>
                </div>
              </dl>

              <section
                className="crease-export-warnings"
                aria-labelledby="instruction-export-warnings-title"
              >
                <h3 id="instruction-export-warnings-title">{copy.warningTitle}</h3>
                {preview.warnings.length > 0 ? (
                  <>
                    <ul>
                      {preview.warnings.map((warning) => (
                        <li key={warning.category}>
                          {instructionExportWarningMessage(warning, locale)}
                        </li>
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
                  <p>{copy.warningFree}</p>
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
            {generationActive ? copy.stop : copy.cancel}
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

function localizedCountUnit(
  locale: 'ja' | 'en',
  count: number,
  japaneseOrPlural: string,
  englishSingular: string,
) {
  if (locale === 'ja') return japaneseOrPlural
  return count === 1 ? ` ${englishSingular}` : ` ${japaneseOrPlural}`
}
