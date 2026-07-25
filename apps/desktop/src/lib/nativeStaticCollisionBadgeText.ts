import type { LocalizedText } from './i18n.ts'

export const NATIVE_COLLISION_BADGE_TEXT: Readonly<Record<
  | 'ariaLabel'
  | 'retryingAriaLabel'
  | 'retryAriaLabel'
  | 'retrying'
  | 'retry'
  | 'pairClassificationAriaLabel',
  LocalizedText
>> = Object.freeze({
  ariaLabel: Object.freeze({
    ja: 'native厳密衝突判定。{description}',
    en: 'Native exact collision check. {description}',
  }),
  retryingAriaLabel: Object.freeze({
    ja: '厳密衝突判定を再試行中',
    en: 'Retrying exact collision check',
  }),
  retryAriaLabel: Object.freeze({
    ja: '厳密衝突判定を再試行',
    en: 'Retry exact collision check',
  }),
  retrying: Object.freeze({ ja: '再判定中', en: 'Checking again' }),
  retry: Object.freeze({ ja: '再試行', en: 'Retry' }),
  pairClassificationAriaLabel: Object.freeze({
    ja: '面ペアごとの衝突分類',
    en: 'Collision classification for each face pair',
  }),
})
