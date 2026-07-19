import { useState } from 'react'

export type CreationDimensionExpressionBinding = Readonly<{
  schema_version: 1
  width_source: string
  height_source: string
  adopted_width_mm: number
  adopted_height_mm: number
}>

export function CreationDimensionExpressionSummary({
  binding,
}: Readonly<{ binding: CreationDimensionExpressionBinding | undefined }>) {
  const [showExpressions, setShowExpressions] = useState(true)
  if (!validBinding(binding)) return null

  return (
    <div className="creation-dimension-expression-summary">
      <span>
        作成時サイズ:
        {' '}
        {showExpressions
          ? `${binding.width_source} × ${binding.height_source} mm`
          : `${formatMillimetres(binding.adopted_width_mm)} × ${formatMillimetres(binding.adopted_height_mm)} mm`}
      </span>
      <button
        type="button"
        aria-pressed={!showExpressions}
        onClick={() => setShowExpressions((current) => !current)}
      >
        {showExpressions ? '評価値を表示' : '式を表示'}
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
