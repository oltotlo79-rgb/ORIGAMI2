import { useEffect, useRef } from 'react'
import type { FoldTechniqueTimelineProposalPreview } from '../lib/foldTechniqueTimelineProposal.ts'
import { FOLD_TECHNIQUE_TIMELINE_PREVIEW_DIALOG_TEXT as TEXT } from '../lib/foldTechniqueTimelinePreviewDialogText.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  useLocale,
} from '../lib/i18n.ts'

type ReadyPreview = Extract<
  FoldTechniqueTimelineProposalPreview,
  { ok: true }
>

type FoldTechniqueTimelinePreviewDialogProps = Readonly<{
  preview: ReadyPreview
  busy: boolean
  stale: boolean
  error: string | null
  onConfirm(): void
  onCancel(): void
}>

export function FoldTechniqueTimelinePreviewDialog({
  preview,
  busy,
  stale,
  error,
  onConfirm,
  onCancel,
}: FoldTechniqueTimelinePreviewDialogProps) {
  const locale = useLocale()
  const dialogRef = useRef<HTMLElement>(null)
  const cancelRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    cancelRef.current?.focus()
  }, [])

  function handleKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape' && !busy) {
      event.preventDefault()
      onCancel()
      return
    }
    if (event.key !== 'Tab') return
    const focusable = Array.from(
      dialogRef.current?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
      ) ?? [],
    )
    const first = focusable[0]
    const last = focusable.at(-1)
    if (!first || !last) return
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return (
    <div className="dialog-backdrop">
      <section
        ref={dialogRef}
        className="new-project-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="fold-technique-timeline-preview-title"
        aria-describedby="fold-technique-timeline-preview-safety"
        aria-busy={busy}
        onKeyDown={handleKeyDown}
      >
        <header>
          <div>
            <span className="dialog-eyebrow">
              {selectLocalizedText(locale, TEXT.eyebrow)}
            </span>
            <h2 id="fold-technique-timeline-preview-title">
              {selectLocalizedText(locale, TEXT.title)}
            </h2>
          </div>
          <button
            type="button"
            className="dialog-close"
            aria-label={selectLocalizedText(locale, TEXT.cancel)}
            disabled={busy}
            onClick={onCancel}
          >
            ×
          </button>
        </header>
        <form onSubmit={(event) => {
          event.preventDefault()
          if (!busy && !stale) onConfirm()
        }}>
          <p id="fold-technique-timeline-preview-safety">
            {selectLocalizedText(locale, TEXT.safety)}
          </p>
          <dl>
            <div>
              <dt>{selectLocalizedText(locale, TEXT.technique)}</dt>
              <dd>{preview.techniqueName}</dd>
            </div>
            <div>
              <dt>{selectLocalizedText(locale, TEXT.operations)}</dt>
              <dd>{preview.operationCount.toLocaleString(locale)}</dd>
            </div>
            <div>
              <dt>{selectLocalizedText(locale, TEXT.steps)}</dt>
              <dd>{preview.proposal.steps.length.toLocaleString(locale)}</dd>
            </div>
            <div>
              <dt>{selectLocalizedText(locale, TEXT.unsupported)}</dt>
              <dd>{preview.unsupportedOperationCount.toLocaleString(locale)}</dd>
            </div>
          </dl>
          {preview.unsupportedOperationCount > 0 && (
            <p role="note">
              {selectLocalizedText(locale, TEXT.unsupportedNote)}
            </p>
          )}
          <fieldset>
            <legend>{selectLocalizedText(locale, TEXT.previewList)}</legend>
            <ol>
              {preview.proposal.steps.map((step, index) => (
                <li key={`${step.source_kind}:${step.source_id}:${step.chunk_index}`}>
                  <strong>{index + 1}. {step.title}</strong>
                  <br />
                  <small>
                    {formatLocalizedText(locale, TEXT.inertStep, {
                      kind: sourceKindLabel(step.source_kind, locale),
                    })}
                  </small>
                  {step.caution && <p>{step.caution}</p>}
                </li>
              ))}
            </ol>
          </fieldset>
          {stale && (
            <p role="alert">
              {selectLocalizedText(locale, TEXT.stale)}
            </p>
          )}
          {error && <p role="alert">{error}</p>}
          {busy && (
            <p role="status" aria-live="polite">
              {selectLocalizedText(locale, TEXT.applying)}
            </p>
          )}
          <footer>
            <button
              ref={cancelRef}
              type="button"
              disabled={busy}
              onClick={onCancel}
            >
              {selectLocalizedText(locale, TEXT.cancel)}
            </button>
            <button
              type="submit"
              className="primary"
              disabled={busy || stale}
            >
              {selectLocalizedText(locale, TEXT.confirm)}
            </button>
          </footer>
        </form>
      </section>
    </div>
  )
}

function sourceKindLabel(
  kind: ReadyPreview['proposal']['steps'][number]['source_kind'],
  locale: 'ja' | 'en',
) {
  return selectLocalizedText(locale, TEXT.sourceKinds[kind])
}
