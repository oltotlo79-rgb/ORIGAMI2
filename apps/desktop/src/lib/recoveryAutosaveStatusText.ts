import type { LocalizedText } from './i18n.ts'

export const RECOVERY_AUTOSAVE_PERSISTENCE_WARNING =
  '自動保存を更新できません。通常の保存を行ってください。自動保存は自動的に再試行されます。'
export const RECOVERY_AUTOSAVE_MONITOR_WARNING =
  '自動保存の状態を確認できません。通常の保存を行ってください。'
export const RECOVERY_AUTOSAVE_RECOVERED_NOTICE =
  '自動保存が再開しました。'
export const RECOVERY_AUTOSAVE_PERSISTENCE_WARNING_EN =
  'Autosave could not be updated. Save normally. Autosave will retry automatically.'
export const RECOVERY_AUTOSAVE_MONITOR_WARNING_EN =
  'Autosave status could not be checked. Save normally.'
export const RECOVERY_AUTOSAVE_RECOVERED_NOTICE_EN =
  'Autosave has resumed.'

export const RECOVERY_AUTOSAVE_STATUS_TEXT: Readonly<Record<
  'persistence' | 'monitor' | 'recovered',
  LocalizedText
>> = Object.freeze({
  persistence: Object.freeze({
    ja: RECOVERY_AUTOSAVE_PERSISTENCE_WARNING,
    en: RECOVERY_AUTOSAVE_PERSISTENCE_WARNING_EN,
  }),
  monitor: Object.freeze({
    ja: RECOVERY_AUTOSAVE_MONITOR_WARNING,
    en: RECOVERY_AUTOSAVE_MONITOR_WARNING_EN,
  }),
  recovered: Object.freeze({
    ja: RECOVERY_AUTOSAVE_RECOVERED_NOTICE,
    en: RECOVERY_AUTOSAVE_RECOVERED_NOTICE_EN,
  }),
})
