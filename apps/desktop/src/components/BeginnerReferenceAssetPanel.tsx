import { APP_TEXT } from '../lib/appText.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import {
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type { useBeginnerCandidateWorkflow } from '../lib/useBeginnerCandidateWorkflow.ts'
import type { useBeginnerEditorState } from '../lib/useBeginnerEditorState.ts'
import type { useBeginnerRecognitionWorkflow } from '../lib/useBeginnerRecognitionWorkflow.ts'
import type { useBeginnerReferenceWorkflow } from '../lib/useBeginnerReferenceWorkflow.ts'
import { BeginnerReferenceSuggestionPanel } from './BeginnerReferenceSuggestionPanel.tsx'

type CandidateWorkflow = ReturnType<typeof useBeginnerCandidateWorkflow>
type EditorState = ReturnType<typeof useBeginnerEditorState>
type RecognitionWorkflow = ReturnType<typeof useBeginnerRecognitionWorkflow>
type ReferenceWorkflow = ReturnType<typeof useBeginnerReferenceWorkflow>

export function BeginnerReferenceAssetPanel({
  locale,
  snapshot,
  coreBusy,
  recoveryBlocking,
  candidateWorkflow,
  editor,
  recognitionWorkflow,
  referenceWorkflow,
}: Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  coreBusy: boolean
  recoveryBlocking: boolean
  candidateWorkflow: CandidateWorkflow
  editor: EditorState
  recognitionWorkflow: RecognitionWorkflow
  referenceWorkflow: ReferenceWorkflow
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const {
    consensusSelectionDraft,
    toggleConsensusReference,
    saveConsensusReferences,
  } = candidateWorkflow
  const {
    beginnerRecognitionBusy,
    invalidateBeginnerRecognition,
  } = recognitionWorkflow
  const {
    beginnerReferenceGeometry,
    requestBeginnerReferenceModelImport,
    activateBeginnerReferenceAsset,
    archiveBeginnerReferenceAsset,
    toggleBeginnerReferenceModelPreview,
    requestBeginnerReferenceSuggestion,
  } = referenceWorkflow
  const constraints = snapshot.beginner_design_profile.generation_constraints

  const consensusAssets = [
    ...(snapshot.underlays?.underlays ?? []).map((underlay, index) => ({
      kind: 'image' as const,
      asset_id: underlay.asset,
      label: `Underlay image ${index + 1} (image)`,
    })),
    ...(snapshot.reference_model_assets ?? []).map((asset, index) => ({
      kind: 'reference_model' as const,
      asset_id: asset.asset_id,
      label: `3D reference ${index + 1} (GLB)`,
    })),
  ].filter(
    (asset, index, all) => all.findIndex(
      (candidate) => candidate.asset_id === asset.asset_id,
    ) === index,
  )

  return (
    <div aria-live="polite">
      <button
        type="button"
        onClick={requestBeginnerReferenceModelImport}
        disabled={coreBusy || recoveryBlocking}
        aria-describedby="beginner-reference-model-help"
      >
        {text(APP_TEXT.import3DReferenceModel)}
      </button>
      {beginnerRecognitionBusy && (
        <button type="button" onClick={invalidateBeginnerRecognition}>
          {text(APP_TEXT.cancelImageRecognition)}
        </button>
      )}
      <p id="beginner-reference-model-help" className="muted">
        {text(APP_TEXT.aGLB20ModelIsAReadOnlyVisual)}
      </p>
      <fieldset aria-describedby="reference-consensus-selection-help">
        <legend>References for consensus</legend>
        <p id="reference-consensus-selection-help" className="muted">
          Select two to four project references. Content hashes are read only
          by the native core.
        </p>
        {consensusAssets.map((asset) => {
          const checked = consensusSelectionDraft.some(
            (selection) => selection.asset_id === asset.asset_id,
          )
          return (
            <label key={asset.asset_id}>
              <input
                type="checkbox"
                checked={checked}
                disabled={!checked && consensusSelectionDraft.length >= 4}
                onChange={() => toggleConsensusReference(
                  asset.kind,
                  asset.asset_id,
                )}
              />
              {asset.label}
            </label>
          )
        })}
        <p role="status" aria-live="polite">
          {`${consensusSelectionDraft.length} of 2–4 references selected.`}
        </p>
        <button
          type="button"
          disabled={
            consensusSelectionDraft.length < 2
            || consensusSelectionDraft.length > 4
            || coreBusy
            || recoveryBlocking
          }
          onClick={saveConsensusReferences}
        >
          Save consensus references
        </button>
      </fieldset>
      {(snapshot.reference_model_assets ?? []).length > 0 && (
        <ul aria-label={text(APP_TEXT.project3DReferenceAssets)}>
          {(snapshot.reference_model_assets ?? []).map((asset, index) => {
            const active = constraints.target_asset?.kind
              === 'reference_model'
              && constraints.target_asset.asset_id === asset.asset_id
            const archived = snapshot.beginner_design_profile
              .archived_reference_model_asset_ids?.includes(
                asset.asset_id,
              ) ?? false
            return (
              <li key={asset.asset_id}>
                {`GLB ${index + 1} · SHA-256 ${asset.sha256.slice(0, 4)
                  .map((byte) => byte.toString(16).padStart(2, '0'))
                  .join('')}`}
                {active ? (
                  <span> · Active reference</span>
                ) : !archived && (
                  <button
                    type="button"
                    onClick={() => activateBeginnerReferenceAsset(
                      asset.asset_id,
                    )}
                  >
                    Activate this reference
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => archiveBeginnerReferenceAsset(
                    asset.asset_id,
                    !archived,
                  )}
                >
                  {archived
                    ? 'Restore archived reference'
                    : 'Archive reference without deleting bytes'}
                </button>
              </li>
            )
          })}
        </ul>
      )}
      {constraints.target_asset?.kind === 'reference_model' && (
        <>
          <p role="status">
            {text(APP_TEXT.aValidated3DReferenceModelIsAttached)}
          </p>
          <button type="button" onClick={toggleBeginnerReferenceModelPreview}>
            {beginnerReferenceGeometry
              ? text(APP_TEXT.hide3DReferencePreview)
              : text(APP_TEXT.show3DReferencePreview)}
          </button>
          <button
            type="button"
            onClick={requestBeginnerReferenceSuggestion}
            disabled={coreBusy || recoveryBlocking}
          >
            {text(APP_TEXT.suggestRangesFromSafeGeometryFeatures)}
          </button>
          <BeginnerReferenceSuggestionPanel
            locale={locale}
            editor={editor}
            workflow={referenceWorkflow}
          />
        </>
      )}
    </div>
  )
}
