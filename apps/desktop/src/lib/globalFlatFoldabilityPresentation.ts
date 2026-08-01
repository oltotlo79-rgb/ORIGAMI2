import {
  GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
  parseGlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityErrorCategory,
  type GlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityPhase,
  type GlobalFlatFoldabilityProofCategory,
  type GlobalFlatFoldabilitySummary,
  type GlobalFlatFoldabilityUnknownReason,
} from './globalFlatFoldability.ts'
import { DEFAULT_LOCALE } from './i18n.ts'
import {
  formatGlobalFlatFoldabilityActiveLive,
  formatGlobalFlatFoldabilityElapsedMilliseconds,
  formatGlobalFlatFoldabilityExhaustiveFaces,
  formatGlobalFlatFoldabilityFaceList,
  formatGlobalFlatFoldabilityFaceNumber,
  formatGlobalFlatFoldabilityItemCount,
  formatGlobalFlatFoldabilityLayerCount,
  formatGlobalFlatFoldabilityMaximumOverlap,
  formatGlobalFlatFoldabilityProgressWork,
  formatGlobalFlatFoldabilityUnknownLive,
  selectGlobalFlatFoldabilityPresentationText,
} from './globalFlatFoldabilityPresentationText.ts'

export type GlobalFlatFoldabilityPresentationKind =
  | 'idle'
  | 'queued'
  | 'running'
  | 'possible'
  | 'impossible'
  | 'unknown'
  | 'cancelled'
  | 'failed'
  | 'stale'

export type GlobalFlatFoldabilityPresentationEntry = Readonly<{
  label: string
  value: string
}>

export type GlobalFlatFoldabilityPresentation = Readonly<{
  kind: GlobalFlatFoldabilityPresentationKind
  icon: string
  label: string
  detail: string
  liveText: string
  active: boolean
  cancelRequested: boolean
  phaseText: string | null
  workText: string | null
  layerViewAvailable: boolean
  summaryEntries: readonly GlobalFlatFoldabilityPresentationEntry[]
  resultEntries: readonly GlobalFlatFoldabilityPresentationEntry[]
}>

export function createGlobalFlatFoldabilityPresentation(
  rawJob: unknown,
  locale: unknown = DEFAULT_LOCALE,
): GlobalFlatFoldabilityPresentation {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  if (rawJob === null) return idlePresentation(locale)
  const job = parseGlobalFlatFoldabilityJobDto(rawJob)
  if (!job) return invalidPresentation(locale)

  switch (job.state) {
    case 'queued':
      return activePresentation(job, 'queued', locale)
    case 'running':
      return activePresentation(job, 'running', locale)
    case 'cancelled':
      return terminalPresentation({
        kind: 'cancelled',
        icon: '■',
        label: copy.cancelledLabel,
        detail: copy.cancelledDetail,
        liveText: copy.cancelledLive,
        summary: job.summary,
      }, locale)
    case 'failed':
      return terminalPresentation({
        kind: 'failed',
        icon: '!',
        label: copy.calculationErrorLabel,
        detail: globalFlatFoldabilityErrorMessage(job.error_category, locale),
        liveText: copy.calculationErrorLive,
        summary: job.summary,
      }, locale)
    case 'stale':
      return terminalPresentation({
        kind: 'stale',
        icon: '↻',
        label: copy.staleLabel,
        detail: copy.staleDetail,
        liveText: copy.staleLive,
        summary: job.summary,
      }, locale)
    case 'completed':
      return completedPresentation(job.result, locale)
  }
}

export function globalFlatFoldabilityPhaseLabel(
  phase: GlobalFlatFoldabilityPhase,
  locale: unknown = DEFAULT_LOCALE,
) {
  return selectGlobalFlatFoldabilityPresentationText(locale).phases[phase]
}

export function globalFlatFoldabilityUnknownReasonMessage(
  reason: GlobalFlatFoldabilityUnknownReason,
  locale: unknown = DEFAULT_LOCALE,
) {
  return selectGlobalFlatFoldabilityPresentationText(locale)
    .unknownReasons[reason]
}

export function globalFlatFoldabilityProofLabel(
  category: GlobalFlatFoldabilityProofCategory,
  locale: unknown = DEFAULT_LOCALE,
) {
  return selectGlobalFlatFoldabilityPresentationText(locale)
    .proofLabels[category]
}

export function globalFlatFoldabilityErrorMessage(
  category: GlobalFlatFoldabilityErrorCategory,
  locale: unknown = DEFAULT_LOCALE,
) {
  return selectGlobalFlatFoldabilityPresentationText(locale)
    .errorMessages[category]
}

function idlePresentation(locale: unknown): GlobalFlatFoldabilityPresentation {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  return Object.freeze({
    kind: 'idle',
    icon: '◇',
    label: copy.idleLabel,
    detail: copy.idleDetail,
    liveText: '',
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    layerViewAvailable: false,
    summaryEntries: staticSummaryEntries(locale),
    resultEntries: Object.freeze([]),
  })
}

function invalidPresentation(
  locale: unknown,
): GlobalFlatFoldabilityPresentation {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  return Object.freeze({
    kind: 'failed',
    icon: '!',
    label: copy.calculationErrorLabel,
    detail: copy.invalidDetail,
    liveText: copy.invalidLive,
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    layerViewAvailable: false,
    summaryEntries: staticSummaryEntries(locale),
    resultEntries: Object.freeze([]),
  })
}

function activePresentation(
  job: Extract<GlobalFlatFoldabilityJobDto, { state: 'queued' | 'running' }>,
  kind: 'queued' | 'running',
  locale: unknown,
): GlobalFlatFoldabilityPresentation {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  const phaseText = globalFlatFoldabilityPhaseLabel(job.progress.phase, locale)
  const workText = formatGlobalFlatFoldabilityProgressWork(
    job.progress.completed_work,
    job.progress.total_work,
    locale,
  )
  const cancelText = job.cancel_requested
    ? copy.cancellingDetail
    : kind === 'queued'
      ? copy.queuedDetail
      : copy.runningDetail
  const label = job.cancel_requested
    ? copy.cancellingLabel
    : kind === 'queued'
      ? copy.queuedLabel
      : copy.runningLabel
  return Object.freeze({
    kind,
    icon: job.cancel_requested ? '■' : kind === 'queued' ? '○' : '▶',
    label,
    detail: cancelText,
    // Work counts remain visible, but a 250 ms poll must not repeatedly
    // interrupt screen readers. Announce only state/cancel and phase changes.
    liveText: formatGlobalFlatFoldabilityActiveLive(label, phaseText, locale),
    active: true,
    cancelRequested: job.cancel_requested,
    phaseText,
    workText,
    layerViewAvailable: false,
    summaryEntries: summaryEntries(job.progress, locale),
    resultEntries: Object.freeze([]),
  })
}

function completedPresentation(
  result: Extract<GlobalFlatFoldabilityJobDto, { state: 'completed' }>['result'],
  locale: unknown,
): GlobalFlatFoldabilityPresentation {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  switch (result.verdict) {
    case 'possible':
      return terminalPresentation({
        kind: 'possible',
        icon: '✓',
        label: copy.possibleLabel,
        detail: copy.possibleDetail,
        liveText: copy.possibleLive,
        summary: result.summary,
        layerViewAvailable: result.layer_order.layer_view_available,
        resultEntries: [
          {
            label: copy.layerOrderModelLabel,
            value: result.layer_order.model_id,
          },
          {
            label: copy.layerCountLabel,
            value: formatGlobalFlatFoldabilityLayerCount(
              result.layer_order.layer_count,
              locale,
            ),
          },
          {
            label: copy.maximumOverlapLabel,
            value: formatGlobalFlatFoldabilityMaximumOverlap(
              result.layer_order.max_ply,
              locale,
            ),
          },
          {
            label: copy.referenceFaceLabel,
            value: formatGlobalFlatFoldabilityFaceNumber(
              result.layer_order.reference_face_number,
              locale,
            ),
          },
          {
            label: copy.layerOrderViewLabel,
            value: result.layer_order.layer_view_available
              ? copy.available
              : copy.unavailable,
          },
        ],
      }, locale)
    case 'impossible': {
      const numberedFaces = formatGlobalFlatFoldabilityFaceList(
        result.proof.face_numbers,
        locale,
      )
      const faceText = result.proof.category === 'exhaustive_search_no_solution'
        ? formatGlobalFlatFoldabilityExhaustiveFaces(
          result.proof.face_numbers,
          result.summary.counts.face_count,
          locale,
        )
        : numberedFaces
      return terminalPresentation({
        kind: 'impossible',
        icon: '✕',
        label: copy.impossibleLabel,
        detail: copy.impossibleDetail,
        liveText: copy.impossibleLive,
        summary: result.summary,
        resultEntries: [
          {
            label: copy.proofTypeLabel,
            value: globalFlatFoldabilityProofLabel(
              result.proof.category,
              locale,
            ),
          },
          {
            label: copy.targetFacesLabel,
            value: faceText,
          },
        ],
      }, locale)
    }
    case 'unknown': {
      const reason = globalFlatFoldabilityUnknownReasonMessage(
        result.reason,
        locale,
      )
      return terminalPresentation({
        kind: 'unknown',
        icon: '?',
        label: copy.unknownLabel,
        detail: reason,
        liveText: formatGlobalFlatFoldabilityUnknownLive(reason, locale),
        summary: result.summary,
        resultEntries: [{
          label: copy.unknownReasonLabel,
          value: reason,
        }],
      }, locale)
    }
  }
}

function terminalPresentation(input: Readonly<{
  kind: Extract<
    GlobalFlatFoldabilityPresentationKind,
    'possible' | 'impossible' | 'unknown' | 'cancelled' | 'failed' | 'stale'
  >
  icon: string
  label: string
  detail: string
  liveText: string
  summary: GlobalFlatFoldabilitySummary
  resultEntries?: readonly GlobalFlatFoldabilityPresentationEntry[]
  layerViewAvailable?: boolean
}>, locale: unknown): GlobalFlatFoldabilityPresentation {
  return Object.freeze({
    kind: input.kind,
    icon: input.icon,
    label: input.label,
    detail: input.detail,
    liveText: input.liveText,
    active: false,
    cancelRequested: false,
    phaseText: null,
    workText: null,
    layerViewAvailable: input.layerViewAvailable === true,
    summaryEntries: summaryEntries(input.summary, locale),
    resultEntries: Object.freeze(
      (input.resultEntries ?? []).map((entry) => Object.freeze(entry)),
    ),
  })
}

function summaryEntries(
  input: GlobalFlatFoldabilitySummary | Extract<
  GlobalFlatFoldabilityJobDto,
    { state: 'queued' | 'running' }
  >['progress'],
  locale: unknown,
): readonly GlobalFlatFoldabilityPresentationEntry[] {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  return Object.freeze([
    ...staticSummaryEntries(locale),
    Object.freeze({
      label: copy.elapsedTimeLabel,
      value: formatGlobalFlatFoldabilityElapsedMilliseconds(
        input.elapsed_ms,
        locale,
      ),
    }),
    Object.freeze({
      label: copy.facesLabel,
      value: formatGlobalFlatFoldabilityItemCount(
        input.counts.face_count,
        locale,
      ),
    }),
    Object.freeze({
      label: copy.overlapCellsLabel,
      value: formatGlobalFlatFoldabilityItemCount(
        input.counts.overlap_cell_count,
        locale,
      ),
    }),
    Object.freeze({
      label: copy.constraintsLabel,
      value: formatGlobalFlatFoldabilityItemCount(
        input.counts.constraint_count,
        locale,
      ),
    }),
    Object.freeze({
      label: copy.searchNodesLabel,
      value: formatGlobalFlatFoldabilityItemCount(
        input.counts.search_node_count,
        locale,
      ),
    }),
  ])
}

function staticSummaryEntries(
  locale: unknown,
): readonly GlobalFlatFoldabilityPresentationEntry[] {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  return Object.freeze([
    Object.freeze({
      label: copy.checkModelLabel,
      value: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    }),
    Object.freeze({
      label: copy.targetClassLabel,
      value: copy.targetClass,
    }),
  ])
}
