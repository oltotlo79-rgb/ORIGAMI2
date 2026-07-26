import type { FormEventHandler } from 'react'

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
import { BeginnerDesignConstraints } from './BeginnerDesignConstraints.tsx'
import { BeginnerDesignSources } from './BeginnerDesignSources.tsx'

type CandidateWorkflow = ReturnType<typeof useBeginnerCandidateWorkflow>
type EditorState = ReturnType<typeof useBeginnerEditorState>
type RecognitionWorkflow = ReturnType<typeof useBeginnerRecognitionWorkflow>
type ReferenceWorkflow = ReturnType<typeof useBeginnerReferenceWorkflow>

export function BeginnerDesignEditorSection({
  locale,
  snapshot,
  coreBusy,
  recoveryBlocking,
  selectedFaceId,
  candidateWorkflow,
  editor,
  recognitionWorkflow,
  referenceWorkflow,
  onSubmit,
}: Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  coreBusy: boolean
  recoveryBlocking: boolean
  selectedFaceId: string | null
  candidateWorkflow: CandidateWorkflow
  editor: EditorState
  recognitionWorkflow: RecognitionWorkflow
  referenceWorkflow: ReferenceWorkflow
  onSubmit: FormEventHandler<HTMLFormElement>
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const constraints = snapshot.beginner_design_profile.generation_constraints
  const genericTree = snapshot.beginner_design_profile
    .generation_provenance?.generic_tree

  return (
    <section
      className="property-section"
      aria-labelledby="beginner-design-heading"
    >
      <h2 id="beginner-design-heading">
        {text(APP_TEXT.beginnerDesignPriorities)}
      </h2>
      <p className="muted">
        {text(APP_TEXT.setsHowFutureOnDeviceDesignCandidatesAreScoredIt)}
      </p>
      <form
        ref={editor.beginnerDesignFormRef}
        key={[
          snapshot.project_instance_id,
          snapshot.beginner_design_profile.preset,
          constraints.maximum_steps,
          constraints.detail_level,
          JSON.stringify(constraints.generic_body_size_tenths_mm),
          JSON.stringify(constraints.generic_body_outline_tenths_mm),
          constraints.generic_body_outline_mode ?? 'symmetric',
          constraints.target_category ?? 'unset',
          JSON.stringify(constraints.target_parts),
          JSON.stringify(constraints.skeleton_segments),
          JSON.stringify(constraints.protrusions),
          JSON.stringify(constraints.bulge_targets),
          JSON.stringify(constraints.target_asset),
          constraints.allowed_techniques.join(','),
        ].join(':')}
        onSubmit={onSubmit}
      >
        {snapshot.beginner_design_profile.outline_edit_authority && (
          <p role="status">
            {formattedText(
              APP_TEXT.savedOutlineEditAuthorityCountEditsImageDigestDigest,
              {
                count: snapshot.beginner_design_profile
                  .outline_edit_authority.edits.length,
                digest: snapshot.beginner_design_profile
                  .outline_edit_authority.source_sha256.slice(0, 4)
                  .map((byte) => byte.toString(16).padStart(2, '0'))
                  .join(''),
              },
            )}
          </p>
        )}
        {genericTree && (
          <div role="status">
            <p>{formattedText(
              APP_TEXT.savedGenericTreeNameOriginSourceOrientationOrientationGeneratorV,
              {
                name: constraints.custom_object_display_name
                  ?? 'Custom object',
                source: genericTree.source,
                orientation: genericTree.orientation,
                version: genericTree.generator_version,
              },
            )}</p>
            {genericTree.instruction_proposal && (
              <ol aria-label={text(APP_TEXT.readOnlyFoldingInstructionProposal)}>
                {genericTree.instruction_proposal.steps.map((step) => (
                  <li key={step.canonical_crease_id}>
                    {step.canonical_crease_id} · depth {step.tree_depth}
                    {' · '}{step.assignment} · {step.target_branch}
                    {' · fixed '}{step.fixed_side}
                    <br />{step.caution}
                  </li>
                ))}
              </ol>
            )}
            {genericTree.instruction_proposal && (
              <button
                type="button"
                onClick={
                  candidateWorkflow.confirmAndAppendGenericTreeInstructions
                }
              >
                {text(APP_TEXT.confirmAndAppendToInstructions)}
              </button>
            )}
          </div>
        )}
        <BeginnerDesignSources
          locale={locale}
          snapshot={snapshot}
          coreBusy={coreBusy}
          recoveryBlocking={recoveryBlocking}
          candidateWorkflow={candidateWorkflow}
          editor={editor}
          recognitionWorkflow={recognitionWorkflow}
          referenceWorkflow={referenceWorkflow}
        />
        <BeginnerDesignConstraints
          locale={locale}
          snapshot={snapshot}
          coreBusy={coreBusy}
          recoveryBlocking={recoveryBlocking}
          selectedFaceId={selectedFaceId}
          editor={editor}
        />
      </form>
    </section>
  )
}
