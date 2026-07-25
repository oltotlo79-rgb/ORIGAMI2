import type { LocalizedText } from './i18n.ts'

export const UNDERLAY_PANEL_TEXT = Object.freeze({
  title: Object.freeze({ ja: '下絵', en: 'Underlays' }),
  add: Object.freeze({ ja: '画像を追加', en: 'Add image' }),
  createLayer: Object.freeze({ ja: '下絵レイヤーを先に作成してください。', en: 'Create an underlay layer first.' }),
  list: Object.freeze({ ja: '下絵一覧', en: 'Underlay list' }),
  item: Object.freeze({ ja: '下絵 {index}', en: 'Underlay {index}' }),
  form: Object.freeze({ ja: '下絵の配置と変形', en: 'Place and transform underlay' }),
  layer: Object.freeze({ ja: 'レイヤー', en: 'Layer' }),
  scaleX: Object.freeze({ ja: '横倍率', en: 'Scale X' }),
  scaleY: Object.freeze({ ja: '縦倍率', en: 'Scale Y' }),
  rotation: Object.freeze({ ja: '回転', en: 'Rotation' }),
  opacity: Object.freeze({ ja: '不透明度', en: 'Opacity' }),
  locked: Object.freeze({ ja: 'このレイヤーはロックされています。', en: 'This layer is locked.' }),
  save: Object.freeze({ ja: '保存', en: 'Save' }),
  delete: Object.freeze({ ja: '削除', en: 'Delete' }),
} satisfies Readonly<Record<string, LocalizedText>>)
