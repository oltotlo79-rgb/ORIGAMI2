import type { LocalizedText } from './i18n.ts'

export const ANNOTATION_PANEL_TEXT = Object.freeze({
  title: Object.freeze({ ja: '注釈', en: 'Annotations' }),
  new: Object.freeze({ ja: '新規', en: 'New' }),
  createLayer: Object.freeze({ ja: '注釈レイヤーを先に作成してください。', en: 'Create an annotation layer first.' }),
  list: Object.freeze({ ja: '注釈一覧', en: 'Annotation list' }),
  edit: Object.freeze({ ja: '注釈編集', en: 'Edit annotation' }),
  text: Object.freeze({ ja: '本文', en: 'Text' }),
  layer: Object.freeze({ ja: 'レイヤー', en: 'Layer' }),
  lockedOption: Object.freeze({ ja: 'ロック', en: 'locked' }),
  anchor: Object.freeze({ ja: '基準', en: 'Anchor' }),
  position: Object.freeze({ ja: '座標', en: 'Position' }),
  vertex: Object.freeze({ ja: '頂点', en: 'Vertex' }),
  fontSize: Object.freeze({ ja: '文字サイズ', en: 'Font size' }),
  textColor: Object.freeze({ ja: '文字色', en: 'Text color' }),
  textOpacity: Object.freeze({ ja: '文字の不透明度', en: 'Text opacity' }),
  bold: Object.freeze({ ja: '太字', en: 'Bold' }),
  italic: Object.freeze({ ja: '斜体', en: 'Italic' }),
  locked: Object.freeze({ ja: 'このレイヤーはロックされています。', en: 'This layer is locked.' }),
  save: Object.freeze({ ja: '保存', en: 'Save' }),
  delete: Object.freeze({ ja: '削除', en: 'Delete' }),
} satisfies Readonly<Record<string, LocalizedText>>)
