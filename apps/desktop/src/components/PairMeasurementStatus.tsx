import { formatLocalizedText, type Locale } from '../lib/i18n'

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
    ? formatLocalizedText(locale, {
        ja: '2頂点間の距離: {value}',
        en: 'Vertex distance: {value}',
      }, { value: formattedValue })
    : kind === 'line'
      ? formatLocalizedText(locale, {
          ja: '2辺間の角度（向きなし）: {value}',
          en: 'Unoriented edge angle: {value}',
        }, { value: formattedValue })
      : formatLocalizedText(locale, {
          ja: '計測: 同じ種類の頂点または辺を2つ選択（頂点 {vertices}/2、辺 {lines}/2）',
          en: 'Measure: select two vertices or two edges (vertices {vertices}/2, edges {lines}/2)',
        }, { vertices: vertexCount, lines: lineCount })

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
