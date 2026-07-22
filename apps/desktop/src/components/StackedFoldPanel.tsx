import { type FormEvent, useEffect, useMemo, useRef, useState } from 'react'
import {
  applyStackedFoldTransaction,
  applyNamedBookFoldTransaction,
  applyNamedReverseFoldTransaction,
  applyNamedAccordionFoldTransaction,
  applyNamedSinkFoldTransaction,
  applyNamedLayerSelectiveTransaction,
  cancelCurrentStackedFoldReadV1,
  cancelStackedFoldTransactionPreview,
  listenStackedFoldReadProgressV1,
  listenCurrentCyclePoseProgressV1,
  proposeCurrentCyclePoseV1,
  proposeCurrentStackedFoldRead,
  readEvenCycleCandidatesV1,
  readBoundedDyadicPoseGraphV1,
  mintDyadicPosePathPreviewV1,
  applyDyadicPosePathPreviewV1,
  readLiveHingeRegistryV1,
  type ProjectSnapshot,
  type CurrentCyclePosePreviewResponseV1,
  type CurrentCyclePoseProgressV1,
  type DyadicPoseGraphReadResponseV1,
  type DyadicPathPreviewResponseV1,
} from '../lib/coreClient'
import { selectLocalizedText, type Locale } from '../lib/i18n'
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
    kind?: 'book' | 'reverse' | 'accordion' | 'sink' | 'layer'
  }> | null
}>

const MAX_CYCLE_SCHEDULE_JSON_BYTES = 65_536

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
}: Props) {
  const t = (ja: string, en: string) => selectLocalizedText(locale, { ja, en })
  const authorityRef = useRef(snapshot)
  authorityRef.current = snapshot
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
  const dyadicGraphSequenceRef = useRef(0)
  const [confirmed, setConfirmed] = useState(false)
  const [applying, setApplying] = useState(false)
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
      if (proof?.version === 1 &&
        proof.model_id === 'native_continuous_layer_transport_certificate_v1' &&
        proof.target_order_sha256.length === 32 &&
        proof.target_order_sha256.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255) &&
        Number.isSafeInteger(proof.transition_count) && proof.transition_count > 0) return proof
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
    selectedLine?.id,
    fixedSide,
    rotationDirection,
    angle,
    cycleScheduleText,
  ])

  useEffect(() => () => {
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
      applying
    ) return
    const token = view.response.transactionProposal.transactionToken
    if (!token || token !== tokenRef.current) return
    setApplying(true)
    let committed = false
    try {
      await applyTransaction(token)
      committed = true
      tokenRef.current = null
      const next = await refreshSnapshot()
      onApplied(next)
      setView({ kind: 'idle' })
      setConfirmed(false)
    } catch {
      setView(committed
        ? { kind: 'refresh_failed' }
        : { kind: 'ready', response: view.response, applyFailed: true })
    } finally {
      setApplying(false)
    }
  }

  function applyTransaction(token: string) {
    return namedBookFold?.kind === 'layer'
      ? applyNamedLayerSelectiveTransaction(token, namedBookFold.document, namedBookFold.techniqueId)
      : namedBookFold?.kind === 'sink'
      ? applyNamedSinkFoldTransaction(token, namedBookFold.document, namedBookFold.techniqueId)
      : namedBookFold?.kind === 'accordion'
      ? applyNamedAccordionFoldTransaction(
          token, namedBookFold.document, namedBookFold.techniqueId,
        )
      : namedBookFold?.kind === 'reverse'
      ? applyNamedReverseFoldTransaction(
          token,
          namedBookFold.document,
          namedBookFold.techniqueId,
        )
      : namedBookFold
        ? applyNamedBookFoldTransaction(
          token,
          namedBookFold.document,
          namedBookFold.techniqueId,
        )
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
        maxStates: dyadicLevelCount === 9 ? 128 : 32,
        maxTransitions: dyadicLevelCount === 9 ? 512 : 128,
        levelCount: dyadicLevelCount,
        ...(authoredCycleSchedule ? { cycleScheduleV1: authoredCycleSchedule } : {}),
      })
      const current = authorityRef.current
      if (sequence !== dyadicGraphSequenceRef.current
        || current.project_instance_id !== authority.project_instance_id
        || current.project_id !== authority.project_id
        || current.revision !== authority.revision) return
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
        maxStates: dyadicLevelCount === 9 ? 128 : 32,
        maxTransitions: dyadicLevelCount === 9 ? 512 : 128,
        levelCount: dyadicLevelCount,
        ...(authoredCycleSchedule ? { cycleScheduleV1: authoredCycleSchedule } : {}),
        expectedPathBindingSha256: graph.certificateBindingSha256,
        expectedPositiveThicknessBindingSha256: graph.positiveThicknessBindingSha256,
        expectedLayerTransportBindingSha256: graph.layerTransportBindingSha256,
      })
      const current = authorityRef.current
      if (current.project_instance_id === authority.project_instance_id
        && current.project_id === authority.project_id
        && current.revision === authority.revision) setDyadicPathPreview(response)
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
        current.project_instance_id !== snapshot.project_instance_id ||
        current.project_id !== snapshot.project_id ||
        current.revision !== snapshot.revision
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
          ? t('連続経路の無衝突証明がありません。', 'The continuous path is not collision-certified.')
          : t('折り後の層順序を証明できません。', 'The target layer order is not certified.'))
    : []

  return (
    <section className="property-section stacked-fold-panel" aria-busy={view.kind === 'reading' || applying}>
      <h2>{t('一直線の折り重ね', 'Straight-line stacked fold')}</h2>
      <p className="muted">
        {selectedLine
          ? t('選択中の線を折り軸としてnative証明を作成します。', 'The selected line is used as the axis for a native proof.')
          : t('2Dキャンバスで折り軸にする線を選択してください。', 'Select a fold-axis line on the 2D canvas.')}
      </p>
      {liveHinges.length > 0 && view.kind !== 'ready' && (
        <fieldset>
          <legend>{t('ヒンジ角度候補', 'Hinge angle candidate')}</legend>
          {liveHinges.map((hinge) => (
            <div key={hinge.edge}>
              <label>
                <span>{t('初期角度（読み取り専用）', 'Initial angle (read only)')}</span>
                <input aria-label={`${t('初期角度', 'Initial angle')} ${hinge.edge}`} type="number" value={hinge.initialAngleDegrees} readOnly />
              </label>
              <label>
                <span>{t('要求角度', 'Requested angle')}</span>
                <input
                  aria-label={`${t('要求角度', 'Requested angle')} ${hinge.edge}`}
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
        <section aria-label={t('偶数単頂点の自動候補', 'Automatic even-cycle candidates')}>
          <h3>{t('偶数単頂点の自動候補', 'Automatic even-cycle candidates')}</h3>
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
              {candidate.edges.join(' / ')} — {t('同一割当・反対軸', 'same assignment, opposite axes')}
            </button>
          ))}
          {evenCycleCandidates.length === 0 && (
            <p data-even-cycle-status={evenCycleStatus}>
              {evenCycleStatus === 'resource_limit'
                ? t('候補探索の上限を超えました。', 'Candidate search exceeded its resource bound.')
                : evenCycleStatus === 'none'
                  ? t('適合する反対ヒンジ対はありません。', 'No matching opposite hinge pair exists.')
                  : t('現在の形状は対応する偶数単頂点サイクルではありません。', 'The current shape is not a supported even single-vertex cycle.')}
            </p>
          )}
        </section>
      )}
      <form onSubmit={(event) => void preview(event)}>
        <label>
          <span>{t('固定側', 'Fixed side')}</span>
          <select value={fixedSide} onChange={(event) => setFixedSide(event.target.value as StackedFoldFixedSide)} disabled={disabled || applying}>
            <option value="left">{t('線の左側', 'Left of line')}</option>
            <option value="right">{t('線の右側', 'Right of line')}</option>
          </select>
        </label>
        <label>
          <span>{t('閉路経路定義（JSON、閉路パターンのみ）', 'Cycle path definition (JSON, cyclic patterns only)')}</span>
          <textarea
            value={cycleScheduleText}
            onChange={(event) => setCycleScheduleText(event.target.value)}
            rows={4}
            maxLength={MAX_CYCLE_SCHEDULE_JSON_BYTES}
            spellCheck={false}
            placeholder={t(
              'version 1 の半角有理スケジュール。未入力の閉路は安全のため適用できません。',
              'Version 1 half-angle rational schedule. Cycles without one cannot be applied.',
            )}
            disabled={disabled || applying}
          />
          {cycleScheduleText.trim() && (
            <small role="status">
              {authoredCycleSchedule
                ? t(
                    `有界schedule: ${authoredCycleSchedule.entries.length}/64 hinge、係数は各9以下`,
                    `Bounded schedule: ${authoredCycleSchedule.entries.length}/64 hinges; at most 9 coefficients each`,
                  )
                : t(
                    'scheduleが不正です。分母は正整数、係数は各1〜9個、角度は0〜180度です。',
                    'Invalid schedule. Denominators must be positive integers, coefficients 1–9 each, and angles 0–180°.',
                  )}
            </small>
          )}
        </label>
        <label>
          <span>{t('回転方向', 'Rotation direction')}</span>
          <select value={rotationDirection} onChange={(event) => setRotationDirection(event.target.value as StackedFoldRotationDirection)} disabled={disabled || applying}>
            <option value="positive">{t('正方向', 'Positive')}</option>
            <option value="negative">{t('負方向', 'Negative')}</option>
          </select>
        </label>
        <label>
          <span>{t('角度（度）', 'Angle (degrees)')}</span>
          <input value={angle} onChange={(event) => setAngle(event.target.value)} type="number" min="0.000001" max="180" step="any" required disabled={disabled || applying} />
        </label>
        <button type="submit" disabled={!selectedLine || disabled || applying || view.kind === 'reading'}>
          {view.kind === 'reading' ? t('証明中…', 'Proving…') : t('安全性を確認', 'Verify safety')}
        </button>
      </form>
      {(authoredCycleSchedule || evenCycleCandidates.length > 0) && (
        <section aria-label={t('現在姿勢の循環折りプレビュー', 'Current-pose cycle preview')}>
          <h3>{t('現在姿勢の循環折り', 'Current-pose cycle')}</h3>
          <button
            ref={cyclePosePreviewButtonRef}
            type="button"
            disabled={disabled || applying || cyclePoseReading}
            onClick={() => void previewCurrentCyclePose(false)}
          >
            {cyclePoseReading
              ? t('経路を証明中…', 'Proving path…')
              : t('現在姿勢から証明', 'Prove from current pose')}
          </button>
            {evenCycleCandidates.length > 0 && (
            <button
              type="button"
              data-testid="automatic-kawasaki-proof"
              disabled={disabled || applying || cyclePoseReading}
              onClick={() => void previewCurrentCyclePose(true)}
            >
              {t('川崎リンクを自動生成して証明', 'Generate and prove Kawasaki linkage')}
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
                    1/{candidate.endpointDenominator}: {t('閉路証明済み', 'Closure certified')} /{' '}
                    {candidate.collisionStatus === 'certified'
                      ? t('衝突証明済み', 'Collision certified')
                      : t('衝突未認証', 'Collision uncertified')}
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
              {dyadicGraphReading ? t('経路探索中…', 'Searching paths…') : t('有界dyadic経路を探索', 'Search bounded dyadic paths')}
            </button>
            <label>
              {t('dyadic段階数', 'Dyadic levels')}
              <select
                aria-label="Dyadic levels"
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
              }}>{t('探索を中止', 'Cancel search')}</button>
            )}
            {dyadicGraphRead && (
              <p data-testid="dyadic-pose-graph-status" role="status">
                {dyadicGraphRead.status}; reason {dyadicGraphRead.reason}; states {dyadicGraphRead.stateCount}; transitions {dyadicGraphRead.transitionCount}; explored {dyadicGraphRead.exploredStateCount}; evaluated {dyadicGraphRead.evaluatedTransitionCount}; read-only
                ; certified transitions {dyadicGraphRead.certifiedTransitionCount}; binding {dyadicGraphRead.certificateBindingSha256 ?? 'unavailable'}; positive thickness {dyadicGraphRead.positiveThicknessCertified ? `certified ${dyadicGraphRead.positiveThicknessTransitionCount}/${dyadicGraphRead.certifiedTransitionCount}` : 'not certified'}; layer transport {dyadicGraphRead.layerTransportCertified ? `certified ${dyadicGraphRead.layerTransportTransitionCount}/${dyadicGraphRead.certifiedTransitionCount}` : 'not certified'}; mutation candidate {dyadicGraphRead.mutationCandidateReady ? 'ready' : 'not ready'}; Apply disabled
              </p>
            )}
            {dyadicGraphRead?.mutationCandidateReady && (
              <button type="button" data-testid="dyadic-path-preview" onClick={() => void mintDyadicPathPreview()}>
                {t('読取専用プレビューを発行', 'Issue read-only preview')}
              </button>
            )}
            {dyadicPathPreview && (
              <>
                <p data-testid="dyadic-path-preview-status" role="status">
                  preview {dyadicPathPreview.previewToken}; target {dyadicPathPreview.targetBindingSha256}; authenticated one-shot
                </p>
                <button type="button" data-testid="dyadic-path-apply" disabled={disabled || applying} onClick={() => void applyDyadicPathPreview()}>
                  {t('認証済み経路を適用', 'Apply authenticated path')}
                </button>
              </>
            )}
          {cyclePoseReading && pathProgress && (
            <p role="status">
              {t(
                `循環経路の状態 ${pathProgress.exploredStateCount}/${pathProgress.stateLimit}、遷移 ${pathProgress.evaluatedTransitionCount}/${pathProgress.transitionLimit}`,
                `Cycle states ${pathProgress.exploredStateCount}/${pathProgress.stateLimit}; transitions ${pathProgress.evaluatedTransitionCount}/${pathProgress.transitionLimit}`,
              )}
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
              {t('循環経路の証明を中止', 'Cancel cycle proof')}
            </button>
          )}
          {cyclePoseProgress?.status === 'cancelled' && (
            <p role="status">
              {t('循環経路の証明を中止しました。再試行できます。', 'Cycle proof cancelled. You can retry.')}
            </p>
          )}
          {cyclePoseError && (
            <p role="alert">
              {t(
                '循環経路を認証できませんでした。プロジェクトは変更されていません。',
                'The cycle path could not be authenticated. The project was not changed.',
              )}
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
                  <dt>{t('閉包区間数', 'Closure intervals')}</dt>
                  <dd>{cyclePosePreview.closureLeafCount}</dd>
                </div>
                <div>
                  <dt>{t('証明の最大深さ', 'Maximum proof depth')}</dt>
                  <dd>{cyclePosePreview.closureMaxDepth}</dd>
                </div>
                <div>
                  <dt>{t('全ヒンジ検証', 'All hinges covered')}</dt>
                  <dd>{cyclePosePreview.checkedHingeCount}/{cyclePosePreview.totalHingeCount}</dd>
                </div>
                <div>
                  <dt>{t('連続経路', 'Continuous path')}</dt>
                  <dd>{t('認証済み', 'Certified')}</dd>
                </div>
                <div>
                  <dt>{t('適用後リビジョン', 'Target revision')}</dt>
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
                {t(
                  'この表示は読み取り専用です。下の適用操作まで作品は変更されません。',
                  'This preview is read-only. The project is unchanged until you explicitly apply it.',
                )}
              </p>
              <button
                type="button"
                disabled={disabled || applying}
                onClick={() => void applyCurrentCyclePose()}
              >
                {t('認証済み循環折りを適用', 'Apply certified cycle fold')}
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
                {t('プレビューを取り消す', 'Cancel preview')}
              </button>
            </div>
          )}
        </section>
      )}
      {!cyclePosePreview && persistedCycleLayerProof && (
        <div role="status" data-testid="persisted-cycle-layer-order-viewer" className="stacked-fold-proof">
          <h4>Applied layer-order proof</h4>
          <p>Transitions: {persistedCycleLayerProof.transition_count}</p>
          <p>Pairs: {persistedCycleLayerProof.pairs.length}</p>
          <p>Proof hash: {persistedCycleLayerProof.target_order_sha256
            .map((byte) => byte.toString(16).padStart(2, '0')).join('')}</p>
        </div>
      )}
      {view.kind === 'failed' && (
        <p role="alert">
          {view.reason === 'stale'
            ? t('編集内容が変わりました。もう一度確認してください。', 'The project changed. Verify again.')
            : view.reason === 'cycle_nonclosing'
              ? t('循環hingeの終端が閉じないため適用できません。', 'The cyclic hinge endpoint does not close, so apply is disabled.')
              : view.reason === 'cycle_path_uncertified'
                ? t('循環hingeの終端は閉じますが、連続経路を証明できないため適用できません。', 'The cyclic endpoint closes, but its continuous path is uncertified, so apply is disabled.')
                : view.reason === 'cycle_path_unsupported'
                  ? t('静的理由: ヒンジグラフとスケジュールが、証明済みの格子・対称セクタ・対向軸直線折りクラスのいずれにも一致しません。適用は無効です。', 'Static reason: the hinge graph and schedule do not match a certified grid, symmetric-sector, or opposite-axis straight-fold class. Apply is disabled.')
                  : view.reason === 'cycle_path_resource_limit'
                    ? t('有界証明の資源上限に達しました。安全または不可能とは判定せず、適用を無効にします。', 'The bounded proof reached its resource limit. This does not claim safety or impossibility, so apply is disabled.')
                    : view.reason === 'cycle_path_no_certified_path'
                      ? t('証明済み遷移だけでは目標への経路が見つかりませんでした。不可能とは判定しません。', 'No path to the target was found using certified transitions only. This does not claim impossibility.')
                      : view.reason === 'cycle_path_cancelled'
                        ? t('有界経路解析を中止しました。部分的な証明は公開していません。', 'The bounded path analysis was cancelled. No partial certificate was published.')
                    : view.reason === 'cycle_path_collision'
                      ? t('予定された連続経路の衝突なし証明を取得できませんでした。適用は無効です。', 'The scheduled continuous path could not receive a collision-clearance certificate, so apply is disabled.')
            : view.reason === 'apply'
              ? t('適用できませんでした。プレビューは失効しました。', 'Apply failed; the preview is no longer trusted.')
              : t('この入力ではnative証明を完成できませんでした。', 'A native proof could not be completed for this input.')}
        </p>
      )}
      {view.kind === 'reading' && (
        <div>
          {pathProgress && (
            <p role="status">
              {t(
                `探索済み状態 ${pathProgress.exploredStateCount}/${pathProgress.stateLimit}、遷移 ${pathProgress.evaluatedTransitionCount}/${pathProgress.transitionLimit}`,
                `Explored states ${pathProgress.exploredStateCount}/${pathProgress.stateLimit}; transitions ${pathProgress.evaluatedTransitionCount}/${pathProgress.transitionLimit}`,
              )}
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
          {t('経路解析を中止', 'Cancel path analysis')}
        </button>
        </div>
      )}
      {view.kind === 'refresh_failed' && (
        <div role="alert">
          <p>{t('折り重ねは適用済みですが、最新表示を取得できませんでした。', 'The stacked fold was applied, but the refreshed project could not be loaded.')}</p>
          <button type="button" disabled={applying} onClick={() => void retryRefresh()}>
            {t('最新表示を再取得', 'Retry refresh')}
          </button>
        </div>
      )}
      {view.kind === 'ready' && (
        <div className="stacked-fold-proof" data-ready={ready}>
          {liveHinges.length > 0 && (
            <fieldset>
              <legend>{t('ヒンジ角度候補', 'Hinge angle candidate')}</legend>
              {liveHinges.map((hinge) => (
                <div key={hinge.edge}>
                  <label>
                    <span>{t('初期角度（読み取り専用）', 'Initial angle (read only)')}</span>
                    <input
                      aria-label={`${t('初期角度', 'Initial angle')} ${hinge.edge}`}
                      type="number"
                      value={hinge.initialAngleDegrees}
                      readOnly
                    />
                  </label>
                  <label>
                    <span>{t('要求角度', 'Requested angle')}</span>
                    <input
                      id={`stacked-fold-proof-hinge-${hinge.edge}`}
                      aria-label={`${t('要求角度', 'Requested angle')} ${hinge.edge}`}
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
                {t(
                  '要求角度を変更すると、正規順序のlinearCandidateV1が内部で構築されます。初期角度はnative姿勢へbit単位で再検証されます。',
                  'Editing a requested angle builds canonical linearCandidateV1 internally. Native revalidates initial angles bit-for-bit.',
                )}
              </p>
            </fieldset>
          )}
          <dl>
            <div><dt>{t('対象面', 'Target faces')}</dt><dd>{view.response.targetFaces.length}</dd></div>
            <div><dt>{t('折り線', 'Creases')}</dt><dd>{view.response.materialSegments.length}</dd></div>
            <div><dt>{t('対象hinge', 'Target hinges')}</dt><dd>{view.response.topologyProof.targetHingeCount}</dd></div>
            <div><dt>{t('終端衝突', 'Endpoint collision')}</dt><dd>{view.response.endpointCollision.hasBlockingHold ? t('停止', 'Blocked') : t('なし', 'Clear')}</dd></div>
            <div><dt>{t('連続経路', 'Continuous path')}</dt><dd>{view.response.continuousPath.continuousClearanceCertified ? t('証明済み', 'Certified') : t('未証明', 'Uncertified')}</dd></div>
            <div><dt>{t('最初の停止確認角', 'First proven blocking sample')}</dt><dd>{view.response.continuousPath.firstSampledBlockingAngleDegrees === null ? t('なし', 'None') : `${view.response.continuousPath.firstSampledBlockingAngleDegrees}°`}</dd></div>
            <div><dt>{t('経路証明モデル', 'Path certificate model')}</dt><dd>{certificateModelText}</dd></div>
            <div><dt>{t('区間leaf数', 'Interval leaves')}</dt><dd>{view.response.continuousPath.intervalLeafCount}</dd></div>
            <div><dt>{t('区間pair work', 'Interval pair work')}</dt><dd>{view.response.continuousPath.intervalPairWork}</dd></div>
            <div><dt>{t('正厚候補', 'Positive-thickness candidates')}</dt><dd>{view.response.continuousPath.positiveEndpointCandidateCount} / {view.response.continuousPath.positiveEndpointCandidateLimit}</dd></div>
            <div><dt>{t('正厚exact呼出', 'Positive-thickness exact calls')}</dt><dd>{view.response.continuousPath.positiveEndpointExactPairCalls}</dd></div>
            <div><dt>{t('候補上限', 'Candidate limit')}</dt><dd>{view.response.continuousPath.intervalCandidateLimit}</dd></div>
            <div><dt>{t('閉路leaf数', 'Closure leaves')}</dt><dd>{view.response.continuousPath.closureLeafCount}</dd></div>
            <div><dt>{t('閉路pair work', 'Closure pair work')}</dt><dd>{view.response.continuousPath.closurePairWork}</dd></div>
            <div><dt>{t('最初の閉路失敗角', 'First closure failure angle')}</dt><dd>{view.response.continuousPath.firstClosureFailureAngleDegrees ?? t('なし', 'None')}</dd></div>
            <div><dt>{t('証明済み紙厚', 'Certified thickness')}</dt><dd>{view.response.continuousPath.paperThicknessMm} mm</dd></div>
            <div><dt>{t('層順序', 'Layer order')}</dt><dd>{view.response.flatEndpointLayerOrder.certified ? t('証明済み', 'Certified') : t('未証明', 'Uncertified')}</dd></div>
            <div><dt>{t('追加頂点 / 辺', 'Added vertices / edges')}</dt><dd>{view.response.transactionProposal.addedVertexCount} / {view.response.transactionProposal.addedEdgeCount}</dd></div>
          </dl>
          {view.response.certifiedPathGraph && (
            <section aria-label={t('証明済み候補経路', 'Certified candidate path')}>
              <h4>{t('証明済み候補経路', 'Certified candidate path')}</h4>
              <p>
                {t(
                  `${view.response.certifiedPathGraph.edges.length} 遷移。read-only previewであり、作品変更を許可しません。`,
                  `${view.response.certifiedPathGraph.edges.length} transition(s). This read-only preview does not authorize project mutation.`,
                )}
              </p>
              <ol>
                {view.response.certifiedPathGraph.edges.map((edge, index) => (
                  <li key={`${edge.sourceFingerprintSha256}:${edge.targetFingerprintSha256}`}>
                    <strong>{t(`遷移 ${index + 1}`, `Transition ${index + 1}`)}</strong>
                    <dl>
                      <div><dt>{t('スケジュール証明', 'Schedule certificate')}</dt><dd>{edge.scheduleCertificateSha256}</dd></div>
                      <div><dt>{t('衝突証明', 'Collision certificate')}</dt><dd>{edge.collisionCertificateSha256}</dd></div>
                      <div><dt>{t('閉路証明', 'Closure certificate')}</dt><dd>{edge.closureCertificateSha256}</dd></div>
                    </dl>
                    {edge.hinges.map((hinge, hingeIndex) => (
                      <button
                        key={hinge}
                        type="button"
                        onClick={() => document.getElementById(
                          `stacked-fold-proof-hinge-${hinge}`,
                        )?.focus()}
                      >
                        {t('関連ヒンジを選択', 'Select related hinge')} {hingeIndex + 1}
                      </button>
                    ))}
                  </li>
                ))}
              </ol>
            </section>
          )}
          <p>{t(
            'この証明は表示された紙厚・2三角形・1ヒンジ・90度以下だけを対象とします。一般の多面、別の紙厚、工作性は保証しません。',
            'This certificate covers only the displayed thickness, two triangular faces, one hinge, and a path up to 90°. It does not guarantee general multi-face folds, another thickness, or physical manufacturability.',
          )}</p>
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
            <p role="alert">{t('適用できませんでした。同じ証明済みpreviewで再試行できます。', 'Apply failed. You can retry with the same certified preview.')}</p>
          )}
          {namedBookFold && (
            <p role="note">
              {t(
                `名前付き技法「${namedBookFold.name}」として認証済み姿勢を手順へ保存します。PDF/SVG折り図にも同じ手順が使われます。`,
                `The certified pose will be saved as the named technique “${namedBookFold.name}”. The same step is used by PDF/SVG instruction exports.`,
              )}
            </p>
          )}
          <label>
            <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} disabled={!ready || applying} />
            {t('証明済みの変更内容を確認しました。', 'I reviewed the certified changes.')}
          </label>
          <button type="button" onClick={() => void apply()} disabled={!ready || !confirmed || applying}>
            {applying
              ? t('適用中…', 'Applying…')
              : namedBookFold
                ? namedBookFold.kind === 'layer'
                  ? t('名前付き層選択技法を適用', 'Apply named layer technique')
                  : namedBookFold.kind === 'sink'
                  ? t('名前付き沈め折りを適用', 'Apply named sink fold')
                  : namedBookFold.kind === 'accordion'
                  ? t('名前付き蛇腹折りを適用', 'Apply named accordion fold')
                  : namedBookFold.kind === 'reverse'
                  ? t('名前付き逆折りを適用', 'Apply named reverse fold')
                  : t('名前付き二つ折りを適用', 'Apply named book fold')
                : t('折り重ねを適用', 'Apply stacked fold')}
          </button>
          {!ready && <p className="muted">{t('未証明のため適用は無効です。', 'Apply is disabled because the case is not fully certified.')}</p>}
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
  const t = (ja: string, en: string) => selectLocalizedText(locale, { ja, en })
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
  return <section className="stacked-fold-layer-viewer" aria-label={t('3D層順ビューア', '3D layer-order viewer')}>
    <h3>{t('重なりセルと層順', 'Overlap cells and layer order')}</h3>
    <p className="muted">{t(
      '認証済みの現在poseと層順を読み取り専用で表示します。',
      'Read-only view of the authenticated current pose and layer order.',
    )}</p>
    <div className="stacked-fold-cell-tabs" role="list">
      {cells.map((cell, index) => <button type="button" role="listitem"
        aria-pressed={cell.cellKeySha256 === active.cellKeySha256}
        key={cell.cellKeySha256} onClick={() => onSelectCell(cell.cellKeySha256)}>
        {t('セル', 'Cell')} {index + 1}
      </button>)}
    </div>
    <svg viewBox="0 0 240 180" role="img"
      aria-label={t('front/back層の分解表示', 'Exploded front/back layer stack')}>
      {active.bottomToTopFaces.map((face, index) => {
        const offset = (active.bottomToTopFaces.length - 1 - index) * 9
        const highlighted = face === selectedFace || face === hoveredFace
        return <polygon key={face} points={polygon} transform={`translate(${offset} ${-offset})`}
          fill={highlighted ? '#f6b73c' : `hsl(${205 + index * 22} 55% 62%)`}
          fillOpacity="0.72" stroke={highlighted ? '#6b3e00' : '#29465b'}
          tabIndex={0} onClick={() => onSelectFace(face)}
          onMouseEnter={() => onHoverFace(face)} onMouseLeave={() => onHoverFace(null)}
          onFocus={() => onHoverFace(face)} onBlur={() => onHoverFace(null)}>
          <title>{t(
            `${index === 0 ? '裏面・最下層' : index === active.bottomToTopFaces.length - 1 ? '表面・最上層' : '中間層'}、面 ${index + 1}`,
            `${index === 0 ? 'Back, bottom' : index === active.bottomToTopFaces.length - 1 ? 'Front, top' : 'Middle layer'}, face ${index + 1}`,
          )}</title>
        </polygon>
      })}
    </svg>
    <ol className="stacked-fold-layer-list">
      {active.bottomToTopFaces.map((face, index) => <li key={face}>
        <button type="button" aria-pressed={face === selectedFace}
          onMouseEnter={() => onHoverFace(face)} onMouseLeave={() => onHoverFace(null)}
          onClick={() => onSelectFace(face)}>
          {index === 0 ? t('裏面 / 最下層', 'Back / bottom')
            : index === active.bottomToTopFaces.length - 1
              ? t('表面 / 最上層', 'Front / top')
              : t('中間層', 'Middle')} · {t('面', 'Face')} {index + 1}
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
    return selectLocalizedText(locale, { ja: 'なし', en: 'None' })
  }
  if (modelId.includes('positive_thickness')) {
    return selectLocalizedText(locale, {
      ja: '正厚の連続経路証明',
      en: 'Positive-thickness continuous-path certificate',
    })
  }
  return selectLocalizedText(locale, {
    ja: '厚さゼロの連続経路証明',
    en: 'Zero-thickness continuous-path certificate',
  })
}
