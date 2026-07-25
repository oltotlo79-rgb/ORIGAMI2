import type { LocalizedText } from './i18n.ts'

export const RECOVERY_STARTUP_TEXT: Readonly<Record<
  | 'eyebrow'
  | 'checkingTitle'
  | 'failedTitle'
  | 'checkingDescription'
  | 'failedDescription'
  | 'retrying'
  | 'retry',
  LocalizedText
>> = Object.freeze({
  eyebrow: Object.freeze({ ja: '起動時の復旧', en: 'Startup recovery' }),
  checkingTitle: Object.freeze({
    ja: '復旧データを確認しています',
    en: 'Checking recovery data',
  }),
  failedTitle: Object.freeze({
    ja: '復旧データを確認できません',
    en: 'Recovery data could not be checked',
  }),
  checkingDescription: Object.freeze({
    ja: '編集を安全に開始できるか確認しています。しばらくお待ちください。',
    en: 'Checking whether editing can start safely. Please wait.',
  }),
  failedDescription: Object.freeze({
    ja: '編集を開始する前に復旧データの確認が必要です。再試行してください。',
    en: 'Recovery data must be checked before editing can begin. Try again.',
  }),
  retrying: Object.freeze({ ja: '再確認中…', en: 'Checking again…' }),
  retry: Object.freeze({ ja: '再試行', en: 'Try again' }),
})
