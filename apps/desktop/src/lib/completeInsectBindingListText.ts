import type { LocalizedText } from './i18n.ts'

export type CompleteInsectBindingListText = Readonly<Record<
  | 'listAriaLabel'
  | 'wingPair'
  | 'antennaPair'
  | 'legPair1'
  | 'legPair2'
  | 'legPair3'
  | 'bindingRow',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const COMPLETE_INSECT_BINDING_LIST_TEXT =
  Object.freeze({
    listAriaLabel: text(
      '完全昆虫の五組binding寸法',
      'Five complete-insect binding dimensions',
    ),
    wingPair: text(
      '翼の組',
      'Wing pair',
    ),
    antennaPair: text(
      '触角の組',
      'Antenna pair',
    ),
    legPair1: text(
      '脚の組1',
      'Leg pair 1',
    ),
    legPair2: text(
      '脚の組2',
      'Leg pair 2',
    ),
    legPair3: text(
      '脚の組3',
      'Leg pair 3',
    ),
    bindingRow: text(
      '{label}・binding {bindingId}・長さ {length}・厚さ {thickness}',
      '{label} · binding {bindingId} · length {length} · thickness {thickness}',
    ),
  }) satisfies CompleteInsectBindingListText
