import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  BEGINNER_GRID_PROGRESS_STATUS_TEXT as TEXT,
} from '../lib/beginnerGridProgressStatusText.ts'

export function BeginnerGridProgressStatus({ locale, busy, enumerated, checked, refined, onCancel }: {
  locale: Locale
  busy: boolean
  enumerated: number
  checked: number
  refined: number
  onCancel: () => void
}) {
  if (!busy) return null
  const safeEnumerated = Number.isInteger(enumerated) ? Math.max(0, Math.min(27, enumerated)) : 0
  const safeChecked = Number.isInteger(checked) ? Math.max(0, Math.min(3, checked)) : 0
  const safeRefined = Number.isInteger(refined) ? Math.max(0, Math.min(24, refined)) : 0
  return <div
    role="group"
    aria-label={selectLocalizedText(locale, TEXT.groupAriaLabel)}
  >
    <button type="button" onClick={onCancel}>
      {selectLocalizedText(locale, TEXT.cancel)}
    </button>
    <p role="status">
      {formatLocalizedText(locale, TEXT.progress, {
        enumerated: safeEnumerated,
        refined: safeRefined,
        checked: safeChecked,
      })}
    </p>
  </div>
}
