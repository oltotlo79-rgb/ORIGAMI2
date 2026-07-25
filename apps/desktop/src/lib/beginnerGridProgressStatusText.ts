import type { LocalizedText } from './i18n.ts'

export type BeginnerGridProgressStatusText = Readonly<Record<
  'groupAriaLabel' | 'cancel' | 'progress',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const BEGINNER_GRID_PROGRESS_STATUS_TEXT =
  Object.freeze({
    groupAriaLabel: text(
      '候補生成と局所改善の進捗',
      'Candidate generation and local refinement progress',
    ),
    cancel: text(
      '候補生成をキャンセル',
      'Cancel candidate generation',
    ),
    progress: text(
      '列挙 {enumerated}/27・局所改善 {refined}/24・大域検証 {checked}/3',
      'Enumerated {enumerated}/27 · refined {refined}/24 · globally checked {checked}/3',
    ),
  }) satisfies BeginnerGridProgressStatusText
