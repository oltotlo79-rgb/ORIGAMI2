import type { LocalizedText } from './i18n.ts'

export const WORKSPACE_LAYOUT_SEPARATOR_TEXT: Readonly<Record<
  'editorLabel' | 'inspectorLabel' | 'timelineLabel',
  LocalizedText
>> = Object.freeze({
  editorLabel: Object.freeze({
    ja: '2Dと3Dの幅を変更',
    en: 'Resize 2D and 3D panels',
  }),
  inspectorLabel: Object.freeze({
    ja: 'プロパティパネルの幅を変更',
    en: 'Resize properties panel',
  }),
  timelineLabel: Object.freeze({
    ja: '折り手順パネルの高さを変更',
    en: 'Resize instruction timeline panel',
  }),
})
