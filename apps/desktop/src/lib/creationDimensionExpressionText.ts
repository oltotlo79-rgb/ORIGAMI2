import type { LocalizedText } from './i18n.ts'

export const CREATION_DIMENSION_EXPRESSION_TEXT: Readonly<Record<
  'label' | 'dimensions' | 'showValue' | 'showExpression',
  LocalizedText
>> = Object.freeze({
  label: Object.freeze({
    ja: '作成時サイズ:',
    en: 'Creation size:',
  }),
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
