import type { LocalizedText } from './i18n.ts'

export const LENGTH_UNIT_CONTROL_TEXT: Readonly<Record<
  | 'legend' | 'unit' | 'millimetres' | 'centimetres' | 'inches'
  | 'paperEdgeRatio' | 'referenceEdge' | 'referenceEdgeAriaLabel'
  | 'invalidSavedReference' | 'edgeOption' | 'ratioNote'
  | 'invalidReferenceWithId' | 'invalidReference' | 'repairNote' | 'noReference',
  LocalizedText
>> = Object.freeze({
  legend: Object.freeze({ ja: '長さの表示単位', en: 'Length display unit' }),
  unit: Object.freeze({ ja: '単位', en: 'Unit' }),
  millimetres: Object.freeze({ ja: 'ミリメートル (mm)', en: 'Millimetres (mm)' }),
  centimetres: Object.freeze({ ja: 'センチメートル (cm)', en: 'Centimetres (cm)' }),
  inches: Object.freeze({ ja: 'インチ (in)', en: 'Inches (in)' }),
  paperEdgeRatio: Object.freeze({ ja: '紙辺比', en: 'Paper-edge ratio' }),
  referenceEdge: Object.freeze({ ja: '基準にする輪郭辺', en: 'Reference boundary edge' }),
  referenceEdgeAriaLabel: Object.freeze({
    ja: '紙辺比の基準輪郭辺',
    en: 'Paper-edge ratio reference boundary edge',
  }),
  invalidSavedReference: Object.freeze({
    ja: '保存された基準辺は無効です',
    en: 'The saved reference edge is invalid',
  }),
  edgeOption: Object.freeze({
    ja: '辺 {index} · {edgeId} · {length}',
    en: 'Edge {index} · {edgeId} · {length}',
  }),
  ratioNote: Object.freeze({
    ja: '基準辺を 1 として表示します。基準辺の長さ変更には自動追従し、別の辺への自動切り替えは行いません。',
    en: 'Displays lengths relative to a reference edge of 1. Changes to that edge are followed automatically; another edge is never selected automatically.',
  }),
  invalidReferenceWithId: Object.freeze({
    ja: '保存された基準辺「{edgeId}」を現在の輪郭で一意に確認できません。',
    en: 'The saved reference edge "{edgeId}" cannot be uniquely identified in the current boundary.',
  }),
  invalidReference: Object.freeze({
    ja: '保存された紙辺比の基準辺が不正です。',
    en: 'The saved paper-edge ratio reference is invalid.',
  }),
  repairNote: Object.freeze({
    ja: '長さは修復用に mm で表示しています。単位または有効な基準辺を選び直してください。',
    en: 'Lengths are displayed in mm for repair. Select a unit or a valid reference edge.',
  }),
  noReference: Object.freeze({
    ja: '紙辺比に使用できる、一意で長さが正の輪郭辺がありません。先に輪郭を修復するか、mm・cm・in を選択してください。',
    en: 'No unique, positive-length boundary edge is available for a paper-edge ratio. Repair the boundary first, or select mm, cm, or in.',
  }),
})
