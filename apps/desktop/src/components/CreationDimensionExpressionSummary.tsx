import { useState } from 'react'

import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n.ts'

export type CreationDimensionExpressionBinding = Readonly<{
  schema_version: 1
  width_source: string
  height_source: string
  adopted_width_mm: number
  adopted_height_mm: number
}>

export function CreationDimensionExpressionSummary({
  binding,
  localeStore: localeStore_ = localeStore,
}: Readonly<{
  binding: CreationDimensionExpressionBinding | undefined
  localeStore?: LocaleStore
}>) {
  const locale = useLocale(localeStore_)
  const [showExpressions, setShowExpressions] = useState(true)
  if (!validBinding(binding)) return null
  const dimensions = showExpressions
    ? formatLocalizedText(locale, CREATION_DIMENSION_TEXT.dimensions, {
      width: binding.width_source,
      height: binding.height_source,
    })
    : formatLocalizedText(locale, CREATION_DIMENSION_TEXT.dimensions, {
      width: formatMillimetres(binding.adopted_width_mm),
      height: formatMillimetres(binding.adopted_height_mm),
    })

  return (
    <div className="creation-dimension-expression-summary">
      <span>
        {selectLocalizedText(locale, CREATION_DIMENSION_TEXT.label)}
        {' '}
        {dimensions}
      </span>
      <button
        type="button"
        aria-pressed={!showExpressions}
        onClick={() => setShowExpressions((current) => !current)}
      >
        {showExpressions
          ? selectLocalizedText(locale, CREATION_DIMENSION_TEXT.showValue)
          : selectLocalizedText(locale, CREATION_DIMENSION_TEXT.showExpression)}
      </button>
    </div>
  )
}

function validBinding(
  binding: CreationDimensionExpressionBinding | undefined,
): binding is CreationDimensionExpressionBinding {
  return Boolean(
    binding
    && binding.schema_version === 1
    && typeof binding.width_source === 'string'
    && binding.width_source.length > 0
    && typeof binding.height_source === 'string'
    && binding.height_source.length > 0
    && Number.isFinite(binding.adopted_width_mm)
    && binding.adopted_width_mm > 0
    && Number.isFinite(binding.adopted_height_mm)
    && binding.adopted_height_mm > 0,
  )
}

function formatMillimetres(value: number) {
  return value.toPrecision(15).replace(/(?:\.0+|(\.\d+?)0+)$/u, '$1')
}

const CREATION_DIMENSION_TEXT = Object.freeze({
  label: Object.freeze({ ja: '作成時サイズ:', en: 'Creation size:' }),
  dimensions: Object.freeze({
    ja: '{width} × {height} mm',
    en: '{width} × {height} mm',
  }),
  showValue: Object.freeze({
    ja: '評価値を表示',
    en: 'Show values',
  }),
  showExpression: Object.freeze({
    ja: '式を表示',
    en: 'Show expressions',
  }),
})
