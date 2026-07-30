import {
  useEffect,
  useRef,
  useState,
} from 'react'

import {
  activateBeginnerReferenceModelAsset,
  applyBeginnerReferenceModelFeatures,
  archiveBeginnerReferenceModelAsset,
  getBeginnerReferenceModelGeometry,
  importBeginnerReferenceModel,
  suggestBeginnerReferenceModelFeatures,
  type BeginnerReferenceModelGeometry,
  type BeginnerReferenceModelSuggestionV1,
  type ProjectSnapshot,
} from './coreClient.ts'
import { resolveBeginnerProtrusionKindsV1 } from './beginnerProtrusionKinds.ts'
import type { LocalizedText } from './i18n.ts'
import {
  beginnerProjectBinding,
  matchesBeginnerProjectBinding,
  type BeginnerNativeEditRunner,
} from './beginnerWorkflowSupport.ts'
import type { useBeginnerEditorState } from './useBeginnerEditorState.ts'

type EditorState = ReturnType<typeof useBeginnerEditorState>

type SurfaceAssignment = {
  range_id: number
  protrusion_id: number
}

type SurfaceEdit = {
  range_id: number
  base_digest_sha256: readonly number[]
  triangle_indices: number[]
  bulge_direction_milli: [number, number, number]
  bulge_amount_tenths_mm: number
}

type ReferenceTransport = Readonly<{
  importModel: typeof importBeginnerReferenceModel
  activateAsset: typeof activateBeginnerReferenceModelAsset
  archiveAsset: typeof archiveBeginnerReferenceModelAsset
  geometry: typeof getBeginnerReferenceModelGeometry
  suggest: typeof suggestBeginnerReferenceModelFeatures
  applySuggestion: typeof applyBeginnerReferenceModelFeatures
}>

const DEFAULT_TRANSPORT: ReferenceTransport = Object.freeze({
  importModel: importBeginnerReferenceModel,
  activateAsset: activateBeginnerReferenceModelAsset,
  archiveAsset: archiveBeginnerReferenceModelAsset,
  geometry: getBeginnerReferenceModelGeometry,
  suggest: suggestBeginnerReferenceModelFeatures,
  applySuggestion: applyBeginnerReferenceModelFeatures,
})

export function useBeginnerReferenceWorkflow(input: Readonly<{
  snapshot: ProjectSnapshot | null
  getCurrentSnapshot: () => ProjectSnapshot | null
  runNativeEdit: BeginnerNativeEditRunner
  confirm: (message: LocalizedText) => boolean
  copy: Readonly<{
    applySuggestion: LocalizedText
    copyEstimatedBridges: LocalizedText
  }>
  editor: Pick<
    EditorState,
    | 'beginnerDesignFormRef'
    | 'setBeginnerBodyOutline'
    | 'setBeginnerBodyOutlineMode'
    | 'setBeginnerProtrusions'
    | 'setBeginnerProtrusionKinds'
    | 'setBeginnerSkeletonSegments'
    | 'setBeginnerComponentBridgeOverride'
  >
  transport?: ReferenceTransport
}>) {
  const [beginnerReferenceGeometry, setBeginnerReferenceGeometry] =
    useState<BeginnerReferenceModelGeometry | null>(null)
  const [beginnerReferenceSuggestion, setBeginnerReferenceSuggestion] =
    useState<BeginnerReferenceModelSuggestionV1 | null>(null)
  const [
    beginnerSurfaceAssignments,
    setBeginnerSurfaceAssignments,
  ] = useState<SurfaceAssignment[]>([])
  const [beginnerSurfaceEdits, setBeginnerSurfaceEdits] =
    useState<SurfaceEdit[]>([])
  const geometryRequestRef = useRef(0)
  const suggestionRequestRef = useRef(0)
  const suggestionBusyRef = useRef(false)
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const snapshotProjectInstanceId = input.snapshot?.project_instance_id
  const snapshotRevision = input.snapshot?.revision

  useEffect(() => {
    geometryRequestRef.current += 1
    suggestionRequestRef.current += 1
    suggestionBusyRef.current = false
    setBeginnerReferenceGeometry(null)
    setBeginnerReferenceSuggestion(null)
    setBeginnerSurfaceAssignments([])
    setBeginnerSurfaceEdits([])
  }, [snapshotProjectInstanceId, snapshotRevision])

  function requestBeginnerReferenceModelImport() {
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.importModel(
      projectId,
      revision,
      projectInstanceId,
    ))
  }

  function activateBeginnerReferenceAsset(assetId: string) {
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.activateAsset(
      projectId,
      revision,
      projectInstanceId,
      assetId,
    ))
  }

  function archiveBeginnerReferenceAsset(
    assetId: string,
    archived: boolean,
  ) {
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.archiveAsset(
      projectId,
      revision,
      projectInstanceId,
      assetId,
      archived,
    ))
  }

  function toggleBeginnerReferenceModelPreview() {
    if (beginnerReferenceGeometry) {
      geometryRequestRef.current += 1
      setBeginnerReferenceGeometry(null)
      return
    }
    const current = input.getCurrentSnapshot()
    if (!current) return
    const binding = beginnerProjectBinding(current)
    const requestId = ++geometryRequestRef.current
    void transport.geometry(
      binding.project_id,
      binding.revision,
      binding.project_instance_id,
    ).then((geometry) => {
      if (
        requestId === geometryRequestRef.current
        && matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
        && matchesBeginnerProjectBinding(
          geometry,
          input.getCurrentSnapshot(),
        )
      ) setBeginnerReferenceGeometry(geometry)
    }).catch(() => {
      if (requestId === geometryRequestRef.current) {
        setBeginnerReferenceGeometry(null)
      }
    })
  }

  function requestBeginnerReferenceSuggestion() {
    if (suggestionBusyRef.current) return
    const current = input.getCurrentSnapshot()
    if (!current) return
    const binding = beginnerProjectBinding(current)
    const requestId = ++suggestionRequestRef.current
    suggestionBusyRef.current = true
    void transport.suggest(
      binding.project_id,
      binding.revision,
      binding.project_instance_id,
    ).then((suggestion) => {
      if (
        requestId !== suggestionRequestRef.current
        || !matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
      ) return
      setBeginnerReferenceSuggestion(suggestion)
      setBeginnerSurfaceAssignments([])
      setBeginnerSurfaceEdits(
        suggestion.surface_ranges.map((range) => ({
          range_id: range.id,
          base_digest_sha256: range.digest_sha256,
          triangle_indices: [...range.triangle_indices],
          bulge_direction_milli: [0, 0, 1_000],
          bulge_amount_tenths_mm: 50,
        })),
      )
    }).catch(() => {
      if (requestId === suggestionRequestRef.current) {
        setBeginnerReferenceSuggestion(null)
        setBeginnerSurfaceAssignments([])
        setBeginnerSurfaceEdits([])
      }
    }).finally(() => {
      if (requestId === suggestionRequestRef.current) {
        suggestionBusyRef.current = false
      }
    })
  }

  function confirmBeginnerReferenceSuggestion() {
    const current = input.getCurrentSnapshot()
    const suggestion = beginnerReferenceSuggestion
    const targetAsset = current?.beginner_design_profile
      .generation_constraints.target_asset
    if (
      !current
      || !suggestion
      || targetAsset?.kind !== 'reference_model'
      || targetAsset.asset_id !== suggestion.asset_id
      || beginnerSurfaceAssignments.length < 2
      || !input.confirm(input.copy.applySuggestion)
    ) return
    const assignments = [...beginnerSurfaceAssignments].sort(
      (left, right) => left.range_id - right.range_id,
    )
    const edits = beginnerSurfaceEdits.filter(
      (edit) => assignments.some(
        (assignment) => assignment.range_id === edit.range_id,
      ),
    ).sort((left, right) => left.range_id - right.range_id)
    void input.runNativeEdit((
      projectId,
      revision,
      projectInstanceId,
    ) => transport.applySuggestion(
      projectId,
      revision,
      projectInstanceId,
      suggestion,
      assignments,
      edits,
    )).then((applied) => {
      if (applied) setBeginnerReferenceSuggestion(null)
    })
  }

  function copyBeginnerReferenceContours() {
    const suggestion = beginnerReferenceSuggestion
    const current = input.getCurrentSnapshot()
    const targetAsset = current?.beginner_design_profile
      .generation_constraints.target_asset
    if (
      !suggestion
      || !current
      || targetAsset?.kind !== 'reference_model'
      || targetAsset.asset_id !== suggestion.asset_id
    ) return
    if (suggestion.generic_body_outline_tenths_mm) {
      input.editor.setBeginnerBodyOutline(
        suggestion.generic_body_outline_tenths_mm.map(
          (point) => [...point] as [number, number],
        ),
      )
      input.editor.setBeginnerBodyOutlineMode(
        suggestion.generic_body_outline_mode === 'general'
          ? 'general'
          : 'symmetric',
      )
    }
    input.editor.setBeginnerProtrusions(
      suggestion.protrusions.map((target) => ({
        ...target,
        ...(target.local_outline_tenths_mm
          ? {
              local_outline_tenths_mm:
                target.local_outline_tenths_mm.map(
                  (point) => [...point] as [number, number],
                ),
            }
          : {}),
      })),
    )
    input.editor.setBeginnerProtrusionKinds(
      resolveBeginnerProtrusionKindsV1(
        current.beginner_design_profile.generation_constraints.target_parts,
        suggestion.protrusions,
        {
          targetCategory: current.beginner_design_profile
            .generation_constraints.target_category,
          allowOrderedGeneric: true,
        },
      ) ?? suggestion.protrusions.map(() => null),
    )
  }

  function copyBeginnerGeneralReferenceTarget() {
    const suggestion = beginnerReferenceSuggestion
    const current = input.getCurrentSnapshot()
    const targetAsset = current?.beginner_design_profile
      .generation_constraints.target_asset
    if (
      !suggestion
      || !current
      || targetAsset?.kind !== 'reference_model'
      || targetAsset.asset_id !== suggestion.asset_id
      || (
        suggestion.inferred_component_bridges
        && !input.confirm(input.copy.copyEstimatedBridges)
      )
    ) return
    if (suggestion.inferred_component_bridges) {
      const category = input.editor.beginnerDesignFormRef.current
        ?.elements.namedItem('target_category')
      if (category instanceof HTMLSelectElement) {
        category.value = 'custom_object'
      }
      input.editor.setBeginnerComponentBridgeOverride({
        schema_version: 1,
        source_asset_sha256: suggestion.source_asset_sha256.slice(),
        component_count: suggestion.component_count,
        reviewed: true,
        bridges: Array.from(
          { length: suggestion.component_count - 1 },
          (_, id) => ({
            id,
            start_component_id: id,
            end_component_id: id + 1,
            accepted: true,
          }),
        ),
      })
    }
    input.editor.setBeginnerProtrusions(
      suggestion.general_protrusion_candidates.map(
        (target) => ({ ...target }),
      ),
    )
    const targetCategory = suggestion.inferred_component_bridges
      ? 'custom_object'
      : current.beginner_design_profile.generation_constraints
          .target_category
    input.editor.setBeginnerProtrusionKinds(
      resolveBeginnerProtrusionKindsV1(
        current.beginner_design_profile.generation_constraints.target_parts,
        suggestion.general_protrusion_candidates,
        {
          targetCategory,
          allowOrderedGeneric: true,
        },
      ) ?? suggestion.general_protrusion_candidates.map(() => null),
    )
    input.editor.setBeginnerSkeletonSegments(
      suggestion.stick_bars.filter(
        (bar) => (
          bar.start_tenths_mm[0] !== bar.end_tenths_mm[0]
          || bar.start_tenths_mm[1] !== bar.end_tenths_mm[1]
        ),
      ).map((bar, index) => ({
        id: index,
        start: {
          x_tenths_mm: bar.start_tenths_mm[0],
          y_tenths_mm: bar.start_tenths_mm[1],
        },
        end: {
          x_tenths_mm: bar.end_tenths_mm[0],
          y_tenths_mm: bar.end_tenths_mm[1],
        },
        thickness_tenths_mm: bar.thickness_tenths_mm,
      })),
    )
  }

  return {
    beginnerReferenceGeometry,
    beginnerReferenceSuggestion,
    beginnerSurfaceAssignments,
    setBeginnerSurfaceAssignments,
    beginnerSurfaceEdits,
    setBeginnerSurfaceEdits,
    requestBeginnerReferenceModelImport,
    activateBeginnerReferenceAsset,
    archiveBeginnerReferenceAsset,
    toggleBeginnerReferenceModelPreview,
    requestBeginnerReferenceSuggestion,
    confirmBeginnerReferenceSuggestion,
    copyBeginnerReferenceContours,
    copyBeginnerGeneralReferenceTarget,
  } as const
}

export type BeginnerSurfaceAssignment = SurfaceAssignment
export type BeginnerSurfaceEdit = SurfaceEdit
