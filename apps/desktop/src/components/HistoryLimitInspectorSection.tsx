import { APP_TEXT } from '../lib/appText.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import type { HistoryLimitSettings } from '../lib/historyLimitClient.ts'
import {
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import { HistoryLimitControl } from './HistoryLimitControl.tsx'

export type HistoryLimitInspectorLoadState =
  | Readonly<{ kind: 'unavailable' }>
  | Readonly<{ kind: 'loading' }>
  | Readonly<{ kind: 'failed' }>
  | Readonly<{ kind: 'ready'; settings: HistoryLimitSettings }>

export type HistoryLimitInspectorSectionProps = Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot | null
  settings: HistoryLimitSettings | null
  loadState: HistoryLimitInspectorLoadState
  disabled: boolean
  onApplied: (settings: HistoryLimitSettings) => void | Promise<void>
  onRetry: () => void
}>

export function HistoryLimitInspectorSection({
  locale,
  snapshot,
  settings,
  loadState,
  disabled,
  onApplied,
  onRetry,
}: HistoryLimitInspectorSectionProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )

  return (
    <section>
      <h2>{text(APP_TEXT.editHistory)}</h2>
      {settings && snapshot ? (
        <HistoryLimitControl
          settings={settings}
          expectedProjectInstanceId={snapshot.project_instance_id}
          expectedProjectId={snapshot.project_id}
          expectedRevision={snapshot.revision}
          disabled={disabled}
          onApplied={onApplied}
        />
      ) : loadState.kind === 'failed' ? (
        <div role="alert">
          <p>
            {text(APP_TEXT.theUndoRedoHistoryLimitCouldNotBeChecked)}
          </p>
          <button
            type="button"
            disabled={disabled}
            onClick={onRetry}
          >
            {text(APP_TEXT.retry)}
          </button>
        </div>
      ) : loadState.kind === 'unavailable' ? (
        <p className="muted">
          {text(APP_TEXT.historyLimitSettingsAreAvailableInTheDesktopApp)}
        </p>
      ) : (
        <p className="muted" role="status" aria-live="polite">
          {text(APP_TEXT.checkingHistoryLimit)}
        </p>
      )}
    </section>
  )
}
