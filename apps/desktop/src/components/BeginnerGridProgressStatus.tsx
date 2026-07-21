export function BeginnerGridProgressStatus({ locale, busy, enumerated, checked, refined, onCancel }: {
  locale: 'ja' | 'en'
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
  return <div role="group" aria-label={locale === 'ja' ? '候補生成と局所改善の進捗' : 'Candidate generation and local refinement progress'}>
    <button type="button" onClick={onCancel}>
      {locale === 'ja' ? '候補生成をキャンセル' : 'Cancel candidate generation'}
    </button>
    <p role="status">
      {locale === 'ja'
        ? `列挙 ${safeEnumerated}/27・局所改善 ${safeRefined}/24・大域検証 ${safeChecked}/3`
        : `Enumerated ${safeEnumerated}/27 · refined ${safeRefined}/24 · globally checked ${safeChecked}/3`}
    </p>
  </div>
}
