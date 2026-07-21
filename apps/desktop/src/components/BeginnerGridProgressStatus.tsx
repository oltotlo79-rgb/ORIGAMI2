export function BeginnerGridProgressStatus({ locale, busy, enumerated, checked, onCancel }: {
  locale: 'ja' | 'en'
  busy: boolean
  enumerated: number
  checked: number
  onCancel: () => void
}) {
  if (!busy) return null
  const safeEnumerated = Number.isInteger(enumerated) ? Math.max(0, Math.min(27, enumerated)) : 0
  const safeChecked = Number.isInteger(checked) ? Math.max(0, Math.min(3, checked)) : 0
  return <div role="group" aria-label={locale === 'ja' ? '完全動物を含む27案探索の進捗' : 'Progress of the 27-design search including complete animals'}>
    <button type="button" onClick={onCancel}>
      {locale === 'ja' ? '27案の評価をキャンセル' : 'Cancel 27-design evaluation'}
    </button>
    <p role="status">
      {locale === 'ja'
        ? `列挙 ${safeEnumerated}/27・大域検証 ${safeChecked}/3`
        : `Enumerated ${safeEnumerated}/27 · globally checked ${safeChecked}/3`}
    </p>
  </div>
}
