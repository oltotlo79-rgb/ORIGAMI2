import { APP_TEXT } from '../lib/appText.ts'
import type { ProjectSnapshot } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type { useBeginnerCandidateWorkflow } from '../lib/useBeginnerCandidateWorkflow.ts'
import type { useBeginnerEditorState } from '../lib/useBeginnerEditorState.ts'
import type { useBeginnerRecognitionWorkflow } from '../lib/useBeginnerRecognitionWorkflow.ts'
import type { useBeginnerReferenceWorkflow } from '../lib/useBeginnerReferenceWorkflow.ts'
import { BeginnerRecognitionPanel } from './BeginnerRecognitionPanel.tsx'
import { BeginnerReferenceAssetPanel } from './BeginnerReferenceAssetPanel.tsx'

type CandidateWorkflow = ReturnType<typeof useBeginnerCandidateWorkflow>
type EditorState = ReturnType<typeof useBeginnerEditorState>
type RecognitionWorkflow = ReturnType<typeof useBeginnerRecognitionWorkflow>
type ReferenceWorkflow = ReturnType<typeof useBeginnerReferenceWorkflow>

export type BeginnerDesignSourcesProps = Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  coreBusy: boolean
  recoveryBlocking: boolean
  candidateWorkflow: CandidateWorkflow
  editor: EditorState
  recognitionWorkflow: RecognitionWorkflow
  referenceWorkflow: ReferenceWorkflow
}>

export function BeginnerDesignSources({
  locale,
  snapshot,
  coreBusy,
  recoveryBlocking,
  candidateWorkflow,
  editor,
  recognitionWorkflow,
  referenceWorkflow,
}: BeginnerDesignSourcesProps) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const constraints = snapshot.beginner_design_profile.generation_constraints

  return (
    <>
      <label className="field">
        <span>{text(APP_TEXT.evaluationPreset)}</span>
        <select
          name="design_preset"
          defaultValue={snapshot.beginner_design_profile.preset}
          disabled={coreBusy || recoveryBlocking}
          aria-describedby="beginner-design-weights"
        >
          <option value="balanced">{text(APP_TEXT.balanced)}</option>
          <option value="shape_priority">
            {text(APP_TEXT.shapeFidelityPriority)}
          </option>
          <option value="foldability_priority">
            {text(APP_TEXT.foldabilityPriority)}
          </option>
        </select>
      </label>
      <p id="beginner-design-weights" className="muted">
        {formattedText(
          APP_TEXT.currentWeightsShapeShapeFoldabilityFoldabilityStepsStepsPaperEfficiency,
          {
            shape: snapshot.beginner_design_profile.shape_fidelity_weight,
            foldability:
              snapshot.beginner_design_profile.foldability_weight,
            steps: snapshot.beginner_design_profile.step_count_weight,
            paper: snapshot.beginner_design_profile.paper_efficiency_weight,
          },
        )}
      </p>
      <label className="field">
        <span>{text(APP_TEXT.targetShapeCategory)}</span>
        <select
          name="target_category"
          required
          defaultValue={constraints.target_category ?? ''}
          disabled={coreBusy || recoveryBlocking}
          aria-describedby="beginner-target-category-help"
        >
          <option value="" disabled>{text(APP_TEXT.selectACategory)}</option>
          <option value="animal">{text(APP_TEXT.animal)}</option>
          <option value="insect">{text(APP_TEXT.insect)}</option>
          <option value="custom_object">{text(APP_TEXT.customObject)}</option>
        </select>
      </label>
      <label className="field">
        <span>{text(APP_TEXT.customObjectDisplayName)}</span>
        <input
          name="custom_object_display_name"
          type="text"
          maxLength={64}
          defaultValue={constraints.custom_object_display_name
            ?? 'Custom object'}
          disabled={coreBusy || recoveryBlocking}
          aria-describedby="beginner-custom-object-name-help"
        />
      </label>
      <p id="beginner-custom-object-name-help" className="muted">
        {text(APP_TEXT.displayMetadataOnlyItDoesNotAffectGeneratorAuthorityOr)}
      </p>
      <p id="beginner-target-category-help" className="muted">
        {text(APP_TEXT.animalAndInsectUseNamedTemplatesCustomObjectIsRouted)}
      </p>
      <label className="field">
        <span>{text(APP_TEXT.referenceImage)}</span>
        <select
          name="target_reference_underlay"
          defaultValue={constraints.target_asset?.kind === 'reference_image'
            ? constraints.target_asset.underlay_id
            : ''}
          disabled={coreBusy || recoveryBlocking}
          aria-describedby="beginner-target-asset-help"
        >
          <option value="">{text(APP_TEXT.none2)}</option>
          {(snapshot.underlays?.underlays ?? []).map((underlay, index) => (
            <option key={underlay.id} value={underlay.id}>
              {formattedText(APP_TEXT.underlayImageIndex, {
                index: index + 1,
              })}
            </option>
          ))}
        </select>
      </label>
      <p id="beginner-target-asset-help" className="muted">
        {text(APP_TEXT.onlyPNGJPEGImagesAlreadyPlacedInThisProjectCan)}
      </p>
      <BeginnerReferenceAssetPanel
        locale={locale}
        snapshot={snapshot}
        coreBusy={coreBusy}
        recoveryBlocking={recoveryBlocking}
        candidateWorkflow={candidateWorkflow}
        editor={editor}
        recognitionWorkflow={recognitionWorkflow}
        referenceWorkflow={referenceWorkflow}
      />
      <BeginnerRecognitionPanel
        locale={locale}
        coreBusy={coreBusy}
        recoveryBlocking={recoveryBlocking}
        workflow={recognitionWorkflow}
      />
    </>
  )
}
