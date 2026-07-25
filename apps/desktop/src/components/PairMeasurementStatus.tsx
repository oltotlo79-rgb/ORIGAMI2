import { formatLocalizedText, type Locale } from '../lib/i18n'
import { PAIR_MEASUREMENT_TEXT } from '../lib/pairMeasurementText'

type Props = Readonly<{
  locale: Locale
  kind: 'pending' | 'vertex' | 'line'
  formattedValue?: string
  vertexCount: number
  lineCount: number
}>

export function PairMeasurementStatus({
  locale,
  kind,
  formattedValue = '',
  vertexCount,
  lineCount,
}: Props) {
  const message = kind === 'vertex'
    ? formatLocalizedText(
        locale,
        PAIR_MEASUREMENT_TEXT.vertexDistance,
        { value: formattedValue },
      )
    : kind === 'line'
      ? formatLocalizedText(
          locale,
          PAIR_MEASUREMENT_TEXT.unorientedEdgeAngle,
          { value: formattedValue },
        )
      : formatLocalizedText(
          locale,
          PAIR_MEASUREMENT_TEXT.pending,
          { vertices: vertexCount, lines: lineCount },
        )

  return (
    <p
      className="measurement-status"
      role="status"
      aria-live="polite"
      data-measurement-kind={kind}
    >
      {message}
    </p>
  )
}
