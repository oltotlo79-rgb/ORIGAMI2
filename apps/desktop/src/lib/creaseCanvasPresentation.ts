import {
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'
import type { SnapKind } from './snap.ts'

export type CreaseCanvasGuideDetail =
  | 'intersection-cluster'
  | 'boundary-t-junction'

type CreaseCanvasTextKey =
  | 'ariaLabel'
  | 'fallback'
  | 'title'
  | 'disabledTitle'
  | 'measurementUnavailable'
  | 'paperEdgeRatio'

const CREASE_CANVAS_TEXT: Readonly<
  Record<CreaseCanvasTextKey, LocalizedText>
> = Object.freeze({
  ariaLabel: Object.freeze({
    ja: '展開図編集キャンバス',
    en: 'Crease-pattern editing canvas',
  }),
  fallback: Object.freeze({
    ja: '展開図。選択ツールでは頂点をドラッグして移動できます。',
    en: 'Crease pattern. With the select tool, drag a vertex to move it.',
  }),
  title: Object.freeze({
    ja: '展開図編集キャンバス。選択ツールでは頂点をドラッグして移動できます。',
    en: 'Crease-pattern editing canvas. With the select tool, drag a vertex to move it.',
  }),
  disabledTitle: Object.freeze({
    ja: '展開図編集キャンバス。現在は操作できません。',
    en: 'Crease-pattern editing canvas. Editing is currently unavailable.',
  }),
  measurementUnavailable: Object.freeze({
    ja: '計測不可',
    en: 'Unavailable',
  }),
  paperEdgeRatio: Object.freeze({
    ja: '紙辺比',
    en: 'paper-edge ratio',
  }),
})

const SNAP_KIND_TEXT: Readonly<Record<SnapKind, LocalizedText>> =
  Object.freeze({
    vertex: Object.freeze({ ja: '頂点', en: 'Vertex' }),
    intersection: Object.freeze({ ja: '交点', en: 'Intersection' }),
    midpoint: Object.freeze({ ja: '中点', en: 'Midpoint' }),
    horizontal: Object.freeze({ ja: '水平', en: 'Horizontal' }),
    vertical: Object.freeze({ ja: '垂直', en: 'Vertical' }),
    parallel: Object.freeze({ ja: '平行', en: 'Parallel' }),
    angle: Object.freeze({ ja: '角度', en: 'Angle' }),
    edge: Object.freeze({ ja: '辺', en: 'Edge' }),
    grid: Object.freeze({ ja: 'グリッド', en: 'Grid' }),
  })

const GUIDE_DETAIL_TEXT: Readonly<
  Record<CreaseCanvasGuideDetail, LocalizedText>
> = Object.freeze({
  'intersection-cluster': Object.freeze({
    ja: '交点クラスタ',
    en: 'Intersection cluster',
  }),
  'boundary-t-junction': Object.freeze({
    ja: '輪郭T字',
    en: 'Boundary T-junction',
  }),
})

const FORMATTED_NUMBER_SOURCE =
  String.raw`[+-]?(?:(?:\d{1,3}(?:,\d{3})+)|\d+)(?:\.\d+)?(?:[eE][+-]?\d+)?`
const MEASUREMENT_PATTERN = new RegExp(
  `^(${FORMATTED_NUMBER_SOURCE})\\s+(mm|cm|in|紙辺比|paper-edge ratio)\\s+\\/\\s+(${FORMATTED_NUMBER_SOURCE})°$`,
  'u',
)
const MAX_MEASUREMENT_LABEL_LENGTH = 160

export function creaseCanvasText(
  locale: Locale,
  key: CreaseCanvasTextKey,
): string {
  return selectLocalizedText(locale, CREASE_CANVAS_TEXT[key])
}

export function creaseCanvasTitle(
  locale: Locale,
  disabled: boolean,
): string {
  return creaseCanvasText(locale, disabled ? 'disabledTitle' : 'title')
}

export function creaseCanvasSnapKindLabel(
  locale: Locale,
  kind: SnapKind,
): string {
  return selectLocalizedText(locale, SNAP_KIND_TEXT[kind])
}

export function creaseCanvasGuideDetailLabel(
  locale: Locale,
  detail: CreaseCanvasGuideDetail,
): string {
  return selectLocalizedText(locale, GUIDE_DETAIL_TEXT[detail])
}

export function creaseCanvasAngleGuideLabel(
  locale: Locale,
  side: 'counterclockwise' | 'clockwise',
  angleDegrees: number,
): string {
  const sign = side === 'counterclockwise' ? '+' : '-'
  return `${creaseCanvasSnapKindLabel(locale, 'angle')} ${sign}${
    formatGuideAngle(angleDegrees)
  }°`
}

/**
 * `measurementLabel` crosses the component boundary as a plain string.
 * Accept only the editor's numeric measurement grammar and known units so
 * native errors, project-authored text, and stale Japanese labels cannot leak
 * into the canvas after the UI switches to English.
 */
export function localizeCreaseCanvasMeasurementLabel(
  value: unknown,
  locale: Locale,
): string {
  const unavailable = creaseCanvasText(
    locale,
    'measurementUnavailable',
  )
  if (typeof value !== 'string' || value.length > MAX_MEASUREMENT_LABEL_LENGTH) {
    return unavailable
  }
  if (value === '計測不可' || value === 'Unavailable') return unavailable

  const match = MEASUREMENT_PATTERN.exec(value)
  if (!match) return unavailable
  const [, length, rawUnit, angle] = match
  const unit = rawUnit === '紙辺比' || rawUnit === 'paper-edge ratio'
    ? creaseCanvasText(locale, 'paperEdgeRatio')
    : rawUnit
  return `${length} ${unit} / ${angle}°`
}

function formatGuideAngle(value: number): string {
  if (!Number.isFinite(value)) return '—'
  if (value !== 0 && Math.abs(value) < 0.001) return value.toExponential(2)
  return String(Number(value.toFixed(3)))
}
