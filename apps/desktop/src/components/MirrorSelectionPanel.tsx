import { APP_TEXT } from '../lib/appText.ts'
import type { MirrorSelectionPreflight } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'

export type MirrorAxisDraft = Readonly<{
  x1: string
  y1: string
  x2: string
  y2: string
}>

export type MirrorSelectionPanelProps = Readonly<{
  locale: Locale
  coreBusy: boolean
  mirrorBusy: boolean
  candidateBusy: boolean
  currentSelectionAvailable: boolean
  selectedVertexCount: number
  selectedEdgeCount: number
  mode: 'move' | 'duplicate'
  axis: MirrorAxisDraft
  preview: Readonly<{ result: MirrorSelectionPreflight }> | null
  onAddCurrentSelection: () => void
  onCancelCandidateGeneration: () => void
  onCancelMirrorSelection: () => void
  onModeChange: (mode: 'move' | 'duplicate') => void
  onAxisChange: (key: keyof MirrorAxisDraft, value: string) => void
  onPreview: () => void | Promise<void>
  onApply: () => void | Promise<void>
}>

export function MirrorSelectionPanel({
  locale,
  coreBusy,
  mirrorBusy,
  candidateBusy,
  currentSelectionAvailable,
  selectedVertexCount,
  selectedEdgeCount,
  mode,
  axis,
  preview,
  onAddCurrentSelection,
  onCancelCandidateGeneration,
  onCancelMirrorSelection,
  onModeChange,
  onAxisChange,
  onPreview,
  onApply,
}: MirrorSelectionPanelProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const issueText = (issue: string | null) => {
    switch (issue) {
      case 'invalid_axis':
        return text(APP_TEXT.theMirrorAxisIsInvalid)
      case 'empty_selection':
        return text(APP_TEXT.theSelectionIsEmpty)
      case 'noncanonical_selection':
      case 'invalid_new_ids':
      case 'core_rejected':
        return text(APP_TEXT.thisEditIsUnsafeForTheCurrentGeometryOrLayers)
      default:
        return text(APP_TEXT.theMirrorEditCannotBeApplied)
    }
  }

  return (
    <section
      className="mirror-selection-panel"
      aria-labelledby="mirror-selection-heading"
    >
      <h3 id="mirror-selection-heading">
        {text(APP_TEXT.mirrorSelection)}
      </h3>
      <p aria-live="polite">
        {formatLocalizedText(
          locale,
          APP_TEXT.verticesVerticesEdgesEdges,
          {
            vertices: selectedVertexCount,
            edges: selectedEdgeCount,
          },
        )}
      </p>
      <div className="button-row">
        <button
          type="button"
          disabled={coreBusy || mirrorBusy || !currentSelectionAvailable}
          onClick={onAddCurrentSelection}
        >
          {text(APP_TEXT.addCurrentSelection)}
        </button>
        {candidateBusy && (
          <button type="button" onClick={onCancelCandidateGeneration}>
            {text(APP_TEXT.cancelCandidateGeneration)}
          </button>
        )}
        <button
          type="button"
          disabled={
            coreBusy || (selectedVertexCount === 0 && selectedEdgeCount === 0)
          }
          onClick={onCancelMirrorSelection}
        >
          {text(APP_TEXT.cancel)}
        </button>
      </div>
      <fieldset disabled={coreBusy || mirrorBusy}>
        <legend>{text(APP_TEXT.operation)}</legend>
        <label>
          <input
            type="radio"
            name="mirror_mode"
            checked={mode === 'duplicate'}
            onChange={() => onModeChange('duplicate')}
          />
          {text(APP_TEXT.duplicate)}
        </label>
        <label>
          <input
            type="radio"
            name="mirror_mode"
            checked={mode === 'move'}
            onChange={() => onModeChange('move')}
          />
          {text(APP_TEXT.move)}
        </label>
      </fieldset>
      <fieldset disabled={coreBusy || mirrorBusy}>
        <legend>{text(APP_TEXT.twoPointMirrorAxis)}</legend>
        {([
          ['x1', '始点 X', 'Start X'],
          ['y1', '始点 Y', 'Start Y'],
          ['x2', '終点 X', 'End X'],
          ['y2', '終点 Y', 'End Y'],
        ] as const).map(([key, ja, en]) => (
          <label className="field" key={key}>
            <span>{text({ ja, en })}</span>
            <input
              aria-label={text({ ja, en })}
              inputMode="decimal"
              value={axis[key]}
              onChange={(event) => onAxisChange(
                key,
                event.currentTarget.value,
              )}
            />
          </label>
        ))}
      </fieldset>
      <div className="button-row">
        <button
          type="button"
          disabled={
            coreBusy
            || mirrorBusy
            || (selectedVertexCount === 0 && selectedEdgeCount === 0)
          }
          onClick={() => void onPreview()}
        >
          {mirrorBusy ? text(APP_TEXT.checking) : text(APP_TEXT.preflight)}
        </button>
        <button
          type="button"
          disabled={coreBusy || mirrorBusy || !preview?.result.allowed}
          onClick={() => void onApply()}
        >
          {text(APP_TEXT.applyMirrorEdit)}
        </button>
      </div>
      {preview && (
        <p
          role="status"
          data-testid="mirror-selection-preflight"
          className={preview.result.allowed ? 'status-good' : 'status-bad'}
        >
          {preview.result.allowed
            ? text(APP_TEXT.readyReviewAndExplicitlyApplyTheEdit)
            : issueText(preview.result.issue)}
        </p>
      )}
    </section>
  )
}
