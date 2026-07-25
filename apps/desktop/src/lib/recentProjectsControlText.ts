import type { LocalizedText } from './i18n.ts'

export type RecentProjectsControlText = Readonly<Record<
  | 'title'
  | 'empty'
  | 'listUnavailable'
  | 'invalidated'
  | 'openFailed',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const RECENT_PROJECTS_CONTROL_TEXT =
  Object.freeze({
    title: text(
      '最近使った作品',
      'Recent projects',
    ),
    empty: text(
      '履歴はありません。',
      'No recent projects.',
    ),
    listUnavailable: text(
      '最近使った作品を確認できません。',
      'Recent projects are unavailable.',
    ),
    invalidated: text(
      '作品が移動または置換されたため一覧から削除しました。',
      'The project moved or was replaced and was removed.',
    ),
    openFailed: text(
      '作品を安全に開けませんでした。',
      'The project could not be opened safely.',
    ),
  }) satisfies RecentProjectsControlText
