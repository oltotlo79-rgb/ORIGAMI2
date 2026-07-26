import { APP_TEXT } from '../lib/appText.ts'
import {
  foldTechniqueLocalizedTextV1,
  type FoldTechniqueFileDocumentV1,
} from '../lib/foldTechniqueEditor.ts'
import {
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'

export type FoldTechniqueInspectorWorkspace = Readonly<{
  document: FoldTechniqueFileDocumentV1
  dirty: boolean
}>

export type FoldTechniqueInspectorSectionProps = Readonly<{
  locale: Locale
  workspace: FoldTechniqueInspectorWorkspace | null
  selectedIndex: number
  coreBusy: boolean
  fileBusy: boolean
  timelineBusy: boolean
  projectAvailable: boolean
  nativeFileAvailable: boolean
  nativeCoreAvailable: boolean
  onSelectTechnique: (index: number) => void
  onCreate: (opener: HTMLButtonElement) => void
  onImport: (opener: HTMLButtonElement) => void | Promise<void>
  onEdit: (opener: HTMLButtonElement) => void
  onSaveAs: () => void | Promise<void>
  onPreviewTimeline: (opener: HTMLButtonElement) => void
}>

export function FoldTechniqueInspectorSection({
  locale,
  workspace,
  selectedIndex,
  coreBusy,
  fileBusy,
  timelineBusy,
  projectAvailable,
  nativeFileAvailable,
  nativeCoreAvailable,
  onSelectTechnique,
  onCreate,
  onImport,
  onEdit,
  onSaveAs,
  onPreviewTimeline,
}: FoldTechniqueInspectorSectionProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )

  return (
    <section className="fold-technique-workspace">
      <h2>
        {text(APP_TEXT.namedFoldTechniques)}
      </h2>
      <p className="muted">
        {text(APP_TEXT.createAndShareMultipleInstructionStepsAsDeclarativeDataThis)}
      </p>
      {workspace && (
        <>
          <dl>
            <div>
              <dt>{text(APP_TEXT.packageID)}</dt>
              <dd>{workspace.document.package_id}</dd>
            </div>
            <div>
              <dt>{text(APP_TEXT.techniques)}</dt>
              <dd>
                {workspace.document.techniques.length.toLocaleString(locale)}
              </dd>
            </div>
            <div>
              <dt>{text(APP_TEXT.shareState)}</dt>
              <dd>
                {workspace.dirty
                  ? text(APP_TEXT.changedSaveAsRequired)
                  : text(APP_TEXT.saved)}
              </dd>
            </div>
          </dl>
          <label className="dialog-field">
            <span>
              {text(APP_TEXT.techniqueToAddToTimeline)}
            </span>
            <select
              value={selectedIndex}
              disabled={coreBusy || fileBusy || timelineBusy}
              onChange={(event) => {
                const nextIndex = Number(event.currentTarget.value)
                if (
                  Number.isSafeInteger(nextIndex)
                  && nextIndex >= 0
                  && nextIndex < workspace.document.techniques.length
                ) onSelectTechnique(nextIndex)
              }}
            >
              {workspace.document.techniques.map(
                (technique, techniqueIndex) => (
                  <option
                    key={`${technique.id}:${technique.version}`}
                    value={techniqueIndex}
                  >
                    {foldTechniqueLocalizedTextV1(
                      technique.names,
                      locale,
                    ) || foldTechniqueLocalizedTextV1(
                      technique.names,
                      locale === 'ja' ? 'en' : 'ja',
                    ) || technique.id}
                  </option>
                ),
              )}
            </select>
          </label>
        </>
      )}
      <div className="property-actions fold-technique-actions">
        <button
          type="button"
          disabled={coreBusy || fileBusy || !nativeFileAvailable}
          aria-haspopup="dialog"
          onClick={(event) => onCreate(event.currentTarget)}
        >
          {text(APP_TEXT.create)}
        </button>
        <button
          type="button"
          disabled={coreBusy || fileBusy || !nativeFileAvailable}
          aria-haspopup="dialog"
          onClick={(event) => void onImport(event.currentTarget)}
        >
          {text(APP_TEXT.importFile)}
        </button>
        <button
          type="button"
          disabled={coreBusy || fileBusy || !workspace}
          aria-haspopup="dialog"
          onClick={(event) => onEdit(event.currentTarget)}
        >
          {text(APP_TEXT.edit)}
        </button>
        <button
          type="button"
          disabled={
            coreBusy
            || fileBusy
            || !workspace
            || !nativeFileAvailable
          }
          onClick={() => void onSaveAs()}
        >
          {text(APP_TEXT.saveAs)}
        </button>
        <button
          type="button"
          disabled={
            coreBusy
            || fileBusy
            || timelineBusy
            || !workspace
            || !projectAvailable
            || !nativeCoreAvailable
          }
          aria-haspopup="dialog"
          onClick={(event) => onPreviewTimeline(event.currentTarget)}
        >
          {text(APP_TEXT.buildTimelineProposal)}
        </button>
      </div>
      {fileBusy && (
        <p role="status" aria-live="polite">
          {text(APP_TEXT.processingTheFoldTechniqueFile)}
        </p>
      )}
      {!nativeFileAvailable && (
        <p className="muted">
          {text(APP_TEXT.safeFileSelectionAndAtomicSavingAreAvailableInThe)}
        </p>
      )}
    </section>
  )
}
