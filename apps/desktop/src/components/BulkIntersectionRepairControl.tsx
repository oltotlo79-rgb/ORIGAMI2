import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import {
  BULK_INTERSECTION_REPAIR_CONTROL_TEXT as TEXT,
} from '../lib/bulkIntersectionRepairControlText.ts'

export function BulkIntersectionRepairControl({
  count, pending, disabled, locale, onConfirm,
}: {
  count: number
  pending: boolean
  disabled: boolean
  locale: Locale
  onConfirm: () => void
}) {
  if (count === 0) return null
  const label = pending
    ? selectLocalizedText(locale, TEXT.repairing)
    : formatLocalizedText(locale, TEXT.repairAll, { count })
  return <button type="button" data-testid="repair-all-unsplit-intersections"
    disabled={disabled || pending}
    onClick={() => {
      const message = formatLocalizedText(locale, TEXT.confirmation, { count })
      if (window.confirm(message)) onConfirm()
    }}>{label}</button>
}
