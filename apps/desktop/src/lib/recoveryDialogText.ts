import {
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

export const RECOVERY_DIALOG_TEXT: Readonly<Record<
  | 'eyebrow' | 'availableTitle' | 'invalidTitle' | 'availableDescription'
  | 'lastUpdated' | 'caution' | 'invalidDescription' | 'actionError'
  | 'restoring' | 'restore' | 'checking' | 'retry' | 'discarding'
  | 'discard' | 'noTimestamp' | 'unavailable',
  LocalizedText
>> = Object.freeze({
  eyebrow: Object.freeze({ ja: '起動時の復旧', en: 'Startup recovery' }),
  availableTitle: Object.freeze({ ja: '未保存の編集内容を復元しますか？', en: 'Restore unsaved edits?' }),
  invalidTitle: Object.freeze({ ja: '復旧データを確認できません', en: 'Recovery data could not be verified' }),
  availableDescription: Object.freeze({
    ja: '前回の終了前に保存できなかった編集内容が見つかりました。復元するか、破棄するかを選んでください。',
    en: 'Edits that could not be saved before the previous session ended were found. Choose whether to restore or discard them.',
  }),
  lastUpdated: Object.freeze({ ja: '最終更新', en: 'Last updated' }),
  caution: Object.freeze({
    ja: '復元後の作品は未保存の新しい編集状態として開きます。元のファイルを自動で上書きすることはありません。',
    en: 'The restored work opens as a new unsaved editing state. The original file is never overwritten automatically.',
  }),
  invalidDescription: Object.freeze({
    ja: '復旧データが破損しているか、このバージョンでは読み取れません。再確認するか、安全に破棄してください。',
    en: 'The recovery data is damaged or cannot be read by this version. Check again or discard it safely.',
  }),
  actionError: Object.freeze({
    ja: '復旧データを処理できませんでした。もう一度お試しください。',
    en: 'The recovery data could not be processed. Try again.',
  }),
  restoring: Object.freeze({ ja: '復元中…', en: 'Restoring…' }),
  restore: Object.freeze({ ja: '復元する', en: 'Restore' }),
  checking: Object.freeze({ ja: '確認中…', en: 'Checking…' }),
  retry: Object.freeze({ ja: '再確認', en: 'Check again' }),
  discarding: Object.freeze({ ja: '破棄中…', en: 'Discarding…' }),
  discard: Object.freeze({ ja: '破棄する', en: 'Discard' }),
  noTimestamp: Object.freeze({ ja: '記録なし', en: 'No record' }),
  unavailable: Object.freeze({ ja: '確認できません', en: 'Unavailable' }),
})

const RECOVERY_TIMESTAMP_LOCALES = Object.freeze({
  ja: 'ja-JP',
  en: 'en-US',
}) satisfies Readonly<Record<Locale, string>>

export function formatRecoveryTimestamp(
  timestamp: number | null,
  locale: Locale,
): string {
  if (timestamp === null) {
    return selectLocalizedText(locale, RECOVERY_DIALOG_TEXT.noTimestamp)
  }
  try {
    const date = new Date(timestamp)
    if (!Number.isFinite(date.getTime())) {
      return selectLocalizedText(locale, RECOVERY_DIALOG_TEXT.unavailable)
    }
    return new Intl.DateTimeFormat(RECOVERY_TIMESTAMP_LOCALES[locale], {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(date)
  } catch {
    return selectLocalizedText(locale, RECOVERY_DIALOG_TEXT.unavailable)
  }
}
