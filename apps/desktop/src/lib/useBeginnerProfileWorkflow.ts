import type { FormEvent } from 'react'

import {
  beginnerTargetPartRecordCountIsAdmissibleV1,
  normalizeCustomObjectDisplayName,
  updateBeginnerDesignProfile,
  type BeginnerDesignProfileV1,
  type BeginnerRecognitionProposalV1,
  type ProjectSnapshot,
} from './coreClient.ts'
import {
  beginnerSemanticFamilyV1,
  beginnerSemanticPhysicalProfileIsAdmissibleV1,
  beginnerTargetPartsHaveSameSemanticsV1,
  type BeginnerProtrusionKindAssignmentV1,
} from './beginnerProtrusionKinds.ts'
import type { BeginnerNativeEditRunner } from './beginnerWorkflowSupport.ts'
import type { useBeginnerEditorState } from './useBeginnerEditorState.ts'

type EditorState = ReturnType<typeof useBeginnerEditorState>
type Constraints = BeginnerDesignProfileV1['generation_constraints']
type TargetPart = Constraints['target_parts'][number]
type Protrusion = NonNullable<Constraints['protrusions']>[number]

const TARGET_PART_KIND_ORDER_V1 = [
  'head',
  'torso',
  'leg',
  'horn',
  'ear',
  'wing',
  'fin',
  'antenna',
  'tail',
] as const satisfies readonly TargetPart['kind'][]

export function selectBeginnerTargetPartsForProfileV1(
  formTargetParts: readonly TargetPart[],
  beginnerProtrusions: readonly Protrusion[],
  beginnerProtrusionKinds:
    readonly BeginnerProtrusionKindAssignmentV1[],
  targetCategory: Constraints['target_category'],
  existingTargetParts?: readonly TargetPart[],
  existingProtrusions?: readonly Protrusion[],
): Constraints['target_parts'] | null {
  if (!beginnerTargetPartRecordCountIsAdmissibleV1(formTargetParts)
    || new Set(formTargetParts.map((part) => part.kind)).size
      !== formTargetParts.length
    || formTargetParts.some((part) => (
      !Number.isInteger(part.count) || part.count < 1 || part.count > 8
    ))
    || formTargetParts.reduce((sum, part) => sum + part.count, 0) > 32) {
    return null
  }
  const baselineIsExact = existingTargetParts !== undefined
    && existingProtrusions !== undefined
    && beginnerTargetPartsHaveSameSemanticsV1(
      formTargetParts,
      existingTargetParts,
    )
    && serializedProfileFieldIsEqualV1(
      beginnerProtrusions,
      existingProtrusions,
    )
    && beginnerSemanticPhysicalProfileIsAdmissibleV1(
      existingTargetParts,
      existingProtrusions,
      targetCategory,
    )
  if (baselineIsExact) {
    return existingTargetParts.map((part) => ({ ...part }))
  }

  const requestedFamily = beginnerSemanticFamilyV1(
    formTargetParts,
    targetCategory,
  )
  if (
    requestedFamily === 'specialized'
    || requestedFamily === 'custom_direct'
    || (requestedFamily === 'empty' && beginnerProtrusions.length === 0)
  ) {
    if (!beginnerSemanticPhysicalProfileIsAdmissibleV1(
      formTargetParts,
      beginnerProtrusions,
      targetCategory,
    )) return null
    return existingTargetParts
      && beginnerTargetPartsHaveSameSemanticsV1(
        formTargetParts,
        existingTargetParts,
      )
      ? existingTargetParts.map((part) => ({ ...part }))
      : formTargetParts.map((part) => ({ ...part }))
  }
  if (requestedFamily === 'generic' && beginnerProtrusions.length === 0) {
    // Native profile storage permits a bounded semantic-only general target.
    // Physical endpoint equality is required once protrusion evidence exists,
    // but must not discard a copied recognition proposal before that evidence
    // has been authored.
    return existingTargetParts
      && beginnerTargetPartsHaveSameSemanticsV1(
        formTargetParts,
        existingTargetParts,
      )
      ? existingTargetParts.map((part) => ({ ...part }))
      : formTargetParts.map((part) => ({ ...part }))
  }
  if (requestedFamily !== 'generic' && requestedFamily !== 'empty') return null

  if (beginnerProtrusions.length > 0
    && beginnerProtrusionKinds.length !== beginnerProtrusions.length) {
    return null
  }
  const countsByKind = new Map<TargetPart['kind'], number>()
  const addCount = (kind: TargetPart['kind'], count: number): boolean => {
    if (!TARGET_PART_KIND_ORDER_V1.includes(kind)
      || !Number.isInteger(count) || count < 1 || count > 8) return false
    const aggregate = (countsByKind.get(kind) ?? 0) + count
    if (!Number.isSafeInteger(aggregate) || aggregate > 8) return false
    countsByKind.set(kind, aggregate)
    return true
  }
  const formPartsToKeep = beginnerProtrusions.length > 0
    ? formTargetParts.filter(
        (part) => part.kind === 'head' || part.kind === 'torso',
      )
    : formTargetParts
  if (!formPartsToKeep.every((part) => addCount(part.kind, part.count))) {
    return null
  }
  const physicalKindOrder: TargetPart['kind'][] = []
  const closedPhysicalKinds = new Set<TargetPart['kind']>()
  let activePhysicalKind: TargetPart['kind'] | undefined
  for (const [index, target] of beginnerProtrusions.entries()) {
    const kind = beginnerProtrusionKinds[index]
    if (kind === null || kind === undefined) return null
    if (kind !== activePhysicalKind) {
      if (closedPhysicalKinds.has(kind)) return null
      if (activePhysicalKind !== undefined) {
        closedPhysicalKinds.add(activePhysicalKind)
      }
      physicalKindOrder.push(kind)
      activePhysicalKind = kind
    }
    if (!addCount(kind, target.count)) return null
  }
  if (targetCategory !== 'custom_object'
    && (countsByKind.get('head') !== 1
      || countsByKind.get('torso') !== 1)) return null
  const compactKindOrder = beginnerProtrusions.length === 0
    ? TARGET_PART_KIND_ORDER_V1
    : [
        'head' as const,
        'torso' as const,
        ...physicalKindOrder,
      ]
  const compact = compactKindOrder.flatMap((kind) => {
    const count = countsByKind.get(kind)
    return count === undefined ? [] : [{ kind, count }]
  })
  if (!beginnerTargetPartRecordCountIsAdmissibleV1(compact)
    || compact.reduce((sum, part) => sum + part.count, 0) > 32) return null
  const compactFamily = beginnerSemanticFamilyV1(
    compact,
    targetCategory,
  )
  if (
    (compactFamily !== 'generic' && compactFamily !== 'specialized')
    || !beginnerSemanticPhysicalProfileIsAdmissibleV1(
      compact,
      beginnerProtrusions,
      targetCategory,
    )
  ) return null
  const preservesExistingOrder = existingTargetParts !== undefined
    && existingProtrusions !== undefined
    && serializedProfileFieldIsEqualV1(
      beginnerProtrusions,
      existingProtrusions,
    )
    && beginnerTargetPartsHaveSameSemanticsV1(
      compact,
      existingTargetParts,
    )
  return preservesExistingOrder
    ? existingTargetParts.map((part) => ({ ...part }))
    : compact
}

function serializedProfileFieldIsEqualV1(
  left: unknown,
  right: unknown,
): boolean {
  try {
    return JSON.stringify(left) === JSON.stringify(right)
  } catch {
    return false
  }
}

/**
 * Keeps project bookkeeping and still-live, non-form evidence while ensuring
 * a form edit cannot silently retain authority for different constraints.
 * Generated fold/proof provenance is intentionally never copied.
 */
export function selectBeginnerNonFormProfileFieldsForSubmitV1(
  current: ProjectSnapshot,
  nextConstraints: Constraints,
): Pick<
  BeginnerDesignProfileV1,
  'archived_reference_model_asset_ids'
> & Partial<Pick<
  BeginnerDesignProfileV1,
  | 'reference_surface_landmarks_tenths_mm'
  | 'outline_edit_authority'
  | 'reference_consensus_v1'
>> {
  const profile = current.beginner_design_profile
  const currentConstraints = profile.generation_constraints
  const constraintsUnchanged = serializedProfileFieldIsEqualV1(
    currentConstraints,
    nextConstraints,
  )
  const archived = new Set(
    profile.archived_reference_model_asset_ids ?? [],
  )
  const currentTarget = currentConstraints.target_asset
  const nextTarget = nextConstraints.target_asset
  const sameLiveReferenceModel = currentTarget?.kind === 'reference_model'
    && nextTarget?.kind === 'reference_model'
    && currentTarget.asset_id === nextTarget.asset_id
    && !archived.has(currentTarget.asset_id)
    && (current.reference_model_assets ?? []).some(
      (asset) => asset.asset_id === currentTarget.asset_id,
    )
  const sameLiveReferenceImage = currentTarget?.kind === 'reference_image'
    && nextTarget?.kind === 'reference_image'
    && currentTarget.asset_id === nextTarget.asset_id
    && currentTarget.underlay_id === nextTarget.underlay_id
    && (current.underlays?.underlays ?? []).some((underlay) =>
      underlay.id === currentTarget.underlay_id
      && underlay.asset === currentTarget.asset_id)
  const outlineAuthority = profile.outline_edit_authority
  const preserveOutlineAuthority = constraintsUnchanged
    && sameLiveReferenceImage
    && outlineAuthority?.source_asset_id === currentTarget.asset_id
  const consensus = profile.reference_consensus_v1
  const consensusBindingsAreLive = consensus?.bindings.every((binding) =>
    binding.kind === 'image'
      ? (current.underlays?.underlays ?? []).some(
          (underlay) => underlay.asset === binding.asset_id,
        )
      : !archived.has(binding.asset_id)
        && (current.reference_model_assets ?? []).some(
          (asset) => asset.asset_id === binding.asset_id,
        )) ?? false
  return {
    archived_reference_model_asset_ids: [
      ...(profile.archived_reference_model_asset_ids ?? []),
    ],
    ...(sameLiveReferenceModel
      && profile.reference_surface_landmarks_tenths_mm
      ? {
          reference_surface_landmarks_tenths_mm:
            profile.reference_surface_landmarks_tenths_mm.map(
              (landmark) => [...landmark] as [number, number, number],
            ),
        }
      : {}),
    ...(preserveOutlineAuthority && outlineAuthority
      ? {
          outline_edit_authority: {
            ...outlineAuthority,
            source_sha256: [...outlineAuthority.source_sha256],
            edits: outlineAuthority.edits.map((edit) => ({ ...edit })),
          },
        }
      : {}),
    ...(constraintsUnchanged && consensusBindingsAreLive && consensus
      ? {
          reference_consensus_v1: {
            ...consensus,
            bindings: consensus.bindings.map((binding) => ({
              ...binding,
              sha256: [...binding.sha256],
            })),
          },
        }
      : {}),
  }
}

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
    const targetParts = selectBeginnerTargetPartsForProfileV1(
      formTargetParts,
      beginnerProtrusions,
      beginnerProtrusionKinds,
      effectiveTargetCategory as Constraints['target_category'],
      current.beginner_design_profile.generation_constraints.target_parts,
      current.beginner_design_profile.generation_constraints.protrusions,
    )
    if (targetParts === null) return
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
      || !beginnerTargetPartRecordCountIsAdmissibleV1(targetParts)
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
      ...selectBeginnerNonFormProfileFieldsForSubmitV1(
        current,
        generationConstraints,
      ),
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
