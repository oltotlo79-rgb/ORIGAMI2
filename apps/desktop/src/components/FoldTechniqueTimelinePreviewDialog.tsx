import { useEffect, useRef } from 'react'
import type { FoldTechniqueTimelineProposalPreview } from '../lib/foldTechniqueTimelineProposal.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  useLocale,
  type LocalizedText,
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

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

const TEXT = Object.freeze({
  eyebrow: localized('適用前の確認', 'Review before applying'),
  title: localized('折り手順タイムライン案', 'Instruction timeline proposal'),
  safety: localized(
    '追加する全項目は説明専用です。現在の3D姿勢を変えず、折り重ねを含む物理コマンドを実行しません。確定すると、一覧全体を1回のUndoで戻せる形で追加します。',
    'Every item is description-only. The current 3D pose is unchanged and no physical command, including stacked folding, is executed. Confirming adds the complete list as one undoable edit.',
  ),
  technique: localized('技法', 'Technique'),
  operations: localized('元の操作数', 'Source operations'),
  steps: localized('追加する説明ステップ数', 'Description steps to add'),
  unsupported: localized('未対応の物理操作数', 'Unsupported physical operations'),
  unsupportedNote: localized(
    '中割り・かぶせ・沈め折り・層選択などの未対応操作は、注意付きの説明テンプレートとしてのみ追加します。',
    'Unsupported motions such as reverse folds, sinks, and layer selection are added only as explanation templates with cautions.',
  ),
  previewList: localized('追加順', 'Append order'),
  inertStep: localized('説明専用・{kind}', 'Description only · {kind}'),
  sourceKinds: Object.freeze({
    technique: localized('技法情報', 'Technique information'),
    parameter: localized('設定値', 'Parameter'),
    precondition: localized('前提条件', 'Precondition'),
    operation: localized('操作', 'Operation'),
  }),
  stale: localized(
    'プロジェクトまたは選択中の技法が変わりました。この案を閉じて作り直してください。',
    'The project or selected technique changed. Close this proposal and rebuild it.',
  ),
  applying: localized(
    '説明ステップを原子的に追加しています…',
    'Appending the description steps atomically…',
  ),
  cancel: localized('キャンセル', 'Cancel'),
  confirm: localized(
    '説明専用手順を追加',
    'Add description-only steps',
  ),
})
