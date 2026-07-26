import {
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  PROOF_PROGRESS_PANEL_TEXT as TEXT,
} from '../lib/proofProgressPanelText.ts'
import './proofProgress.css'

type Props = Readonly<{
  locale: Locale
  confirmed: boolean
  disabled?: boolean
  busy?: boolean
  onConfirmedChange(confirmed: boolean): void
  onApply(): void
}>

/**
 * Visually and semantically separates an explicitly confirmed unproven
 * operation from the normal certified apply control. Authority and one-shot
 * token ownership remain in the parent orchestration layer.
 */
export function SpeculativeStackedFoldApplyControl({
  locale,
  confirmed,
  disabled = false,
  busy = false,
  onConfirmedChange,
  onApply,
}: Props) {
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const unavailable = disabled || busy

  return (
    <fieldset
      className="speculative-apply-control"
      aria-label={text(TEXT.speculativeApplyGroup)}
      aria-busy={busy}
    >
      <legend>{text(TEXT.speculativeApplyGroup)}</legend>
      <p role="note">{text(TEXT.speculativeApplyWarning)}</p>
      <label>
        <input
          type="checkbox"
          checked={confirmed}
          disabled={unavailable}
          onChange={(event) => onConfirmedChange(event.target.checked)}
        />
        {text(TEXT.speculativeConfirmation)}
      </label>
      <button
        className="speculative-apply-button"
        data-apply-mode="speculative_unproven"
        type="button"
        onClick={onApply}
        disabled={!confirmed || unavailable}
      >
        {text(busy ? TEXT.applyingSpeculative : TEXT.applySpeculative)}
      </button>
    </fieldset>
  )
}
