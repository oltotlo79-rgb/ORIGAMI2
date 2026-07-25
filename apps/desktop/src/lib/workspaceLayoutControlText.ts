import type { LocalizedText } from './i18n.ts'

export const WORKSPACE_LAYOUT_CONTROL_TEXT: Readonly<Record<
  | 'summary'
  | 'groupAriaLabel'
  | 'swapPanels'
  | 'movePropertiesLeft'
  | 'movePropertiesRight'
  | 'reset'
  | 'outputAriaLabel'
  | 'properties'
  | 'timeline',
  LocalizedText
>> = Object.freeze({
  summary: Object.freeze({ ja: 'レイアウト', en: 'Layout' }),
  groupAriaLabel: Object.freeze({
    ja: '作業レイアウト',
    en: 'Workspace layout',
  }),
  swapPanels: Object.freeze({
    ja: '2Dと3Dを入れ替え',
    en: 'Swap 2D and 3D',
  }),
  movePropertiesLeft: Object.freeze({
    ja: 'プロパティを左へ',
    en: 'Move properties left',
  }),
  movePropertiesRight: Object.freeze({
    ja: 'プロパティを右へ',
    en: 'Move properties right',
  }),
  reset: Object.freeze({ ja: '初期配置に戻す', en: 'Reset layout' }),
  outputAriaLabel: Object.freeze({
    ja: '現在の作業レイアウト',
    en: 'Current workspace layout',
  }),
  properties: Object.freeze({ ja: 'プロパティ', en: 'Properties' }),
  timeline: Object.freeze({ ja: '手順', en: 'Timeline' }),
})
