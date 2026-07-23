export function BulkIntersectionRepairControl({
  count, pending, disabled, locale, onConfirm,
}: {
  count: number
  pending: boolean
  disabled: boolean
  locale: 'ja' | 'en'
  onConfirm: () => void
}) {
  if (count === 0) return null
  const label = pending
    ? (locale === 'ja' ? '一括修復中…' : 'Repairing…')
    : (locale === 'ja' ? `交差を一括修復（${count}件）` : `Repair all intersections (${count})`)
  return <button type="button" data-testid="repair-all-unsplit-intersections"
    disabled={disabled || pending}
    onClick={() => {
      const message = locale === 'ja'
        ? `${count}件の未分割交差を一括修復しますか？`
        : `Repair ${count} unsplit intersections as one undoable edit?`
      if (window.confirm(message)) onConfirm()
    }}>{label}</button>
}
