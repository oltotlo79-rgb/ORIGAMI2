import type { FormEvent } from 'react'

import {
  normalizeCustomObjectDisplayName,
  updateBeginnerDesignProfile,
  type BeginnerDesignProfileV1,
  type BeginnerRecognitionProposalV1,
  type ProjectSnapshot,
} from './coreClient.ts'
import type { BeginnerNativeEditRunner } from './beginnerWorkflowSupport.ts'
import type { useBeginnerEditorState } from './useBeginnerEditorState.ts'

type EditorState = ReturnType<typeof useBeginnerEditorState>
type Constraints = BeginnerDesignProfileV1['generation_constraints']

export function useBeginnerProfileWorkflow(input: Readonly<{
  getCurrentSnapshot: () => ProjectSnapshot | null
  runNativeEdit: BeginnerNativeEditRunner
  editor: Pick<
    EditorState,
    | 'beginnerBodyOutline'
    | 'beginnerBodyOutlineMode'
    | 'beginnerSkeletonSegments'
    | 'beginnerComponentBridgeOverride'
    | 'beginnerProtrusions'
    | 'beginnerProtrusionKinds'
    | 'beginnerBulgeTargets'
  >
  recognitionProposal: BeginnerRecognitionProposalV1 | null
  silhouetteThresholds: Readonly<{
    alpha: number
    luma: number
    polarity: 'dark_on_light' | 'light_on_dark' | 'alpha_only'
  }>
  silhouetteCropRoi: Constraints['silhouette_crop_roi']
  silhouetteOrientation: 0 | 90 | 180 | 270
  silhouetteMirror: NonNullable<Constraints['silhouette_mirror']>
}>) {
  function submitBeginnerDesignProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = input.getCurrentSnapshot()
    if (!current) return
    const {
      beginnerBodyOutline,
      beginnerBodyOutlineMode,
      beginnerSkeletonSegments,
      beginnerComponentBridgeOverride,
      beginnerProtrusions,
      beginnerProtrusionKinds,
      beginnerBulgeTargets,
    } = input.editor
    const data = new FormData(event.currentTarget)
    const preset = String(data.get('design_preset'))
    const maximumSteps = Number(data.get('maximum_steps'))
    const detailLevel = String(data.get('detail_level'))
    const targetCategory = String(data.get('target_category'))
    const effectiveTargetCategory =
      input.recognitionProposal?.skeleton_quality?.distance_metric
        === 'aabb_squared_distance_v1'
        ? 'custom_object'
        : targetCategory
    const customObjectDisplayName = effectiveTargetCategory === 'custom_object'
      ? normalizeCustomObjectDisplayName(
          String(data.get('custom_object_display_name') ?? ''),
        )
      : null
    const bodyWidthRaw =
      String(data.get('generic_body_width_mm') ?? '').trim()
    const bodyHeightRaw =
      String(data.get('generic_body_height_mm') ?? '').trim()
    const bodySize = bodyWidthRaw === '' && bodyHeightRaw === ''
      ? undefined
      : [
          Math.round(Number(bodyWidthRaw) * 10),
          Math.round(Number(bodyHeightRaw) * 10),
        ] as [number, number]
    const targetUnderlayId = String(data.get('target_reference_underlay'))
    const targetUnderlay = current.underlays?.underlays
      .find((underlay) => underlay.id === targetUnderlayId)
    const formTargetParts = ([
      'head',
      'torso',
      'leg',
      'horn',
      'ear',
      'wing',
      'fin',
      'antenna',
      'tail',
    ] as const).map((kind) => ({
      kind,
      count: Number(data.get(`target_part_${kind}`)),
    })).filter((part) => part.count > 0)
    const targetParts = beginnerProtrusions.length >= 2
      && beginnerProtrusionKinds.length === beginnerProtrusions.length
      ? [
          ...formTargetParts.filter(
            (part) => part.kind === 'head' || part.kind === 'torso',
          ),
          ...beginnerProtrusions.map((target, index) => ({
            kind: beginnerProtrusionKinds[index]!,
            count: target.count,
          })),
        ]
      : formTargetParts
    const allowedTechniques = data.getAll('allowed_techniques').map(String)
    const generationConstraints: Constraints = {
      schema_version: 1,
      maximum_steps: maximumSteps,
      detail_level: detailLevel as Constraints['detail_level'],
      ...(bodySize === undefined
        ? {}
        : { generic_body_size_tenths_mm: bodySize }),
      ...(beginnerBodyOutline.length === 0
        ? {}
        : { generic_body_outline_tenths_mm: beginnerBodyOutline }),
      generic_body_outline_mode: beginnerBodyOutlineMode,
      target_category: effectiveTargetCategory as
        Constraints['target_category'],
      ...(effectiveTargetCategory === 'custom_object'
        && customObjectDisplayName !== null
        ? { custom_object_display_name: customObjectDisplayName }
        : {}),
      target_parts: targetParts,
      skeleton_segments: beginnerSkeletonSegments,
      ...(beginnerComponentBridgeOverride
        ? { component_bridge_override: beginnerComponentBridgeOverride }
        : {}),
      silhouette_thresholds: {
        schema_version: 1,
        ...input.silhouetteThresholds,
      },
      ...(input.silhouetteCropRoi
        ? { silhouette_crop_roi: input.silhouetteCropRoi }
        : {}),
      silhouette_orientation_degrees: input.silhouetteOrientation,
      silhouette_mirror: input.silhouetteMirror,
      protrusions: beginnerProtrusions,
      bulge_targets: beginnerBulgeTargets,
      target_asset: targetUnderlay
        ? {
            kind: 'reference_image',
            underlay_id: targetUnderlay.id,
            asset_id: targetUnderlay.asset,
          }
        : current.beginner_design_profile.generation_constraints
            .target_asset?.kind === 'reference_model'
          ? current.beginner_design_profile.generation_constraints.target_asset
          : null,
      allowed_techniques: allowedTechniques as
        Constraints['allowed_techniques'],
    }
    if (
      !Number.isInteger(maximumSteps)
      || maximumSteps < 1
      || maximumSteps > 500
      || !['simple', 'standard', 'detailed'].includes(detailLevel)
      || !['animal', 'insect', 'custom_object'].includes(targetCategory)
      || (
        effectiveTargetCategory === 'custom_object'
        && customObjectDisplayName === null
      )
      || (
        bodySize !== undefined
        && bodySize.some(
          (axis) => !Number.isInteger(axis) || axis < 1 || axis > 1_000_000,
        )
      )
      || (
        beginnerBodyOutline.length !== 0
        && (
          beginnerBodyOutline.length < 4
          || beginnerBodyOutline.length > 16
        )
      )
      || (targetUnderlayId !== '' && !targetUnderlay)
      || targetParts.some(
        (part) => !Number.isInteger(part.count) || part.count > 8,
      )
      || targetParts.reduce((sum, part) => sum + part.count, 0) > 32
      || allowedTechniques.length < 1
      || allowedTechniques.length > 8
      || new Set(allowedTechniques).size !== allowedTechniques.length
    ) return
    const weights = preset === 'shape_priority'
      ? [60, 20, 10, 10]
      : preset === 'foldability_priority'
        ? [20, 60, 10, 10]
        : preset === 'balanced'
          ? [35, 35, 15, 15]
          : null
    if (!weights) return
    const profile: BeginnerDesignProfileV1 = {
      schema_version: 1,
      preset: preset as BeginnerDesignProfileV1['preset'],
      shape_fidelity_weight: weights[0]!,
      foldability_weight: weights[1]!,
      step_count_weight: weights[2]!,
      paper_efficiency_weight: weights[3]!,
      generation_constraints: generationConstraints,
    }
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => updateBeginnerDesignProfile(
      projectId,
      revision,
      projectInstanceId,
      profile,
    ))
  }

  return { submitBeginnerDesignProfile } as const
}
