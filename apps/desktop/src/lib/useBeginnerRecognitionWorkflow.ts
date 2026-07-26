import { useEffect, useEffectEvent, useRef, useState } from 'react'

import {
  applyBeginnerOutlineCandidate,
  applyBeginnerPartAssignments,
  BeginnerRecognitionError,
  recognizeBeginnerOutlineCandidates,
  recognizeBeginnerPartSuggestions,
  recognizeBeginnerSilhouette,
  recognizeBeginnerTarget,
  type BeginnerDesignProfileV1,
  type BeginnerOutlineCandidatesResponse,
  type BeginnerPartSuggestionsResponse,
  type BeginnerRecognitionProposalV1,
  type ProjectSnapshot,
} from './coreClient.ts'
import type { LocalizedText } from './i18n.ts'
import {
  beginnerProjectBinding,
  matchesBeginnerProjectBinding,
  type BeginnerNativeEditRunner,
} from './beginnerWorkflowSupport.ts'
import type { useBeginnerEditorState } from './useBeginnerEditorState.ts'

type EditorState = ReturnType<typeof useBeginnerEditorState>
type Constraints = BeginnerDesignProfileV1['generation_constraints']
type PartKind = Constraints['target_parts'][number]['kind']
type RecognitionMode = 'marker' | 'silhouette'
type RecognitionFailure = BeginnerRecognitionError['reason']

export type BeginnerPartAssignment = {
  candidate_id: number
  kind: PartKind
  source_candidate_ids?: number[]
  split_fragment?: number
  split_x?: number
}

type RecognitionTransport = Readonly<{
  recognizeTarget: typeof recognizeBeginnerTarget
  recognizeSilhouette: typeof recognizeBeginnerSilhouette
  recognizeOutlines: typeof recognizeBeginnerOutlineCandidates
  recognizeParts: typeof recognizeBeginnerPartSuggestions
  applyOutline: typeof applyBeginnerOutlineCandidate
  applyParts: typeof applyBeginnerPartAssignments
}>

const DEFAULT_TRANSPORT: RecognitionTransport = Object.freeze({
  recognizeTarget: recognizeBeginnerTarget,
  recognizeSilhouette: recognizeBeginnerSilhouette,
  recognizeOutlines: recognizeBeginnerOutlineCandidates,
  recognizeParts: recognizeBeginnerPartSuggestions,
  applyOutline: applyBeginnerOutlineCandidate,
  applyParts: applyBeginnerPartAssignments,
})
const DEFAULT_SCHEDULE_DEBOUNCE = (callback: () => void) => window.setTimeout(callback, 300)
const DEFAULT_CANCEL_DEBOUNCE = (handle: number) => window.clearTimeout(handle)

export function useBeginnerRecognitionWorkflow(input: Readonly<{
  snapshot: ProjectSnapshot | null
  getCurrentSnapshot: () => ProjectSnapshot | null
  operationBlocked: () => boolean
  runNativeEdit: BeginnerNativeEditRunner
  confirm: (message: LocalizedText) => boolean
  copy: Readonly<{
    copyOutline: LocalizedText
    applyParts: LocalizedText
    copyProposal: LocalizedText
    overrideLowConfidence: LocalizedText
  }>
  editor: Pick<
    EditorState,
    | 'beginnerDesignFormRef'
    | 'setBeginnerPartTotal'
    | 'setBeginnerSkeletonSegments'
    | 'setBeginnerBodyOutline'
    | 'setBeginnerBodyOutlineMode'
    | 'setBeginnerProtrusions'
  >
  onMissingReference: () => void
  onRecognitionReady: (mode: RecognitionMode) => void
  onRecognitionFailure: (reason: RecognitionFailure) => void
  onProposalCopied: () => void
  transport?: RecognitionTransport
  scheduleDebounce?: (callback: () => void) => number
  cancelDebounce?: (handle: number) => void
}>) {
  const [beginnerRecognitionProposal, setBeginnerRecognitionProposal] =
    useState<BeginnerRecognitionProposalV1 | null>(null)
  const [acceptedRecognitionProtrusionIds,
    setAcceptedRecognitionProtrusionIds] =
    useState<ReadonlySet<number>>(() => new Set())
  const [beginnerRecognitionBusy, setBeginnerRecognitionBusy] = useState(false)
  const [
    beginnerSilhouetteThresholds,
    setBeginnerSilhouetteThresholds,
  ] = useState<{
    alpha: number
    luma: number
    polarity: 'dark_on_light' | 'light_on_dark' | 'alpha_only'
  }>({ alpha: 128, luma: 127, polarity: 'dark_on_light' })
  const [beginnerSilhouetteCropRoi, setBeginnerSilhouetteCropRoi] = useState<
    Constraints['silhouette_crop_roi']
  >()
  const [beginnerSilhouetteOrientation, setBeginnerSilhouetteOrientation] =
    useState<0 | 90 | 180 | 270>(0)
  const [beginnerSilhouetteMirror, setBeginnerSilhouetteMirror] =
    useState<NonNullable<Constraints['silhouette_mirror']>>({
      schema_version: 1,
      mirror_x: false,
      mirror_y: false,
    })
  const [beginnerOutlineCandidates, setBeginnerOutlineCandidates] = useState<
    BeginnerOutlineCandidatesResponse | null
  >(null)
  const [beginnerPartSuggestions, setBeginnerPartSuggestions] = useState<
    BeginnerPartSuggestionsResponse | null
  >(null)
  const [beginnerPartAssignments, setBeginnerPartAssignments] =
    useState<BeginnerPartAssignment[]>([])
  const [excludedBeginnerPartAssignments,
    setExcludedBeginnerPartAssignments] =
    useState<BeginnerPartAssignment[]>([])
  const requestRef = useRef(0)
  const busyRef = useRef(false)
  const snapshotRef = useRef(input.snapshot)
  snapshotRef.current = input.snapshot
  const snapshotProjectInstanceId = input.snapshot?.project_instance_id
  const snapshotRevision = input.snapshot?.revision
  const transport = input.transport ?? DEFAULT_TRANSPORT
  const scheduleDebounce = input.scheduleDebounce ?? DEFAULT_SCHEDULE_DEBOUNCE
  const cancelDebounce = input.cancelDebounce ?? DEFAULT_CANCEL_DEBOUNCE
  const requestSilhouetteAfterDebounce =
    useEffectEvent(() => requestBeginnerRecognition('silhouette'))

  useEffect(() => {
    requestRef.current += 1
    busyRef.current = false
    setBeginnerRecognitionBusy(false)
    setBeginnerRecognitionProposal(null)
    setAcceptedRecognitionProtrusionIds(new Set())
    const constraints = snapshotRef.current?.beginner_design_profile
      .generation_constraints
    setBeginnerSilhouetteThresholds(
      constraints?.silhouette_thresholds
        ?? { alpha: 128, luma: 127, polarity: 'dark_on_light' },
    )
    setBeginnerSilhouetteCropRoi(constraints?.silhouette_crop_roi)
    setBeginnerSilhouetteOrientation(
      constraints?.silhouette_orientation_degrees ?? 0,
    )
    setBeginnerSilhouetteMirror(
      constraints?.silhouette_mirror
        ?? { schema_version: 1, mirror_x: false, mirror_y: false },
    )
    setBeginnerOutlineCandidates(null)
    setBeginnerPartSuggestions(null)
    setBeginnerPartAssignments([])
    setExcludedBeginnerPartAssignments([])
  }, [snapshotProjectInstanceId, snapshotRevision])

  useEffect(() => {
    const handle = scheduleDebounce(requestSilhouetteAfterDebounce)
    return () => cancelDebounce(handle)
  }, [
    beginnerSilhouetteThresholds.alpha,
    beginnerSilhouetteThresholds.luma,
    beginnerSilhouetteThresholds.polarity,
    beginnerSilhouetteCropRoi?.x_millionths,
    beginnerSilhouetteCropRoi?.y_millionths,
    beginnerSilhouetteCropRoi?.width_millionths,
    beginnerSilhouetteCropRoi?.height_millionths,
    beginnerSilhouetteOrientation,
    beginnerSilhouetteMirror.mirror_x,
    beginnerSilhouetteMirror.mirror_y,
    scheduleDebounce,
    cancelDebounce,
  ])

  function setBusy(busy: boolean) {
    busyRef.current = busy
    setBeginnerRecognitionBusy(busy)
  }

  function invalidateBeginnerRecognition() {
    requestRef.current += 1
    setBusy(false)
    setBeginnerRecognitionProposal(null)
  }

  function selectedUnderlay(current: ProjectSnapshot) {
    const form = input.editor.beginnerDesignFormRef.current
    if (!form) return null
    const underlayId = String(
      new FormData(form).get('target_reference_underlay') ?? '',
    )
    return current.underlays?.underlays.find(
      (item) => item.id === underlayId,
    ) ?? null
  }

  function requestBeginnerRecognition(
    mode: RecognitionMode = 'marker',
  ) {
    const current = input.getCurrentSnapshot()
    if (
      !current
      || busyRef.current
      || input.operationBlocked()
    ) return
    const underlay = selectedUnderlay(current)
    if (!underlay) {
      input.onMissingReference()
      return
    }
    const binding = beginnerProjectBinding(current)
    const requestId = ++requestRef.current
    setBusy(true)
    setBeginnerRecognitionProposal(null)
    const recognition = mode === 'silhouette'
      ? transport.recognizeSilhouette(
          binding.project_id,
          binding.revision,
          binding.project_instance_id,
          underlay.id,
          underlay.asset,
          {
            ...beginnerSilhouetteThresholds,
            crop_roi: beginnerSilhouetteCropRoi,
            orientation_degrees: beginnerSilhouetteOrientation,
            mirror: beginnerSilhouetteMirror,
          },
        )
      : transport.recognizeTarget(
          binding.project_id,
          binding.revision,
          binding.project_instance_id,
          underlay.id,
          underlay.asset,
        )
    void recognition.then((proposal) => {
      if (
        requestId !== requestRef.current
        || !matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
      ) return
      setBeginnerRecognitionProposal(proposal)
      setAcceptedRecognitionProtrusionIds(
        new Set(proposal.protrusions?.map((target) => target.id) ?? []),
      )
      input.onRecognitionReady(mode)
    }).catch((error: unknown) => {
      if (requestId !== requestRef.current) return
      input.onRecognitionFailure(
        error instanceof BeginnerRecognitionError
          ? error.reason
          : 'native_failure',
      )
    }).finally(() => {
      if (requestId === requestRef.current) setBusy(false)
    })
  }

  function requestBeginnerOutlineCandidates() {
    const current = input.getCurrentSnapshot()
    if (
      !current
      || busyRef.current
      || input.operationBlocked()
    ) return
    const underlay = selectedUnderlay(current)
    if (!underlay) return
    const binding = beginnerProjectBinding(current)
    const requestId = ++requestRef.current
    setBusy(true)
    setBeginnerOutlineCandidates(null)
    void transport.recognizeOutlines(
      binding.project_id,
      binding.revision,
      binding.project_instance_id,
      underlay.id,
      underlay.asset,
    ).then((proposal) => {
      if (
        requestId === requestRef.current
        && matchesBeginnerProjectBinding(
          binding,
          input.getCurrentSnapshot(),
        )
        && matchesBeginnerProjectBinding(
          proposal,
          input.getCurrentSnapshot(),
        )
      ) setBeginnerOutlineCandidates(proposal)
    }).catch(() => {
      if (requestId === requestRef.current) {
        setBeginnerOutlineCandidates(null)
      }
    }).finally(() => {
      if (requestId === requestRef.current) setBusy(false)
    })
  }

  function copyBeginnerOutlineCandidate(
    candidate: BeginnerOutlineCandidatesResponse['candidates'][number],
  ) {
    const proposal = beginnerOutlineCandidates
    if (
      !proposal
      || !proposal.candidates.includes(candidate)
      || !matchesBeginnerProjectBinding(
        proposal,
        input.getCurrentSnapshot(),
      )
      || !input.confirm(input.copy.copyOutline)
    ) return
    void input.runNativeEdit(
      () => transport.applyOutline(proposal, candidate, true),
    ).then((applied) => {
      if (applied) setBeginnerOutlineCandidates(null)
    })
  }

  function requestBeginnerPartSuggestions(
    candidate: BeginnerOutlineCandidatesResponse['candidates'][number],
  ) {
    const outline = beginnerOutlineCandidates
    if (
      !outline
      || busyRef.current
      || !outline.candidates.includes(candidate)
      || !matchesBeginnerProjectBinding(
        outline,
        input.getCurrentSnapshot(),
      )
    ) return
    const requestId = ++requestRef.current
    setBusy(true)
    void transport.recognizeParts(outline, candidate).then((proposal) => {
      if (
        requestId === requestRef.current
        && matchesBeginnerProjectBinding(
          proposal,
          input.getCurrentSnapshot(),
        )
      ) {
        setBeginnerPartSuggestions(proposal)
        setBeginnerPartAssignments(
          proposal.suggestions.map((item) => ({
            candidate_id: item.candidate_id,
            kind: item.suggested_kind,
          })),
        )
        setExcludedBeginnerPartAssignments([])
      }
    }).catch(() => {
      if (requestId === requestRef.current) {
        setBeginnerPartSuggestions(null)
      }
    }).finally(() => {
      if (requestId === requestRef.current) setBusy(false)
    })
  }

  function confirmBeginnerPartAssignments() {
    const outline = beginnerOutlineCandidates
    const proposal = beginnerPartSuggestions
    const selected = outline?.candidates.find(
      (candidate) => candidate.id === proposal?.selected_outline_id,
    )
    if (
      !outline
      || !proposal
      || !selected
      || !matchesBeginnerProjectBinding(
        outline,
        input.getCurrentSnapshot(),
      )
      || !matchesBeginnerProjectBinding(
        proposal,
        input.getCurrentSnapshot(),
      )
      || !input.confirm(input.copy.applyParts)
    ) return
    void input.runNativeEdit(
      () => transport.applyParts(
        outline,
        selected,
        beginnerPartAssignments,
      ),
    ).then((applied) => {
      if (applied) setBeginnerPartSuggestions(null)
    })
  }

  function copyBeginnerRecognitionProposal() {
    const proposal = beginnerRecognitionProposal
    const form = input.editor.beginnerDesignFormRef.current
    const current = input.getCurrentSnapshot()
    const liveUnderlay = current?.underlays?.underlays.find(
      (underlay) => (
        underlay.id === proposal?.source_underlay_id
        && underlay.asset === proposal.source_asset_id
      ),
    )
    if (
      !proposal
      || !form
      || !current
      || !liveUnderlay
      || !input.confirm(input.copy.copyProposal)
      || (
        proposal.contour_confidence?.explicit_override_required
        && !input.confirm(input.copy.overrideLowConfidence)
      )
    ) return
    if (proposal.target_parts.length > 0) {
      const counts = new Map(
        proposal.target_parts.map((part) => [part.kind, part.count]),
      )
      form.querySelectorAll<HTMLInputElement>(
        'input[name^="target_part_"]',
      ).forEach((field) => {
        const kind = field.name.slice('target_part_'.length) as PartKind
        field.value = String(counts.get(kind) ?? 0)
      })
      input.editor.setBeginnerPartTotal(
        proposal.target_parts.reduce(
          (sum, part) => sum + part.count,
          0,
        ),
      )
    }
    if (
      proposal.skeleton_quality?.distance_metric
        === 'aabb_squared_distance_v1'
    ) {
      const category = form.elements.namedItem('target_category')
      if (category instanceof HTMLSelectElement) {
        category.value = 'custom_object'
      }
    }
    input.editor.setBeginnerSkeletonSegments(
      proposal.skeleton_segments.map((segment) => ({
        ...segment,
        start: { ...segment.start },
        end: { ...segment.end },
      })),
    )
    if (proposal.generic_body_outline_tenths_mm) {
      input.editor.setBeginnerBodyOutline(
        proposal.generic_body_outline_tenths_mm.map(
          (point) => [...point] as [number, number],
        ),
      )
      input.editor.setBeginnerBodyOutlineMode(
        proposal.generic_body_outline_mode === 'general'
          ? 'general'
          : 'symmetric',
      )
    }
    if (proposal.protrusions) {
      input.editor.setBeginnerProtrusions(
        proposal.protrusions.filter(
          (target) => acceptedRecognitionProtrusionIds.has(target.id),
        ).map((target) => ({
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
    }
    input.onProposalCopied()
  }

  return {
    beginnerRecognitionProposal, acceptedRecognitionProtrusionIds,
    setAcceptedRecognitionProtrusionIds, beginnerRecognitionBusy,
    beginnerSilhouetteThresholds, setBeginnerSilhouetteThresholds,
    beginnerSilhouetteCropRoi, setBeginnerSilhouetteCropRoi,
    beginnerSilhouetteOrientation, setBeginnerSilhouetteOrientation,
    beginnerSilhouetteMirror, setBeginnerSilhouetteMirror,
    beginnerOutlineCandidates, beginnerPartSuggestions,
    beginnerPartAssignments, setBeginnerPartAssignments,
    excludedBeginnerPartAssignments, setExcludedBeginnerPartAssignments,
    invalidateBeginnerRecognition, requestBeginnerRecognition,
    requestBeginnerOutlineCandidates, copyBeginnerOutlineCandidate,
    requestBeginnerPartSuggestions, confirmBeginnerPartAssignments,
    copyBeginnerRecognitionProposal,
  } as const
}
