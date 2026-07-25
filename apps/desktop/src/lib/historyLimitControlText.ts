import type { LocalizedText } from './i18n.ts'

export const HISTORY_LIMIT_CONTROL_TEXT: Readonly<Record<
  | 'invalidValueError' | 'applyError' | 'legend' | 'currentLimit'
  | 'currentLimitAriaLabel' | 'entryCount' | 'unavailable' | 'inputLabel'
  | 'applying' | 'apply' | 'description',
  LocalizedText
>> = Object.freeze({
  invalidValueError: Object.freeze({
    ja: '履歴件数は1から128までの整数で入力してください。',
    en: 'Enter a whole-number history limit from 1 to 128.',
  }),
  applyError: Object.freeze({
    ja: '履歴件数を変更できませんでした。現在のプロジェクトを確認して、もう一度お試しください。',
    en: 'The history limit could not be changed. Check the current project and try again.',
  }),
  legend: Object.freeze({ ja: 'Undo・Redo履歴の上限', en: 'Undo/Redo history limit' }),
  currentLimit: Object.freeze({ ja: '現在の上限:', en: 'Current limit:' }),
  currentLimitAriaLabel: Object.freeze({
    ja: '現在の履歴件数上限',
    en: 'Current history entry limit',
  }),
  entryCount: Object.freeze({ ja: '{count}件', en: '{count} entries' }),
  unavailable: Object.freeze({ ja: '確認できません', en: 'Unavailable' }),
  inputLabel: Object.freeze({ ja: '履歴件数の上限', en: 'History entry limit' }),
  applying: Object.freeze({ ja: '適用中…', en: 'Applying…' }),
  apply: Object.freeze({ ja: '適用', en: 'Apply' }),
  description: Object.freeze({
    ja: '上限を減らすと、古いUndo/Redo履歴は直ちに削除されます。あとで上限を増やしても、削除された履歴は戻りません。',
    en: 'Reducing the limit immediately removes older Undo/Redo entries. Increasing it later does not restore removed history.',
  }),
})
