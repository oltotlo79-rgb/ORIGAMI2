import { type FormEvent, useEffect, useMemo, useRef, useState } from 'react'
import {
  applyStackedFoldTransaction,
  applyNamedBookFoldTransaction,
  applyNamedReverseFoldTransaction,
  applyNamedLayerSelectiveTransaction,
  cancelCurrentStackedFoldReadV1,
  cancelStackedFoldTransactionPreview,
  listenStackedFoldReadProgressV1,
  listenCurrentCyclePoseProgressV1,
  matchesProjectOccGuard,
  proposeCurrentCyclePoseV1,
  proposeCurrentStackedFoldRead,
  readEvenCycleCandidatesV1,
  readBoundedDyadicPoseGraphV1,
  mintDyadicPosePathPreviewV1,
  previewNamedBasicFoldTimeline,
  applyDyadicPosePathPreviewV1,
  readLiveHingeRegistryV1,
  type ProjectSnapshot,
  type CurrentCyclePosePreviewResponseV1,
  type CurrentCyclePoseProgressV1,
  type DyadicPoseGraphReadResponseV1,
  type DyadicPathPreviewResponseV1,
  type BasicFoldTimelinePreviewResponseV1,
} from '../lib/coreClient'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
  type MessageVariables,
} from '../lib/i18n'
import { STACKED_FOLD_PANEL_TEXT as TEXT } from '../lib/stackedFoldPanelText.ts'
import {
  createStackedFoldReadCoordinator,
  type StackedFoldReadCoordinator,
} from '../lib/stackedFoldReadCoordinator'
import type {
  CycleScheduleRequestV1,
  CertifiedPathGraphRequestV1,
  LinearCandidateRequestV1,
  StackedFoldFixedSide,
  StackedFoldReadResponse,
  StackedFoldRotationDirection,
} from '../lib/stackedFoldRead'
import { isCycleScheduleRequestV1 } from '../lib/stackedFoldRead'
import type { LayerOrderViewerCell } from '../lib/currentLayerOrderView'
import type { FoldTechniqueFileDocumentV1 } from '../lib/foldTechniqueEditor'

type SelectedLine = Readonly<{
  id: string
  start: Readonly<{ x: number; y: number }>
  end: Readonly<{ x: number; y: number }>
}>

type Props = Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  selectedLine: SelectedLine | null
  disabled: boolean
  onApplied(snapshot: ProjectSnapshot): void
  refreshSnapshot(): Promise<ProjectSnapshot>
  namedBookFold?: Readonly<{
    document: FoldTechniqueFileDocumentV1
    techniqueId: string
    name: string
    kind?: 'book' | 'mountain' | 'valley' | 'squash' | 'crimp' | 'petal' | 'inside_reverse' | 'outside_reverse' | 'reverse' | 'accordion' | 'sink' | 'layer' | 'layer_selective'
  }> | null
  namedTechniquePalette?: readonly Readonly<{
    techniqueId: string
    name: string
    supported: boolean
    reason?: string
  }>[]
  onSelectNamedTechnique?(techniqueId: string): void
}>

const MAX_CYCLE_SCHEDULE_JSON_BYTES = 65_536
const MAX_PERSISTED_LAYER_ORDER_PAIRS = 50_000
const MAX_RENDERED_PERSISTED_LAYER_ORDER_PAIRS = 200
const CANONICAL_UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/

type View =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'reading' }>
  | Readonly<{ kind: 'ready'; response: StackedFoldReadResponse; applyFailed: boolean }>
  | Readonly<{
      kind: 'failed'
      reason:
        | 'analysis'
        | 'invalid'
        | 'apply'
        | 'stale'
        | 'cycle_nonclosing'
        | 'cycle_path_uncertified'
        | 'cycle_path_unsupported'
        | 'cycle_path_resource_limit'
        | 'cycle_path_no_certified_path'
        | 'cycle_path_cancelled'
        | 'cycle_path_collision'
    }>
  | Readonly<{ kind: 'refresh_failed' }>

export function StackedFoldPanel({
  locale,
  snapshot,
  selectedLine,
  disabled,
  onApplied,
  refreshSnapshot,
  namedBookFold = null,
  namedTechniquePalette = [],
  onSelectNamedTechnique,
}: Props) {
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const formattedText = (
    localized: LocalizedText,
    variables: MessageVariables,
  ) => formatLocalizedText(locale, localized, variables)
  const authorityRef = useRef(snapshot)
  authorityRef.current = snapshot
  const namedAuthorityRef = useRef({ namedBookFold, selectedLine })
  namedAuthorityRef.current = { namedBookFold, selectedLine }
  const [fixedSide, setFixedSide] = useState<StackedFoldFixedSide>('left')
  const [rotationDirection, setRotationDirection] =
    useState<StackedFoldRotationDirection>('positive')
  const [angle, setAngle] = useState('180')
  const [cycleScheduleText, setCycleScheduleText] = useState('')
  const authoredCycleSchedule = useMemo(() => {
    if (!cycleScheduleText.trim()) return null
    try {
      const value: unknown = JSON.parse(cycleScheduleText)
      return isCycleScheduleRequestV1(value) ? value : null
    } catch {
      return null
    }
  }, [cycleScheduleText])
  const [liveHinges, setLiveHinges] = useState<readonly Readonly<{
    edge: string
    initialAngleDegrees: number
  }>[]>([])
  const [requestedHingeAngles, setRequestedHingeAngles] = useState<Record<string, number>>({})
  const [evenCycleCandidates, setEvenCycleCandidates] = useState<readonly Readonly<{
    edges: readonly [string, string]
    reason: 'same_assignment_geometrically_opposite'
  }>[]>([])
  const [evenCycleStatus, setEvenCycleStatus] = useState<string>('unsupported')
  const [kawasakiEndpoints, setKawasakiEndpoints] = useState<readonly Readonly<{
    endpointDenominator: number
    closureStatus: 'certified'
    collisionStatus: 'certified' | 'uncertified'
    authorizesApply: false
  }>[]>([])
  const [selectedKawasakiEndpoint, setSelectedKawasakiEndpoint] =
    useState<1 | 2 | 4 | 8 | 16>(1)
  const [dyadicGraphRead, setDyadicGraphRead] =
    useState<DyadicPoseGraphReadResponseV1 | null>(null)
  const [dyadicGraphReading, setDyadicGraphReading] = useState(false)
  const [dyadicLevelCount, setDyadicLevelCount] = useState<3 | 5 | 9>(3)
  const [dyadicPathPreview, setDyadicPathPreview] =
    useState<DyadicPathPreviewResponseV1 | null>(null)
  const [basicFoldTimelinePreview, setBasicFoldTimelinePreview] =
    useState<BasicFoldTimelinePreviewResponseV1 | null>(null)
  const [basicFoldTimelinePreviewError, setBasicFoldTimelinePreviewError] = useState(false)
  const [basicFoldTimelinePreviewReading, setBasicFoldTimelinePreviewReading] = useState(false)
  const [basicFoldTimelineStepIndex, setBasicFoldTimelineStepIndex] = useState(0)
  const basicFoldTimelineSequenceRef = useRef(0)
  const basicFoldTimelineActiveRef = useRef(false)
  const namedBasicFold = namedBookFold?.kind === 'mountain'
    || namedBookFold?.kind === 'valley' || namedBookFold?.kind === 'squash'
    || namedBookFold?.kind === 'crimp' || namedBookFold?.kind === 'inside_reverse'
    || namedBookFold?.kind === 'outside_reverse' || namedBookFold?.kind === 'sink'
    || namedBookFold?.kind === 'accordion' || namedBookFold?.kind === 'layer_selective'
  const unsupportedNamedPhysicalFold = namedBookFold != null
    && (namedBookFold.kind == null || namedBookFold.kind === 'book'
      || namedBookFold.kind === 'petal')
  useEffect(() => {
    basicFoldTimelineSequenceRef.current += 1
    if (basicFoldTimelineActiveRef.current) {
      basicFoldTimelineActiveRef.current = false
      cancelToken(tokenRef.current)
      tokenRef.current = null
    }
    setBasicFoldTimelinePreviewReading(false)
    setBasicFoldTimelinePreview(null)
    setBasicFoldTimelinePreviewError(false)
  }, [snapshot.project_instance_id, snapshot.project_id, snapshot.revision,
    snapshot.fold_model_fingerprint, disabled, selectedLine?.id,
    namedBookFold?.techniqueId, namedBookFold?.document])
  const dyadicGraphSequenceRef = useRef(0)
  const [confirmed, setConfirmed] = useState(false)
  const [applying, setApplying] = useState(false)
  const applyInFlightRef = useRef(false)
  const [view, setView] = useState<View>({ kind: 'idle' })
  const [selectedCell, setSelectedCell] = useState<string | null>(null)
  const [selectedFace, setSelectedFace] = useState<string | null>(null)
  const [hoveredFace, setHoveredFace] = useState<string | null>(null)
  const tokenRef = useRef<string | null>(null)
  const progressRequestRef = useRef<string | null>(null)
  const progressSequenceRef = useRef(0)
  const cyclePoseSequenceRef = useRef(0)
  const cyclePoseActiveRef = useRef(false)
  const cyclePoseProofRef = useRef<HTMLDivElement | null>(null)
  const cyclePosePreviewButtonRef = useRef<HTMLButtonElement | null>(null)
  const cyclePoseApplyInFlightRef = useRef(false)
  const [pathProgress, setPathProgress] = useState<Readonly<{
    exploredStateCount: number
    evaluatedTransitionCount: number
    stateLimit: number
    transitionLimit: number
  }> | null>(null)
  const [cyclePosePreview, setCyclePosePreview] =
    useState<CurrentCyclePosePreviewResponseV1 | null>(null)
  const [cyclePoseReading, setCyclePoseReading] = useState(false)
  const [cyclePoseError, setCyclePoseError] = useState(false)
  const [cyclePoseProgress, setCyclePoseProgress] =
    useState<CurrentCyclePoseProgressV1 | null>(null)
  const persistedCycleLayerProof = useMemo(() => {
    for (const step of [...(snapshot.instruction_timeline?.steps ?? [])].reverse()) {
      const proof = step.visual.cycle_layer_order_proof_v1
      if (proof === undefined || proof === null) continue
      if (proof?.version === 1 &&
        proof.model_id === 'native_continuous_layer_transport_certificate_v1' &&
        Object.keys(proof).sort().join(',') ===
          'model_id,pairs,target_order_sha256,transition_count,version' &&
        Array.isArray(proof.target_order_sha256) &&
        proof.target_order_sha256.length === 32 &&
        proof.target_order_sha256.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255) &&
        Number.isSafeInteger(proof.transition_count) && proof.transition_count > 0 &&
        Array.isArray(proof.pairs) && proof.pairs.length <= MAX_PERSISTED_LAYER_ORDER_PAIRS &&
        proof.pairs.every((pair, index, pairs) => typeof pair === 'object' && pair !== null &&
          Object.keys(pair).sort().join(',') === 'lower_face,upper_face' &&
          CANONICAL_UUID.test(pair.lower_face) && CANONICAL_UUID.test(pair.upper_face) &&
          pair.lower_face !== pair.upper_face &&
          (index === 0 || `${pairs[index - 1].lower_face}:${pairs[index - 1].upper_face}` <
            `${pair.lower_face}:${pair.upper_face}`))) return proof
      return null
    }
    return null
  }, [snapshot.instruction_timeline?.steps])
  const persistedLayerPairs = persistedCycleLayerProof?.pairs.slice(
    0, MAX_RENDERED_PERSISTED_LAYER_ORDER_PAIRS,
  ) ?? []
  const basicFoldTimelineStep = basicFoldTimelinePreview
    ?.timeline.steps[basicFoldTimelineStepIndex] ?? null
  const savedCompilerProvenance = useMemo(() => {
    for (const step of [...(snapshot.instruction_timeline?.steps ?? [])].reverse()) {
      const metadata = step.visual.named_technique_compiler_v1
      if (metadata?.version === 1
        && metadata.model_id === 'certified_named_technique_compiler_metadata_v1') {
        return { kind: metadata.technique_kind, segmentCount: metadata.segment_count }
      }
    }
    return null
  }, [snapshot.instruction_timeline?.steps])
  const coordinator = useMemo<StackedFoldReadCoordinator>(() =>
    createStackedFoldReadCoordinator({
      transport: proposeCurrentStackedFoldRead,
      getAuthority: () => {
        const current = authorityRef.current
        return {
          projectInstanceId: current.project_instance_id,
          projectId: current.project_id,
          revision: current.revision,
        }
      },
    }), [])

  const cancelToken = (token: string | null) => {
    if (!token) return
    setBasicFoldTimelinePreview(null)
    setBasicFoldTimelinePreviewError(false)
    void cancelStackedFoldTransactionPreview(token).catch(() => undefined)
  }

  useEffect(() => {
    coordinator.invalidate()
    cyclePoseSequenceRef.current += 1
    if (cyclePoseActiveRef.current) {
      cyclePoseActiveRef.current = false
      void cancelCurrentStackedFoldReadV1().catch(() => undefined)
    }
    progressRequestRef.current = null
    setPathProgress(null)
    setCyclePosePreview(null)
    dyadicGraphSequenceRef.current += 1
    setDyadicGraphRead(null)
    setDyadicPathPreview(null)
    setDyadicGraphReading(false)
    setCyclePoseReading(false)
    setCyclePoseError(false)
    setCyclePoseProgress(null)
    cancelToken(tokenRef.current)
    tokenRef.current = null
    setConfirmed(false)
    setSelectedCell(null)
    setSelectedFace(null)
    setHoveredFace(null)
    setView({ kind: 'idle' })
  }, [
    coordinator,
    snapshot.project_instance_id,
    snapshot.project_id,
    snapshot.revision,
    snapshot.fold_model_fingerprint,
    disabled,
    selectedLine?.id,
    fixedSide,
    rotationDirection,
    angle,
    cycleScheduleText,
    namedBookFold?.techniqueId,
    namedBookFold?.kind,
    namedBookFold?.document,
  ])

  useEffect(() => () => {
    basicFoldTimelineSequenceRef.current += 1
    basicFoldTimelineActiveRef.current = false
    coordinator.dispose()
    cyclePoseSequenceRef.current += 1
    if (cyclePoseActiveRef.current) {
      cyclePoseActiveRef.current = false
      void cancelCurrentStackedFoldReadV1().catch(() => undefined)
    }
    cancelToken(tokenRef.current)
  }, [coordinator])

  useEffect(() => {
    if (cyclePosePreview) cyclePoseProofRef.current?.focus()
  }, [cyclePosePreview])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | null = null
    void listenStackedFoldReadProgressV1((progress) => {
      if (progress.requestId !== progressRequestRef.current) return
      setPathProgress((previous) => {
        if (
          previous &&
          (progress.exploredStateCount < previous.exploredStateCount ||
            progress.evaluatedTransitionCount < previous.evaluatedTransitionCount)
        ) return previous
        return progress
      })
    }).then((value) => {
      if (disposed) value()
      else unlisten = value
    }).catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | null = null
    void listenCurrentCyclePoseProgressV1((progress) => {
      if (progress.requestId !== progressRequestRef.current) return
      setCyclePoseProgress(progress)
    }).then((value) => {
      if (disposed) value()
      else unlisten = value
    }).catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    let current = true
    if (!selectedLine) {
      setLiveHinges([])
      setRequestedHingeAngles({})
      setEvenCycleCandidates([])
      setKawasakiEndpoints([])
      return () => {
        current = false
      }
    }
    void readLiveHingeRegistryV1({
      expectedProjectInstanceId: snapshot.project_instance_id,
      expectedProjectId: snapshot.project_id,
      expectedRevision: snapshot.revision,
      first: [selectedLine.start.x, 0, -selectedLine.start.y],
      second: [selectedLine.end.x, 0, -selectedLine.end.y],
      fixedSide,
      rotationDirection,
      requestedAngleDegrees: Number(angle),
    }).then((registry) => {
      if (!current) return
      setLiveHinges(registry.entries)
      setRequestedHingeAngles(Object.fromEntries(
        registry.entries.map((entry) => [entry.edge, entry.initialAngleDegrees]),
      ))
    }).catch(() => {
      if (current) {
        setLiveHinges([])
        setRequestedHingeAngles({})
      }
    })
    void readEvenCycleCandidatesV1({
      expectedProjectInstanceId: snapshot.project_instance_id,
      expectedProjectId: snapshot.project_id,
      expectedRevision: snapshot.revision,
      maxPairTests: 120,
    }).then((automatic) => {
      if (!current) return
      setEvenCycleCandidates(automatic.candidates)
      setKawasakiEndpoints(automatic.kawasakiEndpoints)
      setEvenCycleStatus(automatic.status)
    }).catch(() => {
      if (current) {
        setEvenCycleCandidates([])
        setKawasakiEndpoints([])
        setEvenCycleStatus('unsupported')
      }
    })
    return () => {
      current = false
    }
  }, [
    snapshot.project_instance_id,
    snapshot.project_id,
    snapshot.revision,
    selectedLine,
    fixedSide,
    rotationDirection,
    angle,
  ])

  async function preview(event: FormEvent) {
    event.preventDefault()
    if (!selectedLine || disabled || applying) return
    const requestedAngleDegrees = Number(angle)
    let cycleScheduleV1: CycleScheduleRequestV1 | undefined
    let linearCandidateV1: LinearCandidateRequestV1 | undefined
    let certifiedPathGraphV1: CertifiedPathGraphRequestV1 | undefined
    if (cycleScheduleText.trim()) {
      if (new TextEncoder().encode(cycleScheduleText).byteLength > MAX_CYCLE_SCHEDULE_JSON_BYTES) {
        setView({ kind: 'failed', reason: 'invalid' })
        return
      }
      try {
        const parsed = JSON.parse(cycleScheduleText) as
          | CycleScheduleRequestV1
          | LinearCandidateRequestV1
          | CertifiedPathGraphRequestV1
        if (
          typeof parsed === 'object' &&
          parsed !== null &&
          'states' in parsed
        ) {
          certifiedPathGraphV1 = parsed as CertifiedPathGraphRequestV1
        } else if (
          typeof parsed === 'object' &&
          parsed !== null &&
          Array.isArray(parsed.entries) &&
          parsed.entries.length > 0 &&
          'initialAngleDegrees' in parsed.entries[0]
        ) {
          linearCandidateV1 = parsed as LinearCandidateRequestV1
        } else if (isCycleScheduleRequestV1(parsed)) {
          cycleScheduleV1 = parsed as CycleScheduleRequestV1
        } else {
          setView({ kind: 'failed', reason: 'invalid' })
          return
        }
      } catch {
        setView({ kind: 'failed', reason: 'invalid' })
        return
      }
    } else if (liveHinges.length > 0) {
      linearCandidateV1 = {
        version: 1,
        entries: liveHinges.map((entry) => ({
          edge: entry.edge,
          initialAngleDegrees: entry.initialAngleDegrees,
          requestedAngleDegrees: requestedHingeAngles[entry.edge] ?? entry.initialAngleDegrees,
        })),
      }
    }
    setConfirmed(false)
    const progressRequestId =
      `${snapshot.project_instance_id}:${snapshot.revision}:${++progressSequenceRef.current}`
    progressRequestRef.current = progressRequestId
    setPathProgress(null)
    setView({ kind: 'reading' })
    const result = await coordinator.read({
      progressRequestId,
      expectedProjectInstanceId: snapshot.project_instance_id,
      expectedProjectId: snapshot.project_id,
      expectedRevision: snapshot.revision,
      first: [selectedLine.start.x, 0, -selectedLine.start.y],
      second: [selectedLine.end.x, 0, -selectedLine.end.y],
      fixedSide,
      rotationDirection,
      requestedAngleDegrees,
      ...(cycleScheduleV1 ? { cycleScheduleV1 } : {}),
      ...(linearCandidateV1 ? { linearCandidateV1 } : {}),
      ...(certifiedPathGraphV1 ? { certifiedPathGraphV1 } : {}),
    })
    progressRequestRef.current = null
    if (result.status === 'ready') {
      tokenRef.current = result.response.transactionProposal.transactionToken
      setView({ kind: 'ready', response: result.response, applyFailed: false })
      if (namedBasicFold) void previewNamedBasicFold(result.response)
    } else if (result.status === 'failed') {
      setView({
        kind: 'failed',
        reason: result.reason === 'invalid_response'
          ? 'invalid'
          : result.reason === 'cycle_nonclosing'
              || result.reason === 'cycle_path_uncertified'
              || result.reason === 'cycle_path_unsupported'
              || result.reason === 'cycle_path_resource_limit'
              || result.reason === 'cycle_path_no_certified_path'
              || result.reason === 'cycle_path_cancelled'
              || result.reason === 'cycle_path_collision'
            ? result.reason
            : 'analysis',
      })
    } else if (result.reason === 'stale_authority') {
      setView({ kind: 'failed', reason: 'stale' })
    } else {
      setView({ kind: 'idle' })
    }
  }

  async function apply() {
    if (
      view.kind !== 'ready' ||
      !view.response.transactionProposal.readyForAtomicApply ||
      !confirmed ||
      applying || applyInFlightRef.current || unsupportedNamedPhysicalFold || (namedBasicFold
        && basicFoldTimelinePreview?.transactionToken !== view.response.transactionProposal.transactionToken)
    ) return
    const token = view.response.transactionProposal.transactionToken
    if (!token || token !== tokenRef.current) return
    applyInFlightRef.current = true
    setApplying(true)
    let committed = false
    try {
      await applyTransaction(token)
      committed = true
      tokenRef.current = null
      setBasicFoldTimelinePreview(null)
      setBasicFoldTimelinePreviewError(false)
      const next = await refreshSnapshot()
      onApplied(next)
      setView({ kind: 'idle' })
      setConfirmed(false)
    } catch {
      setView(committed
        ? { kind: 'refresh_failed' }
        : { kind: 'ready', response: view.response, applyFailed: true })
    } finally {
      applyInFlightRef.current = false
      setApplying(false)
    }
  }

  async function previewNamedBasicFold(response = view.kind === 'ready' ? view.response : null) {
    if (!response || !namedBookFold || !namedBasicFold
      || !selectedLine || disabled || applying || basicFoldTimelineActiveRef.current) return
    const sequence = ++basicFoldTimelineSequenceRef.current
    const token = response.transactionProposal.transactionToken
    const segment = response.materialSegments.find((item) =>
      item.assignment === 'mountain' || item.assignment === 'valley')
    if (!token || !segment) return
    const authority = authorityRef.current
    const selectedAuthority = namedAuthorityRef.current
    basicFoldTimelineActiveRef.current = true
    setBasicFoldTimelinePreviewReading(true)
    setBasicFoldTimelinePreview(null)
    setBasicFoldTimelineStepIndex(0)
    setBasicFoldTimelinePreviewError(false)
    try {
      const preview = await previewNamedBasicFoldTimeline({
        token,
        expectedProjectInstanceId: authority.project_instance_id,
        expectedProjectId: authority.project_id,
        expectedRevision: authority.revision,
        expectedSourceModelFingerprint: authority.fold_model_fingerprint,
        foldEdge: selectedLine.id,
        assignment: segment.assignment,
        techniqueKind: namedBookFold.kind as 'mountain' | 'valley' | 'squash' | 'crimp' | 'inside_reverse' | 'outside_reverse' | 'sink' | 'accordion' | 'layer_selective',
        techniqueDocument: namedBookFold.document,
        techniqueId: namedBookFold.techniqueId,
      })
      const current = authorityRef.current
      const currentSelection = namedAuthorityRef.current
      if (sequence !== basicFoldTimelineSequenceRef.current
        || !matchesProjectOccGuard({
          expectedProjectInstanceId: authority.project_instance_id,
          expectedProjectId: authority.project_id,
          expectedRevision: authority.revision,
        }, current)
        || current.fold_model_fingerprint !== authority.fold_model_fingerprint
        || currentSelection.namedBookFold?.document !== selectedAuthority.namedBookFold?.document
        || currentSelection.namedBookFold?.techniqueId !== selectedAuthority.namedBookFold?.techniqueId
        || currentSelection.selectedLine?.id !== selectedAuthority.selectedLine?.id
        || preview.assignment !== segment.assignment
        || tokenRef.current !== token) {
        void cancelStackedFoldTransactionPreview(preview.transactionToken).catch(() => undefined)
        return
      }
      setBasicFoldTimelinePreview(preview)
    } catch {
      if (sequence === basicFoldTimelineSequenceRef.current) {
        setBasicFoldTimelinePreview(null)
        setBasicFoldTimelinePreviewError(true)
      }
    } finally {
      if (sequence === basicFoldTimelineSequenceRef.current) {
        basicFoldTimelineActiveRef.current = false
        setBasicFoldTimelinePreviewReading(false)
      }
    }
  }

  function applyTransaction(token: string) {
    return namedBookFold?.kind === 'layer_selective'
      ? applyNamedLayerSelectiveTransaction(token, namedBookFold.document, namedBookFold.techniqueId)
      : namedBookFold?.kind === 'sink'
      ? basicFoldTimelinePreview?.transactionToken === token
        ? applyNamedBookFoldTransaction(token, namedBookFold.document, namedBookFold.techniqueId,
          basicFoldTimelinePreview)
        : Promise.reject(new Error('certified sink timeline preview required'))
      : namedBookFold?.kind === 'accordion'
      ? basicFoldTimelinePreview?.transactionToken === token
        ? applyNamedBookFoldTransaction(token, namedBookFold.document, namedBookFold.techniqueId,
          basicFoldTimelinePreview)
        : Promise.reject(new Error('certified accordion timeline preview required'))
      : namedBookFold?.kind === 'inside_reverse' || namedBookFold?.kind === 'outside_reverse'
      ? applyNamedReverseFoldTransaction(
          token,
          namedBookFold.document,
          namedBookFold.techniqueId,
        )
      : namedBookFold && !unsupportedNamedPhysicalFold
        ? basicFoldTimelinePreview?.transactionToken === token
          ? applyNamedBookFoldTransaction(
          token,
          namedBookFold.document,
          namedBookFold.techniqueId,
          basicFoldTimelinePreview,
        ) : Promise.reject(new Error('certified basic-fold timeline preview required'))
        : applyStackedFoldTransaction(token)
  }

  async function readDyadicPoseGraph() {
    if (disabled || applying || dyadicGraphReading || liveHinges.length === 0) return
    const sequence = ++dyadicGraphSequenceRef.current
    const authority = authorityRef.current
    setDyadicGraphReading(true)
    setDyadicGraphRead(null)
    setDyadicPathPreview(null)
    try {
      const response = await readBoundedDyadicPoseGraphV1({
        expectedProjectInstanceId: authority.project_instance_id,
        expectedProjectId: authority.project_id,
        expectedRevision: authority.revision,
        targetAngles: liveHinges.map((hinge) => ({
          edge: hinge.edge,
          angleDegrees: requestedHingeAngles[hinge.edge] ?? hinge.initialAngleDegrees,
        })),
        maxStates: dyadicLevelCount === 3 ? 2187 : dyadicLevelCount === 5 ? 125 : 128,
        maxTransitions: dyadicLevelCount === 3 ? 20412 : dyadicLevelCount === 5 ? 600 : 512,
        levelCount: dyadicLevelCount,
        ...(authoredCycleSchedule ? { cycleScheduleV1: authoredCycleSchedule } : {}),
      })
      const current = authorityRef.current
      if (sequence !== dyadicGraphSequenceRef.current
        || !matchesProjectOccGuard({
          expectedProjectInstanceId: authority.project_instance_id,
          expectedProjectId: authority.project_id,
          expectedRevision: authority.revision,
        }, current)) return
      setDyadicGraphRead(response)
    } catch {
      if (sequence === dyadicGraphSequenceRef.current) setDyadicGraphRead(null)
    } finally {
      if (sequence === dyadicGraphSequenceRef.current) setDyadicGraphReading(false)
    }
  }

  async function mintDyadicPathPreview() {
    const graph = dyadicGraphRead
    if (!graph?.mutationCandidateReady || !graph.certificateBindingSha256
      || !graph.positiveThicknessBindingSha256 || !graph.layerTransportBindingSha256
      || disabled || applying || dyadicGraphReading) return
    const authority = authorityRef.current
    try {
      const response = await mintDyadicPosePathPreviewV1({
        expectedProjectInstanceId: authority.project_instance_id,
        expectedProjectId: authority.project_id,
        expectedRevision: authority.revision,
        targetAngles: liveHinges.map((hinge) => ({
          edge: hinge.edge,
          angleDegrees: requestedHingeAngles[hinge.edge] ?? hinge.initialAngleDegrees,
        })),
        maxStates: dyadicLevelCount === 3 ? 2187 : dyadicLevelCount === 5 ? 125 : 128,
        maxTransitions: dyadicLevelCount === 3 ? 20412 : dyadicLevelCount === 5 ? 600 : 512,
        levelCount: dyadicLevelCount,
        ...(authoredCycleSchedule ? { cycleScheduleV1: authoredCycleSchedule } : {}),
        expectedPathBindingSha256: graph.certificateBindingSha256,
        expectedPositiveThicknessBindingSha256: graph.positiveThicknessBindingSha256,
        expectedLayerTransportBindingSha256: graph.layerTransportBindingSha256,
      })
      const current = authorityRef.current
      if (matchesProjectOccGuard({
        expectedProjectInstanceId: authority.project_instance_id,
        expectedProjectId: authority.project_id,
        expectedRevision: authority.revision,
      }, current)) setDyadicPathPreview(response)
    } catch {
      setDyadicPathPreview(null)
    }
  }

  async function applyDyadicPathPreview() {
    const preview = dyadicPathPreview
    if (!preview || disabled || applying) return
    setApplying(true)
    try {
      await applyDyadicPosePathPreviewV1({
        previewToken: preview.previewToken,
        expectedProjectInstanceId: preview.projectInstanceId,
        expectedProjectId: preview.projectId,
        expectedRevision: preview.revision,
        expectedTargetBindingSha256: preview.targetBindingSha256,
        expectedPathBindingSha256: preview.pathBindingSha256,
        expectedPositiveThicknessBindingSha256: preview.positiveThicknessBindingSha256,
        expectedLayerTransportBindingSha256: preview.layerTransportBindingSha256,
      })
      setDyadicPathPreview(null)
      setDyadicGraphRead(null)
      onApplied(await refreshSnapshot())
    } catch {
      setDyadicPathPreview(null)
    } finally {
      setApplying(false)
    }
  }

  async function previewCurrentCyclePose(automaticKawasaki = false) {
    if ((!automaticKawasaki && !authoredCycleSchedule) || disabled || applying || cyclePoseReading) return
    const sequence = ++cyclePoseSequenceRef.current
    void cancelCurrentStackedFoldReadV1().catch(() => undefined)
    cancelToken(tokenRef.current)
    tokenRef.current = null
    setCyclePoseReading(true)
    cyclePoseActiveRef.current = true
    setCyclePoseError(false)
    setCyclePoseProgress(null)
    const progressRequestId =
      `current-cycle:${snapshot.project_instance_id}:${snapshot.revision}:${sequence}`
    progressRequestRef.current = progressRequestId
    setPathProgress(null)
    try {
      const response = await proposeCurrentCyclePoseV1({
        progressRequestId,
        expectedProjectInstanceId: snapshot.project_instance_id,
        expectedProjectId: snapshot.project_id,
        expectedRevision: snapshot.revision,
        cycleScheduleV1: automaticKawasaki
          ? { version: 2, entries: [], endpointDenominator: selectedKawasakiEndpoint }
          : authoredCycleSchedule!,
      })
      const current = authorityRef.current
      if (
        sequence !== cyclePoseSequenceRef.current ||
        !matchesProjectOccGuard({
          expectedProjectInstanceId: snapshot.project_instance_id,
          expectedProjectId: snapshot.project_id,
          expectedRevision: snapshot.revision,
        }, current)
      ) {
        cancelToken(response.transactionToken)
        return
      }
      tokenRef.current = response.transactionToken
      setCyclePosePreview(response)
    } catch {
      setCyclePosePreview(null)
      setCyclePoseError(true)
    } finally {
      if (sequence === cyclePoseSequenceRef.current) {
        cyclePoseActiveRef.current = false
        progressRequestRef.current = null
        setCyclePoseReading(false)
      }
    }
  }

  async function applyCurrentCyclePose() {
    const token = cyclePosePreview?.transactionToken
    if (
      !token || token !== tokenRef.current || disabled || applying ||
      cyclePoseApplyInFlightRef.current
    ) return
    cyclePoseApplyInFlightRef.current = true
    setApplying(true)
    try {
      await applyTransaction(token)
      tokenRef.current = null
      setCyclePosePreview(null)
      const next = await refreshSnapshot()
      onApplied(next)
    } catch {
      setCyclePoseError(true)
    } finally {
      cyclePoseApplyInFlightRef.current = false
      setApplying(false)
    }
  }

  async function retryRefresh() {
    setApplying(true)
    try {
      onApplied(await refreshSnapshot())
      setView({ kind: 'idle' })
    } catch {
      setView({ kind: 'refresh_failed' })
    } finally {
      setApplying(false)
    }
  }

  const ready = view.kind === 'ready' && view.response.transactionProposal.readyForAtomicApply
  const certificateModelText = view.kind === 'ready'
    ? describeCertificateModel(
        view.response.continuousPath.continuousCertificateModelId,
        locale,
      )
    : ''
  const failureText = view.kind === 'ready'
    ? view.response.transactionProposal.failureClasses.map((failure) =>
        failure === 'continuous_path_uncertified'
          ? text(TEXT.theContinuousPathIsNotCollisionCertified)
          : text(TEXT.theTargetLayerOrderIsNotCertified))
    : []

  return (
    <section className="property-section stacked-fold-panel" aria-busy={view.kind === 'reading' || applying}>
      {namedTechniquePalette.length > 0 && (
        <fieldset aria-label={text(TEXT.techniquePalette)}>
          <legend>{text(TEXT.techniquePalette)}</legend>
          <p id="technique-palette-help">
            {text(TEXT.chooseATechniqueReviewItsSafetyPreviewThenApplyIt)}
          </p>
          <div role="list" aria-describedby="technique-palette-help">
            {namedTechniquePalette.map((item) => (
              <div role="listitem" key={item.techniqueId}>
                <button
                  type="button"
                  aria-pressed={namedBookFold?.techniqueId === item.techniqueId}
                  aria-describedby={!item.supported ? `technique-reason-${item.techniqueId}` : undefined}
                  disabled={disabled || applying || !item.supported}
                  onClick={() => onSelectNamedTechnique?.(item.techniqueId)}
                >
                  {item.name}
                </button>
                {!item.supported && (
                  <span id={`technique-reason-${item.techniqueId}`}>
                    {item.reason ?? text(TEXT.unsupportedAsACertifiedPhysicalOperation)}
                  </span>
                )}
              </div>
            ))}
          </div>
        </fieldset>
      )}
      <h2>{text(TEXT.straightLineStackedFold)}</h2>
      <p className="muted">
        {selectedLine
          ? text(TEXT.theSelectedLineIsUsedAsTheAxisForA)
          : text(TEXT.selectAFoldAxisLineOnThe2DCanvas)}
      </p>
      <p className="muted">
        {savedCompilerProvenance
          ? formattedText(TEXT.savedCompilerProvenance, {
            kind: savedCompilerProvenance.kind,
            count: savedCompilerProvenance.segmentCount,
          })
          : text(TEXT.noSavedCompilerProofInformation)}
      </p>
      {liveHinges.length > 0 && view.kind !== 'ready' && (
        <fieldset>
          <legend>{text(TEXT.hingeAngleCandidate)}</legend>
          {liveHinges.map((hinge, index) => (
            <div key={`${hinge.edge}:${index}`}>
              <label>
                <span>{text(TEXT.initialAngleReadOnly)}</span>
                <input aria-label={`${text(TEXT.initialAngle)} ${hinge.edge}`} type="number" value={hinge.initialAngleDegrees} readOnly />
              </label>
              <label>
                <span>{text(TEXT.requestedAngle)}</span>
                <input
                  aria-label={`${text(TEXT.requestedAngle)} ${hinge.edge}`}
                  type="number"
                  min="0"
                  max="180"
                  step="any"
                  value={requestedHingeAngles[hinge.edge] ?? hinge.initialAngleDegrees}
                  disabled={disabled || applying}
                  onChange={(event) => {
                    const requested = Number(event.target.value)
                    if (!Number.isFinite(requested) || requested < 0 || requested > 180) return
                    setRequestedHingeAngles((current) => ({ ...current, [hinge.edge]: requested }))
                  }}
                />
              </label>
            </div>
          ))}
        </fieldset>
      )}
      {liveHinges.length > 0 && view.kind !== 'ready' && (
        <section aria-label={text(TEXT.automaticEvenCycleCandidates)}>
          <h3>{text(TEXT.automaticEvenCycleCandidates)}</h3>
          {evenCycleCandidates.map((candidate) => (
            <button
              type="button"
              key={candidate.edges.join(':')}
              data-testid="even-cycle-candidate"
              disabled={disabled || applying}
              onClick={() => {
                const selected = new Set(candidate.edges)
                const requested = Number(angle)
                setRequestedHingeAngles(Object.fromEntries(liveHinges.map((hinge) => [
                  hinge.edge,
                  selected.has(hinge.edge) ? requested : hinge.initialAngleDegrees,
                ])))
              }}
            >
              {candidate.edges.join(' / ')} — {text(TEXT.sameAssignmentOppositeAxes)}
            </button>
          ))}
          {evenCycleCandidates.length === 0 && (
            <p data-even-cycle-status={evenCycleStatus}>
              {evenCycleStatus === 'resource_limit'
                ? text(TEXT.candidateSearchExceededItsResourceBound)
                : evenCycleStatus === 'none'
                  ? text(TEXT.noMatchingOppositeHingePairExists)
                  : text(TEXT.theCurrentShapeIsNotASupportedEvenSingleVertex)}
            </p>
          )}
        </section>
      )}
      <form onSubmit={(event) => void preview(event)}>
        <label>
          <span>{text(TEXT.fixedSide)}</span>
          <select value={fixedSide} onChange={(event) => setFixedSide(event.target.value as StackedFoldFixedSide)} disabled={disabled || applying}>
            <option value="left">{text(TEXT.leftOfLine)}</option>
            <option value="right">{text(TEXT.rightOfLine)}</option>
          </select>
        </label>
        <label>
          <span>{text(TEXT.cyclePathDefinitionJSONCyclicPatternsOnly)}</span>
          <textarea
            value={cycleScheduleText}
            onChange={(event) => setCycleScheduleText(event.target.value)}
            rows={4}
            maxLength={MAX_CYCLE_SCHEDULE_JSON_BYTES}
            spellCheck={false}
            placeholder={text(TEXT.version1HalfAngleRationalScheduleCyclesWithoutOneCannot)}
            disabled={disabled || applying}
          />
          {cycleScheduleText.trim() && (
            <small role="status">
              {authoredCycleSchedule
                ? formattedText(TEXT.boundedSchedule, {
                  count: authoredCycleSchedule.entries.length,
                })
                : text(TEXT.invalidScheduleDenominatorsMustBePositiveIntegersCoefficients19)}
            </small>
          )}
        </label>
        <label>
          <span>{text(TEXT.rotationDirection)}</span>
          <select value={rotationDirection} onChange={(event) => setRotationDirection(event.target.value as StackedFoldRotationDirection)} disabled={disabled || applying}>
            <option value="positive">{text(TEXT.positive)}</option>
            <option value="negative">{text(TEXT.negative)}</option>
          </select>
        </label>
        <label>
          <span>{text(TEXT.angleDegrees)}</span>
          <input value={angle} onChange={(event) => setAngle(event.target.value)} type="number" min="0.000001" max="180" step="any" required disabled={disabled || applying} />
        </label>
        <button type="submit" disabled={!selectedLine || disabled || applying || view.kind === 'reading'}>
          {view.kind === 'reading' ? text(TEXT.proving) : text(TEXT.verifySafety)}
        </button>
      </form>
      {(authoredCycleSchedule || evenCycleCandidates.length > 0) && (
        <section aria-label={text(TEXT.currentPoseCyclePreview)}>
          <h3>{text(TEXT.currentPoseCycle)}</h3>
          <button
            ref={cyclePosePreviewButtonRef}
            type="button"
            disabled={!authoredCycleSchedule || disabled || applying || cyclePoseReading}
            onClick={() => void previewCurrentCyclePose(false)}
          >
            {cyclePoseReading
              ? text(TEXT.provingPath)
              : text(TEXT.proveFromCurrentPose)}
          </button>
            {evenCycleCandidates.length > 0 && (
            <button
              type="button"
              data-testid="automatic-kawasaki-proof"
              disabled={disabled || applying || cyclePoseReading}
              onClick={() => void previewCurrentCyclePose(true)}
            >
              {text(TEXT.generateAndProveKawasakiLinkage)}
              </button>
            )}
            {kawasakiEndpoints.length > 0 && (
              <ul data-testid="kawasaki-endpoint-candidates">
                {kawasakiEndpoints.map((candidate) => (
                  <li key={candidate.endpointDenominator}>
                    <button
                      type="button"
                      aria-pressed={selectedKawasakiEndpoint === candidate.endpointDenominator}
                      onClick={() => setSelectedKawasakiEndpoint(candidate.endpointDenominator as 1 | 2 | 4 | 8 | 16)}
                    >
                    1/{candidate.endpointDenominator}: {text(TEXT.closureCertified)} /{' '}
                    {candidate.collisionStatus === 'certified'
                      ? text(TEXT.collisionCertified)
                      : text(TEXT.collisionUncertified)}
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <button
              type="button"
              data-testid="dyadic-pose-graph-read"
              disabled={disabled || applying || dyadicGraphReading || liveHinges.length === 0}
              onClick={() => void readDyadicPoseGraph()}
            >
              {dyadicGraphReading ? text(TEXT.searchingPaths) : text(TEXT.searchBoundedDyadicPaths)}
            </button>
            <label>
              {text(TEXT.dyadicLevels)}
              <select
                aria-label={text(TEXT.dyadicLevels)}
                value={dyadicLevelCount}
                disabled={disabled || applying || dyadicGraphReading}
                onChange={(event) => setDyadicLevelCount(Number(event.target.value) as 3 | 5 | 9)}
              >
                {[3, 5, 9].map((level) => <option key={level} value={level}>{level}</option>)}
              </select>
            </label>
            {dyadicGraphReading && (
              <button type="button" onClick={() => {
                dyadicGraphSequenceRef.current += 1
                setDyadicGraphReading(false)
                void cancelCurrentStackedFoldReadV1().catch(() => undefined)
              }}>{text(TEXT.cancelSearch)}</button>
            )}
            {dyadicGraphRead && (
              <p data-testid="dyadic-pose-graph-status" role="status">
                {dyadicGraphRead.status}; reason {dyadicGraphRead.reason}; states {dyadicGraphRead.stateCount}; transitions {dyadicGraphRead.transitionCount}; explored {dyadicGraphRead.exploredStateCount}; evaluated {dyadicGraphRead.evaluatedTransitionCount}; read-only
                ; certified transitions {dyadicGraphRead.certifiedTransitionCount}; binding {dyadicGraphRead.certificateBindingSha256 ?? 'unavailable'}; positive thickness {dyadicGraphRead.positiveThicknessCertified ? `certified ${dyadicGraphRead.positiveThicknessTransitionCount}/${dyadicGraphRead.certifiedTransitionCount}` : 'not certified'}; layer transport {dyadicGraphRead.layerTransportCertified ? `certified ${dyadicGraphRead.layerTransportTransitionCount}/${dyadicGraphRead.certifiedTransitionCount}` : 'not certified'}; mutation candidate {dyadicGraphRead.mutationCandidateReady ? 'ready' : 'not ready'}; Apply disabled
              </p>
            )}
            {dyadicGraphRead?.mutationCandidateReady && (
              <button type="button" data-testid="dyadic-path-preview" onClick={() => void mintDyadicPathPreview()}>
                {text(TEXT.issueReadOnlyPreview)}
              </button>
            )}
            {dyadicPathPreview && (
              <>
                <p data-testid="dyadic-path-preview-status" role="status">
                  preview {dyadicPathPreview.previewToken}; target {dyadicPathPreview.targetBindingSha256}; authenticated one-shot
                </p>
                <button type="button" data-testid="dyadic-path-apply" disabled={disabled || applying} onClick={() => void applyDyadicPathPreview()}>
                  {text(TEXT.applyAuthenticatedPath)}
                </button>
              </>
            )}
          {cyclePoseReading && pathProgress && (
            <p role="status">
              {formattedText(TEXT.cyclePathProgress, {
                states: pathProgress.exploredStateCount,
                stateLimit: pathProgress.stateLimit,
                transitions: pathProgress.evaluatedTransitionCount,
                transitionLimit: pathProgress.transitionLimit,
              })}
            </p>
          )}
          {cyclePoseReading && (
            <button
              type="button"
              onClick={() => {
                const cancelledRequestId = progressRequestRef.current ?? 'current-cycle-cancelled'
                cyclePoseSequenceRef.current += 1
                cyclePoseActiveRef.current = false
                progressRequestRef.current = null
                setPathProgress(null)
                setCyclePoseReading(false)
                setCyclePoseProgress({
                  version: 1,
                  requestId: cancelledRequestId,
                  status: 'cancelled',
                  completedWork: 2,
                  totalWork: 2,
                  authorizesProjectMutation: false,
                })
                void cancelCurrentStackedFoldReadV1().catch(() => undefined)
              }}
            >
              {text(TEXT.cancelCycleProof)}
            </button>
          )}
          {cyclePoseProgress?.status === 'cancelled' && (
            <p role="status">
              {text(TEXT.cycleProofCancelledYouCanRetry)}
            </p>
          )}
          {cyclePoseError && (
            <p role="alert">
              {text(TEXT.theCyclePathCouldNotBeAuthenticatedTheProjectWas)}
            </p>
          )}
          {cyclePosePreview && (
            <div
              ref={cyclePoseProofRef}
              role="status"
              tabIndex={-1}
              className="stacked-fold-proof"
            >
              <dl>
                <div>
                  <dt>{text(TEXT.closureIntervals)}</dt>
                  <dd>{cyclePosePreview.closureLeafCount}</dd>
                </div>
                <div>
                  <dt>{text(TEXT.maximumProofDepth)}</dt>
                  <dd>{cyclePosePreview.closureMaxDepth}</dd>
                </div>
                <div>
                  <dt>{text(TEXT.allHingesCovered)}</dt>
                  <dd>{cyclePosePreview.checkedHingeCount}/{cyclePosePreview.totalHingeCount}</dd>
                </div>
                <div>
                  <dt>{text(TEXT.continuousPath)}</dt>
                  <dd>{text(TEXT.certified)}</dd>
                </div>
                <div>
                  <dt>{text(TEXT.targetRevision)}</dt>
                  <dd>{cyclePosePreview.targetRevision}</dd>
                </div>
                <div>
                  <dt>Layer-order transitions</dt>
                  <dd data-testid="cycle-layer-transition-count">{cyclePosePreview.continuousLayerTransitionCount}</dd>
                </div>
                <div>
                  <dt>Layer-order pairs</dt>
                  <dd>{cyclePosePreview.continuousLayerPairOrderCount}</dd>
                </div>
                <div>
                  <dt>Layer-order proof hash</dt>
                  <dd>{cyclePosePreview.continuousLayerTargetOrderSha256 ?? 'Unavailable'}</dd>
                </div>
              </dl>
              {cyclePosePreview.continuousLayerTransportModelId && (
                <div data-testid="cycle-layer-order-viewer">
                  <h4>Layer-order preview</h4>
                  <p>Source: {cyclePosePreview.sourceLayerOrder.length}</p>
                  <p>Target: {cyclePosePreview.targetLayerOrder.length}</p>
                  <ol>
                    {cyclePosePreview.targetLayerOrder.map((pair) => (
                      <li key={`${pair.lowerFace}:${pair.upperFace}`}>
                        {pair.lowerFace} → {pair.upperFace}
                      </li>
                    ))}
                  </ol>
                </div>
              )}
              <p>
                {text(TEXT.thisPreviewIsReadOnlyTheProjectIsUnchangedUntil)}
              </p>
              <button
                type="button"
                disabled={disabled || applying}
                onClick={() => void applyCurrentCyclePose()}
              >
                {text(TEXT.applyCertifiedCycleFold)}
              </button>
              <button
                type="button"
                disabled={applying}
                onClick={() => {
                  cancelToken(cyclePosePreview.transactionToken)
                  tokenRef.current = null
                  setCyclePosePreview(null)
                  queueMicrotask(() => cyclePosePreviewButtonRef.current?.focus())
                }}
              >
                {text(TEXT.cancelPreview)}
              </button>
            </div>
          )}
        </section>
      )}
      {!cyclePosePreview && persistedCycleLayerProof && (
        <section
          aria-label={text(TEXT.appliedLayerOrderViewer)}
          data-testid="persisted-cycle-layer-order-viewer"
          className="stacked-fold-proof"
        >
          <h4>{text(TEXT.appliedLayerOrderProof)}</h4>
          <p>{text(TEXT.transitions)}: {persistedCycleLayerProof.transition_count}</p>
          <p>{text(TEXT.pairs)}: {persistedCycleLayerProof.pairs.length}</p>
          <p>{text(TEXT.proofHash)}: {persistedCycleLayerProof.target_order_sha256
            .map((byte) => byte.toString(16).padStart(2, '0')).join('')}</p>
          <ol aria-label={text(TEXT.canonicalProofPairsLowerToUpper)}>
            {persistedLayerPairs.map((pair) => (
              <li key={`${pair.lower_face}:${pair.upper_face}`}>
                <button type="button" aria-pressed={selectedFace === pair.lower_face}
                  onClick={() => setSelectedFace(pair.lower_face)}>
                  {pair.lower_face}
                </button>
                {' → '}
                <button type="button" aria-pressed={selectedFace === pair.upper_face}
                  onClick={() => setSelectedFace(pair.upper_face)}>
                  {pair.upper_face}
                </button>
              </li>
            ))}
          </ol>
          {persistedCycleLayerProof.pairs.length > persistedLayerPairs.length && (
            <p>{formattedText(TEXT.persistedLayerPairsOmitted, {
              visible: persistedLayerPairs.length,
              remaining:
                persistedCycleLayerProof.pairs.length - persistedLayerPairs.length,
            })}</p>
          )}
          <p>{text(TEXT.thisIsAReadOnlyViewOfTheProofPersisted)}</p>
          <p>{text(TEXT.thisViewShowsOnlyThePersistedProofItDoesNot)}</p>
        </section>
      )}
      {view.kind === 'failed' && (
        <p role="alert">
          {view.reason === 'stale'
            ? text(TEXT.theProjectChangedVerifyAgain)
            : view.reason === 'cycle_nonclosing'
              ? text(TEXT.theCyclicHingeEndpointDoesNotCloseSoApplyIs)
              : view.reason === 'cycle_path_uncertified'
                ? text(TEXT.theCyclicEndpointClosesButItsContinuousPathIsUncertified)
                : view.reason === 'cycle_path_unsupported'
                  ? text(TEXT.staticReasonTheHingeGraphAndScheduleDoNotMatch)
                  : view.reason === 'cycle_path_resource_limit'
                    ? text(TEXT.theBoundedProofReachedItsResourceLimitThisDoesNot)
                    : view.reason === 'cycle_path_no_certified_path'
                      ? text(TEXT.noPathToTheTargetWasFoundUsingCertifiedTransitions)
                      : view.reason === 'cycle_path_cancelled'
                        ? text(TEXT.theBoundedPathAnalysisWasCancelledNoPartialCertificateWas)
                    : view.reason === 'cycle_path_collision'
                      ? text(TEXT.theScheduledContinuousPathCouldNotReceiveACollisionClearance)
            : view.reason === 'apply'
              ? text(TEXT.applyFailedThePreviewIsNoLongerTrusted)
              : text(TEXT.aNativeProofCouldNotBeCompletedForThisInput)}
        </p>
      )}
      {view.kind === 'reading' && (
        <div>
          {pathProgress && (
            <p role="status">
              {formattedText(TEXT.searchPathProgress, {
                states: pathProgress.exploredStateCount,
                stateLimit: pathProgress.stateLimit,
                transitions: pathProgress.evaluatedTransitionCount,
                transitionLimit: pathProgress.transitionLimit,
              })}
            </p>
          )}
        <button
          type="button"
          onClick={() => {
            progressRequestRef.current = null
            setPathProgress(null)
            void cancelCurrentStackedFoldReadV1().catch(() => undefined)
          }}
        >
          {text(TEXT.cancelPathAnalysis)}
        </button>
        </div>
      )}
      {view.kind === 'refresh_failed' && (
        <div role="alert">
          <p>{text(TEXT.theStackedFoldWasAppliedButTheRefreshedProjectCould)}</p>
          <button type="button" disabled={applying} onClick={() => void retryRefresh()}>
            {text(TEXT.retryRefresh)}
          </button>
        </div>
      )}
      {view.kind === 'ready' && (
        <div className="stacked-fold-proof" data-ready={ready}>
          {liveHinges.length > 0 && (
            <fieldset>
              <legend>{text(TEXT.hingeAngleCandidate)}</legend>
              {liveHinges.map((hinge, index) => (
                <div key={`${hinge.edge}:${index}`}>
                  <label>
                    <span>{text(TEXT.initialAngleReadOnly)}</span>
                    <input
                      aria-label={`${text(TEXT.initialAngle)} ${hinge.edge}`}
                      type="number"
                      value={hinge.initialAngleDegrees}
                      readOnly
                    />
                  </label>
                  <label>
                    <span>{text(TEXT.requestedAngle)}</span>
                    <input
                      id={`stacked-fold-proof-hinge-${hinge.edge}`}
                      aria-label={`${text(TEXT.requestedAngle)} ${hinge.edge}`}
                      type="number"
                      min="0"
                      max="180"
                      step="any"
                      value={requestedHingeAngles[hinge.edge] ?? hinge.initialAngleDegrees}
                      disabled={disabled || applying}
                      onChange={(event) => {
                        const requested = Number(event.target.value)
                        if (!Number.isFinite(requested) || requested < 0 || requested > 180) return
                        setRequestedHingeAngles((current) => ({
                          ...current,
                          [hinge.edge]: requested,
                        }))
                      }}
                    />
                  </label>
                </div>
              ))}
              <p className="muted">
                {text(TEXT.editingARequestedAngleBuildsCanonicalLinearCandidateV1InternallyNativeRe)}
              </p>
            </fieldset>
          )}
          <dl>
            <div><dt>{text(TEXT.targetFaces)}</dt><dd>{view.response.targetFaces.length}</dd></div>
            <div><dt>{text(TEXT.creases)}</dt><dd>{view.response.materialSegments.length}</dd></div>
            <div><dt>{text(TEXT.targetHinges)}</dt><dd>{view.response.topologyProof.targetHingeCount}</dd></div>
            <div><dt>{text(TEXT.endpointCollision)}</dt><dd>{view.response.endpointCollision.hasBlockingHold ? text(TEXT.blocked) : text(TEXT.clear)}</dd></div>
            <div><dt>{text(TEXT.continuousPath)}</dt><dd>{view.response.continuousPath.continuousClearanceCertified ? text(TEXT.certified2) : text(TEXT.uncertified)}</dd></div>
            <div><dt>{text(TEXT.firstProvenBlockingSample)}</dt><dd>{view.response.continuousPath.firstSampledBlockingAngleDegrees === null ? text(TEXT.none) : `${view.response.continuousPath.firstSampledBlockingAngleDegrees}°`}</dd></div>
            <div><dt>{text(TEXT.pathCertificateModel)}</dt><dd>{certificateModelText}</dd></div>
            <div><dt>{text(TEXT.intervalLeaves)}</dt><dd>{view.response.continuousPath.intervalLeafCount}</dd></div>
            <div><dt>{text(TEXT.intervalPairWork)}</dt><dd>{view.response.continuousPath.intervalPairWork}</dd></div>
            <div><dt>{text(TEXT.positiveThicknessCandidates)}</dt><dd>{view.response.continuousPath.positiveEndpointCandidateCount} / {view.response.continuousPath.positiveEndpointCandidateLimit}</dd></div>
            <div><dt>{text(TEXT.positiveThicknessExactCalls)}</dt><dd>{view.response.continuousPath.positiveEndpointExactPairCalls}</dd></div>
            <div><dt>{text(TEXT.candidateLimit)}</dt><dd>{view.response.continuousPath.intervalCandidateLimit}</dd></div>
            <div><dt>{text(TEXT.closureLeaves)}</dt><dd>{view.response.continuousPath.closureLeafCount}</dd></div>
            <div><dt>{text(TEXT.closurePairWork)}</dt><dd>{view.response.continuousPath.closurePairWork}</dd></div>
            <div><dt>{text(TEXT.firstClosureFailureAngle)}</dt><dd>{view.response.continuousPath.firstClosureFailureAngleDegrees ?? text(TEXT.none)}</dd></div>
            <div><dt>{text(TEXT.certifiedThickness)}</dt><dd>{view.response.continuousPath.paperThicknessMm} mm</dd></div>
            <div><dt>{text(TEXT.layerOrder)}</dt><dd>{view.response.flatEndpointLayerOrder.certified ? text(TEXT.certified2) : text(TEXT.uncertified)}</dd></div>
            <div><dt>{text(TEXT.addedVerticesEdges)}</dt><dd>{view.response.transactionProposal.addedVertexCount} / {view.response.transactionProposal.addedEdgeCount}</dd></div>
          </dl>
          {view.response.certifiedPathGraph && (
            <section aria-label={text(TEXT.certifiedCandidatePath)}>
              <h4>{text(TEXT.certifiedCandidatePath)}</h4>
              <p>
                {formattedText(TEXT.certifiedPathTransitionCount, {
                  count: view.response.certifiedPathGraph.edges.length,
                })}
              </p>
              <ol>
                {view.response.certifiedPathGraph.edges.map((edge, index) => (
                  <li key={`${edge.sourceFingerprintSha256}:${edge.targetFingerprintSha256}`}>
                    <strong>{formattedText(TEXT.transitionIndex, {
                      index: index + 1,
                    })}</strong>
                    <dl>
                      <div><dt>{text(TEXT.scheduleCertificate)}</dt><dd>{edge.scheduleCertificateSha256}</dd></div>
                      <div><dt>{text(TEXT.collisionCertificate)}</dt><dd>{edge.collisionCertificateSha256}</dd></div>
                      <div><dt>{text(TEXT.closureCertificate)}</dt><dd>{edge.closureCertificateSha256}</dd></div>
                    </dl>
                    {edge.hinges.map((hinge, hingeIndex) => (
                      <button
                        key={`${hinge}:${hingeIndex}`}
                        type="button"
                        onClick={() => document.getElementById(
                          `stacked-fold-proof-hinge-${hinge}`,
                        )?.focus()}
                      >
                        {text(TEXT.selectRelatedHinge)} {hingeIndex + 1}
                      </button>
                    ))}
                  </li>
                ))}
              </ol>
            </section>
          )}
          <p>{text(TEXT.thisCertificateCoversOnlyTheDisplayedThicknessTwoTriangularFaces)}</p>
          <LayerOrderViewer
            locale={locale}
            cells={view.response.crossedCells}
            selectedCell={selectedCell}
            selectedFace={selectedFace}
            hoveredFace={hoveredFace}
            onSelectCell={setSelectedCell}
            onSelectFace={setSelectedFace}
            onHoverFace={setHoveredFace}
          />
          {failureText.map((failure) => <p role="status" key={failure}>{failure}</p>)}
          {view.applyFailed && (
            <p role="alert">{text(TEXT.applyFailedYouCanRetryWithTheSameCertifiedPreview)}</p>
          )}
          {namedBookFold && (
            <p role="note">
              {formattedText(TEXT.namedTechniqueWillBeSaved, {
                name: namedBookFold.name,
              })}
            </p>
          )}
          {namedBasicFold && (
            <section aria-label={text(TEXT.namedBasicFoldTimelinePreview)}>
              <button type="button" onClick={() => void previewNamedBasicFold()}
                aria-busy={basicFoldTimelinePreviewReading}
                disabled={!ready || applying || basicFoldTimelinePreviewReading}>
                {text(TEXT.previewCertifiedTimeline)}
              </button>
              {basicFoldTimelinePreviewReading && (
                <p role="status" aria-live="polite">{text(TEXT.buildingCertifiedTimeline)}</p>
              )}
              {basicFoldTimelinePreview && (
                <div role="status" tabIndex={0}
                  aria-label={text(TEXT.certifiedTimelineStepPlayer)}
                  onKeyDown={(event) => {
                    const last = basicFoldTimelinePreview.timeline.steps.length - 1
                    if (event.key === 'ArrowLeft') setBasicFoldTimelineStepIndex((value) => Math.max(0, value - 1))
                    else if (event.key === 'ArrowRight') setBasicFoldTimelineStepIndex((value) => Math.min(last, value + 1))
                    else if (event.key === 'Home') setBasicFoldTimelineStepIndex(0)
                    else if (event.key === 'End') setBasicFoldTimelineStepIndex(last)
                    else return
                    event.preventDefault()
                  }}>
                  <p>{text(TEXT.readOnlyPreviewNoMutationAuthorityIsIncluded)}</p>
                  <p aria-live="polite">{text(TEXT.step)} {basicFoldTimelineStepIndex + 1} / {basicFoldTimelinePreview.timeline.steps.length}</p>
                  <button type="button" disabled={basicFoldTimelineStepIndex === 0}
                    onClick={() => setBasicFoldTimelineStepIndex((value) => Math.max(0, value - 1))}>{text(TEXT.previousStep)}</button>
                  <button type="button" disabled={basicFoldTimelineStepIndex + 1 >= basicFoldTimelinePreview.timeline.steps.length}
                    onClick={() => setBasicFoldTimelineStepIndex((value) => Math.min(basicFoldTimelinePreview.timeline.steps.length - 1, value + 1))}>{text(TEXT.nextStep)}</button>
                  {basicFoldTimelineStep && (
                    <section aria-label={text(TEXT.stepDetailPreview)}>
                      <h4>{basicFoldTimelineStep.title}</h4>
                      <p>{basicFoldTimelineStep.description}</p>
                      <dl>
                        <div><dt>{text(TEXT.fixedFace)}</dt><dd>{basicFoldTimelineStep.pose.fixed_face ?? text(TEXT.none)}</dd></div>
                        <div><dt>{text(TEXT.hingeCount)}</dt><dd>{basicFoldTimelineStep.pose.hinge_angles.length}</dd></div>
                        <div><dt>{text(TEXT.pathProof)}</dt><dd>{basicFoldTimelineStep.visual.path_certificate_reference_v1 ? text(TEXT.referenced) : text(TEXT.none)}</dd></div>
                      </dl>
                      <ol>{basicFoldTimelineStep.pose.hinge_angles.map((hinge, index) => <li key={`${hinge.edge}:${index}`}>{hinge.edge}: {hinge.angle_degrees}°</li>)}</ol>
                    </section>
                  )}
                </div>
              )}
              {basicFoldTimelinePreviewError && (
                <p role="alert">{text(TEXT.couldNotBuildACertifiedTimelineTreeStaleCancelledOr)}</p>
              )}
            </section>
          )}
          {unsupportedNamedPhysicalFold && (
            <div role="alert">
            <p>{text(
              namedBookFold?.kind === 'petal'
                ? TEXT.petalFoldUnsupported
                : TEXT.basicFoldKindUnsupported,
            )}</p>
            {namedBookFold?.kind === 'petal' && (
              <div aria-label={text(TEXT.missingPetalFoldProofPremises)}>
                <p>{text(TEXT.requiredGraphSegmentsAtLeast3InOneGraphChain)}</p>
                <ul>
                  <li>{text(TEXT.liftedFlapTopologyAuthority)}</li>
                  <li>{text(TEXT.adjacentFaceOpeningPathAuthority)}</li>
                  <li>{text(TEXT.finalFlatteningEndpointAuthority)}</li>
                  <li>{text(TEXT.continuousLayerOrderAuthorityAcrossEverySegment)}</li>
                </ul>
                <p>{text(TEXT.v1CertificatesDoNotBindThesePremisesTogetherSoNo)}</p>
              </div>
            )}
            </div>
          )}
          <label>
            <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} disabled={!ready || applying || unsupportedNamedPhysicalFold || (namedBasicFold && basicFoldTimelinePreview?.transactionToken !== tokenRef.current)} />
            {text(TEXT.iReviewedTheCertifiedChanges)}
          </label>
          <button type="button" onClick={() => void apply()} disabled={!ready || !confirmed || applying || unsupportedNamedPhysicalFold || (namedBasicFold && basicFoldTimelinePreview?.transactionToken !== tokenRef.current)}>
            {applying
              ? text(TEXT.applying)
              : namedBookFold
                ? namedBookFold.kind === 'layer' || namedBookFold.kind === 'layer_selective'
                  ? text(TEXT.applyNamedLayerTechnique)
                  : namedBookFold.kind === 'sink'
                  ? text(TEXT.applyNamedSinkFold)
                  : namedBookFold.kind === 'accordion'
                  ? text(TEXT.applyNamedAccordionFold)
                  : namedBookFold.kind === 'reverse' || namedBookFold.kind === 'inside_reverse'
                    || namedBookFold.kind === 'outside_reverse'
                  ? text(TEXT.applyNamedReverseFold)
                  : text(TEXT.applyNamedBookFold)
                : text(TEXT.applyStackedFold)}
          </button>
          {!ready && <p className="muted">{text(TEXT.applyIsDisabledBecauseTheCaseIsNotFullyCertified)}</p>}
        </div>
      )}
    </section>
  )
}

export function LayerOrderViewer({
  locale,
  cells,
  selectedCell,
  selectedFace,
  hoveredFace,
  onSelectCell,
  onSelectFace,
  onHoverFace,
}: Readonly<{
  locale: Locale
  cells: readonly LayerOrderViewerCell[]
  selectedCell: string | null
  selectedFace: string | null
  hoveredFace: string | null
  onSelectCell(value: string): void
  onSelectFace(value: string): void
  onHoverFace(value: string | null): void
}>) {
  const text = (localized: LocalizedText) =>
    selectLocalizedText(locale, localized)
  const formattedText = (
    localized: LocalizedText,
    variables: MessageVariables,
  ) => formatLocalizedText(locale, localized, variables)
  const active = cells.find((cell) => cell.cellKeySha256 === selectedCell) ?? cells[0]
  if (!active) return null
  const xs = active.boundaryWorld.map((point) => point[0])
  const zs = active.boundaryWorld.map((point) => point[2])
  const minX = Math.min(...xs); const maxX = Math.max(...xs)
  const minZ = Math.min(...zs); const maxZ = Math.max(...zs)
  const spanX = Math.max(maxX - minX, 1)
  const spanZ = Math.max(maxZ - minZ, 1)
  const polygon = active.boundaryWorld.map((point) =>
    `${20 + ((point[0] - minX) / spanX) * 180},${20 + ((point[2] - minZ) / spanZ) * 110}`,
  ).join(' ')
  return <section className="stacked-fold-layer-viewer" aria-label={text(TEXT.text3dLayerOrderViewer)}>
    <h3>{text(TEXT.overlapCellsAndLayerOrder)}</h3>
    <p className="muted">{text(TEXT.readOnlyViewOfTheAuthenticatedCurrentPoseAndLayer)}</p>
    <div className="stacked-fold-cell-tabs" role="list">
      {cells.map((cell, index) => <button type="button" role="listitem"
        aria-pressed={cell.cellKeySha256 === active.cellKeySha256}
        key={cell.cellKeySha256} onClick={() => onSelectCell(cell.cellKeySha256)}>
        {text(TEXT.cell)} {index + 1}
      </button>)}
    </div>
    <svg viewBox="0 0 240 180" role="img"
      aria-label={text(TEXT.explodedFrontBackLayerStack)}>
      {active.bottomToTopFaces.map((face, index) => {
        const offset = (active.bottomToTopFaces.length - 1 - index) * 9
        const highlighted = face === selectedFace || face === hoveredFace
        return <polygon key={`${face}:${index}`} points={polygon} transform={`translate(${offset} ${-offset})`}
          fill={highlighted ? '#f6b73c' : `hsl(${205 + index * 22} 55% 62%)`}
          fillOpacity="0.72" stroke={highlighted ? '#6b3e00' : '#29465b'}
          tabIndex={0} onClick={() => onSelectFace(face)}
          onMouseEnter={() => onHoverFace(face)} onMouseLeave={() => onHoverFace(null)}
          onFocus={() => onHoverFace(face)} onBlur={() => onHoverFace(null)}>
          <title>{formattedText(
            index === 0
              ? TEXT.backBottomFaceIndex
              : index === active.bottomToTopFaces.length - 1
                ? TEXT.frontTopFaceIndex
                : TEXT.middleLayerFaceIndex,
            { index: index + 1 },
          )}</title>
        </polygon>
      })}
    </svg>
    <ol className="stacked-fold-layer-list">
      {active.bottomToTopFaces.map((face, index) => <li key={`${face}:${index}`}>
        <button type="button" aria-pressed={face === selectedFace}
          onMouseEnter={() => onHoverFace(face)} onMouseLeave={() => onHoverFace(null)}
          onClick={() => onSelectFace(face)}>
          {index === 0 ? text(TEXT.backBottom)
            : index === active.bottomToTopFaces.length - 1
              ? text(TEXT.frontTop)
              : text(TEXT.middle)} · {text(TEXT.face)} {index + 1}
        </button>
      </li>)}
    </ol>
  </section>
}

function describeCertificateModel(
  modelId: string | null,
  locale: Locale,
): string {
  if (modelId === null) {
    return selectLocalizedText(locale, TEXT.none)
  }
  if (modelId.includes('positive_thickness')) {
    return selectLocalizedText(locale, TEXT.positiveThicknessContinuousPathCertificate)
  }
  return selectLocalizedText(locale, TEXT.zeroThicknessContinuousPathCertificate)
}
