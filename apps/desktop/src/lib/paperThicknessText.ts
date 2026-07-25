import type { LocalizedText } from './i18n.ts'

export const PAPER_THICKNESS_TEXT: Readonly<Record<
  'ariaLabel' | 'title' | 'description' | 'increase' | 'decrease' | 'paperEdgeRatio',
  LocalizedText
>> = Object.freeze({
  ariaLabel: Object.freeze({ ja: '紙厚', en: 'Paper thickness' }),
  title: Object.freeze({
    ja: '上下ボタンと矢印キーは物理量0.01 mm刻み。値は{unit}で直接入力できます',
    en: 'Step buttons and arrow keys change the physical value by 0.01 mm. Values can be entered directly in {unit}.',
  }),
  description: Object.freeze({
    ja: '上下ボタンと矢印キーは表示単位に関係なく、紙厚を物理量0.01 mmずつ増減します。値は直接入力できます。',
    en: 'Step buttons and arrow keys increase or decrease paper thickness by a physical 0.01 mm, regardless of the display unit. Values can also be entered directly.',
  }),
  increase: Object.freeze({
    ja: '紙厚を0.01 mm増やす',
    en: 'Increase paper thickness by 0.01 mm',
  }),
  decrease: Object.freeze({
    ja: '紙厚を0.01 mm減らす',
    en: 'Decrease paper thickness by 0.01 mm',
  }),
  paperEdgeRatio: Object.freeze({
    ja: '紙辺比',
    en: 'paper-edge ratio',
  }),
})
