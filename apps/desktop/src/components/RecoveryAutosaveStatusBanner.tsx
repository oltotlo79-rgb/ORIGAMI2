import type { RecoveryAutosaveMonitorView } from '../lib/recoveryAutosaveStatusClient.ts'
import {
  localeStore,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
} from '../lib/i18n.ts'
import { RECOVERY_AUTOSAVE_STATUS_TEXT } from '../lib/recoveryAutosaveStatusText.ts'
export {
  RECOVERY_AUTOSAVE_MONITOR_WARNING,
  RECOVERY_AUTOSAVE_MONITOR_WARNING_EN,
  RECOVERY_AUTOSAVE_PERSISTENCE_WARNING,
  RECOVERY_AUTOSAVE_PERSISTENCE_WARNING_EN,
  RECOVERY_AUTOSAVE_RECOVERED_NOTICE,
  RECOVERY_AUTOSAVE_RECOVERED_NOTICE_EN,
} from '../lib/recoveryAutosaveStatusText.ts'

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
        {selectLocalizedText(locale, RECOVERY_AUTOSAVE_STATUS_TEXT.persistence)}
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
        {selectLocalizedText(locale, RECOVERY_AUTOSAVE_STATUS_TEXT.monitor)}
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
        {selectLocalizedText(locale, RECOVERY_AUTOSAVE_STATUS_TEXT.recovered)}
      </p>
    )
  }

  return null
}
