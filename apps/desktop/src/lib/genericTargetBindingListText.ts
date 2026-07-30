import type { LocalizedText } from './i18n.ts'

export type GenericTargetBindingListText = Readonly<Record<
  | 'ariaLabel'
  | 'bindingRow'
  | 'symmetryAsymmetric'
  | 'symmetryBilateral'
  | 'symmetryRadial',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const GENERIC_TARGET_BINDING_LIST_TEXT =
  Object.freeze({
    ariaLabel: text(
      '上限付き汎用対象binding寸法',
      'Bounded generic target binding dimensions',
    ),
    bindingRow: text(
      'binding {id}・{symmetry}・数 {count}・長さ {length}・厚さ {thickness}',
      'Binding {id} · {symmetry} · count {count} · length {length} · thickness {thickness}',
    ),
    symmetryAsymmetric: text('非対称単独', 'asymmetric single'),
    symmetryBilateral: text('左右対称', 'bilateral'),
    symmetryRadial: text('放射対称', 'radial'),
  }) satisfies GenericTargetBindingListText
