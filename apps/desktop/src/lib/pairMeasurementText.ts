import type { LocalizedText } from './i18n.ts'

export const PAIR_MEASUREMENT_TEXT: Readonly<Record<
  'vertexDistance' | 'unorientedEdgeAngle' | 'pending',
  LocalizedText
>> = Object.freeze({
  vertexDistance: Object.freeze({
    ja: '2頂点間の距離: {value}',
    en: 'Vertex distance: {value}',
  }),
  unorientedEdgeAngle: Object.freeze({
    ja: '2辺間の角度（向きなし）: {value}',
    en: 'Unoriented edge angle: {value}',
  }),
  pending: Object.freeze({
    ja: '計測: 同じ種類の頂点または辺を2つ選択（頂点 {vertices}/2、辺 {lines}/2）',
    en: 'Measure: select two vertices or two edges (vertices {vertices}/2, edges {lines}/2)',
  }),
})
