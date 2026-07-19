import type { RecoveryAutosaveMonitorView } from '../lib/recoveryAutosaveStatusClient.ts'
import {
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n.ts'

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

type RecoveryAutosaveStatusBannerProps = Readonly<{
  view: RecoveryAutosaveMonitorView
  localeStore?: LocaleStore
}>

export function RecoveryAutosaveStatusBanner({
  view,
  localeStore: localeStore_ = localeStore,
}: RecoveryAutosaveStatusBannerProps) {
  const locale = useLocale(localeStore_)
  if (view.kind === 'persistence_failed') {
    return (
      <aside
        className="recovery-autosave-warning is-persistence-failed"
        role="alert"
        aria-live="assertive"
        aria-atomic="true"
      >
        {selectLocalizedText(locale, RECOVERY_AUTOSAVE_TEXT.persistence)}
      </aside>
    )
  }

  if (view.kind === 'monitor_unavailable') {
    return (
      <aside
        className="recovery-autosave-warning is-monitor-unavailable"
        role="alert"
        aria-live="assertive"
        aria-atomic="true"
      >
        {selectLocalizedText(locale, RECOVERY_AUTOSAVE_TEXT.monitor)}
      </aside>
    )
  }

  if (view.kind === 'operational' && view.recovered) {
    return (
      <p
        className="visually-hidden"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        {selectLocalizedText(locale, RECOVERY_AUTOSAVE_TEXT.recovered)}
      </p>
    )
  }

  return null
}

const RECOVERY_AUTOSAVE_TEXT = Object.freeze({
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
