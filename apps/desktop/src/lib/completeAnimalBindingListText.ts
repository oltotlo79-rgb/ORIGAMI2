import type { LocalizedText } from './i18n.ts'

export type CompleteAnimalBindingListText = Readonly<Record<
  | 'ariaLabel'
  | 'fourPartCount'
  | 'fivePartCount'
  | 'bindingRow',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const COMPLETE_ANIMAL_BINDING_LIST_TEXT =
  Object.freeze({
    ariaLabel: text(
      '完全動物の{partCount}部位binding寸法',
      '{partCount} complete-animal binding dimensions',
    ),
    fourPartCount: text('四', 'Four'),
    fivePartCount: text('五', 'Five'),
    bindingRow: text(
      'binding {id}・数 {count}・長さ {length}・厚さ {thickness}',
      'Binding {id} · count {count} · length {length} · thickness {thickness}',
    ),
  }) satisfies CompleteAnimalBindingListText
