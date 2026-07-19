import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  type FormEvent,
  useCallback,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
} from 'react'
import {
  CreaseCanvas,
  type CreaseCanvasRenderMetrics,
  type CreaseLine,
  type PaperBounds,
  type PaperPolygonPoint,
} from './components/CreaseCanvas'
import { CreaseExportDialog } from './components/CreaseExportDialog'
import { DiagnosticsDialog } from './components/DiagnosticsDialog'
import { FoldImportDialog } from './components/FoldImportDialog'
import { FoldPreview } from './components/FoldPreview'
import { GlobalFlatFoldabilityPanel } from './components/GlobalFlatFoldabilityPanel'
import { InstructionExportDialog } from './components/InstructionExportDialog'
import { InstructionTimelinePanel } from './components/InstructionTimelinePanel'
import { LengthUnitControl } from './components/LengthUnitControl'
import { LengthValueInput } from './components/LengthValueInput'
import { SvgImportDialog } from './components/SvgImportDialog'
import { ThemeControl } from './components/ThemeControl'
import {
  addEdge,
  addVertex,
  analyzeProjectTopology,
  applyFoldImport,
  applySvgImport,
  beginInstructionExportGeneration,
  cancelCreasePatternExport,
  cancelFoldImport,
  cancelInstructionExport,
  cancelSvgImport,
  connectEdgeIntersection,
  connectIntersectionCluster,
  connectTJunction,
  generateBenchmarkPattern,
  getInstructionExportProgress,
  getProjectSnapshot,
  isNativeCoreAvailable,
  moveVertex,
  newProject,
  openProject,
  previewCreasePatternExport,
  previewFoldImport,
  previewInstructionExport,
  previewSvgImport,
  redo,
  removeBoundaryVertex,
  removeEdge,
  removeVertex,
  resizeRectangularPaper,
  saveProject,
  saveProjectAs,
  saveCreasePatternExport,
  saveInstructionExport,
  setLengthDisplayUnit,
  splitBoundaryEdge,
  splitEdge,
  undo,
  updatePaperProperties,
  type ProjectSnapshot,
  type ProjectTopologyResponse,
  type RgbaColor,
  type ValidationSnapshot,
  validateSvgImportSettings,
  validateProject,
} from './lib/coreClient'
import {
  creasePatternExportFormatLabel,
  type CreasePatternExportFormat,
  type CreasePatternExportPreview,
} from './lib/creaseExport'
import {
  INSTRUCTION_EXPORT_PROFILE,
  INSTRUCTION_EXPORT_PROJECTION_PROFILE,
  createInstructionExportError,
  instructionExportErrorMessage,
  instructionExportFormatLabel,
  type InstructionExportFormat,
  type InstructionExportPhase,
  type InstructionExportPreview,
} from './lib/instructionExport'
import type { FoldImportPreview, FoldImportSettings } from './lib/foldImport'
import type {
  SvgImportPreview,
  SvgImportSettings,
  SvgImportSettingsDraft,
  SvgImportSettingsValidation,
} from './lib/svgImport'
import { buildFoldPreviewModel } from './lib/foldPreviewModel'
import type { FoldPreviewHingeAngle } from './lib/foldPreviewKinematics'
import type { FoldPreviewAppliedPoseSnapshot } from './lib/foldPreviewAppliedPose'
import {
  createNativeStaticCollisionInspectionCoordinator,
  createNativeStaticCollisionNativeTransport,
  nativeStaticCollisionPoseKey,
  type NativeStaticCollisionPose,
} from './lib/nativeStaticCollisionNative'
import {
  selectBoundNativeStaticCollisionView,
  type BoundNativeStaticCollisionView,
} from './lib/nativeStaticCollisionView'
import type { InstructionStepPresentation } from './lib/instructionTimeline'
import { formatPaperThicknessInput } from './lib/paperThicknessInput'
import { PaperThicknessInput } from './components/PaperThicknessInput'
import {
  collectBoundaryLengthReferences,
  formatLength,
  formatLengthInput,
  formatLengthPoint,
  formatLengthValue,
  MILLIMETRE_LENGTH_DISPLAY_UNIT,
  ratioReferenceAxis,
  readLengthInputMillimetres,
  resolveLengthDisplayUnit,
} from './lib/lengthUnit'
import {
  ANGLE_SNAP_PRESETS,
  DEFAULT_SNAP_SETTINGS,
  DEFAULT_ANGLE_SNAP_CONFIG,
  toggleSnapSetting,
  type AngleSnapConfig,
  type AngleSnapReferenceKind,
  type SnapSettings,
} from './lib/snap'
import {
  isSupportedIntersectionPlacement,
  type VertexPlacement,
} from './lib/vertexPlacement'
import {
  measureBenchmarkPayloadBytes,
  prepareBenchmarkRenderData,
} from './lib/renderBenchmark'
import {
  createLocalFlatFoldabilityPresentation,
  localFlatFoldabilityConditionLabel,
  localFlatFoldabilityReasonLabel,
  type LocalFlatFoldabilityPresentation,
} from './lib/localFlatFoldabilityPresentation'
import {
  DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET,
  type GlobalFlatFoldabilityJobDto,
  type GlobalFlatFoldabilityTimePreset,
} from './lib/globalFlatFoldability'
import {
  createGlobalFlatFoldabilityCoordinator,
  type GlobalFlatFoldabilityCoordinator,
} from './lib/globalFlatFoldabilityCoordinator'
import { createGlobalFlatFoldabilityNativeTransport } from './lib/globalFlatFoldabilityNative'
import { reportUnexpected } from './lib/diagnosticsRuntime'
import { isDiagnosticsShareAvailable } from './lib/diagnosticsShare'
import { resolveFileKeyboardShortcut } from './lib/fileKeyboardShortcut'
import './App.css'

const SNAP_OPTIONS: ReadonlyArray<{ kind: keyof SnapSettings; label: string }> = [
  { kind: 'grid', label: 'グリッド' },
  { kind: 'vertex', label: '頂点' },
  { kind: 'intersection', label: '交点' },
  { kind: 'edge', label: '辺' },
  { kind: 'midpoint', label: '中点' },
  { kind: 'horizontal', label: '水平' },
  { kind: 'vertical', label: '垂直' },
  { kind: 'parallel', label: '平行' },
  { kind: 'angle', label: '角度' },
]

const nativeStaticCollisionTransport =
  createNativeStaticCollisionNativeTransport()
const nativeStaticCollisionCoordinator =
  createNativeStaticCollisionInspectionCoordinator(
    nativeStaticCollisionTransport,
  )

type BenchmarkRun = Readonly<{
  requestId: number
  requestedEdgeCount: number
  lines: CreaseLine[]
  vertices: Array<{ id: string; x: number; y: number }>
  bounds: PaperBounds
  payloadBytes: number
  responseMs: number
  preparationMs: number
  startedAt: number
}>

type FoldAngleOverrides = Readonly<{
  projectId: string | null
  values: ReadonlyMap<string, number>
}>

type FixedFaceChoice = Readonly<{
  projectId: string | null
  faceId: string | null
}>

function App() {
  const [selectedLineId, setSelectedLineId] = useState<string | null>(null)
  const [selectedVertexId, setSelectedVertexId] = useState<string | null>(null)
  const [foldAngle, setFoldAngle] = useState(52)
  const [foldAngleOverrides, setFoldAngleOverrides] = useState<FoldAngleOverrides>({
    projectId: null,
    values: new Map(),
  })
  const [fixedFaceChoice, setFixedFaceChoice] = useState<FixedFaceChoice>({
    projectId: null,
    faceId: null,
  })
  const [appliedFoldPose, setAppliedFoldPose] =
    useState<FoldPreviewAppliedPoseSnapshot | null>(null)
  const [boundNativeStaticCollisionView, setBoundNativeStaticCollisionView] =
    useState<BoundNativeStaticCollisionView>({
      requestKey: null,
      view: { kind: 'idle' },
    })
  const [
    nativeStaticCollisionRetrySequence,
    setNativeStaticCollisionRetrySequence,
  ] = useState(0)
  const [manualPoseChangeSequence, setManualPoseChangeSequence] = useState(0)
  const [activeTool, setActiveTool] = useState('select')
  const [benchmarkStatus, setBenchmarkStatus] = useState('未実行')
  const [benchmarkRun, setBenchmarkRun] = useState<BenchmarkRun | null>(null)
  const [benchmarkLoading, setBenchmarkLoading] = useState(false)
  const [nativeSnapshot, setNativeSnapshot] = useState<ProjectSnapshot | null>(null)
  const [topologyResponse, setTopologyResponse] = useState<ProjectTopologyResponse | null>(null)
  const [topologyStatus, setTopologyStatus] = useState(
    isNativeCoreAvailable() ? '面・ヒンジ解析待ち' : '3D解析はデスクトップ版で利用できます',
  )
  const [validation, setValidation] = useState<ValidationSnapshot | null>(null)
  const [globalFlatFoldabilityJob, setGlobalFlatFoldabilityJob] =
    useState<GlobalFlatFoldabilityJobDto | null>(null)
  const [globalFlatFoldabilityTimeLimit, setGlobalFlatFoldabilityTimeLimit] =
    useState<GlobalFlatFoldabilityTimePreset>(
      DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET,
    )
  const [coreStatus, setCoreStatus] = useState(
    isNativeCoreAvailable() ? 'コア接続中…' : 'ブラウザ試作モード',
  )
  const [pendingEdgeStart, setPendingEdgeStart] = useState<string | null>(null)
  const [cancelInteractionToken, setCancelInteractionToken] = useState(0)
  const [fileOperation, setFileOperation] = useState<
    | 'open'
    | 'save'
    | 'save_as'
    | 'fold_import'
    | 'svg_import'
    | 'crease_export'
    | 'instruction_export'
    | null
  >(null)
  const [coreBusy, setCoreBusy] = useState(false)
  const [newProjectOpen, setNewProjectOpen] = useState(false)
  const [newProjectError, setNewProjectError] = useState<string | null>(null)
  const [diagnosticsDialogOpen, setDiagnosticsDialogOpen] = useState(false)
  const [foldImportPreview, setFoldImportPreview] = useState<FoldImportPreview | null>(null)
  const [foldImportError, setFoldImportError] = useState<string | null>(null)
  const [svgImportPreview, setSvgImportPreview] = useState<SvgImportPreview | null>(null)
  const [svgImportError, setSvgImportError] = useState<string | null>(null)
  const [svgImportValidation, setSvgImportValidation] =
    useState<SvgImportSettingsValidation | null>(null)
  const [creaseExportOpen, setCreaseExportOpen] = useState(false)
  const [creaseExportFormat, setCreaseExportFormat] =
    useState<CreasePatternExportFormat>('fold')
  const [creaseExportPreview, setCreaseExportPreview] =
    useState<CreasePatternExportPreview | null>(null)
  const [creaseExportError, setCreaseExportError] = useState<string | null>(null)
  const [creaseExportNotice, setCreaseExportNotice] = useState<string | null>(null)
  const [instructionExportOpen, setInstructionExportOpen] = useState(false)
  const [instructionExportFormat, setInstructionExportFormat] =
    useState<InstructionExportFormat>('pdf')
  const [instructionExportPreview, setInstructionExportPreview] =
    useState<InstructionExportPreview | null>(null)
  const [instructionExportGenerationActive, setInstructionExportGenerationActive] =
    useState(false)
  const [instructionExportPhase, setInstructionExportPhase] =
    useState<InstructionExportPhase>('validating')
  const [instructionExportError, setInstructionExportError] = useState<string | null>(null)
  const [instructionExportNotice, setInstructionExportNotice] = useState<string | null>(null)
  const [parallelReferenceEdgeId, setParallelReferenceEdgeId] = useState<string | null>(null)
  const [angleDegrees, setAngleDegrees] = useState(DEFAULT_ANGLE_SNAP_CONFIG.angleDegrees)
  const [angleDegreesInput, setAngleDegreesInput] = useState(
    String(DEFAULT_ANGLE_SNAP_CONFIG.angleDegrees),
  )
  const [angleReferenceKind, setAngleReferenceKind] = useState<AngleSnapReferenceKind>(
    DEFAULT_ANGLE_SNAP_CONFIG.referenceKind,
  )
  const [snapSettings, setSnapSettings] = useState<SnapSettings>(() => ({
    ...DEFAULT_SNAP_SETTINGS,
  }))
  const coreOperationRef = useRef(false)
  const latestSnapshotRef = useRef<ProjectSnapshot | null>(null)
  const globalFlatFoldabilityCoordinatorRef =
    useRef<GlobalFlatFoldabilityCoordinator | null>(null)
  const angleInputRef = useRef<HTMLInputElement>(null)
  const benchmarkRequestIdRef = useRef(0)
  const topologyRequestIdRef = useRef(0)
  const diagnosticsButtonRef = useRef<HTMLButtonElement>(null)
  const foldImportButtonRef = useRef<HTMLButtonElement>(null)
  const svgImportButtonRef = useRef<HTMLButtonElement>(null)
  const creaseExportButtonRef = useRef<HTMLButtonElement>(null)
  const creaseExportRequestIdRef = useRef(0)
  const instructionExportButtonRef = useRef<HTMLButtonElement>(null)
  const instructionExportRequestIdRef = useRef(0)
  const instructionExportGenerationIdRef = useRef<string | null>(null)
  const nativeStaticCollisionRequest = useMemo(() => {
    const project = nativeSnapshot
    const pose = appliedFoldPose
    if (
      !isNativeCoreAvailable()
      || !project
      || !pose
      || pose.state === 'running'
      || pose.projectId !== project.project_id
      || pose.revision !== project.revision
    ) return null
    const request: NativeStaticCollisionPose = {
      projectInstanceId: project.project_instance_id,
      projectId: project.project_id,
      revision: project.revision,
      fixedFaceId: pose.fixedFaceId,
      completeHingeAngles: pose.hingeAngles.map((angle) => ({
        edgeId: angle.edgeId,
        angleDegrees: angle.angleDegrees,
      })),
    }
    const requestKey = nativeStaticCollisionPoseKey(request)
    return requestKey ? { requestKey, request } : null
  }, [appliedFoldPose, nativeSnapshot])
  const nativeStaticCollisionState = selectBoundNativeStaticCollisionView(
    appliedFoldPose?.state === 'running',
    nativeStaticCollisionRequest?.requestKey ?? null,
    boundNativeStaticCollisionView,
  )
  const modalOpen = newProjectOpen
    || diagnosticsDialogOpen
    || foldImportPreview !== null
    || svgImportPreview !== null
    || creaseExportOpen
    || instructionExportOpen
  const closeDiagnosticsDialog = useCallback(() => {
    setDiagnosticsDialogOpen(false)
    requestAnimationFrame(() => diagnosticsButtonRef.current?.focus())
  }, [])
  const applySnapshot = useCallback((
    snapshot: ProjectSnapshot,
    forceReplacement = false,
  ) => {
    topologyRequestIdRef.current += 1
    latestSnapshotRef.current = snapshot
    globalFlatFoldabilityCoordinatorRef.current?.invalidate({
      projectId: snapshot.project_id,
      revision: snapshot.revision,
      foldModelFingerprint: snapshot.fold_model_fingerprint,
    }, forceReplacement)
    setNativeSnapshot(snapshot)
    setValidation(null)
    setTopologyResponse(null)
    setTopologyStatus('面・ヒンジ解析待ち')
  }, [])
  const nativeLines = useMemo<CreaseLine[]>(() => {
    if (!nativeSnapshot) return []
    const positions = new Map(
      nativeSnapshot.crease_pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
    )
    return nativeSnapshot.crease_pattern.edges.flatMap((edge) => {
      const start = positions.get(edge.start)
      const end = positions.get(edge.end)
      if (
        !start ||
        !end ||
        (edge.kind !== 'mountain' &&
          edge.kind !== 'valley' &&
          edge.kind !== 'auxiliary' &&
          edge.kind !== 'boundary' &&
          edge.kind !== 'cut')
      ) return []
      return [{
        id: edge.id,
        startVertexId: edge.start,
        endVertexId: edge.end,
        x1: start.x,
        y1: start.y,
        x2: end.x,
        y2: end.y,
        kind: edge.kind,
      }]
    })
  }, [nativeSnapshot])
  const nativeVertices = useMemo(
    () => nativeSnapshot?.crease_pattern.vertices.map((vertex) => ({
      id: vertex.id,
      x: vertex.position.x,
      y: vertex.position.y,
    })) ?? [],
    [nativeSnapshot],
  )
  const displayedLines = benchmarkRun?.lines ?? nativeLines
  const displayedVertices = benchmarkRun?.vertices ?? nativeVertices
  const firstDisplayedLineById = useMemo(() => {
    const index = new Map<string, CreaseLine>()
    for (const line of displayedLines) {
      if (!index.has(line.id)) index.set(line.id, line)
    }
    return index
  }, [displayedLines])
  const selectedLine = selectedLineId
    ? firstDisplayedLineById.get(selectedLineId)
    : undefined
  const parallelReferenceLine = useMemo(
    () => resolveUniqueParallelReference(nativeLines, parallelReferenceEdgeId),
    [nativeLines, parallelReferenceEdgeId],
  )
  const angleSnapConfig = useMemo<AngleSnapConfig>(() => ({
    angleDegrees,
    referenceKind: angleReferenceKind,
  }), [angleDegrees, angleReferenceKind])
  const parsedAngleInput = Number(angleDegreesInput)
  const angleInputIsValid = angleDegreesInput.trim().length > 0
    && Number.isFinite(parsedAngleInput)
    && parsedAngleInput > 0
    && parsedAngleInput <= 90
  const selectedAnglePreset = angleInputIsValid
    && ANGLE_SNAP_PRESETS.some((preset) => preset === parsedAngleInput)
    ? String(parsedAngleInput)
    : 'custom'
  const selectedLineMeasurement = selectedLine ? measureCreaseLine(selectedLine) : null
  const selectedVertex = useMemo(
    () => nativeSnapshot?.crease_pattern.vertices.find(
      (vertex) => vertex.id === selectedVertexId,
    ),
    [nativeSnapshot, selectedVertexId],
  )
  const localFlatFoldabilityPresentation = useMemo(() => {
    if (
      !validation
      || !nativeSnapshot
      || validation.project_id !== nativeSnapshot.project_id
      || validation.revision !== nativeSnapshot.revision
    ) return null
    return createLocalFlatFoldabilityPresentation(
      validation.local_flat_foldability,
      nativeSnapshot.crease_pattern.vertices.map((vertex) => vertex.id),
    )
  }, [nativeSnapshot, validation])
  const selectedLocalFlatFoldability = selectedVertexId
    ? localFlatFoldabilityPresentation?.verticesById.get(selectedVertexId)
    : undefined
  const canvasLocalFlatFoldabilityHighlights = !benchmarkRun
    && localFlatFoldabilityPresentation?.kind === 'ready'
    ? localFlatFoldabilityPresentation.highlights
    : undefined
  const localFlatFoldabilitySummaryId = localFlatFoldabilityPresentation && !benchmarkRun
    ? 'local-flat-foldability-summary'
    : undefined
  const firstBenchmarkVertexById = useMemo(() => {
    const index = new Map<string, { id: string; x: number; y: number }>()
    for (const vertex of benchmarkRun?.vertices ?? []) {
      if (!index.has(vertex.id)) index.set(vertex.id, vertex)
    }
    return index
  }, [benchmarkRun])
  const selectedBenchmarkVertex = selectedVertexId
    ? firstBenchmarkVertexById.get(selectedVertexId)
    : undefined
  const boundaryVertexIds = useMemo(() => new Set(
    nativeSnapshot?.paper.boundary_vertices ?? [],
  ), [nativeSnapshot])
  const paperBoundaryVertexCount = boundaryVertexIds.size
  const selectedVertexIsBoundary = selectedVertex
    ? boundaryVertexIds.has(selectedVertex.id)
    : false
  const paperBounds = useMemo(
    () => resolvePaperBounds(nativeSnapshot),
    [nativeSnapshot],
  )
  const paperPolygon = useMemo(
    () => resolvePaperPolygon(nativeSnapshot),
    [nativeSnapshot],
  )
  const boundaryLengthReferences = useMemo(
    () => collectBoundaryLengthReferences(nativeSnapshot),
    [nativeSnapshot],
  )
  const lengthDisplayUnit = useMemo(
    () => resolveLengthDisplayUnit(nativeSnapshot, boundaryLengthReferences),
    [boundaryLengthReferences, nativeSnapshot],
  )
  const displayedLengthUnit = benchmarkRun
    ? MILLIMETRE_LENGTH_DISPLAY_UNIT
    : lengthDisplayUnit
  const rectangularPaperSize = useMemo(
    () => resolveRectangularPaperSize(nativeSnapshot),
    [nativeSnapshot],
  )
  const rectangularRatioReferenceAxis = ratioReferenceAxis(lengthDisplayUnit)
  const foldPreviewModel = useMemo(
    () => buildFoldPreviewModel(nativeSnapshot, topologyResponse),
    [nativeSnapshot, topologyResponse],
  )
  const fixedFaceOptions = useMemo(() => (
    foldPreviewModel?.kind === 'single_fold'
      ? foldPreviewModel.faces
      : foldPreviewModel?.kind === 'fold_graph'
        && foldPreviewModel.kinematics.kind === 'tree'
        ? foldPreviewModel.faces
        : []
  ), [foldPreviewModel])
  const canonicalFixedFaceId = foldPreviewModel?.kind === 'single_fold'
    ? foldPreviewModel.fixedFace.id
    : foldPreviewModel?.kind === 'fold_graph'
      && foldPreviewModel.kinematics.kind === 'tree'
      ? foldPreviewModel.kinematics.rootFaceId
      : null
  const fixedFaceChoiceIsCurrent = foldPreviewModel
    && fixedFaceChoice.projectId === foldPreviewModel.projectId
    && fixedFaceChoice.faceId
    && fixedFaceOptions.some((face) => face.id === fixedFaceChoice.faceId)
  const effectiveFixedFaceId = fixedFaceChoiceIsCurrent
    ? fixedFaceChoice.faceId
    : canonicalFixedFaceId
  const effectiveFixedFaceIndex = effectiveFixedFaceId
    ? fixedFaceOptions.findIndex((face) => face.id === effectiveFixedFaceId)
    : -1
  const effectiveFixedFaceLabel = effectiveFixedFaceIndex >= 0
    ? `面 ${effectiveFixedFaceIndex + 1}`
    : undefined
  const fixedFaceEnabled = fixedFaceOptions.length > 1 && !benchmarkRun
  const foldPreviewHingeIds = useMemo(() => new Set(
    foldPreviewModel?.kind === 'single_fold'
      ? [foldPreviewModel.hinge.edgeId]
      : foldPreviewModel?.kind === 'fold_graph'
        ? foldPreviewModel.hinges.map((hinge) => hinge.edgeId)
        : [],
  ), [foldPreviewModel])
  const selectedPreviewHingeId = !benchmarkRun
    && selectedLineId
    && foldPreviewHingeIds.has(selectedLineId)
    ? selectedLineId
    : null
  const foldPreviewStatus = topologyResponse?.simulation_ready && !foldPreviewModel
    ? '3D入力の整合性検証で遮断'
    : topologyStatus
  const foldPreviewStatusClass = foldPreviewModel
    ? 'status-valid'
    : topologyResponse
      ? 'status-invalid'
      : 'status-ready'
  const foldAngleEnabled = foldPreviewModel?.kind === 'single_fold'
    || (
      foldPreviewModel?.kind === 'fold_graph'
      && foldPreviewModel.kinematics.kind === 'tree'
    )
  const foldTreeHingeAngles = useMemo<readonly FoldPreviewHingeAngle[] | undefined>(() => {
    if (
      foldPreviewModel?.kind !== 'fold_graph'
      || foldPreviewModel.kinematics.kind !== 'tree'
    ) return undefined
    const overrides = foldAngleOverrides.projectId === foldPreviewModel.projectId
      ? foldAngleOverrides.values
      : null
    return foldPreviewModel.kinematics.joints.map((joint) => ({
      edgeId: joint.hinge.edgeId,
      angleDegrees: overrides?.get(joint.hinge.edgeId) ?? foldAngle,
    }))
  }, [foldAngle, foldAngleOverrides, foldPreviewModel])
  const foldPreviewPoseModelKey = foldPreviewModel
    ? [
        foldPreviewModel.projectId,
        foldPreviewModel.revision,
        foldPreviewModel.kind,
        foldPreviewModel.kind === 'fold_graph'
          ? foldPreviewModel.kinematics.kind
          : '',
      ].join(':')
    : null

  const applyInstructionStepPose = useCallback((
    step: InstructionStepPresentation,
  ) => {
    const current = latestSnapshotRef.current
    const preview = foldPreviewModel
    if (
      !current
      || !preview
      || step.stale
      || preview.projectId !== current.project_id
      || preview.revision !== current.revision
      || step.pose.source_model_fingerprint !== current.fold_model_fingerprint
    ) return false

    if (preview.kind === 'planar') {
      if (step.pose.fixed_face !== null || step.pose.hinge_angles.length !== 0) {
        return false
      }
      setFixedFaceChoice({ projectId: preview.projectId, faceId: null })
      setFoldAngleOverrides({ projectId: preview.projectId, values: new Map() })
      return true
    }

    const fixedFace = step.pose.fixed_face
    if (!fixedFace || !preview.faces.some(({ id }) => id === fixedFace)) return false
    const expectedHingeIds = preview.kind === 'single_fold'
      ? [preview.hinge.edgeId]
      : preview.kinematics.kind === 'tree'
        ? preview.kinematics.joints.map(({ hinge }) => hinge.edgeId)
        : []
    if (
      expectedHingeIds.length === 0
      || step.pose.hinge_angles.length !== expectedHingeIds.length
    ) return false
    const angles = new Map(
      step.pose.hinge_angles.map(({ edge, angle_degrees }) => [edge, angle_degrees]),
    )
    if (
      angles.size !== expectedHingeIds.length
      || expectedHingeIds.some((edgeId) => !angles.has(edgeId))
    ) return false

    setFixedFaceChoice({ projectId: preview.projectId, faceId: fixedFace })
    if (preview.kind === 'single_fold') {
      const angleDegrees = angles.get(preview.hinge.edgeId)
      if (angleDegrees === undefined) return false
      setFoldAngle(angleDegrees)
      setFoldAngleOverrides({ projectId: preview.projectId, values: new Map() })
      return true
    }
    if (preview.kinematics.kind !== 'tree') return false
    setFoldAngleOverrides({
      projectId: preview.projectId,
      values: angles,
    })
    return true
  }, [foldPreviewModel])

  const updateUniformFoldAngle = (value: number) => {
    const nextAngle = normalizeFoldAngle(value)
    if (nextAngle === null) return
    setManualPoseChangeSequence((sequence) => sequence + 1)
    setFoldAngle(nextAngle)
    setFoldAngleOverrides({
      projectId: foldPreviewModel?.projectId ?? null,
      values: new Map(),
    })
  }

  const updateHingeFoldAngle = (edgeId: string, value: number) => {
    const nextAngle = normalizeFoldAngle(value)
    if (
      nextAngle === null
      || foldPreviewModel?.kind !== 'fold_graph'
      || foldPreviewModel.kinematics.kind !== 'tree'
      || !foldPreviewModel.kinematics.joints.some((joint) => joint.hinge.edgeId === edgeId)
    ) return
    setManualPoseChangeSequence((sequence) => sequence + 1)
    const projectId = foldPreviewModel.projectId
    const activeEdgeIds = new Set(
      foldPreviewModel.kinematics.joints.map((joint) => joint.hinge.edgeId),
    )
    setFoldAngleOverrides((current) => {
      const values = new Map<string, number>()
      if (current.projectId === projectId) {
        for (const [currentEdgeId, currentAngle] of current.values) {
          if (activeEdgeIds.has(currentEdgeId)) values.set(currentEdgeId, currentAngle)
        }
      }
      values.set(edgeId, nextAngle)
      return { projectId, values }
    })
  }
  const paperSizeLabel = paperBounds
    ? `${formatLengthValue(
        paperBounds.maxX - paperBounds.minX,
        lengthDisplayUnit,
      )} × ${formatLength(
        paperBounds.maxY - paperBounds.minY,
        lengthDisplayUnit,
      )}`
    : '寸法不明'
  const paperCenter = paperBounds
    ? {
        x: (paperBounds.minX + paperBounds.maxX) / 2,
        y: (paperBounds.minY + paperBounds.maxY) / 2,
      }
    : null
  const paperFrontColor = rgbaToCss(nativeSnapshot?.paper.front.color)
  const paperFormKey = nativeSnapshot
    ? [
        nativeSnapshot.project_id,
        nativeSnapshot.paper.thickness_mm,
        rgbaToHex(nativeSnapshot.paper.front.color),
        rgbaToHex(nativeSnapshot.paper.back.color),
        nativeSnapshot.paper.cutting_allowed,
        lengthDisplayUnit.key,
      ].join(':')
    : 'paper-unavailable'
  const paperResizeFormKey = nativeSnapshot && rectangularPaperSize
    ? `${nativeSnapshot.project_id}:${rectangularPaperSize.width}:${rectangularPaperSize.height}:${lengthDisplayUnit.key}`
    : `${nativeSnapshot?.project_id ?? 'paper-unavailable'}:not-rectangular`
  const snapStatusLabel = SNAP_OPTIONS
    .filter(({ kind }) => snapSettings[kind])
    .map(({ label }) => label)
    .join('・') || 'なし'

  const runShortcutFileOperation = useEffectEvent((
    operation: 'open' | 'save' | 'save_as',
  ) => {
    void runFileOperation(operation)
  })

  useEffect(() => {
    if (!isNativeCoreAvailable()) return
    let mounted = true
    const coordinator = createGlobalFlatFoldabilityCoordinator<number>({
      transport: createGlobalFlatFoldabilityNativeTransport(),
      scheduler: {
        setTimeout: (callback, delayMs) => window.setTimeout(callback, delayMs),
        clearTimeout: (handle) => window.clearTimeout(handle),
      },
      onState: ({ job }) => {
        if (mounted) setGlobalFlatFoldabilityJob(job)
      },
    })
    if (!coordinator) return
    globalFlatFoldabilityCoordinatorRef.current = coordinator

    return () => {
      mounted = false
      if (globalFlatFoldabilityCoordinatorRef.current === coordinator) {
        globalFlatFoldabilityCoordinatorRef.current = null
      }
      coordinator.dispose()
    }
  }, [])

  useEffect(() => {
    if (!isNativeCoreAvailable()) return
    getProjectSnapshot()
      .then((snapshot) => {
        applySnapshot(snapshot)
        setCoreStatus(`Rustコア revision ${snapshot.revision}`)
      })
      .catch((error: unknown) => {
        reportUnexpected('app.project_snapshot')
        setCoreStatus(`コアエラー: ${String(error)}`)
      })
  }, [applySnapshot])

  useEffect(() => {
    const current = nativeStaticCollisionRequest
    if (!current) {
      setBoundNativeStaticCollisionView({
        requestKey: null,
        view: { kind: 'idle' },
      })
      return
    }

    let disposed = false
    setBoundNativeStaticCollisionView({
      requestKey: current.requestKey,
      view: { kind: 'checking' },
    })
    void nativeStaticCollisionCoordinator
      .inspectLatest(current.request)
      .then((diagnostic) => {
        if (!disposed) {
          setBoundNativeStaticCollisionView({
            requestKey: current.requestKey,
            view: { kind: 'ready', diagnostic },
          })
        }
      }).catch(() => {
        if (!disposed) {
          setBoundNativeStaticCollisionView({
            requestKey: current.requestKey,
            view: { kind: 'failed' },
          })
        }
      })

    return () => {
      disposed = true
    }
  }, [nativeStaticCollisionRequest, nativeStaticCollisionRetrySequence])

  useEffect(() => {
    if (!isNativeCoreAvailable() || !nativeSnapshot) return
    const requestId = ++topologyRequestIdRef.current
    const expectedProjectId = nativeSnapshot.project_id
    const expectedRevision = nativeSnapshot.revision
    let disposed = false
    setTopologyStatus('面・ヒンジ解析中…')

    analyzeProjectTopology(expectedProjectId, expectedRevision)
      .then((response) => {
        const current = latestSnapshotRef.current
        if (
          disposed
          || requestId !== topologyRequestIdRef.current
          || !current
          || current.project_id !== response.project_id
          || current.revision !== response.revision
        ) return
        setTopologyResponse(response)
        if (response.simulation_ready && response.snapshot) {
          setTopologyStatus(
            `${response.snapshot.faces.length}面・${response.snapshot.hinge_adjacency.length}ヒンジ`,
          )
        } else {
          setTopologyStatus(`3D解析で遮断（${response.issues.length}件）`)
        }
      })
      .catch((error: unknown) => {
        if (disposed || requestId !== topologyRequestIdRef.current) return
        const current = latestSnapshotRef.current
        if (
          !current
          || current.project_id !== expectedProjectId
          || current.revision !== expectedRevision
        ) return
        reportUnexpected('app.topology_analysis')
        setTopologyResponse(null)
        setTopologyStatus(`3D解析エラー: ${String(error)}`)
      })

    return () => {
      disposed = true
    }
  }, [nativeSnapshot])

  useEffect(() => {
    if (parallelReferenceEdgeId && !parallelReferenceLine) {
      setParallelReferenceEdgeId(null)
    }
  }, [parallelReferenceEdgeId, parallelReferenceLine])

  useEffect(() => {
    if (nativeSnapshot?.cutting_allowed || activeTool !== 'cut') return
    setActiveTool('select')
    setPendingEdgeStart(null)
  }, [activeTool, nativeSnapshot?.cutting_allowed])

  useEffect(() => {
    if (!isNativeCoreAvailable()) return

    let disposed = false
    let unlisten: (() => void) | undefined
    void getCurrentWindow().onCloseRequested((event) => {
      if (coreOperationRef.current) {
        event.preventDefault()
        setCoreStatus('処理が完了してから終了してください')
        return
      }
      if (!latestSnapshotRef.current?.is_dirty) return
      const discard = window.confirm(
        '未保存の変更があります。変更を破棄して終了しますか？\nキャンセルすると編集画面に戻ります。',
      )
      if (!discard) event.preventDefault()
    }).then((stopListening) => {
      if (disposed) stopListening()
      else unlisten = stopListening
    }).catch((error: unknown) => {
      if (!disposed) {
        reportUnexpected('app.close_guard')
        setCoreStatus(`終了確認の初期化エラー: ${String(error)}`)
      }
    })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  const runNativeEdit = useCallback(async (
    action: (projectId: string, revision: number) => Promise<ProjectSnapshot>,
  ) => {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return false
    coreOperationRef.current = true
    setCoreBusy(true)
    setCancelInteractionToken((token) => token + 1)
    try {
      const snapshot = await action(current.project_id, current.revision)
      applySnapshot(snapshot)
      setValidation(null)
      setCoreStatus(`Rustコア revision ${snapshot.revision}`)
      return true
    } catch (error) {
      setCoreStatus(`コアエラー: ${String(error)}`)
      return false
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }, [applySnapshot])

  const startGlobalFlatFoldability = useCallback((
    timeLimitSeconds: GlobalFlatFoldabilityTimePreset,
  ) => {
    const current = latestSnapshotRef.current
    if (
      !current
      || coreOperationRef.current
      || benchmarkLoading
      || benchmarkRun
    ) return
    globalFlatFoldabilityCoordinatorRef.current?.start(
      {
        projectId: current.project_id,
        revision: current.revision,
        foldModelFingerprint: current.fold_model_fingerprint,
      },
      timeLimitSeconds,
    )
  }, [benchmarkLoading, benchmarkRun])

  const cancelGlobalFlatFoldability = useCallback(() => {
    globalFlatFoldabilityCoordinatorRef.current?.cancel()
  }, [])

  const deleteSelection = useCallback(async () => {
    if (benchmarkRun) {
      setCoreStatus('性能テストの図は読み取り専用です。通常図へ戻ると編集できます')
      return
    }
    if (selectedLine) {
      if (selectedLine.kind === 'boundary') {
        setCoreStatus('輪郭線の追加・削除は紙形状編集から行います')
        return
      }
      const removed = await runNativeEdit((projectId, revision) =>
        removeEdge(projectId, revision, selectedLine.id))
      if (removed) setSelectedLineId(null)
      return
    }
    if (selectedVertex) {
      if (selectedVertexIsBoundary && paperBoundaryVertexCount <= 3) {
        setCoreStatus('輪郭は最低3点必要なため、この輪郭頂点は削除できません')
        return
      }
      const removed = await runNativeEdit((projectId, revision) =>
        selectedVertexIsBoundary
          ? removeBoundaryVertex(projectId, revision, selectedVertex.id)
          : removeVertex(projectId, revision, selectedVertex.id))
      if (!removed) return
      setSelectedVertexId(null)
      setSelectedLineId(null)
      setPendingEdgeStart(null)
      setActiveTool('select')
      setCoreStatus(selectedVertexIsBoundary
        ? '輪郭頂点を削除し、隣接する輪郭辺を統合しました（元に戻すで復元できます）'
        : '頂点を削除しました（元に戻すで復元できます）')
    }
  }, [
    benchmarkRun,
    paperBoundaryVertexCount,
    runNativeEdit,
    selectedLine,
    selectedVertex,
    selectedVertexIsBoundary,
  ])

  async function splitSelectedBoundaryEdge() {
    const current = latestSnapshotRef.current
    if (!current || selectedLine?.kind !== 'boundary' || coreOperationRef.current) return
    const previousVertexIds = new Set(
      current.crease_pattern.vertices.map((vertex) => vertex.id),
    )
    const result: { snapshot: ProjectSnapshot | null } = { snapshot: null }
    const succeeded = await runNativeEdit(async (projectId, revision) => {
      const snapshot = await splitBoundaryEdge(projectId, revision, selectedLine.id, 0.5)
      result.snapshot = snapshot
      return snapshot
    })
    if (!succeeded || !result.snapshot) return

    const boundaryIds = new Set(result.snapshot.paper.boundary_vertices)
    const addedVertex = result.snapshot.crease_pattern.vertices.find((vertex) =>
      !previousVertexIds.has(vertex.id) && boundaryIds.has(vertex.id))
    setSelectedLineId(null)
    setPendingEdgeStart(null)
    if (!addedVertex) {
      setSelectedVertexId(null)
      setCoreStatus('輪郭辺を分割しましたが、新しい頂点を特定できませんでした')
      return
    }
    setSelectedVertexId(addedVertex.id)
    setActiveTool('select')
    setCoreStatus('輪郭辺を中点で分割し、新しい頂点を選択しました')
  }

  async function placeCanvasVertex(placement: VertexPlacement) {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    const previousVertexIds = new Set(
      current.crease_pattern.vertices.map((vertex) => vertex.id),
    )
    const result: { snapshot: ProjectSnapshot | null; connectedVertexId: string | null } = {
      snapshot: null,
      connectedVertexId: null,
    }
    const succeeded = await runNativeEdit(async (projectId, revision) => {
      let snapshot: ProjectSnapshot
      if (placement.operation === 'add') {
        snapshot = await addVertex(projectId, revision, placement.x, placement.y)
      } else if (placement.operation === 'split-edge') {
        const edge = current.crease_pattern.edges.find(({ id }) => id === placement.edgeId)
        if (!edge) throw new Error(`分割対象の辺が見つかりません: ${placement.edgeId}`)
        snapshot = edge.kind === 'boundary'
          ? await splitBoundaryEdge(projectId, revision, placement.edgeId, placement.fraction)
          : await splitEdge(projectId, revision, placement.edgeId, placement.fraction)
      } else {
        if (!isSupportedIntersectionPlacement(
          placement,
          current.crease_pattern.edges,
        )) throw new Error('交点接続の対象辺が不正です')
        const response = placement.operation === 'connect-intersection'
          ? await connectEdgeIntersection(
              projectId,
              revision,
              placement.firstEdgeId,
              placement.secondEdgeId,
            )
          : placement.operation === 'connect-t-junction'
            ? await connectTJunction(
                projectId,
                revision,
                placement.firstEdgeId,
                placement.secondEdgeId,
              )
            : await connectIntersectionCluster(
                projectId,
                revision,
                placement.targets,
                placement.junctionVertexId,
              )
        snapshot = response.snapshot
        result.connectedVertexId = response.vertex_id
      }
      result.snapshot = snapshot
      return snapshot
    })
    if (!succeeded || !result.snapshot) return

    if (
      placement.operation === 'connect-intersection'
      || placement.operation === 'connect-t-junction'
      || placement.operation === 'connect-intersection-cluster'
    ) {
      if (
        !result.connectedVertexId
        || !result.snapshot.crease_pattern.vertices.some(
          ({ id }) => id === result.connectedVertexId,
        )
        || (
          placement.operation === 'connect-t-junction'
          && result.connectedVertexId !== placement.junctionVertexId
        )
        || (
          placement.operation === 'connect-intersection-cluster'
          && placement.junctionVertexId !== undefined
          && result.connectedVertexId !== placement.junctionVertexId
        )
      ) {
        setCoreStatus('交点を接続しましたが、接続頂点を確認できませんでした')
        return
      }
      setSelectedLineId(null)
      setPendingEdgeStart(null)
      setSelectedVertexId(result.connectedVertexId)
      setCoreStatus(placement.operation === 'connect-t-junction'
        ? 'T字交点を接続しました（元に戻す1回で復元できます）'
        : placement.operation === 'connect-intersection-cluster'
          ? `${placement.targets.length}本の辺を交点クラスタとして接続しました（元に戻す1回で復元できます）`
          : '交点で2本の辺を原子的に分割しました（元に戻す1回で復元できます）')
      return
    }

    const addedVertices = result.snapshot.crease_pattern.vertices.filter(
      ({ id }) => !previousVertexIds.has(id),
    )
    setSelectedLineId(null)
    setPendingEdgeStart(null)
    if (addedVertices.length !== 1) {
      setSelectedVertexId(null)
      setCoreStatus('頂点を作成しましたが、新しい頂点を一意に特定できませんでした')
      return
    }
    setSelectedVertexId(addedVertices[0].id)
    setCoreStatus(placement.operation === 'split-edge'
      ? '辺を分割し、新しい頂点を選択しました（元に戻すで復元できます）'
      : '頂点を追加して選択しました（元に戻すで復元できます）')
  }

  useEffect(() => {
    function handleKeyboardShortcut(event: KeyboardEvent) {
      if (event.key.toLowerCase() === 'escape' && newProjectOpen) {
        event.preventDefault()
        if (coreBusy) return
        setNewProjectOpen(false)
        setNewProjectError(null)
        return
      }
      if (modalOpen) return
      if (isEditingText(event.target)) return

      const key = event.key.toLowerCase()
      const primaryModifier = event.ctrlKey || event.metaKey
      const fileShortcut = resolveFileKeyboardShortcut(event)
      if (fileShortcut) {
        event.preventDefault()
        if (coreBusy || !nativeSnapshot) return
        if (fileShortcut === 'new') {
          setNewProjectError(null)
          setNewProjectOpen(true)
        } else {
          runShortcutFileOperation(fileShortcut)
        }
        return
      }
      if (primaryModifier && key === 'z') {
        event.preventDefault()
        if (event.repeat) return
        if (event.shiftKey) {
          if (nativeSnapshot?.can_redo) void runNativeEdit(redo)
        } else if (nativeSnapshot?.can_undo) {
          void runNativeEdit(undo)
        }
        return
      }
      if (primaryModifier && key === 'y') {
        event.preventDefault()
        if (!event.repeat && nativeSnapshot?.can_redo) void runNativeEdit(redo)
        return
      }
      if (key === 'delete' || key === 'backspace') {
        if (!selectedLine && !selectedVertex) return
        event.preventDefault()
        if (!event.repeat) void deleteSelection()
        return
      }
      if (key === 'escape') {
        setSelectedLineId(null)
        setSelectedVertexId(null)
        setPendingEdgeStart(null)
        setCancelInteractionToken((token) => token + 1)
      }
    }

    window.addEventListener('keydown', handleKeyboardShortcut)
    return () => window.removeEventListener('keydown', handleKeyboardShortcut)
  }, [coreBusy, deleteSelection, modalOpen, nativeSnapshot, newProjectOpen, runNativeEdit, selectedLine, selectedVertex])

  function selectVertexForEdge(vertexId: string) {
    if (
      activeTool !== 'mountain'
      && activeTool !== 'valley'
      && activeTool !== 'auxiliary'
      && activeTool !== 'cut'
    ) return
    if (!pendingEdgeStart) {
      setPendingEdgeStart(vertexId)
      setCoreStatus('線の終点を選択してください')
      return
    }
    if (pendingEdgeStart === vertexId) {
      setCoreStatus('始点とは異なる頂点を選択してください')
      return
    }
    const start = pendingEdgeStart
    setPendingEdgeStart(null)
    void runNativeEdit((projectId, revision) =>
      addEdge(projectId, revision, start, vertexId, activeTool))
  }

  function selectCanvasVertex(vertexId: string) {
    if (activeTool === 'select' || activeTool === 'vertex') {
      setSelectedVertexId(vertexId)
      setSelectedLineId(null)
      return
    }
    selectVertexForEdge(vertexId)
  }

  function submitVertexPosition(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedVertex) return
    const currentVertices = current.crease_pattern.vertices.filter(
      (vertex) => vertex.id === selectedVertex.id,
    )
    if (currentVertices.length !== 1) return
    const currentVertex = currentVertices[0]
    const currentUnit = resolveLengthDisplayUnit(current)
    const x = readLengthInputMillimetres(
      event.currentTarget,
      'x_display',
      currentVertex.position.x,
      currentUnit,
    )
    const y = readLengthInputMillimetres(
      event.currentTarget,
      'y_display',
      currentVertex.position.y,
      currentUnit,
    )
    if (x === null || y === null) {
      setCoreStatus('座標には有限の数値を入力してください')
      return
    }
    void runNativeEdit((projectId, revision) =>
      moveVertex(projectId, revision, selectedVertex.id, x, y))
  }

  function submitPaperProperties(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return

    const form = new FormData(event.currentTarget)
    const currentUnit = resolveLengthDisplayUnit(current)
    const thicknessMm = readLengthInputMillimetres(
      event.currentTarget,
      'thickness_display',
      current.paper.thickness_mm,
      currentUnit,
    )
    const frontColor = parseHexColor(String(form.get('front_color') ?? ''))
    const backColor = parseHexColor(String(form.get('back_color') ?? ''))
    if (thicknessMm === null || thicknessMm < 0) {
      setCoreStatus('紙厚には0以上の有限の数値を入力してください')
      return
    }
    if (!frontColor || !backColor) {
      setCoreStatus('表色と裏色には有効な色を指定してください')
      return
    }

    void runNativeEdit((projectId, revision) =>
      updatePaperProperties(projectId, revision, {
        thicknessMm,
        frontColor: { ...frontColor, alpha: current.paper.front.color.alpha },
        backColor: { ...backColor, alpha: current.paper.back.color.alpha },
        cuttingAllowed: form.get('cutting_allowed') === 'on',
      }))
  }

  function submitPaperResize(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    const currentSize = resolveRectangularPaperSize(current)
    if (!currentSize) {
      setCoreStatus('現在の紙は軸平行な長方形ではないため、サイズを変更できません')
      return
    }

    const currentUnit = resolveLengthDisplayUnit(current)
    const referenceAxis = ratioReferenceAxis(currentUnit)
    const widthMm = referenceAxis === 'width'
      ? currentSize.width
      : readLengthInputMillimetres(
          event.currentTarget,
          'width_display',
          currentSize.width,
          currentUnit,
        )
    const heightMm = referenceAxis === 'height'
      ? currentSize.height
      : readLengthInputMillimetres(
          event.currentTarget,
          'height_display',
          currentSize.height,
          currentUnit,
        )
    if (widthMm === null || widthMm <= 0) {
      setCoreStatus('用紙の幅には0より大きい有限の数値を入力してください')
      return
    }
    if (heightMm === null || heightMm <= 0) {
      setCoreStatus('用紙の高さには0より大きい有限の数値を入力してください')
      return
    }

    void runNativeEdit((projectId, revision) =>
      resizeRectangularPaper(projectId, revision, widthMm, heightMm))
  }

  function changeLengthDisplayUnit(
    unit: Parameters<typeof setLengthDisplayUnit>[2],
  ) {
    if (coreOperationRef.current) return
    void runNativeEdit((projectId, revision) =>
      setLengthDisplayUnit(projectId, revision, unit))
  }

  async function runValidation() {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    coreOperationRef.current = true
    setCoreBusy(true)
    setValidation(null)
    setCoreStatus(`revision ${current.revision}: 検証中…`)
    setCancelInteractionToken((token) => token + 1)
    try {
      const result = await validateProject()
      const latest = latestSnapshotRef.current
      if (
        !latest
        || result.project_id !== current.project_id
        || result.revision !== current.revision
        || result.project_id !== latest.project_id
        || result.revision !== latest.revision
      ) {
        setCoreStatus('検証中に内容が変更されたため、再度検証してください')
        return
      }
      const localPresentation = createLocalFlatFoldabilityPresentation(
        result.local_flat_foldability,
        latest.crease_pattern.vertices.map((vertex) => vertex.id),
      )
      setValidation(result)
      if (localPresentation.kind === 'invalid') {
        reportValidationUnexpected()
      }
      const geometryStatus = result.is_valid
        ? '幾何検証に合格'
        : `幾何問題${result.issues.length}件`
      setCoreStatus(
        `revision ${result.revision}: ${geometryStatus}・`
        + localFlatFoldabilityCoreStatus(localPresentation),
      )
    } catch (error) {
      reportValidationUnexpected()
      setValidation(null)
      setCoreStatus(`検証エラー: ${String(error)}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function submitNewProject(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return

    const form = new FormData(event.currentTarget)
    const name = String(form.get('name') ?? '').trim()
    const widthMm = Number(form.get('width_mm'))
    const heightMm = Number(form.get('height_mm'))
    const thicknessInput = String(form.get('thickness_mm') ?? '').trim()
    const thicknessMm = Number(thicknessInput)
    const frontColor = parseHexColor(String(form.get('front_color') ?? ''))
    const backColor = parseHexColor(String(form.get('back_color') ?? ''))

    if (!name) {
      setNewProjectError('作品名を入力してください。')
      return
    }
    if ([...name].length > 120 || hasControlCharacter(name)) {
      setNewProjectError('作品名は制御文字を含まない120文字以内にしてください。')
      return
    }
    if (!Number.isFinite(widthMm) || widthMm <= 0) {
      setNewProjectError('幅には0より大きい有限の数値を入力してください。')
      return
    }
    if (!Number.isFinite(heightMm) || heightMm <= 0) {
      setNewProjectError('高さには0より大きい有限の数値を入力してください。')
      return
    }
    if (!thicknessInput || !Number.isFinite(thicknessMm) || thicknessMm < 0) {
      setNewProjectError('紙厚には0以上の有限の数値を入力してください。')
      return
    }
    if (!frontColor || !backColor) {
      setNewProjectError('表色と裏色を選択してください。')
      return
    }
    if (
      current.is_dirty &&
      !window.confirm('未保存の変更があります。保存せずに新しいプロジェクトを作成しますか？')
    ) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setNewProjectError(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const snapshot = await newProject(current.project_id, current.revision, {
        name,
        widthMm,
        heightMm,
        thicknessMm,
        cuttingAllowed: form.get('cutting_allowed') === 'on',
        frontColor,
        backColor,
      })
      applySnapshot(snapshot, true)
      setValidation(null)
      setSelectedLineId(null)
      setSelectedVertexId(null)
      setPendingEdgeStart(null)
      setParallelReferenceEdgeId(null)
      setActiveTool('select')
      setNewProjectOpen(false)
      setCoreStatus(`「${snapshot.name}」を作成しました。保存先はまだ設定されていません。`)
    } catch (error) {
      const message = String(error)
      setNewProjectError(`作成できませんでした: ${message}`)
      setCoreStatus(`新規作成エラー: ${message}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function runFileOperation(operation: 'open' | 'save' | 'save_as') {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    if (
      operation === 'open' &&
      current.is_dirty &&
      !window.confirm('未保存の変更があります。保存せずに別のプロジェクトを開きますか？')
    ) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation(operation)
    setCancelInteractionToken((token) => token + 1)
    try {
      const response = await (
        operation === 'open'
          ? openProject()
          : operation === 'save'
            ? saveProject()
            : saveProjectAs()
      )
      applySnapshot(
        response.project,
        operation === 'open' && !response.canceled,
      )
      if (response.canceled) {
        setCoreStatus('ファイル操作をキャンセルしました')
        return
      }
      if (operation === 'open') {
        setValidation(null)
        setSelectedLineId(null)
        setSelectedVertexId(null)
        setPendingEdgeStart(null)
        setParallelReferenceEdgeId(null)
      }
      setCoreStatus(operation === 'open'
        ? `「${response.project.name}」を開きました`
        : `「${response.project.name}」を保存しました`)
    } catch (error) {
      setCoreStatus(`ファイルエラー: ${String(error)}`)
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function beginFoldImport() {
    if (!latestSnapshotRef.current || coreOperationRef.current) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('fold_import')
    setFoldImportError(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const response = await previewFoldImport()
      if (response.canceled) {
        setCoreStatus('FOLD取込をキャンセルしました')
        return
      }
      if (!response.preview) {
        throw new Error('取込プレビューが返されませんでした')
      }
      setFoldImportPreview(response.preview)
      setCoreStatus('FOLDの線種・縮尺を確認してください')
    } catch (error) {
      setCoreStatus(`FOLD読込エラー: ${String(error)}`)
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function closeFoldImportDialog() {
    const preview = foldImportPreview
    if (!preview || coreOperationRef.current) return

    coreOperationRef.current = true
    setCoreBusy(true)
    try {
      await cancelFoldImport(preview.import_id)
      setCoreStatus('FOLD取込をキャンセルしました')
    } catch (error) {
      setCoreStatus(`FOLD取込の後始末エラー: ${String(error)}`)
    } finally {
      setFoldImportPreview(null)
      setFoldImportError(null)
      coreOperationRef.current = false
      setCoreBusy(false)
      requestAnimationFrame(() => foldImportButtonRef.current?.focus())
    }
  }

  async function confirmFoldImport(settings: FoldImportSettings) {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    if (
      current.is_dirty
      && !window.confirm('未保存の変更があります。保存せずにFOLD展開図へ置き換えますか？')
    ) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setFoldImportError(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const snapshot = await applyFoldImport(
        current.project_id,
        current.revision,
        settings,
      )
      applySnapshot(snapshot, true)
      setBenchmarkRun(null)
      setBenchmarkStatus('FOLD取込により通常の展開図へ戻りました')
      setFoldImportPreview(null)
      setSelectedLineId(null)
      setSelectedVertexId(null)
      setPendingEdgeStart(null)
      setParallelReferenceEdgeId(null)
      setAppliedFoldPose(null)
      setFoldAngleOverrides({ projectId: null, values: new Map() })
      setFixedFaceChoice({ projectId: null, faceId: null })
      setActiveTool('select')
      setCoreStatus(`FOLDから「${snapshot.name}」を取り込みました。保存先はまだ設定されていません。`)
      requestAnimationFrame(() => foldImportButtonRef.current?.focus())
    } catch (error) {
      const message = String(error)
      setFoldImportError(`取り込めませんでした: ${message}`)
      setCoreStatus(`FOLD取込エラー: ${message}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function beginSvgImport() {
    if (!latestSnapshotRef.current || coreOperationRef.current) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('svg_import')
    setSvgImportError(null)
    setSvgImportValidation(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const response = await previewSvgImport()
      if (response.canceled) {
        setCoreStatus('SVG取込をキャンセルしました')
        return
      }
      if (!response.preview) {
        throw new Error('取込プレビューが返されませんでした')
      }
      setSvgImportPreview(response.preview)
      setCoreStatus('SVGの外周・線種・縮尺を確認してください')
    } catch (error) {
      setCoreStatus(`SVG読込エラー: ${String(error)}`)
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function closeSvgImportDialog() {
    const preview = svgImportPreview
    if (!preview || coreOperationRef.current) return

    coreOperationRef.current = true
    setCoreBusy(true)
    try {
      await cancelSvgImport(preview.import_id)
      setCoreStatus('SVG取込をキャンセルしました')
      setSvgImportPreview(null)
      setSvgImportError(null)
      setSvgImportValidation(null)
      requestAnimationFrame(() => svgImportButtonRef.current?.focus())
    } catch (error) {
      const message = String(error)
      setSvgImportError(`取消を完了できませんでした: ${message}`)
      setCoreStatus(`SVG取込の後始末エラー: ${message}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function validateSvgImportDraft(settings: SvgImportSettingsDraft) {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setSvgImportError(null)
    setSvgImportValidation(null)
    try {
      const validation = await validateSvgImportSettings(
        current.project_id,
        current.revision,
        settings,
      )
      setSvgImportValidation(validation)
      setCoreStatus(
        `SVG外周を検証しました: ${validation.width_mm.toLocaleString()} × ${
          validation.height_mm.toLocaleString()
        } mm`,
      )
    } catch (error) {
      const message = String(error)
      setSvgImportError(`外周を検証できませんでした: ${message}`)
      setCoreStatus(`SVG外周検証エラー: ${message}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function confirmSvgImport(settings: SvgImportSettings) {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    const replaceDirtyProjectConfirmed = current.is_dirty
    if (
      replaceDirtyProjectConfirmed
      && !window.confirm('未保存の変更があります。保存せずにSVG展開図へ置き換えますか？')
    ) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setSvgImportError(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const snapshot = await applySvgImport(
        current.project_id,
        current.revision,
        settings,
        replaceDirtyProjectConfirmed,
      )
      applySnapshot(snapshot, true)
      setBenchmarkRun(null)
      setBenchmarkStatus('SVG取込により通常の展開図へ戻りました')
      setSvgImportPreview(null)
      setSvgImportValidation(null)
      setSelectedLineId(null)
      setSelectedVertexId(null)
      setPendingEdgeStart(null)
      setParallelReferenceEdgeId(null)
      setAppliedFoldPose(null)
      setFoldAngleOverrides({ projectId: null, values: new Map() })
      setFixedFaceChoice({ projectId: null, faceId: null })
      setActiveTool('select')
      setCoreStatus(`SVGから「${snapshot.name}」を取り込みました。保存先はまだ設定されていません。`)
      requestAnimationFrame(() => svgImportButtonRef.current?.focus())
    } catch (error) {
      const message = String(error)
      setSvgImportError(`取り込めませんでした: ${message}`)
      setCoreStatus(`SVG取込エラー: ${message}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function prepareCreaseExport(format: CreasePatternExportFormat) {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return

    const requestId = ++creaseExportRequestIdRef.current
    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('crease_export')
    setCreaseExportPreview(null)
    setCreaseExportError(null)
    setCreaseExportNotice(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const response = await previewCreasePatternExport(
        current.project_id,
        current.revision,
        format,
      )
      if (requestId !== creaseExportRequestIdRef.current) {
        await cancelCreasePatternExport(response.preview.export_id).catch(() => undefined)
        return
      }
      const latest = latestSnapshotRef.current
      const preview = response.preview
      if (
        !latest
        || preview.format !== format
        || preview.expected_project_id !== current.project_id
        || preview.expected_revision !== current.revision
        || latest.project_id !== current.project_id
        || latest.revision !== current.revision
      ) {
        await cancelCreasePatternExport(preview.export_id).catch(() => undefined)
        throw new Error('編集中のプロジェクトと一致しない書き出しプレビューを拒否しました')
      }
      setCreaseExportPreview(preview)
      setCoreStatus(
        `${creasePatternExportFormatLabel(preview.format)}書き出しの情報損失を確認してください`,
      )
    } catch (error) {
      if (requestId !== creaseExportRequestIdRef.current) return
      const message = String(error)
      setCreaseExportError(`書き出しデータを準備できませんでした: ${message}`)
      setCoreStatus(`展開図書き出しエラー: ${message}`)
    } finally {
      if (requestId === creaseExportRequestIdRef.current) {
        setFileOperation(null)
        coreOperationRef.current = false
        setCoreBusy(false)
      }
    }
  }

  function beginCreaseExport() {
    if (!latestSnapshotRef.current || coreOperationRef.current) return
    setCreaseExportOpen(true)
    setCreaseExportFormat('fold')
    setCreaseExportPreview(null)
    setCreaseExportError(null)
    setCreaseExportNotice(null)
    void prepareCreaseExport('fold')
  }

  function changeCreaseExportFormat(format: CreasePatternExportFormat) {
    if (format === creaseExportFormat || coreOperationRef.current) return
    setCreaseExportFormat(format)
    void prepareCreaseExport(format)
  }

  async function closeCreaseExportDialog() {
    if (coreOperationRef.current) return
    const preview = creaseExportPreview
    creaseExportRequestIdRef.current += 1
    if (!preview) {
      setCreaseExportOpen(false)
      setCreaseExportError(null)
      setCreaseExportNotice(null)
      requestAnimationFrame(() => creaseExportButtonRef.current?.focus())
      return
    }

    coreOperationRef.current = true
    setCoreBusy(true)
    try {
      await cancelCreasePatternExport(preview.export_id)
      setCreaseExportOpen(false)
      setCreaseExportPreview(null)
      setCreaseExportError(null)
      setCreaseExportNotice(null)
      setCoreStatus('展開図書き出しをキャンセルしました')
      requestAnimationFrame(() => creaseExportButtonRef.current?.focus())
    } catch (error) {
      const message = String(error)
      setCreaseExportError(`取消を完了できませんでした: ${message}`)
      setCoreStatus(`展開図書き出しの後始末エラー: ${message}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function saveCurrentCreaseExport(warningsAcknowledged: boolean) {
    const current = latestSnapshotRef.current
    const preview = creaseExportPreview
    if (!current || !preview || coreOperationRef.current) return
    if (
      current.project_id !== preview.expected_project_id
      || current.revision !== preview.expected_revision
    ) {
      setCreaseExportError('編集内容が変わったため、書き出しデータを作り直してください。')
      return
    }

    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('crease_export')
    setCreaseExportError(null)
    setCreaseExportNotice(null)
    try {
      const response = await saveCreasePatternExport(
        preview.export_id,
        current.project_id,
        current.revision,
        warningsAcknowledged,
      )
      if (response.canceled) {
        setCreaseExportNotice('保存先の選択をキャンセルしました。確認画面から再試行できます。')
        setCoreStatus('展開図の保存先選択をキャンセルしました')
        return
      }
      setCreaseExportOpen(false)
      setCreaseExportPreview(null)
      setCreaseExportNotice(null)
      setCoreStatus(`${preview.suggested_file_name}を書き出しました`)
      requestAnimationFrame(() => creaseExportButtonRef.current?.focus())
    } catch (error) {
      const message = String(error)
      setCreaseExportError(`書き出せませんでした: ${message}`)
      setCoreStatus(`展開図書き出しエラー: ${message}`)
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function prepareInstructionExport(format: InstructionExportFormat) {
    const current = latestSnapshotRef.current
    if (!current || !foldPreviewModel || coreOperationRef.current) return

    const requestId = ++instructionExportRequestIdRef.current
    instructionExportGenerationIdRef.current = null
    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('instruction_export')
    setInstructionExportGenerationActive(true)
    setInstructionExportPhase('validating')
    setInstructionExportPreview(null)
    setInstructionExportError(null)
    setInstructionExportNotice(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const generation = await beginInstructionExportGeneration()
      if (generation.profile !== INSTRUCTION_EXPORT_PROFILE) {
        await cancelInstructionExport(generation.export_id).catch(() => undefined)
        throw createInstructionExportError('document_contract_invalid')
      }
      if (requestId !== instructionExportRequestIdRef.current) {
        await cancelInstructionExport(generation.export_id).catch(() => undefined)
        return
      }
      instructionExportGenerationIdRef.current = generation.export_id
      void pollInstructionExportProgress(generation.export_id, requestId)
      const response = await previewInstructionExport(
        generation.export_id,
        current.project_id,
        current.revision,
        format,
      )
      if (requestId !== instructionExportRequestIdRef.current) {
        await cancelInstructionExport(response.preview.export_id).catch(() => undefined)
        return
      }
      const latest = latestSnapshotRef.current
      const preview = response.preview
      if (
        !latest
        || preview.export_id !== generation.export_id
        || preview.format !== format
        || preview.profile !== INSTRUCTION_EXPORT_PROFILE
        || preview.projection_profile !== INSTRUCTION_EXPORT_PROJECTION_PROFILE
        || preview.expected_project_id !== current.project_id
        || preview.expected_revision !== current.revision
        || latest.project_id !== current.project_id
        || latest.revision !== current.revision
      ) {
        await cancelInstructionExport(preview.export_id).catch(() => undefined)
        throw createInstructionExportError('document_contract_invalid')
      }
      setInstructionExportPreview(preview)
      setInstructionExportPhase('ready')
      setCoreStatus(
        `${instructionExportFormatLabel(preview.format)}の内容と注意事項を確認してください。`,
      )
    } catch (error) {
      if (requestId !== instructionExportRequestIdRef.current) return
      instructionExportGenerationIdRef.current = null
      const message = instructionExportErrorMessage(error)
      setInstructionExportError(`折り図を準備できませんでした: ${message}`)
      setCoreStatus(`折り図書き出しエラー: ${message}`)
    } finally {
      if (requestId === instructionExportRequestIdRef.current) {
        setInstructionExportGenerationActive(false)
        setFileOperation(null)
        coreOperationRef.current = false
        setCoreBusy(false)
      }
    }
  }

  async function pollInstructionExportProgress(exportId: string, requestId: number) {
    while (
      requestId === instructionExportRequestIdRef.current
      && instructionExportGenerationIdRef.current === exportId
    ) {
      await new Promise((resolve) => window.setTimeout(resolve, 100))
      if (
        requestId !== instructionExportRequestIdRef.current
        || instructionExportGenerationIdRef.current !== exportId
      ) return
      try {
        const progress = await getInstructionExportProgress(exportId)
        if (
          requestId !== instructionExportRequestIdRef.current
          || instructionExportGenerationIdRef.current !== exportId
          || progress.export_id !== exportId
        ) return
        setInstructionExportPhase(progress.phase)
        if (progress.phase === 'ready') return
      } catch (error) {
        if (
          requestId !== instructionExportRequestIdRef.current
          || instructionExportGenerationIdRef.current !== exportId
        ) return
        setInstructionExportNotice(
          `進捗表示を更新できませんでした: ${instructionExportErrorMessage(error)} 生成結果を待っています。`,
        )
        return
      }
    }
  }

  function beginInstructionExport() {
    if (!latestSnapshotRef.current || !foldPreviewModel || coreOperationRef.current) return
    setInstructionExportOpen(true)
    setInstructionExportFormat('pdf')
    setInstructionExportPreview(null)
    setInstructionExportError(null)
    setInstructionExportNotice(null)
    void prepareInstructionExport('pdf')
  }

  function changeInstructionExportFormat(format: InstructionExportFormat) {
    if (format === instructionExportFormat || coreOperationRef.current) return
    setInstructionExportFormat(format)
    void prepareInstructionExport(format)
  }

  async function closeInstructionExportDialog() {
    if (coreOperationRef.current && !instructionExportGenerationActive) return
    const preview = instructionExportPreview
    const exportId = instructionExportGenerationIdRef.current ?? preview?.export_id ?? null
    instructionExportRequestIdRef.current += 1
    instructionExportGenerationIdRef.current = null
    setInstructionExportGenerationActive(false)
    if (coreOperationRef.current) {
      setInstructionExportOpen(false)
      setInstructionExportPreview(null)
      setInstructionExportError(null)
      setInstructionExportNotice(null)
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
      setCoreStatus('折り図の生成を中止しています。')
      requestAnimationFrame(() => instructionExportButtonRef.current?.focus())
      if (exportId) {
        try {
          await cancelInstructionExport(exportId)
          setCoreStatus('折り図の生成を中止しました。')
        } catch {
          setCoreStatus('折り図の生成は終了済みです。')
        }
      }
      return
    }
    if (!preview) {
      setInstructionExportOpen(false)
      setInstructionExportError(null)
      setInstructionExportNotice(null)
      requestAnimationFrame(() => instructionExportButtonRef.current?.focus())
      return
    }

    coreOperationRef.current = true
    setCoreBusy(true)
    try {
      await cancelInstructionExport(preview.export_id)
      instructionExportGenerationIdRef.current = null
      setInstructionExportOpen(false)
      setInstructionExportPreview(null)
      setInstructionExportError(null)
      setInstructionExportNotice(null)
      setCoreStatus('折り図の書き出しをキャンセルしました。')
      requestAnimationFrame(() => instructionExportButtonRef.current?.focus())
    } catch (error) {
      const message = instructionExportErrorMessage(error)
      setInstructionExportError(`キャンセルを完了できませんでした: ${message}`)
      setCoreStatus(`折り図キャンセルエラー: ${message}`)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function saveCurrentInstructionExport(warningsAcknowledged: boolean) {
    const current = latestSnapshotRef.current
    const preview = instructionExportPreview
    if (!current || !preview || coreOperationRef.current) return
    if (
      current.project_id !== preview.expected_project_id
      || current.revision !== preview.expected_revision
    ) {
      setInstructionExportError(
        '編集内容が変わったため、折り図データを作り直してください。',
      )
      return
    }

    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('instruction_export')
    setInstructionExportError(null)
    setInstructionExportNotice(null)
    try {
      const response = await saveInstructionExport(
        preview.export_id,
        current.project_id,
        current.revision,
        warningsAcknowledged,
      )
      if (response.canceled) {
        setInstructionExportNotice(
          '保存先の選択をキャンセルしました。この画面からもう一度保存できます。',
        )
        setCoreStatus('折り図の保存先選択をキャンセルしました。')
        return
      }
      setInstructionExportOpen(false)
      instructionExportGenerationIdRef.current = null
      setInstructionExportPreview(null)
      setInstructionExportNotice(null)
      setCoreStatus(`${preview.suggested_file_name}を書き出しました。`)
      requestAnimationFrame(() => instructionExportButtonRef.current?.focus())
    } catch (error) {
      const message = instructionExportErrorMessage(error)
      setInstructionExportError(`折り図を書き出せませんでした: ${message}`)
      setCoreStatus(`折り図書き出しエラー: ${message}`)
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function toggleBenchmark() {
    if (benchmarkRun) {
      setBenchmarkRun(null)
      setBenchmarkStatus('通常の展開図に戻りました')
      setSelectedLineId(null)
      setSelectedVertexId(null)
      return
    }
    if (benchmarkLoading) return

    setBenchmarkLoading(true)
    setBenchmarkStatus('10,000本の実データを生成・転送中…')
    setSelectedLineId(null)
    setSelectedVertexId(null)
    setPendingEdgeStart(null)
    const requestId = ++benchmarkRequestIdRef.current
    const startedAt = performance.now()
    try {
      const result = await generateBenchmarkPattern(10_000)
      const responseMs = performance.now() - startedAt
      const preparationStartedAt = performance.now()
      const payloadBytes = measureBenchmarkPayloadBytes(result)
      const prepared = prepareBenchmarkRenderData(result)
      const preparationMs = performance.now() - preparationStartedAt
      const run: BenchmarkRun = {
        requestId,
        requestedEdgeCount: prepared.requestedEdgeCount,
        lines: prepared.lines.map((line) => ({ ...line })),
        vertices: prepared.vertices.map((vertex) => ({ ...vertex })),
        bounds: { ...prepared.bounds },
        payloadBytes,
        responseMs,
        preparationMs,
        startedAt,
      }
      setBenchmarkRun(run)
      setBenchmarkStatus(
        `${run.lines.length.toLocaleString()}本 · ${formatBytes(payloadBytes)} · `
        + `生成+転送 ${responseMs.toFixed(1)}ms · Canvas計測中…`,
      )
    } catch (error) {
      reportUnexpected('app.benchmark')
      setBenchmarkStatus(`ベンチマーク失敗: ${String(error)}`)
    } finally {
      setBenchmarkLoading(false)
    }
  }

  function recordBenchmarkRenderMetrics(metrics: CreaseCanvasRenderMetrics) {
    const run = benchmarkRun
    if (!run || !Object.is(metrics.requestId, run.requestId)) return
    const endToEndMs = performance.now() - run.startedAt
    const uiPreparationMs = Math.max(
      0,
      endToEndMs - run.responseMs - run.preparationMs - metrics.totalDurationMs,
    )
    setBenchmarkStatus(
      `${metrics.lineCount.toLocaleString()}本 · ${formatBytes(run.payloadBytes)} · `
      + `生成+転送 ${run.responseMs.toFixed(1)}ms · 変換 ${run.preparationMs.toFixed(1)}ms · `
      + `UI準備 ${uiPreparationMs.toFixed(1)}ms · 初描画 ${metrics.initialDrawMs.toFixed(1)}ms · `
      + `${metrics.sampleFrameCount}f ${metrics.framesPerSecond.toFixed(1)} FPS · `
      + `p95 ${metrics.p95DrawMs.toFixed(1)}ms`,
    )
  }

  return (
    <main className="app-shell">
      <header className="titlebar" inert={modalOpen}>
        <div className="brand-mark" aria-hidden="true">◇</div>
        <strong>ORIGAMI2</strong>
        <span
          className="document-name"
          title={nativeSnapshot?.current_path ?? undefined}
        >
          {nativeSnapshot?.name ?? '無題のプロジェクト'}
          {nativeSnapshot?.is_dirty ? ' *' : ''}
        </span>
        <nav className="top-actions" aria-label="プロジェクト操作">
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title="新規 (Ctrl/Cmd+N)"
            aria-keyshortcuts="Control+N Meta+N"
            onClick={() => {
              setNewProjectError(null)
              setNewProjectOpen(true)
            }}
          >
            新規
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot?.can_undo}
            onClick={() => runNativeEdit(undo)}
            title="元に戻す (Ctrl/Cmd+Z)"
            aria-keyshortcuts="Control+Z Meta+Z"
          >
            元に戻す
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot?.can_redo}
            onClick={() => runNativeEdit(redo)}
            title="やり直す (Ctrl/Cmd+Shift+Z / Ctrl+Y)"
            aria-keyshortcuts="Control+Shift+Z Meta+Shift+Z Control+Y"
          >
            やり直す
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot || !paperCenter}
            onClick={() => {
              if (!paperCenter) return
              void runNativeEdit((projectId, revision) =>
                addVertex(projectId, revision, paperCenter.x, paperCenter.y))
            }}
          >
            中央に頂点
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title="開く (Ctrl/Cmd+O)"
            aria-keyshortcuts="Control+O Meta+O"
            onClick={() => void runFileOperation('open')}
          >
            {fileOperation === 'open' ? '開いています…' : '開く'}
          </button>
          <button
            ref={foldImportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void beginFoldImport()}
            aria-haspopup="dialog"
          >
            {fileOperation === 'fold_import' ? '解析中…' : 'FOLD取込'}
          </button>
          <button
            ref={svgImportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void beginSvgImport()}
            aria-haspopup="dialog"
          >
            {fileOperation === 'svg_import' ? '解析中…' : 'SVG取込'}
          </button>
          <button
            ref={creaseExportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={beginCreaseExport}
            aria-haspopup="dialog"
          >
            {fileOperation === 'crease_export' ? '生成中…' : '書出し'}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title="保存 (Ctrl/Cmd+S)"
            aria-keyshortcuts="Control+S Meta+S"
            onClick={() => void runFileOperation('save')}
          >
            {fileOperation === 'save' ? '保存中…' : '保存'}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title="別名保存 (Ctrl/Cmd+Shift+S)"
            aria-keyshortcuts="Control+Shift+S Meta+Shift+S"
            onClick={() => void runFileOperation('save_as')}
          >
            {fileOperation === 'save_as' ? '保存中…' : '別名保存'}
          </button>
          <button
            type="button"
            className="primary"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void runValidation()}
          >
            検証
          </button>
        </nav>
      </header>

      <section className="workspace" inert={modalOpen}>
        <aside className="tool-rail" aria-label="作図ツール">
          {[
            ['select', '↖', '選択'],
            ['vertex', '＋', '頂点'],
            ['mountain', '━', '山折り'],
            ['valley', '┅', '谷折り'],
            ['auxiliary', '┈', '補助線'],
            ['cut', '✂', '切断'],
            ['measure', '∠', '計測'],
          ].map(([id, icon, label]) => (
            <button
              type="button"
              key={id}
              disabled={coreBusy || (id === 'cut' && !nativeSnapshot?.cutting_allowed)}
              className={activeTool === id ? 'active' : ''}
              onClick={() => {
                setActiveTool(id)
                setPendingEdgeStart(null)
              }}
              title={label}
              aria-label={label}
              aria-pressed={activeTool === id}
            >
              {icon}
            </button>
          ))}
        </aside>

        <section className="editor-grid">
          <article className="panel crease-panel">
            <div className="panel-heading">
              <span>2D 展開図</span>
              <span className="panel-meta">
                {benchmarkRun
                  ? `性能テスト · ${displayedLines.length.toLocaleString()}本`
                  : `${paperSizeLabel} · ${displayedLines.length.toLocaleString()}本`}
              </span>
            </div>
            <CreaseCanvas
              lines={displayedLines}
              paperBounds={benchmarkRun?.bounds ?? paperBounds}
              paperPolygon={benchmarkRun ? undefined : paperPolygon}
              paperColor={paperFrontColor}
              vertices={displayedVertices}
              tool={benchmarkRun ? 'select' : activeTool}
              selectedVertexId={selectedVertexId}
              pendingVertexId={pendingEdgeStart}
              selectedLineId={selectedLineId}
              measurementLabel={formatLineMeasurementLabel(
                selectedLineMeasurement,
                displayedLengthUnit,
              )}
              snapSettings={snapSettings}
              parallelReference={benchmarkRun ? null : parallelReferenceLine}
              angleConfig={angleSnapConfig}
              validationVertexHighlights={canvasLocalFlatFoldabilityHighlights}
              ariaDescribedBy={localFlatFoldabilitySummaryId}
              cancelInteractionToken={cancelInteractionToken}
              disabled={coreBusy || benchmarkLoading}
              renderMetricsRequestId={benchmarkRun?.requestId ?? null}
              onRenderMetrics={recordBenchmarkRenderMetrics}
              onSelectLine={(lineId) => {
                setSelectedLineId(lineId)
                if (lineId) setSelectedVertexId(null)
              }}
              onPlaceVertex={benchmarkRun
                ? undefined
                : (placement) => void placeCanvasVertex(placement)}
              onPlacementBlocked={benchmarkRun
                ? undefined
                : (reason) => {
                    if (reason === 'intersection-truncated') {
                      setCoreStatus('交点候補が過密なため配置できません。拡大して再試行してください')
                    } else if (reason === 'intersection-blocked') {
                      setCoreStatus('未対応または曖昧な交点クラスタのため配置できません。辺や頂点の重複を確認してください')
                    }
                  }}
              onSelectVertex={benchmarkRun
                ? (vertexId) => {
                    setSelectedVertexId(vertexId)
                    setSelectedLineId(null)
                  }
                : selectCanvasVertex}
              onMoveVertex={benchmarkRun
                ? undefined
                : (vertexId, x, y) => {
                    void runNativeEdit((projectId, revision) =>
                      moveVertex(projectId, revision, vertexId, x, y))
                  }}
            />
          </article>

          <article className="panel preview-panel">
            <div className="panel-heading">
              <span>3D プレビュー</span>
              <span className={foldPreviewStatusClass}>{foldPreviewStatus}</span>
            </div>
            <FoldPreview
              angle={foldAngle}
              hingeAngles={foldTreeHingeAngles}
              selectedHingeId={selectedPreviewHingeId}
              fixedFaceId={effectiveFixedFaceId}
              onSelectHinge={benchmarkRun || foldPreviewHingeIds.size === 0
                ? undefined
                : (edgeId) => {
                    setSelectedLineId(edgeId)
                    if (edgeId) setSelectedVertexId(null)
                  }}
              onChooseFixedFace={!fixedFaceEnabled
                ? undefined
                : (faceId) => {
                    if (
                      !foldPreviewModel
                      || !fixedFaceOptions.some((face) => face.id === faceId)
                    ) return
                    setManualPoseChangeSequence((sequence) => sequence + 1)
                    setFixedFaceChoice({
                      projectId: foldPreviewModel.projectId,
                      faceId,
                    })
                  }}
              onRequestFoldAngle={
                !benchmarkRun && foldPreviewModel?.kind === 'single_fold'
                  ? updateUniformFoldAngle
                  : undefined
              }
              onCommitHingeFoldAngle={
                !benchmarkRun
                && foldPreviewModel?.kind === 'fold_graph'
                && foldPreviewModel.kinematics.kind === 'tree'
                  ? updateHingeFoldAngle
                  : undefined
              }
              onAppliedPoseChange={setAppliedFoldPose}
              nativeCollisionState={
                isNativeCoreAvailable() && foldPreviewModel
                  ? nativeStaticCollisionState
                  : undefined
              }
              nativeCollisionObservedPose={appliedFoldPose}
              onRetryNativeCollision={() => {
                const current = nativeStaticCollisionRequest
                if (!current) return
                setBoundNativeStaticCollisionView({
                  requestKey: current.requestKey,
                  view: { kind: 'checking' },
                })
                setNativeStaticCollisionRetrySequence((current) =>
                  current === Number.MAX_SAFE_INTEGER ? 0 : current + 1)
              }}
              model={foldPreviewModel}
              statusMessage={foldPreviewStatus}
              frontColor={nativeSnapshot?.paper.front.color}
              backColor={nativeSnapshot?.paper.back.color}
              thicknessMm={nativeSnapshot?.paper.thickness_mm}
              lengthDisplayUnit={lengthDisplayUnit}
            />
            <div className="fixed-face-control">
              <label htmlFor="fixed-face">固定面</label>
              <select
                id="fixed-face"
                value={effectiveFixedFaceId ?? ''}
                disabled={!fixedFaceEnabled}
                title={effectiveFixedFaceLabel}
                onChange={(event) => {
                  if (!foldPreviewModel || !fixedFaceEnabled) return
                  setManualPoseChangeSequence((sequence) => sequence + 1)
                  setFixedFaceChoice({
                    projectId: foldPreviewModel.projectId,
                    faceId: event.currentTarget.value,
                  })
                }}
              >
                {fixedFaceOptions.length > 0
                  ? fixedFaceOptions.map((face, index) => (
                      <option value={face.id} key={face.id}>面 {index + 1}</option>
                    ))
                  : <option value="">選択不可</option>}
              </select>
              <span>{fixedFaceEnabled ? '青枠・固定' : '—'}</span>
            </div>
            <div className="fold-control">
              <label htmlFor="fold-angle">
                {foldPreviewModel?.kind === 'fold_graph'
                  && foldPreviewModel.kinematics.kind === 'tree'
                  ? '全ヒンジ'
                  : '指定折り量'}
              </label>
              <input
                id="fold-angle"
                type="range"
                min="0"
                max="180"
                step="0.1"
                disabled={!foldAngleEnabled}
                value={foldAngle}
                onChange={(event) => updateUniformFoldAngle(event.currentTarget.valueAsNumber)}
              />
              {foldAngleEnabled ? (
                <span className="fold-angle-number">
                  <input
                    type="number"
                    min="0"
                    max="180"
                    step="0.1"
                    aria-label={
                      foldPreviewModel?.kind === 'fold_graph'
                        ? '全ヒンジの指定折り量（度）'
                        : '指定折り量（度）'
                    }
                    value={foldAngle}
                    onChange={(event) => updateUniformFoldAngle(event.currentTarget.valueAsNumber)}
                  />
                  <span aria-hidden="true">°</span>
                </span>
              ) : <output className="fold-angle-unavailable">—</output>}
            </div>
            {foldPreviewModel?.kind === 'fold_graph'
              && foldPreviewModel.kinematics.kind === 'tree'
              && foldTreeHingeAngles ? (
                <section className="hinge-angle-controls" aria-labelledby="hinge-angle-title">
                  <div className="hinge-angle-heading">
                    <strong id="hinge-angle-title">ヒンジ別の折り量</strong>
                    <span>橙枠=従属面・衝突未検証</span>
                  </div>
                  {foldPreviewModel.kinematics.joints.map((joint, index) => {
                    const hingeAngle = foldTreeHingeAngles[index]?.angleDegrees ?? foldAngle
                    const label = joint.hinge.assignment === 'mountain' ? '山折り' : '谷折り'
                    const inputId = `hinge-angle-${joint.hinge.edgeId}`
                    const selected = selectedLineId === joint.hinge.edgeId
                    return (
                      <div className="hinge-angle-row" key={joint.hinge.edgeId}>
                        <button
                          type="button"
                          className="hinge-select-button"
                          aria-pressed={benchmarkRun ? false : selected}
                          aria-label={`${index + 1}番目の${label}を2D・3Dで${selected ? '選択解除' : '選択'}`}
                          disabled={Boolean(benchmarkRun)}
                          title={`2D・3Dで選択: ${joint.hinge.edgeId}`}
                          onClick={() => {
                            setSelectedLineId(selected ? null : joint.hinge.edgeId)
                            setSelectedVertexId(null)
                          }}
                        >
                          {index + 1}. {label}
                        </button>
                        <input
                          id={inputId}
                          type="range"
                          min="0"
                          max="180"
                          step="0.1"
                          aria-label={`${index + 1}番目の${label}の折り量`}
                          value={hingeAngle}
                          onChange={(event) => updateHingeFoldAngle(
                            joint.hinge.edgeId,
                            event.currentTarget.valueAsNumber,
                          )}
                        />
                        <span className="fold-angle-number">
                          <input
                            type="number"
                            min="0"
                            max="180"
                            step="0.1"
                            aria-label={`${index + 1}番目の${label}の角度`}
                            value={hingeAngle}
                            onChange={(event) => updateHingeFoldAngle(
                              joint.hinge.edgeId,
                              event.currentTarget.valueAsNumber,
                            )}
                          />
                          <span aria-hidden="true">°</span>
                        </span>
                      </div>
                    )
                  })}
                </section>
              ) : null}
          </article>
        </section>

        <aside className="inspector panel">
          <div className="panel-heading">プロパティ</div>
          <section>
            <h2>選択要素</h2>
            {selectedLine ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedLine.id}</dd></div>
                  <div><dt>種類</dt><dd>{lineKindLabel(selectedLine.kind)}</dd></div>
                  <div>
                    <dt>始点</dt>
                    <dd>{formatLengthPoint(
                      selectedLine.x1,
                      selectedLine.y1,
                      displayedLengthUnit,
                    )}</dd>
                  </div>
                  <div>
                    <dt>終点</dt>
                    <dd>{formatLengthPoint(
                      selectedLine.x2,
                      selectedLine.y2,
                      displayedLengthUnit,
                    )}</dd>
                  </div>
                  <div><dt>ΔX</dt><dd>{formatLength(selectedLineMeasurement?.deltaX, displayedLengthUnit)}</dd></div>
                  <div><dt>ΔY</dt><dd>{formatLength(selectedLineMeasurement?.deltaY, displayedLengthUnit)}</dd></div>
                  <div><dt>長さ</dt><dd>{formatLength(selectedLineMeasurement?.length, displayedLengthUnit)}</dd></div>
                  <div><dt>角度</dt><dd>{formatMeasurementValue(selectedLineMeasurement?.angleDegrees, '°', 2)}</dd></div>
                </dl>
                {benchmarkRun ? (
                  <p className="muted">性能テストの図は選択・計測のみ可能です。</p>
                ) : (
                  <div className="property-actions">
                    <button
                      type="button"
                      aria-pressed={parallelReferenceEdgeId === selectedLine.id}
                      disabled={coreBusy}
                      onClick={() => setParallelReferenceEdgeId((current) => (
                        current === selectedLine.id ? null : selectedLine.id
                      ))}
                    >
                      {parallelReferenceEdgeId === selectedLine.id
                        ? '方向参照を解除'
                        : '方向参照に設定'}
                    </button>
                    {selectedLine.kind === 'boundary' ? (
                      <button
                        type="button"
                        disabled={coreBusy}
                        onClick={() => void splitSelectedBoundaryEdge()}
                      >
                        輪郭辺を中点で分割
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="danger"
                        disabled={coreBusy}
                        onClick={() => void deleteSelection()}
                      >
                        線を削除
                      </button>
                    )}
                  </div>
                )}
                {selectedLine.kind === 'boundary' && (
                  <p className="muted">分割後に選択される新しい頂点を移動して、紙の輪郭を編集できます。</p>
                )}
              </>
            ) : selectedBenchmarkVertex ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedBenchmarkVertex.id}</dd></div>
                  <div><dt>種類</dt><dd>性能テスト頂点</dd></div>
                  <div><dt>X</dt><dd>{selectedBenchmarkVertex.x}</dd></div>
                  <div><dt>Y</dt><dd>{selectedBenchmarkVertex.y}</dd></div>
                </dl>
                <p className="muted">性能テストの図は選択・計測のみ可能です。</p>
              </>
            ) : selectedVertex ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedVertex.id}</dd></div>
                  <div><dt>種類</dt><dd>頂点</dd></div>
                </dl>
                <form
                  key={`${selectedVertex.id}:${selectedVertex.position.x}:${selectedVertex.position.y}:${lengthDisplayUnit.key}`}
                  className="coordinate-form"
                  onSubmit={submitVertexPosition}
                >
                  <label className="field">
                    {`X (${lengthDisplayUnit.label})`}
                    <LengthValueInput
                      name="x_display"
                      disabled={coreBusy}
                      initialMillimetres={selectedVertex.position.x}
                      unit={lengthDisplayUnit}
                      ariaLabel={`頂点のX座標 (${lengthDisplayUnit.label})`}
                    />
                  </label>
                  <label className="field">
                    {`Y (${lengthDisplayUnit.label})`}
                    <LengthValueInput
                      name="y_display"
                      disabled={coreBusy}
                      initialMillimetres={selectedVertex.position.y}
                      unit={lengthDisplayUnit}
                      ariaLabel={`頂点のY座標 (${lengthDisplayUnit.label})`}
                    />
                  </label>
                  <div className="property-actions">
                    <button type="submit" disabled={coreBusy}>座標を更新</button>
                    <button
                      type="button"
                      className="danger"
                      disabled={
                        coreBusy ||
                        (selectedVertexIsBoundary && paperBoundaryVertexCount <= 3)
                      }
                      onClick={() => void deleteSelection()}
                    >
                      {selectedVertexIsBoundary
                        ? '輪郭頂点を削除して辺を統合'
                        : '頂点を削除'}
                    </button>
                  </div>
                  <p className="muted">
                    {selectedVertexIsBoundary
                      ? `輪郭は最低3点必要です（現在${paperBoundaryVertexCount}点）。この操作は元に戻せます。接続線がある場合など、安全に統合できない削除は拒否されます。`
                      : '接続線がある頂点は、線を削除してから削除します。'}
                  </p>
                </form>
              </>
            ) : <p className="muted">線または頂点を選択してください</p>}
          </section>
          {validation && (
            <section className={validation.is_valid ? 'validation-report valid' : 'validation-report invalid'}>
              <h2>幾何検証</h2>
              {validation.is_valid ? (
                <p>問題は見つかりませんでした。</p>
              ) : (
                <>
                  <p>{validation.issues.length}件の問題が見つかりました。</p>
                  <ul>
                    {validation.issues.slice(0, 20).map((issue, index) => {
                      const edgeId = issue.edges.find((id) =>
                        nativeLines.some((line) => line.id === id))
                      const vertexId = issue.vertices.find((id) =>
                        nativeSnapshot?.crease_pattern.vertices.some((vertex) => vertex.id === id))
                      const label = validationIssueLabel(issue.code)
                      return (
                        <li key={`${issue.code}:${index}`}>
                          {edgeId || vertexId ? (
                            <button
                              type="button"
                              onClick={() => {
                                if (edgeId) {
                                  setSelectedLineId(edgeId)
                                  setSelectedVertexId(null)
                                } else if (vertexId) {
                                  setSelectedVertexId(vertexId)
                                  setSelectedLineId(null)
                                }
                              }}
                            >
                              {label}
                            </button>
                          ) : <span>{label}</span>}
                        </li>
                      )
                    })}
                  </ul>
                </>
              )}
            </section>
          )}
          {localFlatFoldabilityPresentation && !benchmarkRun && (
            <section
              className={`local-flat-foldability-report is-${
                localFlatFoldabilityPresentation.kind === 'ready'
                  ? localFlatFoldabilityPresentation.reportStatus
                  : localFlatFoldabilityPresentation.kind
              }`}
            >
              <h2>局所平坦折り条件</h2>
              <p
                id="local-flat-foldability-summary"
                className="local-flat-foldability-summary"
                role="status"
                aria-live="polite"
                aria-atomic="true"
              >
                {localFlatFoldabilityPresentation.summaryText}
              </p>
              {localFlatFoldabilityPresentation.maxExactFoldDegree !== null && (
                <p className="local-flat-foldability-coverage">
                  対応範囲: 紙内部の単一頂点・ゼロ厚モデル、
                  折り線次数{localFlatFoldabilityPresentation.maxExactFoldDegree}以下
                </p>
              )}
              {localFlatFoldabilityPresentation.kind === 'ready' && (
                <>
                  <ul
                    className="local-flat-foldability-counts"
                    aria-label="局所平坦折り条件の頂点別件数"
                  >
                    {([
                      ['satisfied', '成立', localFlatFoldabilityPresentation.counts.satisfied],
                      ['violated', '不成立', localFlatFoldabilityPresentation.counts.violated],
                      [
                        'not-applicable',
                        '対象外',
                        localFlatFoldabilityPresentation.counts.notApplicable,
                      ],
                      [
                        'indeterminate',
                        '判定不能',
                        localFlatFoldabilityPresentation.counts.indeterminate,
                      ],
                    ] as const).map(([kind, label, count]) => (
                      <li key={kind} className={`is-${kind}`}>
                        <span>{label}</span>
                        <strong>{count.toLocaleString()}</strong>
                      </li>
                    ))}
                  </ul>
                  {selectedLocalFlatFoldability && (
                    <div className="selected-local-flat-foldability">
                      <h3>選択頂点の局所条件</h3>
                      <dl>
                        <div>
                          <dt>総合</dt>
                          <dd>
                            {localFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.verdict,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>川崎条件</dt>
                          <dd>
                            {localFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.kawasaki,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>前川条件</dt>
                          <dd>
                            {localFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.maekawa,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>折り線次数</dt>
                          <dd>{selectedLocalFlatFoldability.foldDegree}</dd>
                        </div>
                        <div>
                          <dt>山折り / 谷折り</dt>
                          <dd>
                            {selectedLocalFlatFoldability.mountainCount}
                            {' / '}
                            {selectedLocalFlatFoldability.valleyCount}
                          </dd>
                        </div>
                      </dl>
                      {selectedLocalFlatFoldability.reason && (
                        <p className="local-flat-foldability-reason">
                          {localFlatFoldabilityReasonLabel(
                            selectedLocalFlatFoldability.reason,
                            localFlatFoldabilityPresentation.maxExactFoldDegree,
                          )}
                        </p>
                      )}
                    </div>
                  )}
                  {localFlatFoldabilityPresentation.visibleItems.length > 0 && (
                    <>
                      <h3>確認が必要な頂点</h3>
                      <ul className="local-flat-foldability-items">
                        {localFlatFoldabilityPresentation.visibleItems.map((item) => {
                          const verdictLabel = localFlatFoldabilityConditionLabel(item.verdict)
                          const reasonLabel = localFlatFoldabilityReasonLabel(
                            item.reason,
                            localFlatFoldabilityPresentation.maxExactFoldDegree,
                          )
                          return (
                            <li key={item.vertexId}>
                              <button
                                type="button"
                                aria-pressed={selectedVertexId === item.vertexId}
                                aria-label={
                                  `頂点${item.ordinal}、局所必要条件${verdictLabel}。`
                                  + `川崎条件${localFlatFoldabilityConditionLabel(item.kawasaki)}、`
                                  + `前川条件${localFlatFoldabilityConditionLabel(item.maekawa)}。`
                                  + reasonLabel
                                }
                                onClick={() => {
                                  setSelectedVertexId(item.vertexId)
                                  setSelectedLineId(null)
                                }}
                              >
                                <span className={`local-verdict is-${item.verdict}`}>
                                  {verdictLabel}
                                </span>
                                <span>頂点 {item.ordinal}</span>
                                <span className="local-flat-foldability-item-detail">
                                  {reasonLabel || (
                                    `川崎 ${localFlatFoldabilityConditionLabel(item.kawasaki)}・`
                                    + `前川 ${localFlatFoldabilityConditionLabel(item.maekawa)}`
                                  )}
                                </span>
                              </button>
                            </li>
                          )
                        })}
                      </ul>
                      {localFlatFoldabilityPresentation.hiddenItemCount > 0 && (
                        <p className="muted">
                          ほか
                          {localFlatFoldabilityPresentation.hiddenItemCount.toLocaleString()}
                          頂点。頂点を選択すると個別結果を確認できます。
                        </p>
                      )}
                    </>
                  )}
                </>
              )}
              <p className="local-flat-foldability-disclaimer">
                成立はこのモデルで確認した局所必要条件だけを表します。
                展開図全体が平坦に折り畳めることや、実際の折り経路は保証しません。
              </p>
            </section>
          )}
          <GlobalFlatFoldabilityPanel
            job={globalFlatFoldabilityJob}
            timeLimitSeconds={globalFlatFoldabilityTimeLimit}
            startDisabled={
              coreBusy
              || benchmarkLoading
              || Boolean(benchmarkRun)
              || !nativeSnapshot
              || !isNativeCoreAvailable()
            }
            onTimeLimitChange={setGlobalFlatFoldabilityTimeLimit}
            onStart={startGlobalFlatFoldability}
            onCancel={cancelGlobalFlatFoldability}
          />
          <section>
            <h2>紙</h2>
            <LengthUnitControl
              unit={lengthDisplayUnit}
              references={boundaryLengthReferences}
              disabled={coreBusy || !nativeSnapshot}
              onChange={changeLengthDisplayUnit}
            />
            <form
              key={paperFormKey}
              className="paper-properties-form"
              onSubmit={submitPaperProperties}
              noValidate
            >
              <div className="field">
                <label htmlFor="paper-thickness-mm">厚さ</label>
                <PaperThicknessInput
                  id="paper-thickness-mm"
                  name="thickness_display"
                  initialValue={lengthDisplayUnit.effectiveUnit === 'mm'
                    ? formatPaperThicknessInput(
                        nativeSnapshot?.paper.thickness_mm,
                      )
                    : formatLengthInput(
                        nativeSnapshot?.paper.thickness_mm,
                        lengthDisplayUnit,
                      )}
                  sourceMillimetres={nativeSnapshot?.paper.thickness_mm}
                  unit={lengthDisplayUnit}
                  disabled={coreBusy || !nativeSnapshot}
                />
                <span>{lengthDisplayUnit.label}</span>
              </div>
              <div className="paper-color-fields">
                <label className="paper-color-field">
                  <span>表色</span>
                  <input
                    name="front_color"
                    type="color"
                    defaultValue={rgbaToHex(nativeSnapshot?.paper.front.color, '#ffffff')}
                    disabled={coreBusy || !nativeSnapshot}
                  />
                </label>
                <label className="paper-color-field">
                  <span>裏色</span>
                  <input
                    name="back_color"
                    type="color"
                    defaultValue={rgbaToHex(nativeSnapshot?.paper.back.color, '#f8f8f5')}
                    disabled={coreBusy || !nativeSnapshot}
                  />
                </label>
              </div>
              <label className="check">
                <input
                  name="cutting_allowed"
                  type="checkbox"
                  defaultChecked={nativeSnapshot?.paper.cutting_allowed ?? false}
                  disabled={coreBusy || !nativeSnapshot}
                />{' '}
                切断を許可
              </label>
              <div className="property-actions">
                <button type="submit" disabled={coreBusy || !nativeSnapshot}>
                  紙設定を更新
                </button>
              </div>
            </form>
            <div className="paper-size-editor">
              <h3>用紙サイズ</h3>
              <form
                key={paperResizeFormKey}
                className="paper-size-form"
                onSubmit={submitPaperResize}
                noValidate
              >
                <div className="paper-size-fields">
                  <label className="field">
                    <span>幅</span>
                    <LengthValueInput
                      name="width_display"
                      minimumMillimetres={0}
                      initialMillimetres={rectangularPaperSize?.width ?? 0}
                      unit={lengthDisplayUnit}
                      readOnly={rectangularRatioReferenceAxis === 'width'}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      ariaLabel={`用紙の幅 (${lengthDisplayUnit.label})`}
                    />
                    <span>{lengthDisplayUnit.label}</span>
                  </label>
                  <label className="field">
                    <span>高さ</span>
                    <LengthValueInput
                      name="height_display"
                      minimumMillimetres={0}
                      initialMillimetres={rectangularPaperSize?.height ?? 0}
                      unit={lengthDisplayUnit}
                      readOnly={rectangularRatioReferenceAxis === 'height'}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      ariaLabel={`用紙の高さ (${lengthDisplayUnit.label})`}
                    />
                    <span>{lengthDisplayUnit.label}</span>
                  </label>
                </div>
                {!rectangularPaperSize && (
                  <p className="paper-size-note">
                    軸平行な長方形として判定できない紙は、この画面ではサイズ変更できません。
                  </p>
                )}
                <p className="paper-size-note">
                  サイズ変更時は、折り線を含むすべての頂点を左上基準で比例変換します。
                </p>
                {rectangularRatioReferenceAxis && (
                  <p className="paper-size-note">
                    紙辺比では基準辺と平行な
                    {rectangularRatioReferenceAxis === 'width' ? '幅' : '高さ'}
                    は 1 のまま読み取り専用です。直交する寸法だけを変更し、
                    基準辺の物理長は維持します。
                  </p>
                )}
                <div className="property-actions">
                  <button
                    type="submit"
                    disabled={coreBusy || !nativeSnapshot || !rectangularPaperSize}
                  >
                    用紙サイズを変更
                  </button>
                </div>
              </form>
            </div>
          </section>
          <section>
            <h2>スナップ</h2>
            <div className="chip-row" aria-label="スナップ設定">
              {SNAP_OPTIONS.map(({ kind, label }) => (
                <button
                  key={kind}
                  type="button"
                  className={`chip${snapSettings[kind] ? ' active' : ''}`}
                  aria-pressed={snapSettings[kind]}
                  disabled={coreBusy}
                  onClick={() => setSnapSettings((current) => toggleSnapSetting(current, kind))}
                >
                  {label}
                </button>
              ))}
            </div>
            <div className="angle-snap-settings">
              <h3>角度スナップ</h3>
              <label className="angle-snap-field">
                <span>プリセット</span>
                <select
                  value={selectedAnglePreset}
                  disabled={coreBusy}
                  onChange={(event) => {
                    if (event.target.value === 'custom') {
                      angleInputRef.current?.focus()
                      angleInputRef.current?.select()
                      return
                    }
                    const nextDegrees = Number(event.target.value)
                    setAngleDegrees(nextDegrees)
                    setAngleDegreesInput(String(nextDegrees))
                  }}
                >
                  {ANGLE_SNAP_PRESETS.map((preset) => (
                    <option key={preset} value={preset}>{preset}°</option>
                  ))}
                  <option value="custom">任意角</option>
                </select>
              </label>
              <label className="angle-snap-field">
                <span>角度</span>
                <span className="angle-input-with-unit">
                  <input
                    ref={angleInputRef}
                    type="number"
                    min="0"
                    max="90"
                    step="any"
                    value={angleDegreesInput}
                    disabled={coreBusy}
                    aria-invalid={!angleInputIsValid}
                    aria-describedby={!angleInputIsValid ? 'angle-snap-error' : undefined}
                    onChange={(event) => {
                      const nextInput = event.target.value
                      const nextDegrees = Number(nextInput)
                      setAngleDegreesInput(nextInput)
                      if (
                        nextInput.trim().length > 0
                        && Number.isFinite(nextDegrees)
                        && nextDegrees > 0
                        && nextDegrees <= 90
                      ) setAngleDegrees(nextDegrees)
                    }}
                  />
                  <span>°</span>
                </span>
              </label>
              {!angleInputIsValid && (
                <p id="angle-snap-error" className="field-error" role="alert">
                  角度は0より大きく90以下で入力してください。最後の正常値を使用します。
                </p>
              )}
              <div className="angle-reference-setting">
                <span>基準</span>
                <div className="chip-row" role="group" aria-label="角度スナップの基準">
                  <button
                    type="button"
                    className={`chip${angleReferenceKind === 'global-horizontal' ? ' active' : ''}`}
                    aria-pressed={angleReferenceKind === 'global-horizontal'}
                    disabled={coreBusy}
                    onClick={() => setAngleReferenceKind('global-horizontal')}
                  >
                    水平
                  </button>
                  <button
                    type="button"
                    className={`chip${angleReferenceKind === 'edge' ? ' active' : ''}`}
                    aria-pressed={angleReferenceKind === 'edge'}
                    disabled={coreBusy}
                    onClick={() => setAngleReferenceKind('edge')}
                  >
                    方向参照辺
                  </button>
                </div>
              </div>
              <p className="muted">
                現在: {formatAngleDegrees(angleDegrees)}°・
                {angleReferenceKind === 'global-horizontal' ? '水平基準' : '方向参照辺基準'}
              </p>
              {snapSettings.angle && angleReferenceKind === 'edge' && !parallelReferenceLine && (
                <p className="field-error" role="status">
                  線を選択して方向参照に設定してください。暗黙に水平基準へは切り替えません。
                </p>
              )}
            </div>
            {parallelReferenceLine ? (
              <div className="property-actions">
                <span className="muted" title={parallelReferenceLine.id}>
                  方向参照（平行・角度）: {lineKindLabel(parallelReferenceLine.kind)}
                </span>
                <button
                  type="button"
                  disabled={coreBusy}
                  onClick={() => setParallelReferenceEdgeId(null)}
                >
                  参照を解除
                </button>
              </div>
            ) : (
              <p className="muted">
                線を選択して「方向参照に設定」を押すと、平行・角度スナップの基準にできます。
              </p>
            )}
          </section>
        </aside>
      </section>

      <InstructionTimelinePanel
        snapshot={nativeSnapshot}
        appliedPose={appliedFoldPose}
        poseModelKey={foldPreviewPoseModelKey}
        manualPoseChangeSequence={manualPoseChangeSequence}
        coreBusy={coreBusy}
        benchmarkActive={benchmarkLoading || Boolean(benchmarkRun)}
        fileOperationActive={fileOperation !== null}
        exportAvailable={Boolean(foldPreviewModel)}
        exportButtonRef={instructionExportButtonRef}
        inert={modalOpen}
        runNativeEdit={runNativeEdit}
        applyStepPose={applyInstructionStepPose}
        onExport={beginInstructionExport}
      />

      {newProjectOpen && (
        <div className="dialog-backdrop">
          <section
            className="new-project-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="new-project-title"
          >
            <header>
              <div>
                <span className="dialog-eyebrow">一枚紙から開始</span>
                <h2 id="new-project-title">新しいプロジェクト</h2>
              </div>
              <button
                type="button"
                className="dialog-close"
                disabled={coreBusy}
                onClick={() => {
                  setNewProjectOpen(false)
                  setNewProjectError(null)
                }}
                aria-label="閉じる"
              >
                ×
              </button>
            </header>
            <form onSubmit={submitNewProject} noValidate>
              <label className="dialog-field dialog-field-wide">
                <span>作品名</span>
                <input
                  name="name"
                  defaultValue="無題の作品"
                  maxLength={120}
                  required
                  autoFocus
                  disabled={coreBusy}
                />
              </label>

              <fieldset>
                <legend>用紙サイズ</legend>
                <div className="dialog-grid two-columns">
                  <label className="dialog-field">
                    <span>幅</span>
                    <span className="number-with-unit">
                      <input
                        name="width_mm"
                        type="number"
                        defaultValue="400"
                        min="0"
                        step="any"
                        required
                        disabled={coreBusy}
                      />
                      mm
                    </span>
                  </label>
                  <label className="dialog-field">
                    <span>高さ</span>
                    <span className="number-with-unit">
                      <input
                        name="height_mm"
                        type="number"
                        defaultValue="400"
                        min="0"
                        step="any"
                        required
                        disabled={coreBusy}
                      />
                      mm
                    </span>
                  </label>
                </div>
              </fieldset>

              <fieldset>
                <legend>材料設定</legend>
                <div className="dialog-grid three-columns">
                  <div className="dialog-field">
                    <label htmlFor="new-project-paper-thickness-mm">紙厚</label>
                    <span className="number-with-unit">
                      <PaperThicknessInput
                        id="new-project-paper-thickness-mm"
                        initialValue="0.10"
                        disabled={coreBusy}
                      />
                      mm
                    </span>
                  </div>
                  <label className="dialog-field color-field">
                    <span>表色</span>
                    <input
                      name="front_color"
                      type="color"
                      defaultValue="#ffffff"
                      disabled={coreBusy}
                    />
                  </label>
                  <label className="dialog-field color-field">
                    <span>裏色</span>
                    <input
                      name="back_color"
                      type="color"
                      defaultValue="#f8f8f5"
                      disabled={coreBusy}
                    />
                  </label>
                </div>
                <label className="dialog-check">
                  <input name="cutting_allowed" type="checkbox" disabled={coreBusy} />
                  この作品で切断線の作成を許可する
                </label>
              </fieldset>

              <p className="dialog-note">
                左上を (0, 0) mm とする長方形の用紙と、4本の輪郭線を作成します。
              </p>
              {newProjectError && <p className="dialog-error" role="alert">{newProjectError}</p>}
              <footer>
                <button
                  type="button"
                  disabled={coreBusy}
                  onClick={() => {
                    setNewProjectOpen(false)
                    setNewProjectError(null)
                  }}
                >
                  キャンセル
                </button>
                <button type="submit" className="primary" disabled={coreBusy}>
                  {coreBusy ? '作成中…' : '作成'}
                </button>
              </footer>
            </form>
          </section>
        </div>
      )}

      {foldImportPreview && (
        <FoldImportDialog
          key={foldImportPreview.import_id}
          preview={foldImportPreview}
          busy={coreBusy}
          error={foldImportError}
          onCancel={() => void closeFoldImportDialog()}
          onImport={(settings) => void confirmFoldImport(settings)}
        />
      )}

      {svgImportPreview && (
        <SvgImportDialog
          key={svgImportPreview.import_id}
          preview={svgImportPreview}
          validation={svgImportValidation}
          busy={coreBusy}
          error={svgImportError}
          onInvalidateValidation={() => {
            setSvgImportValidation(null)
            setSvgImportError(null)
          }}
          onValidate={(settings) => void validateSvgImportDraft(settings)}
          onCancel={() => void closeSvgImportDialog()}
          onImport={(settings) => void confirmSvgImport(settings)}
        />
      )}

      {creaseExportOpen && (
        <CreaseExportDialog
          format={creaseExportFormat}
          preview={creaseExportPreview}
          busy={coreBusy}
          error={creaseExportError}
          notice={creaseExportNotice}
          onFormatChange={changeCreaseExportFormat}
          onRetry={() => void prepareCreaseExport(creaseExportFormat)}
          onSave={(warningsAcknowledged) => {
            void saveCurrentCreaseExport(warningsAcknowledged)
          }}
          onCancel={() => void closeCreaseExportDialog()}
        />
      )}

      {instructionExportOpen && (
        <InstructionExportDialog
          format={instructionExportFormat}
          preview={instructionExportPreview}
          busy={coreBusy}
          generationActive={instructionExportGenerationActive}
          phase={instructionExportPhase}
          error={instructionExportError}
          notice={instructionExportNotice}
          onFormatChange={changeInstructionExportFormat}
          onRetry={() => void prepareInstructionExport(instructionExportFormat)}
          onSave={(warningsAcknowledged) => {
            void saveCurrentInstructionExport(warningsAcknowledged)
          }}
          onCancel={() => void closeInstructionExportDialog()}
        />
      )}

      <DiagnosticsDialog
        open={diagnosticsDialogOpen}
        onClose={closeDiagnosticsDialog}
      />

      <footer className="statusbar" inert={modalOpen}>
        <span>ツール: {benchmarkRun ? '性能テスト選択' : toolLabel(activeTool)}</span>
        <span>{coreStatus}</span>
        <span>スナップ: {snapStatusLabel}</span>
        <span className="status-spacer" />
        <ThemeControl />
        {isDiagnosticsShareAvailable() && (
          <button
            ref={diagnosticsButtonRef}
            type="button"
            className="diagnostics-button"
            aria-haspopup="dialog"
            onClick={() => setDiagnosticsDialogOpen(true)}
          >
            診断情報
          </button>
        )}
        <button
          type="button"
          className="benchmark-button"
          disabled={coreBusy || benchmarkLoading}
          onClick={() => void toggleBenchmark()}
        >
          {benchmarkLoading ? '読込中…' : benchmarkRun ? '通常図へ戻る' : '10,000本テスト'}
        </button>
        <span className="benchmark-status" aria-live="polite" title={benchmarkStatus}>
          {benchmarkStatus}
        </span>
      </footer>
    </main>
  )
}

function lineKindLabel(kind: CreaseLine['kind']) {
  return {
    mountain: '山折り',
    valley: '谷折り',
    auxiliary: '補助線',
    boundary: '輪郭線',
    cut: '切断線',
  }[kind]
}

function normalizeFoldAngle(value: number) {
  if (!Number.isFinite(value)) return null
  return Math.min(180, Math.max(0, value))
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes < 0) return 'サイズ不明'
  if (bytes < 1_000) return `${bytes} B`
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(2)} MB`
}

function toolLabel(tool: string) {
  return {
    select: '選択',
    vertex: '頂点',
    mountain: '山折り',
    valley: '谷折り',
    auxiliary: '補助線',
    cut: '切断',
    measure: '計測',
  }[tool]
}

function validationIssueLabel(code: string) {
  return {
    non_finite_vertex: '有限でない頂点座標',
    duplicate_vertex: '同じ位置の重複頂点',
    missing_endpoint: '存在しない端点を参照する線',
    zero_length_edge: '長さ0の線',
    unsplit_intersection: '分割されていない交差・重なり',
    intersection_calculation_failed: '交差計算に失敗',
    non_finite_thickness: '紙の厚さが有限値ではありません',
    negative_thickness: '紙の厚さは0 mm以上にする必要があります',
    too_few_boundary_vertices: '紙の輪郭には3つ以上の頂点が必要です',
    duplicate_boundary_vertex: '紙の輪郭に同じ頂点が重複しています',
    missing_boundary_vertex: '紙の輪郭が存在しない頂点を参照しています',
    non_finite_boundary_vertex: '紙の輪郭頂点の座標が有限値ではありません',
    missing_boundary_edge: '紙の輪郭線が不足しています',
    duplicate_boundary_edge: '紙の輪郭線が重複しています',
    unexpected_boundary_edge: '紙の輪郭に余分な輪郭線があります',
    zero_length_boundary_edge: '紙の輪郭に長さ0の辺があります',
    boundary_self_intersection: '紙の輪郭が自己交差しています',
    boundary_intersection_calculation_failed: '紙の輪郭の交差判定に失敗しました',
    zero_area_boundary: '紙の輪郭の面積が0です',
    boundary_area_calculation_failed: '紙の輪郭の面積計算に失敗しました',
  }[code] ?? code
}

function localFlatFoldabilityCoreStatus(
  presentation: LocalFlatFoldabilityPresentation,
) {
  if (presentation.kind === 'invalid') return '局所判定結果を確認不能'
  if (presentation.kind === 'blocked') return '局所判定を前段の幾何問題で遮断'
  if (presentation.reportStatus === 'necessary_conditions_satisfied') {
    return `局所必要条件が${presentation.counts.satisfied}頂点で成立`
  }
  if (presentation.reportStatus === 'not_applicable') return '局所判定の対象頂点なし'
  if (presentation.reportStatus === 'violated') {
    return `局所必要条件に不成立${presentation.counts.violated}頂点`
  }
  return `局所判定不能${presentation.counts.indeterminate}頂点`
}

function reportValidationUnexpected() {
  reportUnexpected('app.validation')
}

function resolvePaperBounds(snapshot: ProjectSnapshot | null): PaperBounds | undefined {
  if (!snapshot) return undefined
  const positions = new Map(
    snapshot.crease_pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
  )
  const points = snapshot.paper.boundary_vertices.flatMap((id) => {
    const point = positions.get(id)
    return point ? [point] : []
  })
  if (points.length < 2) return undefined

  const bounds = points.reduce<PaperBounds>((current, point) => ({
    minX: Math.min(current.minX, point.x),
    minY: Math.min(current.minY, point.y),
    maxX: Math.max(current.maxX, point.x),
    maxY: Math.max(current.maxY, point.y),
  }), {
    minX: Number.POSITIVE_INFINITY,
    minY: Number.POSITIVE_INFINITY,
    maxX: Number.NEGATIVE_INFINITY,
    maxY: Number.NEGATIVE_INFINITY,
  })
  if (
    !Object.values(bounds).every(Number.isFinite) ||
    bounds.maxX <= bounds.minX ||
    bounds.maxY <= bounds.minY
  ) return undefined
  return bounds
}

function resolvePaperPolygon(snapshot: ProjectSnapshot | null): PaperPolygonPoint[] {
  if (!snapshot) return []
  const positions = new Map(
    snapshot.crease_pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
  )
  const points: PaperPolygonPoint[] = []
  for (const id of snapshot.paper.boundary_vertices) {
    const position = positions.get(id)
    if (!position) return []
    points.push({ id, x: position.x, y: position.y })
  }
  return points
}

type RectangularPaperSize = {
  width: number
  height: number
}

function resolveRectangularPaperSize(
  snapshot: ProjectSnapshot | null,
): RectangularPaperSize | null {
  if (!snapshot) return null
  const boundaryIds = snapshot.paper.boundary_vertices
  if (boundaryIds.length !== 4 || new Set(boundaryIds).size !== 4) return null

  const positions = new Map(
    snapshot.crease_pattern.vertices.map((vertex) => [vertex.id, vertex.position]),
  )
  const points: Array<{ x: number; y: number }> = []
  for (const id of boundaryIds) {
    const point = positions.get(id)
    if (!point || !Number.isFinite(point.x) || !Number.isFinite(point.y)) return null
    points.push(point)
  }

  const minX = Math.min(...points.map((point) => point.x))
  const minY = Math.min(...points.map((point) => point.y))
  const maxX = Math.max(...points.map((point) => point.x))
  const maxY = Math.max(...points.map((point) => point.y))
  const width = maxX - minX
  const height = maxY - minY
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    return null
  }

  const corners = new Set<string>()
  for (const point of points) {
    const horizontalSide = point.x === minX ? 'left' : point.x === maxX ? 'right' : null
    const verticalSide = point.y === minY ? 'top' : point.y === maxY ? 'bottom' : null
    if (!horizontalSide || !verticalSide) return null
    corners.add(`${horizontalSide}:${verticalSide}`)
  }
  if (corners.size !== 4) return null

  for (let index = 0; index < points.length; index += 1) {
    const current = points[index]
    const next = points[(index + 1) % points.length]
    const sharesX = current.x === next.x
    const sharesY = current.y === next.y
    if (sharesX === sharesY) return null
  }

  return { width, height }
}

type LineMeasurement = {
  deltaX: number
  deltaY: number
  length: number
  angleDegrees: number
}

function resolveUniqueParallelReference(
  lines: readonly CreaseLine[],
  referenceEdgeId: string | null,
) {
  if (!referenceEdgeId) return null
  let reference: CreaseLine | null = null
  for (const line of lines) {
    if (line.id !== referenceEdgeId) continue
    if (reference) return null
    reference = line
  }
  if (
    !reference
    || ![reference.x1, reference.y1, reference.x2, reference.y2].every(Number.isFinite)
    || (reference.x1 === reference.x2 && reference.y1 === reference.y2)
  ) return null
  return reference
}

function measureCreaseLine(
  line: Pick<CreaseLine, 'x1' | 'y1' | 'x2' | 'y2'>,
): LineMeasurement | null {
  if (![line.x1, line.y1, line.x2, line.y2].every(Number.isFinite)) return null
  const rawDeltaX = line.x2 - line.x1
  const rawDeltaY = line.y2 - line.y1
  if (!Number.isFinite(rawDeltaX) || !Number.isFinite(rawDeltaY)) return null
  const deltaX = Object.is(rawDeltaX, -0) ? 0 : rawDeltaX
  const deltaY = Object.is(rawDeltaY, -0) ? 0 : rawDeltaY
  const length = Math.hypot(deltaX, deltaY)
  if (!Number.isFinite(length) || length <= 0) return null
  const rawAngle = Math.atan2(deltaY, deltaX) * 180 / Math.PI
  if (!Number.isFinite(rawAngle)) return null
  const angleDegrees = Object.is(rawAngle, -0) ? 0 : rawAngle
  return { deltaX, deltaY, length, angleDegrees }
}

function formatMeasurementValue(
  value: number | null | undefined,
  unit: string,
  maximumFractionDigits = 3,
) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '計測不可'
  const normalized = Object.is(value, -0) ? 0 : value
  return `${normalized.toLocaleString('ja-JP', { maximumFractionDigits })}${unit}`
}

function formatAngleDegrees(value: number) {
  if (!Number.isFinite(value)) return '—'
  if (value !== 0 && Math.abs(value) < 0.000001) return value.toExponential(3)
  return String(Number(value.toFixed(6)))
}

function formatLineMeasurementLabel(
  measurement: LineMeasurement | null,
  unit: ReturnType<typeof resolveLengthDisplayUnit>,
) {
  if (!measurement) return '計測不可'
  return `${formatLength(measurement.length, unit)} / ${formatMeasurementValue(measurement.angleDegrees, '°', 2)}`
}

function rgbaToCss(color: RgbaColor | undefined) {
  if (!color) return '#fffdf9'
  return `rgba(${color.red}, ${color.green}, ${color.blue}, ${color.alpha / 255})`
}

function rgbaToHex(color: RgbaColor | undefined, fallback = '#fffdf9') {
  if (!color) return fallback
  const channels = [color.red, color.green, color.blue]
  if (!channels.every(Number.isFinite)) return fallback
  const toHex = (channel: number) => Math.round(Math.min(255, Math.max(0, channel)))
    .toString(16)
    .padStart(2, '0')
  return `#${toHex(color.red)}${toHex(color.green)}${toHex(color.blue)}`
}

function parseHexColor(value: string): RgbaColor | null {
  if (!/^#[0-9a-f]{6}$/iu.test(value)) return null
  return {
    red: Number.parseInt(value.slice(1, 3), 16),
    green: Number.parseInt(value.slice(3, 5), 16),
    blue: Number.parseInt(value.slice(5, 7), 16),
    alpha: 255,
  }
}

function hasControlCharacter(value: string) {
  return [...value].some((character) => {
    const codePoint = character.codePointAt(0) ?? 0
    return codePoint <= 31 || (codePoint >= 127 && codePoint <= 159)
  })
}

function isEditingText(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.matches('input, textarea')) return true
  return target.isContentEditable || Boolean(target.closest('[contenteditable="true"]'))
}

export default App
