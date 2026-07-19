import type { RecoveryAutosaveMonitorView } from '../lib/recoveryAutosaveStatusClient.ts'

export const RECOVERY_AUTOSAVE_PERSISTENCE_WARNING =
  '自動保存を更新できません。通常の保存を行ってください。自動保存は自動的に再試行されます。'
export const RECOVERY_AUTOSAVE_MONITOR_WARNING =
  '自動保存の状態を確認できません。通常の保存を行ってください。'
export const RECOVERY_AUTOSAVE_RECOVERED_NOTICE =
  '自動保存が再開しました。'

type RecoveryAutosaveStatusBannerProps = Readonly<{
  view: RecoveryAutosaveMonitorView
}>

export function RecoveryAutosaveStatusBanner({
  view,
}: RecoveryAutosaveStatusBannerProps) {
  if (view.kind === 'persistence_failed') {
    return (
      <aside
        className="recovery-autosave-warning is-persistence-failed"
        role="alert"
        aria-live="assertive"
        aria-atomic="true"
      >
        {RECOVERY_AUTOSAVE_PERSISTENCE_WARNING}
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
        {RECOVERY_AUTOSAVE_MONITOR_WARNING}
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
        {RECOVERY_AUTOSAVE_RECOVERED_NOTICE}
      </p>
    )
  }

  return null
}
