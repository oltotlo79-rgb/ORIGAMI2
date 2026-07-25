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
import {
  formatInstructionExportDialogCount,
  formatInstructionExportDialogOption,
  formatInstructionExportDialogProgress,
  formatInstructionExportDialogRevision,
  INSTRUCTION_EXPORT_COPY as TEXT,
  instructionExportDialogSummary,
} from '../lib/instructionExportDialogText.ts'
import {
  selectLocalizedText,
  type LocalizedText,
  useLocale,
} from '../lib/i18n.ts'

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
  const locale = useLocale()
  const localized = (text: LocalizedText) => selectLocalizedText(locale, text)
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
            <span className="dialog-eyebrow">{localized(TEXT.eyebrow)}</span>
            <h2 id="instruction-export-title">{localized(TEXT.title)}</h2>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="dialog-close"
            disabled={busy && !generationActive}
            onClick={onCancel}
            aria-label={localized(TEXT.close)}
          >
            {localized(TEXT.closeGlyph)}
          </button>
        </header>

        <div className="crease-export-dialog-body">
          <p id="instruction-export-description" className="dialog-note">
            {localized(TEXT.description)}
          </p>

          <label className="crease-export-format">
            <span>{localized(TEXT.format)}</span>
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
                  {formatInstructionExportDialogOption(
                    option.value,
                    instructionExportFormatLabel(option.value, locale),
                    locale,
                  )}
                </option>
              ))}
            </select>
          </label>

          {busy && !preview && (
            <p className="crease-export-loading" role="status">
              {formatInstructionExportDialogProgress(
                instructionExportFormatLabel(format, locale),
                instructionExportPhaseLabel(phase, locale),
                locale,
              )}
            </p>
          )}

          {error && (
            <div className="crease-export-error">
              <p className="dialog-error" role="alert">{error}</p>
              {!busy && (
                <button type="button" onClick={onRetry}>
                  {localized(preview ? TEXT.rebuild : TEXT.retry)}
                </button>
              )}
            </div>
          )}

          {preview && (
            <>
              <dl className="crease-export-metadata">
                <div>
                  <dt>{localized(TEXT.metadata.format)}</dt>
                  <dd>{instructionExportFormatLabel(preview.format, locale)}</dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.specification)}</dt>
                  <dd>
                    {instructionExportDialogSummary(
                      preview.format,
                      preview.format_summary,
                      locale,
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.profile)}</dt>
                  <dd>{preview.profile}</dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.projection)}</dt>
                  <dd>{preview.projection_profile}</dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.suggestedName)}</dt>
                  <dd>{preview.suggested_file_name}</dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.size)}</dt>
                  <dd>{formatInstructionExportBytes(preview.byte_count, locale)}</dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.steps)}</dt>
                  <dd>
                    {formatInstructionExportDialogCount(
                      preview.step_count,
                      'steps',
                      locale,
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.pages)}</dt>
                  <dd>
                    {formatInstructionExportDialogCount(
                      preview.page_count,
                      'pages',
                      locale,
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.cautions)}</dt>
                  <dd>
                    {formatInstructionExportDialogCount(
                      preview.caution_count,
                      'cautions',
                      locale,
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{localized(TEXT.metadata.revision)}</dt>
                  <dd>
                    {formatInstructionExportDialogRevision(
                      preview.expected_revision,
                      locale,
                    )}
                  </dd>
                </div>
              </dl>

              <section
                className="crease-export-warnings"
                aria-labelledby="instruction-export-warnings-title"
              >
                <h3 id="instruction-export-warnings-title">
                  {localized(TEXT.warningTitle)}
                </h3>
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
                      {localized(TEXT.acknowledge)}
                    </label>
                  </>
                ) : (
                  <p>{localized(TEXT.warningFree)}</p>
                )}
              </section>
            </>
          )}

          <p
            className="crease-export-notice"
            role="status"
            aria-live="polite"
          >
            {notice ?? localized(TEXT.emptyNotice)}
          </p>
        </div>

        <footer>
          <button
            type="button"
            disabled={busy && !generationActive}
            onClick={onCancel}
          >
            {localized(generationActive ? TEXT.stop : TEXT.cancel)}
          </button>
          <button
            type="button"
            className="primary"
            disabled={!canSave}
            onClick={() => onSave(warningsAcknowledged)}
          >
            {localized(busy ? TEXT.processing : TEXT.save)}
          </button>
        </footer>
      </section>
    </div>
  )
}
