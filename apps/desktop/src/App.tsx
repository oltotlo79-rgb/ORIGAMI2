import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  type CSSProperties,
  type FormEvent,
  useCallback,
  useEffect,
  useEffectEvent,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react'
import {
  CreaseCanvas,
  type CreaseCanvasRenderMetrics,
  type CreaseLine,
  type PaperBounds,
  type PaperPolygonPoint,
} from './components/CreaseCanvas'
import { CreaseExportDialog } from './components/CreaseExportDialog'
import { CreationDimensionExpressionSummary } from './components/CreationDimensionExpressionSummary'
import { DiagnosticsDialog } from './components/DiagnosticsDialog'
import { FoldImportDialog } from './components/FoldImportDialog'
import { FoldPreview } from './components/FoldPreview'
import { GeometricConstraintPanel } from './components/GeometricConstraintPanel'
import { GlobalFlatFoldabilityPanel } from './components/GlobalFlatFoldabilityPanel'
import { HistoryLimitControl } from './components/HistoryLimitControl'
import { InstructionExportDialog } from './components/InstructionExportDialog'
import { InstructionTimelinePanel } from './components/InstructionTimelinePanel'
import { KeyboardShortcutControl } from './components/KeyboardShortcutControl'
import { LanguageControl } from './components/LanguageControl'
import { LengthUnitControl } from './components/LengthUnitControl'
import { LengthValueInput } from './components/LengthValueInput'
import { NumericExpressionInput } from './components/NumericExpressionInput'
import { ProjectLayerPanel } from './components/ProjectLayerPanel'
import { RecoveryAutosaveStatusBanner } from './components/RecoveryAutosaveStatusBanner'
import { RecoveryDialog } from './components/RecoveryDialog'
import { RecoveryStartupOverlay } from './components/RecoveryStartupOverlay'
import { SvgImportDialog } from './components/SvgImportDialog'
import { ThemeControl } from './components/ThemeControl'
import { WorkspaceLayoutControl } from './components/WorkspaceLayoutControl'
import { WorkspaceLayoutSeparator } from './components/WorkspaceLayoutSeparator'
import {
  addEdge,
  addEdgeOrientationConstraint,
  addVertex,
  analyzeGeometricConstraints,
  analyzeProjectTopology,
  applyFoldImport,
  applySvgImport,
  assignEdgeToProjectLayer,
  beginInstructionExportGeneration,
  cancelCreasePatternExport,
  cancelFoldImport,
  cancelInstructionExport,
  cancelSvgImport,
  connectEdgeIntersection,
  connectIntersectionCluster,
  connectTJunction,
  createProjectLayer,
  deleteProjectLayer,
  generateBenchmarkPattern,
  getInstructionExportProgress,
  getProjectSnapshot as requestProjectSnapshot,
  isNativeCoreAvailable,
  moveProjectLayer,
  moveVertex,
  newProject,
  openProject,
  previewCreasePatternExport,
  previewFoldImport,
  previewInstructionExport,
  previewSvgImport,
  redo,
  renameProjectLayer,
  removeBoundaryVertex,
  removeEdge,
  removeGeometricConstraint,
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
import { normalizeGeometricConstraintDocument } from './lib/geometricConstraints'
import {
  DEFAULT_PROJECT_LAYER_DOCUMENT_V1,
  normalizeProjectLayerDocument,
  type LayerContentKindV1,
} from './lib/projectLayers'
import { buildFoldPreviewModel } from './lib/foldPreviewModel'
import { isExpectedNativeEditSnapshot } from './lib/projectSnapshotBinding'
import {
  cancelWindowClosePrepare,
  createWindowCloseHandshake,
  createWindowCloseHandshakeState,
  discardRecoveryCandidate,
  getRecoveryCandidate,
  prepareWindowClose,
  restoreRecoveryCandidate,
  WINDOW_CLOSE_STATUS,
  type RecoveryCandidateAvailable,
  type RecoveryCandidateInvalid,
} from './lib/recoveryClient'
import {
  createRecoveryAutosaveStatusPoller,
  type RecoveryAutosaveMonitorView,
} from './lib/recoveryAutosaveStatusClient'
import {
  historyLimitClient,
  type HistoryLimitSettings,
} from './lib/historyLimitClient'
import { useGeometricConstraintPreflight } from './lib/useGeometricConstraintPreflight'
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
  lengthDisplayUnitLabel,
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
import {
  keyboardShortcutAriaValue,
  keyboardShortcutDisplayValue,
  keyboardShortcutStore,
  resolveConfiguredKeyboardShortcut,
} from './lib/keyboardShortcutSettings'
import { workspaceLayoutStore } from './lib/workspaceLayout'
import {
  evaluatePositiveMillimetreExpression,
  numericExpressionNativeErrorCategory,
} from './lib/numericExpressionNative'
import {
  formatLocalizedText,
  selectLocalizedText,
  useLocale,
  type Locale,
  type LocalizedText,
  type MessageVariables,
} from './lib/i18n'
import {
  appConfirmationText,
  appErrorLocalizedText,
} from './lib/appMessages'
import './App.css'

const SNAP_OPTIONS: ReadonlyArray<{
  kind: keyof SnapSettings
  label: LocalizedText
}> = [
  { kind: 'grid', label: { ja: 'グリッド', en: 'Grid' } },
  { kind: 'vertex', label: { ja: '頂点', en: 'Vertex' } },
  { kind: 'intersection', label: { ja: '交点', en: 'Intersection' } },
  { kind: 'edge', label: { ja: '辺', en: 'Edge' } },
  { kind: 'midpoint', label: { ja: '中点', en: 'Midpoint' } },
  { kind: 'horizontal', label: { ja: '水平', en: 'Horizontal' } },
  { kind: 'vertical', label: { ja: '垂直', en: 'Vertical' } },
  { kind: 'parallel', label: { ja: '平行', en: 'Parallel' } },
  { kind: 'angle', label: { ja: '角度', en: 'Angle' } },
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

type RecoveryStartupState =
  | Readonly<{ kind: 'ready' }>
  | Readonly<{ kind: 'checking' }>
  | Readonly<{ kind: 'failed' }>
  | Readonly<{
      kind: 'candidate'
      candidate: RecoveryCandidateAvailable | RecoveryCandidateInvalid
    }>

type HistoryLimitLoadState =
  | Readonly<{ kind: 'unavailable' }>
  | Readonly<{ kind: 'loading' }>
  | Readonly<{ kind: 'failed' }>
  | Readonly<{ kind: 'ready'; settings: HistoryLimitSettings }>

type WorkspaceLayoutStyle = CSSProperties & {
  '--workspace-editor-two-d-share': string
  '--workspace-editor-three-d-share': string
  '--workspace-inspector-width': string
  '--workspace-timeline-height': string
}

type AppMessage = Readonly<{
  text: LocalizedText
  variables?: MessageVariables
}>

function appMessage(
  text: LocalizedText,
  variables?: MessageVariables,
): AppMessage {
  return Object.freeze({ text, variables })
}

function appMessageWithLocalizedVariables(
  text: LocalizedText,
  variables: (locale: Locale) => MessageVariables,
): AppMessage {
  return appMessage({
    ja: formatLocalizedText('ja', text, variables('ja')),
    en: formatLocalizedText('en', text, variables('en')),
  })
}

function instructionExportErrorAppMessage(
  error: unknown,
  text: LocalizedText,
): AppMessage {
  return appMessageWithLocalizedVariables(text, (locale) => ({
    error: instructionExportErrorMessage(error, locale),
  }))
}

function appMessageText(
  locale: Locale,
  message: AppMessage | null,
): string | null {
  if (!message) return null
  return formatLocalizedText(locale, message.text, message.variables)
}

function windowCloseAppMessage(message: string): AppMessage {
  const translated = new Map<string, LocalizedText>([
    [WINDOW_CLOSE_STATUS.recoveryBlocked, {
      ja: WINDOW_CLOSE_STATUS.recoveryBlocked,
      en: 'Finish reviewing the recovery data before quitting.',
    }],
    [WINDOW_CLOSE_STATUS.coreBlocked, {
      ja: WINDOW_CLOSE_STATUS.coreBlocked,
      en: 'Wait for the current operation to finish before quitting.',
    }],
    [WINDOW_CLOSE_STATUS.cancelled, {
      ja: WINDOW_CLOSE_STATUS.cancelled,
      en: 'Quit was cancelled. You can continue editing.',
    }],
    [WINDOW_CLOSE_STATUS.preparing, {
      ja: WINDOW_CLOSE_STATUS.preparing,
      en: 'Safely organizing recovery data before quitting…',
    }],
    [WINDOW_CLOSE_STATUS.stale, {
      ja: WINDOW_CLOSE_STATUS.stale,
      en: 'The project changed while preparing to quit. Please quit again.',
    }],
    [WINDOW_CLOSE_STATUS.failed, {
      ja: WINDOW_CLOSE_STATUS.failed,
      en: 'Quit preparation could not finish. Keep the app open and try again.',
    }],
  ])
  return appMessage(
    translated.get(message)
      ?? appErrorLocalizedText('window_close_status_invalid'),
  )
}

function App() {
  const locale = useLocale()
  const text = (localized: LocalizedText) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: LocalizedText,
    variables?: MessageVariables,
  ) => formatLocalizedText(locale, localized, variables)
  const keyboardShortcuts = useSyncExternalStore(
    keyboardShortcutStore.subscribe,
    keyboardShortcutStore.getSnapshot,
    keyboardShortcutStore.getServerSnapshot,
  )
  const workspaceLayout = useSyncExternalStore(
    workspaceLayoutStore.subscribe,
    workspaceLayoutStore.getSnapshot,
    workspaceLayoutStore.getServerSnapshot,
  )
  const workspaceLayoutStyle: WorkspaceLayoutStyle = {
    '--workspace-editor-two-d-share':
      `${workspaceLayout.editorTwoDPercent}fr`,
    '--workspace-editor-three-d-share':
      `${100 - workspaceLayout.editorTwoDPercent}fr`,
    '--workspace-inspector-width': `${workspaceLayout.inspectorWidthPx}px`,
    '--workspace-timeline-height': `${workspaceLayout.timelineHeightPx}px`,
  }
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
  const [benchmarkStatusMessage, setBenchmarkStatus] = useState<AppMessage>(
    () => appMessage({ ja: '未実行', en: 'Not run' }),
  )
  const [benchmarkRun, setBenchmarkRun] = useState<BenchmarkRun | null>(null)
  const [benchmarkLoading, setBenchmarkLoading] = useState(false)
  const [nativeSnapshot, setNativeSnapshot] = useState<ProjectSnapshot | null>(null)
  const [recoveryStartup, setRecoveryStartup] = useState<RecoveryStartupState>(
    () => isNativeCoreAvailable()
      ? { kind: 'checking' }
      : { kind: 'ready' },
  )
  const [recoveryAutosaveMonitor, setRecoveryAutosaveMonitor] =
    useState<RecoveryAutosaveMonitorView>(() => (
      isNativeCoreAvailable()
        ? { kind: 'checking' }
        : { kind: 'inactive' }
    ))
  const [recoveryActionBusy, setRecoveryActionBusy] = useState(false)
  const [recoveryActionError, setRecoveryActionError] = useState(false)
  const [historyLimitLoadState, setHistoryLimitLoadState] =
    useState<HistoryLimitLoadState>(() => (
      isNativeCoreAvailable()
        ? { kind: 'loading' }
        : { kind: 'unavailable' }
    ))
  const [historyLimitRetrySequence, setHistoryLimitRetrySequence] = useState(0)
  const [geometricConstraintDocumentInvalid, setGeometricConstraintDocumentInvalid] =
    useState(false)
  const [projectLayerDocumentInvalid, setProjectLayerDocumentInvalid] =
    useState(false)
  const [topologyResponse, setTopologyResponse] = useState<ProjectTopologyResponse | null>(null)
  const [topologyStatusMessage, setTopologyStatus] = useState<AppMessage>(
    () => isNativeCoreAvailable()
      ? appMessage({
          ja: '面・ヒンジ解析待ち',
          en: 'Waiting for face and hinge analysis',
        })
      : appMessage({
          ja: '3D解析はデスクトップ版で利用できます',
          en: '3D analysis is available in the desktop app',
        }),
  )
  const [validation, setValidation] = useState<ValidationSnapshot | null>(null)
  const [globalFlatFoldabilityJob, setGlobalFlatFoldabilityJob] =
    useState<GlobalFlatFoldabilityJobDto | null>(null)
  const [globalFlatFoldabilityTimeLimit, setGlobalFlatFoldabilityTimeLimit] =
    useState<GlobalFlatFoldabilityTimePreset>(
      DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET,
    )
  const [coreStatusMessage, setCoreStatus] = useState<AppMessage>(
    () => isNativeCoreAvailable()
      ? appMessage({ ja: 'コア接続中…', en: 'Connecting to core…' })
      : appMessage({ ja: 'ブラウザ試作モード', en: 'Browser prototype mode' }),
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
  const [newProjectErrorMessage, setNewProjectError] =
    useState<AppMessage | null>(null)
  const [diagnosticsDialogOpen, setDiagnosticsDialogOpen] = useState(false)
  const [foldImportPreview, setFoldImportPreview] = useState<FoldImportPreview | null>(null)
  const [foldImportErrorMessage, setFoldImportError] =
    useState<AppMessage | null>(null)
  const [svgImportPreview, setSvgImportPreview] = useState<SvgImportPreview | null>(null)
  const [svgImportErrorMessage, setSvgImportError] =
    useState<AppMessage | null>(null)
  const [svgImportValidation, setSvgImportValidation] =
    useState<SvgImportSettingsValidation | null>(null)
  const [creaseExportOpen, setCreaseExportOpen] = useState(false)
  const [creaseExportFormat, setCreaseExportFormat] =
    useState<CreasePatternExportFormat>('fold')
  const [creaseExportPreview, setCreaseExportPreview] =
    useState<CreasePatternExportPreview | null>(null)
  const [creaseExportErrorMessage, setCreaseExportError] =
    useState<AppMessage | null>(null)
  const [creaseExportNoticeMessage, setCreaseExportNotice] =
    useState<AppMessage | null>(null)
  const [instructionExportOpen, setInstructionExportOpen] = useState(false)
  const [instructionExportFormat, setInstructionExportFormat] =
    useState<InstructionExportFormat>('pdf')
  const [instructionExportPreview, setInstructionExportPreview] =
    useState<InstructionExportPreview | null>(null)
  const [instructionExportGenerationActive, setInstructionExportGenerationActive] =
    useState(false)
  const [instructionExportPhase, setInstructionExportPhase] =
    useState<InstructionExportPhase>('validating')
  const [instructionExportErrorState, setInstructionExportError] =
    useState<AppMessage | null>(null)
  const [instructionExportNoticeMessage, setInstructionExportNotice] =
    useState<AppMessage | null>(null)
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
  const benchmarkStatus = appMessageText(
    locale,
    benchmarkStatusMessage,
  ) ?? ''
  const topologyStatus = appMessageText(locale, topologyStatusMessage) ?? ''
  const coreStatus = appMessageText(locale, coreStatusMessage) ?? ''
  const newProjectError = appMessageText(locale, newProjectErrorMessage)
  const foldImportError = appMessageText(locale, foldImportErrorMessage)
  const svgImportError = appMessageText(locale, svgImportErrorMessage)
  const creaseExportError = appMessageText(locale, creaseExportErrorMessage)
  const creaseExportNotice = appMessageText(locale, creaseExportNoticeMessage)
  const instructionExportError = appMessageText(
    locale,
    instructionExportErrorState,
  )
  const instructionExportNotice = appMessageText(
    locale,
    instructionExportNoticeMessage,
  )
  const recoveryBlocking = recoveryStartup.kind !== 'ready'
  const coreOperationRef = useRef(false)
  const latestSnapshotRef = useRef<ProjectSnapshot | null>(null)
  const initialProjectSnapshotRequestRef =
    useRef<Promise<ProjectSnapshot> | null>(null)
  const recoveryMountedRef = useRef(true)
  const recoveryStartupStartedRef = useRef(false)
  const recoveryRequestSequenceRef = useRef(0)
  const recoveryOperationRef = useRef(false)
  const windowCloseHandshakeStateRef =
    useRef(createWindowCloseHandshakeState())
  const historyLimitRequestSequenceRef = useRef(0)
  const recoveryStartupRef = useRef<RecoveryStartupState>(recoveryStartup)
  const recoveryBlockingRef = useRef(recoveryBlocking)
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
  recoveryStartupRef.current = recoveryStartup
  recoveryBlockingRef.current = recoveryBlocking
  const getProjectSnapshot = useCallback(() => {
    const pending = initialProjectSnapshotRequestRef.current
    if (pending) return pending
    const request = Promise.resolve().then(() => requestProjectSnapshot())
    initialProjectSnapshotRequestRef.current = request
    return request
  }, [])
  const analyzeCurrentGeometricConstraints = useCallback(async (
    expectedProjectInstanceId: string,
    expectedProjectId: string,
    expectedRevision: number,
  ) => {
    const response = await analyzeGeometricConstraints(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
    )
    const current = latestSnapshotRef.current
    if (
      !current
      || current.project_instance_id !== response.project_instance_id
      || current.project_id !== response.project_id
      || current.revision !== response.revision
    ) {
      throw new Error('stale geometric-constraint preflight response')
    }
    return response
  }, [])
  const reportGeometricConstraintAnalysisFailure = useCallback(() => {
    reportUnexpected('app.validation')
  }, [])
  const {
    preflight: geometricConstraintPreflight,
    analyzing: geometricConstraintAnalysisBusy,
    failed: geometricConstraintAnalysisFailed,
    retry: retryGeometricConstraintAnalysis,
  } = useGeometricConstraintPreflight({
    snapshot: nativeSnapshot,
    enabled: isNativeCoreAvailable() && !geometricConstraintDocumentInvalid,
    analyze: analyzeCurrentGeometricConstraints,
    onFailure: reportGeometricConstraintAnalysisFailure,
  })
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
    || recoveryBlocking
  const closeDiagnosticsDialog = useCallback(() => {
    setDiagnosticsDialogOpen(false)
    requestAnimationFrame(() => diagnosticsButtonRef.current?.focus())
  }, [])
  const applySnapshot = useCallback((
    snapshot: ProjectSnapshot,
    forceReplacement = false,
  ) => {
    const rawConstraints = snapshot.geometric_constraints === undefined
      ? { schema_version: 1, constraints: [] }
      : snapshot.geometric_constraints
    const geometricConstraints = normalizeGeometricConstraintDocument(rawConstraints)
    const constraintDocumentInvalid = geometricConstraints === null
    const projectLayers = normalizeProjectLayerDocument(
      snapshot.project_layers,
      snapshot.crease_pattern.edges,
    )
    const layerDocumentInvalid = projectLayers === null
    if (constraintDocumentInvalid || layerDocumentInvalid) {
      reportUnexpected('app.validation')
    }
    const admittedSnapshot: ProjectSnapshot = {
      ...snapshot,
      geometric_constraints: geometricConstraints ?? {
        schema_version: 1,
        constraints: [],
      },
      project_layers:
        projectLayers ?? DEFAULT_PROJECT_LAYER_DOCUMENT_V1,
    }
    topologyRequestIdRef.current += 1
    latestSnapshotRef.current = admittedSnapshot
    globalFlatFoldabilityCoordinatorRef.current?.invalidate({
      projectId: admittedSnapshot.project_id,
      revision: admittedSnapshot.revision,
      foldModelFingerprint: admittedSnapshot.fold_model_fingerprint,
    }, forceReplacement)
    setNativeSnapshot(admittedSnapshot)
    setGeometricConstraintDocumentInvalid(constraintDocumentInvalid)
    setProjectLayerDocumentInvalid(layerDocumentInvalid)
    setValidation(null)
    setTopologyResponse(null)
    setTopologyStatus(appMessage({
      ja: '面・ヒンジ解析待ち',
      en: 'Waiting for face and hinge analysis',
    }))
  }, [])
  const acceptAppliedHistoryLimit = useCallback(async (
    settings: HistoryLimitSettings,
  ) => {
    const current = latestSnapshotRef.current
    if (
      !current
      || current.project_instance_id !== settings.projectInstanceId
      || current.project_id !== settings.projectId
      || current.revision !== settings.revision
    ) return

    const refreshed = await requestProjectSnapshot()
    const latest = latestSnapshotRef.current
    if (
      latest !== current
      || refreshed.project_instance_id !== settings.projectInstanceId
      || refreshed.project_id !== settings.projectId
      || refreshed.revision !== settings.revision
    ) return

    applySnapshot(refreshed)
    setHistoryLimitLoadState({ kind: 'ready', settings })
    setCoreStatus(appMessage({
      ja: 'Undo・Redo履歴の上限を{limit}件に変更しました。',
      en: 'Undo/redo history limit changed to {limit}.',
    }, { limit: settings.historyEntryLimit }))
  }, [applySnapshot])
  const resetRecoveredProjectUi = useCallback(() => {
    benchmarkRequestIdRef.current += 1
    setBenchmarkLoading(false)
    setBenchmarkRun(null)
    setBenchmarkStatus(appMessage({
      ja: '復元した編集内容を表示しています',
      en: 'Showing restored edits',
    }))
    setSelectedLineId(null)
    setSelectedVertexId(null)
    setPendingEdgeStart(null)
    setParallelReferenceEdgeId(null)
    setAppliedFoldPose(null)
    setFoldAngleOverrides({ projectId: null, values: new Map() })
    setFixedFaceChoice({ projectId: null, faceId: null })
    setActiveTool('select')
    setCancelInteractionToken((token) => token + 1)
  }, [])
  const checkRecoveryStartup = useCallback(async (
    refreshSnapshot: boolean,
  ) => {
    if (!isNativeCoreAvailable() || recoveryOperationRef.current) return
    recoveryOperationRef.current = true
    if (refreshSnapshot) initialProjectSnapshotRequestRef.current = null
    const requestId = ++recoveryRequestSequenceRef.current
    setRecoveryActionBusy(true)
    setRecoveryActionError(false)
    setRecoveryStartup({ kind: 'checking' })
    setCoreStatus(appMessage({
      ja: '復旧データを確認しています…',
      en: 'Checking recovery data…',
    }))
    try {
      const [snapshot, candidate] = await Promise.all([
        getProjectSnapshot(),
        getRecoveryCandidate(),
      ])
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
      ) return
      applySnapshot(snapshot)
      if (candidate.status === 'none') {
        setRecoveryStartup({ kind: 'ready' })
        setCoreStatus(appMessage({
          ja: 'Rustコア revision {revision}',
          en: 'Rust core revision {revision}',
        }, { revision: snapshot.revision }))
      } else {
        setRecoveryStartup({ kind: 'candidate', candidate })
        setCoreStatus(appMessage({
          ja: '未保存の復旧データについて判断してください。',
          en: 'Choose how to handle the unsaved recovery data.',
        }))
      }
    } catch {
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
      ) return
      reportUnexpected('app.project_snapshot')
      setRecoveryStartup({ kind: 'failed' })
      setCoreStatus(appMessage({
        ja: '復旧データを確認できませんでした。再試行してください。',
        en: 'Recovery data could not be checked. Please try again.',
      }))
    } finally {
      if (
        recoveryMountedRef.current
        && requestId === recoveryRequestSequenceRef.current
      ) {
        recoveryOperationRef.current = false
        setRecoveryActionBusy(false)
      }
    }
  }, [applySnapshot, getProjectSnapshot])
  const restoreStartupRecovery = useCallback(async (
    candidate: RecoveryCandidateAvailable,
  ) => {
    const state = recoveryStartupRef.current
    const current = latestSnapshotRef.current
    if (
      recoveryOperationRef.current
      || !current
      || !sameRecoveryCandidate(state, candidate)
    ) return
    recoveryOperationRef.current = true
    const requestId = ++recoveryRequestSequenceRef.current
    setRecoveryActionBusy(true)
    setRecoveryActionError(false)
    setCancelInteractionToken((token) => token + 1)
    try {
      const recoveredSnapshot = await restoreRecoveryCandidate(candidate, {
        project_instance_id: current.project_instance_id,
        project_id: current.project_id,
        revision: current.revision,
      })
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
        || latestSnapshotRef.current !== current
        || !sameRecoveryCandidate(recoveryStartupRef.current, candidate)
      ) return
      applySnapshot(recoveredSnapshot, true)
      resetRecoveredProjectUi()
      setRecoveryStartup({ kind: 'ready' })
      setCoreStatus(appMessage({
        ja: '未保存の編集内容を復元しました。保存先を選んで保存してください。',
        en: 'Unsaved edits were restored. Choose a location and save them.',
      }))
    } catch {
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
        || !sameRecoveryCandidate(recoveryStartupRef.current, candidate)
      ) return
      setRecoveryActionError(true)
      setCoreStatus(appMessage({
        ja: '復旧データを復元できませんでした。もう一度お試しください。',
        en: 'Recovery data could not be restored. Please try again.',
      }))
    } finally {
      if (
        recoveryMountedRef.current
        && requestId === recoveryRequestSequenceRef.current
      ) {
        recoveryOperationRef.current = false
        setRecoveryActionBusy(false)
      }
    }
  }, [applySnapshot, resetRecoveredProjectUi])
  const discardStartupRecovery = useCallback(async (
    candidate: RecoveryCandidateAvailable | RecoveryCandidateInvalid,
  ) => {
    if (
      recoveryOperationRef.current
      || !sameRecoveryCandidate(recoveryStartupRef.current, candidate)
    ) return
    recoveryOperationRef.current = true
    const requestId = ++recoveryRequestSequenceRef.current
    setRecoveryActionBusy(true)
    setRecoveryActionError(false)
    try {
      await discardRecoveryCandidate(candidate)
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
        || !sameRecoveryCandidate(recoveryStartupRef.current, candidate)
      ) return
      setRecoveryStartup({ kind: 'ready' })
      setCoreStatus(appMessage({
        ja: '復旧データを破棄しました。',
        en: 'Recovery data was discarded.',
      }))
    } catch {
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
        || !sameRecoveryCandidate(recoveryStartupRef.current, candidate)
      ) return
      setRecoveryActionError(true)
      setCoreStatus(appMessage({
        ja: '復旧データを破棄できませんでした。もう一度お試しください。',
        en: 'Recovery data could not be discarded. Please try again.',
      }))
    } finally {
      if (
        recoveryMountedRef.current
        && requestId === recoveryRequestSequenceRef.current
      ) {
        recoveryOperationRef.current = false
        setRecoveryActionBusy(false)
      }
    }
  }, [])
  const retryRecoveryStartup = useCallback(() => {
    return checkRecoveryStartup(true)
  }, [checkRecoveryStartup])
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
  const creationDimensionExpression =
    nativeSnapshot?.numeric_expressions?.rectangular_paper_creation
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
    ? formattedText({
        ja: '面 {index}',
        en: 'Face {index}',
      }, { index: effectiveFixedFaceIndex + 1 })
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
    ? text({
        ja: '3D入力の整合性検証で遮断',
        en: 'Blocked by 3D input consistency validation',
      })
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
  const lengthDisplayUnitLabelText = lengthDisplayUnitLabel(
    lengthDisplayUnit,
    locale,
  )
  const paperSizeLabel = paperBounds
    ? `${formatLengthValue(
        paperBounds.maxX - paperBounds.minX,
        lengthDisplayUnit,
        locale,
      )} × ${formatLength(
        paperBounds.maxY - paperBounds.minY,
        lengthDisplayUnit,
        locale,
      )}`
    : text({ ja: '寸法不明', en: 'Unknown dimensions' })
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
  const boundHistoryLimitSettings =
    historyLimitLoadState.kind === 'ready'
    && nativeSnapshot
    && historyLimitLoadState.settings.projectInstanceId
      === nativeSnapshot.project_instance_id
    && historyLimitLoadState.settings.projectId === nativeSnapshot.project_id
    && historyLimitLoadState.settings.revision === nativeSnapshot.revision
      ? historyLimitLoadState.settings
      : null
  const snapStatusLabel = SNAP_OPTIONS
    .filter(({ kind }) => snapSettings[kind])
    .map(({ label }) => text(label))
    .join(text({ ja: '・', en: ', ' }))
    || text({ ja: 'なし', en: 'None' })

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
    recoveryMountedRef.current = true
    return () => {
      recoveryMountedRef.current = false
    }
  }, [])

  useEffect(() => {
    if (!isNativeCoreAvailable()) return
    getProjectSnapshot()
    if (recoveryStartupStartedRef.current) return
    recoveryStartupStartedRef.current = true
    void checkRecoveryStartup(false)
  }, [checkRecoveryStartup, getProjectSnapshot])

  useEffect(() => {
    const nativeAvailable = isNativeCoreAvailable()
    if (!nativeAvailable || recoveryStartup.kind !== 'ready') return

    const poller = createRecoveryAutosaveStatusPoller({
      nativeAvailable,
      onChange: setRecoveryAutosaveMonitor,
    })
    const refreshWhenVisible = () => {
      if (document.visibilityState === 'visible') poller.refresh()
    }
    const refreshWhenFocused = () => poller.refresh()
    poller.start()
    document.addEventListener('visibilitychange', refreshWhenVisible)
    window.addEventListener('focus', refreshWhenFocused)

    return () => {
      document.removeEventListener('visibilitychange', refreshWhenVisible)
      window.removeEventListener('focus', refreshWhenFocused)
      poller.dispose()
    }
  }, [recoveryStartup.kind])

  useEffect(() => {
    if (!isNativeCoreAvailable()) {
      setHistoryLimitLoadState({ kind: 'unavailable' })
      return
    }
    if (!nativeSnapshot || recoveryBlocking) {
      setHistoryLimitLoadState({ kind: 'loading' })
      return
    }

    const expected = Object.freeze({
      expectedProjectInstanceId: nativeSnapshot.project_instance_id,
      expectedProjectId: nativeSnapshot.project_id,
      expectedRevision: nativeSnapshot.revision,
    })
    const requestId = ++historyLimitRequestSequenceRef.current
    let disposed = false
    setHistoryLimitLoadState({ kind: 'loading' })

    void historyLimitClient.get(expected).then((settings) => {
      const current = latestSnapshotRef.current
      if (
        disposed
        || requestId !== historyLimitRequestSequenceRef.current
        || !current
        || current.project_instance_id !== settings.projectInstanceId
        || current.project_id !== settings.projectId
        || current.revision !== settings.revision
      ) return
      setHistoryLimitLoadState({ kind: 'ready', settings })
    }).catch(() => {
      const current = latestSnapshotRef.current
      if (
        disposed
        || requestId !== historyLimitRequestSequenceRef.current
        || !current
        || current.project_instance_id !== expected.expectedProjectInstanceId
        || current.project_id !== expected.expectedProjectId
        || current.revision !== expected.expectedRevision
      ) return
      setHistoryLimitLoadState({ kind: 'failed' })
    })

    return () => {
      disposed = true
    }
  }, [
    historyLimitRetrySequence,
    nativeSnapshot,
    recoveryBlocking,
  ])

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
    setTopologyStatus(appMessage({
      ja: '面・ヒンジ解析中…',
      en: 'Analyzing faces and hinges…',
    }))

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
          setTopologyStatus(appMessage({
            ja: '{faces}面・{hinges}ヒンジ',
            en: '{faces} faces · {hinges} hinges',
          }, {
            faces: response.snapshot.faces.length,
            hinges: response.snapshot.hinge_adjacency.length,
          }))
        } else {
          setTopologyStatus(appMessage({
            ja: '3D解析で遮断（{count}件）',
            en: '3D analysis blocked ({count} issues)',
          }, { count: response.issues.length }))
        }
      })
      .catch(() => {
        if (disposed || requestId !== topologyRequestIdRef.current) return
        const current = latestSnapshotRef.current
        if (
          !current
          || current.project_id !== expectedProjectId
          || current.revision !== expectedRevision
        ) return
        reportUnexpected('app.topology_analysis')
        setTopologyResponse(null)
        setTopologyStatus(appMessage(
          appErrorLocalizedText('topology_analysis_failed'),
        ))
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
    const appWindow = getCurrentWindow()
    const reportCloseGuardFailure = () =>
      reportUnexpected('app.close_guard')
    const closeHandshake = createWindowCloseHandshake(
      windowCloseHandshakeStateRef.current,
      {
        getBlocker: () => {
          if (
            recoveryBlockingRef.current
            || recoveryOperationRef.current
          ) return 'recovery'
          return (
            coreOperationRef.current
            && !windowCloseHandshakeStateRef.current.interaction_locked
          )
            ? 'core'
            : null
        },
        getProjectState: () => {
          const current = latestSnapshotRef.current
          if (!current) return null
          return {
            project_instance_id: current.project_instance_id,
            project_id: current.project_id,
            revision: current.revision,
            is_dirty: current.is_dirty,
          }
        },
        confirmDiscard: () => window.confirm(
          appConfirmationText(locale, 'quitDiscard'),
        ),
        prepare: prepareWindowClose,
        cancel: cancelWindowClosePrepare,
        requestClose: () => appWindow.close(),
        setInteractionLocked: (locked) => {
          coreOperationRef.current = locked
          if (recoveryMountedRef.current) setCoreBusy(locked)
        },
        setStatus: (message) => {
          setCoreStatus(windowCloseAppMessage(message))
        },
        reportFailure: reportCloseGuardFailure,
      },
    )
    void appWindow.onCloseRequested((event) => {
      closeHandshake.handle(event)
    }).then((stopListening) => {
      if (disposed) stopListening()
      else unlisten = stopListening
    }).catch(() => {
      if (!disposed) {
        reportCloseGuardFailure()
        setCoreStatus(appMessage({
          ja: '終了確認を開始できませんでした。アプリを開いたまま、もう一度お試しください。',
          en: 'The quit check could not start. Keep the app open and try again.',
        }))
      }
    })

    return () => {
      disposed = true
      closeHandshake.dispose()
      unlisten?.()
    }
  }, [locale])

  const runNativeEdit = useCallback(async (
    action: (
      projectId: string,
      revision: number,
      projectInstanceId: string,
    ) => Promise<ProjectSnapshot>,
  ) => {
    const current = latestSnapshotRef.current
    if (
      !current
      || coreOperationRef.current
      || recoveryBlockingRef.current
    ) return false
    coreOperationRef.current = true
    setCoreBusy(true)
    setCancelInteractionToken((token) => token + 1)
    try {
      const snapshot = await action(
        current.project_id,
        current.revision,
        current.project_instance_id,
      )
      if (
        latestSnapshotRef.current !== current
        || !isExpectedNativeEditSnapshot(
          snapshot,
          current.project_instance_id,
          current.project_id,
          current.revision,
        )
      ) {
        reportUnexpected('app.project_snapshot')
        setCoreStatus(appMessage({
          ja: 'コアエラー: 編集結果を現在のプロジェクトへ結合できませんでした',
          en: 'Core error: the edit result could not be merged into the current project',
        }))
        return false
      }
      applySnapshot(snapshot)
      setValidation(null)
      setCoreStatus(appMessage({
        ja: 'Rustコア revision {revision}',
        en: 'Rust core revision {revision}',
      }, { revision: snapshot.revision }))
      return true
    } catch {
      setCoreStatus(appMessage(
        appErrorLocalizedText('native_edit_failed'),
      ))
      return false
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }, [applySnapshot])

  const runProjectLayerEdit = useCallback((
    action: (
      projectId: string,
      revision: number,
      projectInstanceId: string,
      baseSnapshot: ProjectSnapshot,
    ) => Promise<ProjectSnapshot>,
  ) => runNativeEdit((projectId, revision, projectInstanceId) => {
    const baseSnapshot = latestSnapshotRef.current
    if (
      !baseSnapshot
      || baseSnapshot.project_instance_id !== projectInstanceId
      || baseSnapshot.project_id !== projectId
      || baseSnapshot.revision !== revision
    ) return Promise.reject(new Error('stale layer mutation base'))
    return action(
      projectId,
      revision,
      projectInstanceId,
      baseSnapshot,
    )
  }), [runNativeEdit])

  const createLayerFromPanel = useCallback((
    name: string,
    contentKind: LayerContentKindV1,
  ) => runProjectLayerEdit((
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
  ) => createProjectLayer(
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
    name,
    contentKind,
  )), [runProjectLayerEdit])

  const renameLayerFromPanel = useCallback((
    layerId: string,
    name: string,
  ) => runProjectLayerEdit((
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
  ) => renameProjectLayer(
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
    layerId,
    name,
  )), [runProjectLayerEdit])

  const moveLayerFromPanel = useCallback((
    layerId: string,
    targetIndex: number,
  ) => runProjectLayerEdit((
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
  ) => moveProjectLayer(
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
    layerId,
    targetIndex,
  )), [runProjectLayerEdit])

  const deleteLayerFromPanel = useCallback((
    layerId: string,
  ) => runProjectLayerEdit((
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
  ) => deleteProjectLayer(
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
    layerId,
  )), [runProjectLayerEdit])

  const assignSelectedEdgeToLayer = useCallback((
    layerId: string,
  ) => {
    if (!selectedLine || benchmarkRun) return Promise.resolve(false)
    return runProjectLayerEdit((
      projectId,
      revision,
      projectInstanceId,
      baseSnapshot,
    ) => assignEdgeToProjectLayer(
      projectId,
      revision,
      projectInstanceId,
      baseSnapshot,
      selectedLine.id,
      layerId,
    ))
  }, [benchmarkRun, runProjectLayerEdit, selectedLine])

  const addSelectedEdgeOrientationConstraint = useCallback((
    orientation: 'horizontal' | 'vertical',
  ) => {
    if (!selectedLine || benchmarkRun) return
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      addEdgeOrientationConstraint(
        projectId,
        revision,
        projectInstanceId,
        selectedLine.id,
        orientation,
      ))
  }, [benchmarkRun, runNativeEdit, selectedLine])

  const removeConstraint = useCallback((constraintId: string) => {
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      removeGeometricConstraint(
        projectId,
        revision,
        projectInstanceId,
        constraintId,
      ))
  }, [runNativeEdit])

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
      setCoreStatus(appMessage({
        ja: '性能テストの図は読み取り専用です。通常図へ戻ると編集できます',
        en: 'The benchmark pattern is read-only. Return to the normal pattern to edit.',
      }))
      return
    }
    if (selectedLine) {
      if (selectedLine.kind === 'boundary') {
        setCoreStatus(appMessage({
          ja: '輪郭線の追加・削除は紙形状編集から行います',
          en: 'Add or remove boundary edges through paper shape editing.',
        }))
        return
      }
      const removed = await runNativeEdit((projectId, revision, projectInstanceId) =>
        removeEdge(projectId, revision, projectInstanceId, selectedLine.id))
      if (removed) setSelectedLineId(null)
      return
    }
    if (selectedVertex) {
      if (selectedVertexIsBoundary && paperBoundaryVertexCount <= 3) {
        setCoreStatus(appMessage({
          ja: '輪郭は最低3点必要なため、この輪郭頂点は削除できません',
          en: 'This boundary vertex cannot be deleted because a boundary needs at least three points.',
        }))
        return
      }
      const removed = await runNativeEdit((projectId, revision, projectInstanceId) =>
        selectedVertexIsBoundary
          ? removeBoundaryVertex(projectId, revision, projectInstanceId, selectedVertex.id)
          : removeVertex(projectId, revision, projectInstanceId, selectedVertex.id))
      if (!removed) return
      setSelectedVertexId(null)
      setSelectedLineId(null)
      setPendingEdgeStart(null)
      setActiveTool('select')
      setCoreStatus(selectedVertexIsBoundary
        ? appMessage({
            ja: '輪郭頂点を削除し、隣接する輪郭辺を統合しました（元に戻すで復元できます）',
            en: 'Deleted the boundary vertex and merged its adjacent edges (Undo can restore it).',
          })
        : appMessage({
            ja: '頂点を削除しました（元に戻すで復元できます）',
            en: 'Deleted the vertex (Undo can restore it).',
          }))
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
    const succeeded = await runNativeEdit(async (projectId, revision, projectInstanceId) => {
      const snapshot = await splitBoundaryEdge(
        projectId,
        revision,
        projectInstanceId,
        selectedLine.id,
        0.5,
      )
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
      setCoreStatus(appMessage({
        ja: '輪郭辺を分割しましたが、新しい頂点を特定できませんでした',
        en: 'The boundary edge was split, but the new vertex could not be identified.',
      }))
      return
    }
    setSelectedVertexId(addedVertex.id)
    setActiveTool('select')
    setCoreStatus(appMessage({
      ja: '輪郭辺を中点で分割し、新しい頂点を選択しました',
      en: 'Split the boundary edge at its midpoint and selected the new vertex.',
    }))
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
    const succeeded = await runNativeEdit(async (projectId, revision, projectInstanceId) => {
      let snapshot: ProjectSnapshot
      if (placement.operation === 'add') {
        snapshot = await addVertex(
          projectId,
          revision,
          projectInstanceId,
          placement.x,
          placement.y,
        )
      } else if (placement.operation === 'split-edge') {
        const edge = current.crease_pattern.edges.find(({ id }) => id === placement.edgeId)
        if (!edge) {
          throw new Error(formattedText({
            ja: '分割対象の辺が見つかりません: {edgeId}',
            en: 'The edge to split was not found: {edgeId}',
          }, { edgeId: placement.edgeId }))
        }
        snapshot = edge.kind === 'boundary'
          ? await splitBoundaryEdge(
              projectId,
              revision,
              projectInstanceId,
              placement.edgeId,
              placement.fraction,
            )
          : await splitEdge(
              projectId,
              revision,
              projectInstanceId,
              placement.edgeId,
              placement.fraction,
            )
      } else {
        if (!isSupportedIntersectionPlacement(
          placement,
          current.crease_pattern.edges,
        )) {
          throw new Error(text({
            ja: '交点接続の対象辺が不正です',
            en: 'The edges selected for intersection connection are invalid.',
          }))
        }
        const response = placement.operation === 'connect-intersection'
          ? await connectEdgeIntersection(
              projectId,
              revision,
              projectInstanceId,
              placement.firstEdgeId,
              placement.secondEdgeId,
            )
          : placement.operation === 'connect-t-junction'
            ? await connectTJunction(
                projectId,
                revision,
                projectInstanceId,
                placement.firstEdgeId,
                placement.secondEdgeId,
              )
            : await connectIntersectionCluster(
                projectId,
                revision,
                projectInstanceId,
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
        setCoreStatus(appMessage({
          ja: '交点を接続しましたが、接続頂点を確認できませんでした',
          en: 'The intersection was connected, but the connected vertex could not be verified.',
        }))
        return
      }
      setSelectedLineId(null)
      setPendingEdgeStart(null)
      setSelectedVertexId(result.connectedVertexId)
      setCoreStatus(placement.operation === 'connect-t-junction'
        ? appMessage({
            ja: 'T字交点を接続しました（元に戻す1回で復元できます）',
            en: 'Connected the T-junction (one Undo restores it).',
          })
        : placement.operation === 'connect-intersection-cluster'
          ? appMessage({
              ja: '{count}本の辺を交点クラスタとして接続しました（元に戻す1回で復元できます）',
              en: 'Connected {count} edges as an intersection cluster (one Undo restores it).',
            }, { count: placement.targets.length })
          : appMessage({
              ja: '交点で2本の辺を原子的に分割しました（元に戻す1回で復元できます）',
              en: 'Atomically split two edges at their intersection (one Undo restores it).',
            }))
      return
    }

    const addedVertices = result.snapshot.crease_pattern.vertices.filter(
      ({ id }) => !previousVertexIds.has(id),
    )
    setSelectedLineId(null)
    setPendingEdgeStart(null)
    if (addedVertices.length !== 1) {
      setSelectedVertexId(null)
      setCoreStatus(appMessage({
        ja: '頂点を作成しましたが、新しい頂点を一意に特定できませんでした',
        en: 'A vertex was created, but it could not be uniquely identified.',
      }))
      return
    }
    setSelectedVertexId(addedVertices[0].id)
    setCoreStatus(placement.operation === 'split-edge'
      ? appMessage({
          ja: '辺を分割し、新しい頂点を選択しました（元に戻すで復元できます）',
          en: 'Split the edge and selected the new vertex (Undo can restore it).',
        })
      : appMessage({
          ja: '頂点を追加して選択しました（元に戻すで復元できます）',
          en: 'Added and selected a vertex (Undo can restore it).',
        }))
  }

  useEffect(() => {
    function handleKeyboardShortcut(event: KeyboardEvent) {
      const key = event.key.toLowerCase()
      if (key === 'escape' && newProjectOpen) {
        if (event.repeat || event.isComposing) return
        event.preventDefault()
        if (coreBusy) return
        setNewProjectOpen(false)
        setNewProjectError(null)
        return
      }
      if (recoveryBlocking) {
        if (key === 'escape') event.preventDefault()
        return
      }
      if (modalOpen) return
      if (isEditingText(event.target)) return

      const configuredShortcut = resolveConfiguredKeyboardShortcut(
        event,
        keyboardShortcuts,
      )
      if (configuredShortcut) {
        event.preventDefault()
        if (coreBusy || !nativeSnapshot) return
        if (configuredShortcut === 'new') {
          setNewProjectError(null)
          setNewProjectOpen(true)
        } else if (
          configuredShortcut === 'open'
          || configuredShortcut === 'save'
          || configuredShortcut === 'save_as'
        ) {
          runShortcutFileOperation(configuredShortcut)
        } else if (
          configuredShortcut === 'undo'
          && nativeSnapshot.can_undo
        ) {
          void runNativeEdit(undo)
        } else if (
          configuredShortcut === 'redo'
          && nativeSnapshot.can_redo
        ) {
          void runNativeEdit(redo)
        }
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
  }, [coreBusy, deleteSelection, keyboardShortcuts, modalOpen, nativeSnapshot, newProjectOpen, recoveryBlocking, runNativeEdit, selectedLine, selectedVertex])

  function selectVertexForEdge(vertexId: string) {
    if (
      activeTool !== 'mountain'
      && activeTool !== 'valley'
      && activeTool !== 'auxiliary'
      && activeTool !== 'cut'
    ) return
    if (!pendingEdgeStart) {
      setPendingEdgeStart(vertexId)
      setCoreStatus(appMessage({
        ja: '線の終点を選択してください',
        en: 'Select the line endpoint.',
      }))
      return
    }
    if (pendingEdgeStart === vertexId) {
      setCoreStatus(appMessage({
        ja: '始点とは異なる頂点を選択してください',
        en: 'Select a vertex different from the start point.',
      }))
      return
    }
    const start = pendingEdgeStart
    setPendingEdgeStart(null)
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      addEdge(projectId, revision, projectInstanceId, start, vertexId, activeTool))
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
      setCoreStatus(appMessage({
        ja: '座標には有限の数値を入力してください',
        en: 'Enter finite numeric coordinates.',
      }))
      return
    }
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      moveVertex(projectId, revision, projectInstanceId, selectedVertex.id, x, y))
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
      setCoreStatus(appMessage({
        ja: '紙厚には0以上の有限の数値を入力してください',
        en: 'Enter a finite paper thickness of 0 or greater.',
      }))
      return
    }
    if (!frontColor || !backColor) {
      setCoreStatus(appMessage({
        ja: '表色と裏色には有効な色を指定してください',
        en: 'Choose valid front and back colors.',
      }))
      return
    }

    void runNativeEdit((projectId, revision, projectInstanceId) =>
      updatePaperProperties(projectId, revision, projectInstanceId, {
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
      setCoreStatus(appMessage({
        ja: '現在の紙は軸平行な長方形ではないため、サイズを変更できません',
        en: 'The current paper is not an axis-aligned rectangle, so it cannot be resized here.',
      }))
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
      setCoreStatus(appMessage({
        ja: '用紙の幅には0より大きい有限の数値を入力してください',
        en: 'Enter a finite paper width greater than 0.',
      }))
      return
    }
    if (heightMm === null || heightMm <= 0) {
      setCoreStatus(appMessage({
        ja: '用紙の高さには0より大きい有限の数値を入力してください',
        en: 'Enter a finite paper height greater than 0.',
      }))
      return
    }

    void runNativeEdit((projectId, revision, projectInstanceId) =>
      resizeRectangularPaper(projectId, revision, projectInstanceId, widthMm, heightMm))
  }

  function changeLengthDisplayUnit(
    unit: Parameters<typeof setLengthDisplayUnit>[3],
  ) {
    if (coreOperationRef.current) return
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      setLengthDisplayUnit(projectId, revision, projectInstanceId, unit))
  }

  async function runValidation() {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    coreOperationRef.current = true
    setCoreBusy(true)
    setValidation(null)
    setCoreStatus(appMessage({
      ja: 'revision {revision}: 検証中…',
      en: 'revision {revision}: validating…',
    }, { revision: current.revision }))
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
        setCoreStatus(appMessage({
          ja: '検証中に内容が変更されたため、再度検証してください',
          en: 'The project changed during validation. Please validate again.',
        }))
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
      setCoreStatus(appMessage({
        ja: formatLocalizedText('ja', {
          ja: 'revision {revision}: {geometry}・{local}',
          en: '',
        }, {
          revision: result.revision,
          geometry: result.is_valid
            ? '幾何検証に合格'
            : formatLocalizedText('ja', {
                ja: '幾何問題{count}件',
                en: '',
              }, { count: result.issues.length }),
          local: localFlatFoldabilityCoreStatus(localPresentation, 'ja'),
        }),
        en: formatLocalizedText('en', {
          ja: '',
          en: 'revision {revision}: {geometry} · {local}',
        }, {
          revision: result.revision,
          geometry: result.is_valid
            ? 'Geometry passed'
            : formatLocalizedText('en', {
                ja: '',
                en: '{count} geometry issues',
              }, { count: result.issues.length }),
          local: localFlatFoldabilityCoreStatus(localPresentation, 'en'),
        }),
      }))
    } catch {
      reportValidationUnexpected()
      setValidation(null)
      setCoreStatus(appMessage(
        appErrorLocalizedText('validation_failed'),
      ))
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function submitNewProject(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (
      !current
      || coreOperationRef.current
      || recoveryBlockingRef.current
    ) return

    const form = new FormData(event.currentTarget)
    const name = String(form.get('name') ?? '').trim()
    const widthExpression = String(form.get('width_expression') ?? '')
    const heightExpression = String(form.get('height_expression') ?? '')
    const thicknessInput = String(form.get('thickness_mm') ?? '').trim()
    const thicknessMm = Number(thicknessInput)
    const frontColor = parseHexColor(String(form.get('front_color') ?? ''))
    const backColor = parseHexColor(String(form.get('back_color') ?? ''))

    if (!name) {
      setNewProjectError(appMessage({
        ja: '作品名を入力してください。',
        en: 'Enter a project name.',
      }))
      return
    }
    if ([...name].length > 120 || hasControlCharacter(name)) {
      setNewProjectError(appMessage({
        ja: '作品名は制御文字を含まない120文字以内にしてください。',
        en: 'Use at most 120 characters and do not include control characters.',
      }))
      return
    }
    if (!widthExpression.trim()) {
      setNewProjectError(appMessage({
        ja: '幅の式を入力してください。',
        en: 'Enter a width expression.',
      }))
      return
    }
    if (!heightExpression.trim()) {
      setNewProjectError(appMessage({
        ja: '高さの式を入力してください。',
        en: 'Enter a height expression.',
      }))
      return
    }
    if (!thicknessInput || !Number.isFinite(thicknessMm) || thicknessMm < 0) {
      setNewProjectError(appMessage({
        ja: '紙厚には0以上の有限の数値を入力してください。',
        en: 'Enter a finite paper thickness of 0 or greater.',
      }))
      return
    }
    if (!frontColor || !backColor) {
      setNewProjectError(appMessage({
        ja: '表色と裏色を選択してください。',
        en: 'Choose front and back colors.',
      }))
      return
    }
    if (
      current.is_dirty &&
      !window.confirm(appConfirmationText(locale, 'newProject'))
    ) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setNewProjectError(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      await evaluatePositiveMillimetreExpression(widthExpression)
      await evaluatePositiveMillimetreExpression(heightExpression)
      const snapshot = await newProject(
        current.project_instance_id,
        current.project_id,
        current.revision,
        {
          name,
          widthExpression,
          heightExpression,
          thicknessMm,
          cuttingAllowed: form.get('cutting_allowed') === 'on',
          frontColor,
          backColor,
        },
      )
      applySnapshot(snapshot, true)
      setValidation(null)
      setSelectedLineId(null)
      setSelectedVertexId(null)
      setPendingEdgeStart(null)
      setParallelReferenceEdgeId(null)
      setActiveTool('select')
      setNewProjectOpen(false)
      setCoreStatus(appMessage({
        ja: '「{name}」を作成しました。保存先はまだ設定されていません。',
        en: 'Created “{name}”. A save location has not been set yet.',
      }, { name: snapshot.name }))
    } catch (error) {
      const japaneseMessage = newProjectExpressionErrorMessage(error, 'ja')
        ?? '新しいプロジェクトを作成できませんでした。'
      const englishMessage = newProjectExpressionErrorMessage(error, 'en')
        ?? 'The new project could not be created.'
      setNewProjectError(appMessage({
        ja: formatLocalizedText('ja', {
          ja: '作成できませんでした: {message}',
          en: '',
        }, { message: japaneseMessage }),
        en: formatLocalizedText('en', {
          ja: '',
          en: 'Could not create the project: {message}',
        }, { message: englishMessage }),
      }))
      setCoreStatus(appMessage({
        ja: formatLocalizedText('ja', {
          ja: '新規作成エラー: {message}',
          en: '',
        }, { message: japaneseMessage }),
        en: formatLocalizedText('en', {
          ja: '',
          en: 'New project error: {message}',
        }, { message: englishMessage }),
      }))
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function runFileOperation(operation: 'open' | 'save' | 'save_as') {
    const current = latestSnapshotRef.current
    if (
      !current
      || coreOperationRef.current
      || recoveryBlockingRef.current
    ) return
    if (
      operation === 'open' &&
      current.is_dirty &&
      !window.confirm(appConfirmationText(locale, 'openProject'))
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
        setCoreStatus(appMessage({
          ja: 'ファイル操作をキャンセルしました',
          en: 'File operation cancelled',
        }))
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
        ? appMessage({
            ja: '「{name}」を開きました',
            en: 'Opened “{name}”',
          }, { name: response.project.name })
        : appMessage({
            ja: '「{name}」を保存しました',
            en: 'Saved “{name}”',
          }, { name: response.project.name }))
    } catch {
      setCoreStatus(appMessage(
        appErrorLocalizedText('file_operation_failed'),
      ))
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
        setCoreStatus(appMessage({
          ja: 'FOLD取込をキャンセルしました',
          en: 'FOLD import cancelled',
        }))
        return
      }
      if (!response.preview) {
        throw new Error(text({
          ja: '取込プレビューが返されませんでした',
          en: 'No import preview was returned.',
        }))
      }
      setFoldImportPreview(response.preview)
      setCoreStatus(appMessage({
        ja: 'FOLDの線種・縮尺を確認してください',
        en: 'Review the FOLD line types and scale.',
      }))
    } catch {
      setCoreStatus(appMessage(
        appErrorLocalizedText('fold_read_failed'),
      ))
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
      setCoreStatus(appMessage({
        ja: 'FOLD取込をキャンセルしました',
        en: 'FOLD import cancelled',
      }))
    } catch {
      setCoreStatus(appMessage(
        appErrorLocalizedText('fold_cleanup_failed'),
      ))
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
      && !window.confirm(appConfirmationText(locale, 'replaceWithFold'))
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
      setBenchmarkStatus(appMessage({
        ja: 'FOLD取込により通常の展開図へ戻りました',
        en: 'Returned to the normal crease pattern after FOLD import',
      }))
      setFoldImportPreview(null)
      setSelectedLineId(null)
      setSelectedVertexId(null)
      setPendingEdgeStart(null)
      setParallelReferenceEdgeId(null)
      setAppliedFoldPose(null)
      setFoldAngleOverrides({ projectId: null, values: new Map() })
      setFixedFaceChoice({ projectId: null, faceId: null })
      setActiveTool('select')
      setCoreStatus(appMessage({
        ja: 'FOLDから「{name}」を取り込みました。保存先はまだ設定されていません。',
        en: 'Imported “{name}” from FOLD. A save location has not been set yet.',
      }, { name: snapshot.name }))
      requestAnimationFrame(() => foldImportButtonRef.current?.focus())
    } catch {
      const safeError = appMessage(
        appErrorLocalizedText('fold_import_failed'),
      )
      setFoldImportError(safeError)
      setCoreStatus(safeError)
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
        setCoreStatus(appMessage({
          ja: 'SVG取込をキャンセルしました',
          en: 'SVG import cancelled',
        }))
        return
      }
      if (!response.preview) {
        throw new Error(text({
          ja: '取込プレビューが返されませんでした',
          en: 'No import preview was returned.',
        }))
      }
      setSvgImportPreview(response.preview)
      setCoreStatus(appMessage({
        ja: 'SVGの外周・線種・縮尺を確認してください',
        en: 'Review the SVG boundary, line types, and scale.',
      }))
    } catch {
      setCoreStatus(appMessage(
        appErrorLocalizedText('svg_read_failed'),
      ))
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
      setCoreStatus(appMessage({
        ja: 'SVG取込をキャンセルしました',
        en: 'SVG import cancelled',
      }))
      setSvgImportPreview(null)
      setSvgImportError(null)
      setSvgImportValidation(null)
      requestAnimationFrame(() => svgImportButtonRef.current?.focus())
    } catch {
      const safeError = appMessage(
        appErrorLocalizedText('svg_cleanup_failed'),
      )
      setSvgImportError(safeError)
      setCoreStatus(safeError)
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
      setCoreStatus(appMessage({
        ja: formatLocalizedText('ja', {
          ja: 'SVG外周を検証しました: {width} × {height} mm',
          en: '',
        }, {
          width: validation.width_mm.toLocaleString('ja'),
          height: validation.height_mm.toLocaleString('ja'),
        }),
        en: formatLocalizedText('en', {
          ja: '',
          en: 'Validated SVG boundary: {width} × {height} mm',
        }, {
          width: validation.width_mm.toLocaleString('en'),
          height: validation.height_mm.toLocaleString('en'),
        }),
      }))
    } catch {
      const safeError = appMessage(
        appErrorLocalizedText('svg_boundary_validation_failed'),
      )
      setSvgImportError(safeError)
      setCoreStatus(safeError)
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
      && !window.confirm(appConfirmationText(locale, 'replaceWithSvg'))
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
      setBenchmarkStatus(appMessage({
        ja: 'SVG取込により通常の展開図へ戻りました',
        en: 'Returned to the normal crease pattern after SVG import',
      }))
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
      setCoreStatus(appMessage({
        ja: 'SVGから「{name}」を取り込みました。保存先はまだ設定されていません。',
        en: 'Imported “{name}” from SVG. A save location has not been set yet.',
      }, { name: snapshot.name }))
      requestAnimationFrame(() => svgImportButtonRef.current?.focus())
    } catch {
      const safeError = appMessage(
        appErrorLocalizedText('svg_import_failed'),
      )
      setSvgImportError(safeError)
      setCoreStatus(safeError)
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
        throw new Error(text({
          ja: '編集中のプロジェクトと一致しない書き出しプレビューを拒否しました',
          en: 'Rejected an export preview that does not match the current project.',
        }))
      }
      setCreaseExportPreview(preview)
      setCoreStatus(appMessage({
        ja: formatLocalizedText('ja', {
          ja: '{format}書き出しの情報損失を確認してください',
          en: '',
        }, { format: localizedCreaseExportFormatLabel(preview.format, 'ja') }),
        en: formatLocalizedText('en', {
          ja: '',
          en: 'Review information loss for the {format} export.',
        }, { format: localizedCreaseExportFormatLabel(preview.format, 'en') }),
      }))
    } catch {
      if (requestId !== creaseExportRequestIdRef.current) return
      const safeError = appMessage(
        appErrorLocalizedText('crease_export_prepare_failed'),
      )
      setCreaseExportError(safeError)
      setCoreStatus(safeError)
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
      setCoreStatus(appMessage({
        ja: '展開図書き出しをキャンセルしました',
        en: 'Crease-pattern export cancelled',
      }))
      requestAnimationFrame(() => creaseExportButtonRef.current?.focus())
    } catch {
      const safeError = appMessage(
        appErrorLocalizedText('crease_export_cleanup_failed'),
      )
      setCreaseExportError(safeError)
      setCoreStatus(safeError)
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
      setCreaseExportError(appMessage({
        ja: '編集内容が変わったため、書き出しデータを作り直してください。',
        en: 'The project changed. Rebuild the export data.',
      }))
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
        setCreaseExportNotice(appMessage({
          ja: '保存先の選択をキャンセルしました。確認画面から再試行できます。',
          en: 'Save location selection was cancelled. You can retry from the review screen.',
        }))
        setCoreStatus(appMessage({
          ja: '展開図の保存先選択をキャンセルしました',
          en: 'Crease-pattern save location selection cancelled',
        }))
        return
      }
      setCreaseExportOpen(false)
      setCreaseExportPreview(null)
      setCreaseExportNotice(null)
      setCoreStatus(appMessage({
        ja: '{fileName}を書き出しました',
        en: 'Exported {fileName}',
      }, { fileName: preview.suggested_file_name }))
      requestAnimationFrame(() => creaseExportButtonRef.current?.focus())
    } catch {
      const safeError = appMessage(
        appErrorLocalizedText('crease_export_save_failed'),
      )
      setCreaseExportError(safeError)
      setCoreStatus(safeError)
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
      setCoreStatus(appMessage({
        ja: formatLocalizedText('ja', {
          ja: '{format}の内容と注意事項を確認してください。',
          en: '',
        }, {
          format: localizedInstructionExportFormatLabel(preview.format, 'ja'),
        }),
        en: formatLocalizedText('en', {
          ja: '',
          en: 'Review the {format} content and notices.',
        }, {
          format: localizedInstructionExportFormatLabel(preview.format, 'en'),
        }),
      }))
    } catch (error) {
      if (requestId !== instructionExportRequestIdRef.current) return
      instructionExportGenerationIdRef.current = null
      setInstructionExportError(instructionExportErrorAppMessage(error, {
        ja: '折り図を準備できませんでした: {error}',
        en: 'Could not prepare the instructions: {error}',
      }))
      setCoreStatus(instructionExportErrorAppMessage(error, {
        ja: '折り図書き出しエラー: {error}',
        en: 'Instruction export error: {error}',
      }))
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
        setInstructionExportNotice(instructionExportErrorAppMessage(error, {
          ja: '進捗表示を更新できませんでした: {error} 生成結果を待っています。',
          en: 'Progress could not be updated: {error} Waiting for the generated result.',
        }))
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
      setCoreStatus(appMessage({
        ja: '折り図の生成を中止しています。',
        en: 'Stopping instruction generation…',
      }))
      requestAnimationFrame(() => instructionExportButtonRef.current?.focus())
      if (exportId) {
        try {
          await cancelInstructionExport(exportId)
          setCoreStatus(appMessage({
            ja: '折り図の生成を中止しました。',
            en: 'Instruction generation stopped.',
          }))
        } catch {
          setCoreStatus(appMessage({
            ja: '折り図の生成は終了済みです。',
            en: 'Instruction generation has already finished.',
          }))
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
      setCoreStatus(appMessage({
        ja: '折り図の書き出しをキャンセルしました。',
        en: 'Instruction export cancelled.',
      }))
      requestAnimationFrame(() => instructionExportButtonRef.current?.focus())
    } catch (error) {
      setInstructionExportError(instructionExportErrorAppMessage(error, {
        ja: 'キャンセルを完了できませんでした: {error}',
        en: 'Could not cancel: {error}',
      }))
      setCoreStatus(instructionExportErrorAppMessage(error, {
        ja: '折り図キャンセルエラー: {error}',
        en: 'Instruction cancellation error: {error}',
      }))
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
      setInstructionExportError(appMessage({
        ja: '編集内容が変わったため、折り図データを作り直してください。',
        en: 'The project changed. Rebuild the instruction data.',
      }))
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
        setInstructionExportNotice(appMessage({
          ja: '保存先の選択をキャンセルしました。この画面からもう一度保存できます。',
          en: 'Save location selection was cancelled. You can save again from this screen.',
        }))
        setCoreStatus(appMessage({
          ja: '折り図の保存先選択をキャンセルしました。',
          en: 'Instruction save location selection cancelled.',
        }))
        return
      }
      setInstructionExportOpen(false)
      instructionExportGenerationIdRef.current = null
      setInstructionExportPreview(null)
      setInstructionExportNotice(null)
      setCoreStatus(appMessage({
        ja: '{fileName}を書き出しました。',
        en: 'Exported {fileName}.',
      }, { fileName: preview.suggested_file_name }))
      requestAnimationFrame(() => instructionExportButtonRef.current?.focus())
    } catch (error) {
      setInstructionExportError(instructionExportErrorAppMessage(error, {
        ja: '折り図を書き出せませんでした: {error}',
        en: 'Could not export the instructions: {error}',
      }))
      setCoreStatus(instructionExportErrorAppMessage(error, {
        ja: '折り図書き出しエラー: {error}',
        en: 'Instruction export error: {error}',
      }))
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function toggleBenchmark() {
    if (benchmarkRun) {
      setBenchmarkRun(null)
      setBenchmarkStatus(appMessage({
        ja: '通常の展開図に戻りました',
        en: 'Returned to the normal crease pattern',
      }))
      setSelectedLineId(null)
      setSelectedVertexId(null)
      return
    }
    if (benchmarkLoading) return

    setBenchmarkLoading(true)
    setBenchmarkStatus(appMessage({
      ja: '10,000本の実データを生成・転送中…',
      en: 'Generating and transferring 10,000 real edges…',
    }))
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
      setBenchmarkStatus(appMessageWithLocalizedVariables({
        ja: '{count}本 · {bytes} · 生成+転送 {responseMs}ms · Canvas計測中…',
        en: '{count} edges · {bytes} · generation + transfer {responseMs} ms · measuring canvas…',
      }, (locale) => ({
        count: run.lines.length.toLocaleString(locale),
        bytes: formatBytes(payloadBytes, locale),
        responseMs: responseMs.toFixed(1),
      })))
    } catch {
      reportUnexpected('app.benchmark')
      setBenchmarkStatus(appMessage(
        appErrorLocalizedText('benchmark_failed'),
      ))
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
    setBenchmarkStatus(appMessageWithLocalizedVariables({
      ja: '{count}本 · {bytes} · 生成+転送 {responseMs}ms · 変換 {preparationMs}ms · UI準備 {uiMs}ms · 初描画 {drawMs}ms · {frames}f {fps} FPS · p95 {p95}ms',
      en: '{count} edges · {bytes} · generation + transfer {responseMs} ms · conversion {preparationMs} ms · UI preparation {uiMs} ms · initial draw {drawMs} ms · {frames}f {fps} FPS · p95 {p95} ms',
    }, (locale) => ({
      count: metrics.lineCount.toLocaleString(locale),
      bytes: formatBytes(run.payloadBytes, locale),
      responseMs: run.responseMs.toFixed(1),
      preparationMs: run.preparationMs.toFixed(1),
      uiMs: uiPreparationMs.toFixed(1),
      drawMs: metrics.initialDrawMs.toFixed(1),
      frames: metrics.sampleFrameCount,
      fps: metrics.framesPerSecond.toFixed(1),
      p95: metrics.p95DrawMs.toFixed(1),
    })))
  }

  return (
    <main className="app-shell" style={workspaceLayoutStyle}>
      <RecoveryAutosaveStatusBanner view={recoveryAutosaveMonitor} />
      <header className="titlebar" inert={modalOpen}>
        <div className="brand-mark" aria-hidden="true">◇</div>
        <strong>ORIGAMI2</strong>
        <span className="document-name">
          {nativeSnapshot?.name ?? text({
            ja: '無題のプロジェクト',
            en: 'Untitled project',
          })}
          {nativeSnapshot?.is_dirty ? ' *' : ''}
        </span>
        <nav
          className="top-actions"
          aria-label={text({
            ja: 'プロジェクト操作',
            en: 'Project actions',
          })}
        >
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText({
              ja: '新規 ({shortcut})',
              en: 'New ({shortcut})',
            }, {
              shortcut: keyboardShortcutDisplayValue('new', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('new', keyboardShortcuts)}
            onClick={() => {
              setNewProjectError(null)
              setNewProjectOpen(true)
            }}
          >
            {text({ ja: '新規', en: 'New' })}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot?.can_undo}
            onClick={() => runNativeEdit(undo)}
            title={formattedText({
              ja: '元に戻す ({shortcut})',
              en: 'Undo ({shortcut})',
            }, {
              shortcut: keyboardShortcutDisplayValue('undo', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('undo', keyboardShortcuts)}
          >
            {text({ ja: '元に戻す', en: 'Undo' })}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot?.can_redo}
            onClick={() => runNativeEdit(redo)}
            title={formattedText({
              ja: 'やり直す ({shortcut})',
              en: 'Redo ({shortcut})',
            }, {
              shortcut: keyboardShortcutDisplayValue('redo', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('redo', keyboardShortcuts)}
          >
            {text({ ja: 'やり直す', en: 'Redo' })}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot || !paperCenter}
            onClick={() => {
              if (!paperCenter) return
              void runNativeEdit((projectId, revision, projectInstanceId) =>
                addVertex(projectId, revision, projectInstanceId, paperCenter.x, paperCenter.y))
            }}
          >
            {text({ ja: '中央に頂点', en: 'Vertex at center' })}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText({
              ja: '開く ({shortcut})',
              en: 'Open ({shortcut})',
            }, {
              shortcut: keyboardShortcutDisplayValue('open', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('open', keyboardShortcuts)}
            onClick={() => void runFileOperation('open')}
          >
            {fileOperation === 'open'
              ? text({ ja: '開いています…', en: 'Opening…' })
              : text({ ja: '開く', en: 'Open' })}
          </button>
          <button
            ref={foldImportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void beginFoldImport()}
            aria-haspopup="dialog"
          >
            {fileOperation === 'fold_import'
              ? text({ ja: '解析中…', en: 'Analyzing…' })
              : text({ ja: 'FOLD取込', en: 'Import FOLD' })}
          </button>
          <button
            ref={svgImportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void beginSvgImport()}
            aria-haspopup="dialog"
          >
            {fileOperation === 'svg_import'
              ? text({ ja: '解析中…', en: 'Analyzing…' })
              : text({ ja: 'SVG取込', en: 'Import SVG' })}
          </button>
          <button
            ref={creaseExportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={beginCreaseExport}
            aria-haspopup="dialog"
          >
            {fileOperation === 'crease_export'
              ? text({ ja: '生成中…', en: 'Generating…' })
              : text({ ja: '書出し', en: 'Export' })}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText({
              ja: '保存 ({shortcut})',
              en: 'Save ({shortcut})',
            }, {
              shortcut: keyboardShortcutDisplayValue('save', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('save', keyboardShortcuts)}
            onClick={() => void runFileOperation('save')}
          >
            {fileOperation === 'save'
              ? text({ ja: '保存中…', en: 'Saving…' })
              : text({ ja: '保存', en: 'Save' })}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText({
              ja: '別名保存 ({shortcut})',
              en: 'Save as ({shortcut})',
            }, {
              shortcut: keyboardShortcutDisplayValue('save_as', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('save_as', keyboardShortcuts)}
            onClick={() => void runFileOperation('save_as')}
          >
            {fileOperation === 'save_as'
              ? text({ ja: '保存中…', en: 'Saving…' })
              : text({ ja: '別名保存', en: 'Save as' })}
          </button>
          <button
            type="button"
            className="primary"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void runValidation()}
          >
            {text({ ja: '検証', en: 'Validate' })}
          </button>
        </nav>
      </header>

      <section className="workspace" inert={modalOpen} id="workspace-main" data-inspector-side={workspaceLayout.inspectorSide}>
        <aside
          className="tool-rail"
          aria-label={text({ ja: '作図ツール', en: 'Drawing tools' })}
        >
          {([
            { id: 'select', icon: '↖', label: { ja: '選択', en: 'Select' } },
            { id: 'vertex', icon: '＋', label: { ja: '頂点', en: 'Vertex' } },
            { id: 'mountain', icon: '━', label: { ja: '山折り', en: 'Mountain fold' } },
            { id: 'valley', icon: '┅', label: { ja: '谷折り', en: 'Valley fold' } },
            { id: 'auxiliary', icon: '┈', label: { ja: '補助線', en: 'Auxiliary line' } },
            { id: 'cut', icon: '✂', label: { ja: '切断', en: 'Cut' } },
            { id: 'measure', icon: '∠', label: { ja: '計測', en: 'Measure' } },
          ] as const).map(({ id, icon, label }) => (
            <button
              type="button"
              key={id}
              disabled={coreBusy || (id === 'cut' && !nativeSnapshot?.cutting_allowed)}
              className={activeTool === id ? 'active' : ''}
              onClick={() => {
                setActiveTool(id)
                setPendingEdgeStart(null)
              }}
              title={text(label)}
              aria-label={text(label)}
              aria-pressed={activeTool === id}
            >
              {icon}
            </button>
          ))}
        </aside>

        <section
          id="workspace-editor-panels"
          className="editor-grid"
          data-panel-order={workspaceLayout.panelOrder}
        >
          <article id="crease-editor-panel" className="panel crease-panel">
            <div className="panel-heading">
              <span>{text({ ja: '2D 展開図', en: '2D crease pattern' })}</span>
              <span className="panel-meta">
                {benchmarkRun
                  ? formattedText({
                      ja: '性能テスト · {count}本',
                      en: 'Benchmark · {count} edges',
                    }, { count: displayedLines.length.toLocaleString(locale) })
                  : formattedText({
                      ja: '{size} · {count}本',
                      en: '{size} · {count} edges',
                    }, {
                      size: paperSizeLabel,
                      count: displayedLines.length.toLocaleString(locale),
                    })}
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
                locale,
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
                      setCoreStatus(appMessage({
                        ja: '交点候補が過密なため配置できません。拡大して再試行してください',
                        en: 'Too many intersection candidates. Zoom in and try again.',
                      }))
                    } else if (reason === 'intersection-blocked') {
                      setCoreStatus(appMessage({
                        ja: '未対応または曖昧な交点クラスタのため配置できません。辺や頂点の重複を確認してください',
                        en: 'This intersection cluster is unsupported or ambiguous. Check for overlapping edges or vertices.',
                      }))
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
                    void runNativeEdit((projectId, revision, projectInstanceId) =>
                      moveVertex(projectId, revision, projectInstanceId, vertexId, x, y))
                  }}
            />
          </article>

          <WorkspaceLayoutSeparator kind="editor" />

          <article id="fold-preview-panel" className="panel preview-panel">
            <div className="panel-heading">
              <span>{text({ ja: '3D プレビュー', en: '3D preview' })}</span>
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
              <label htmlFor="fixed-face">
                {text({ ja: '固定面', en: 'Fixed face' })}
              </label>
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
                      <option value={face.id} key={face.id}>
                        {formattedText({
                          ja: '面 {index}',
                          en: 'Face {index}',
                        }, { index: index + 1 })}
                      </option>
                    ))
                  : (
                      <option value="">
                        {text({ ja: '選択不可', en: 'Unavailable' })}
                      </option>
                    )}
              </select>
              <span>
                {fixedFaceEnabled
                  ? text({ ja: '青枠・固定', en: 'Blue outline · fixed' })
                  : '—'}
              </span>
            </div>
            <div className="fold-control">
              <label htmlFor="fold-angle">
                {foldPreviewModel?.kind === 'fold_graph'
                  && foldPreviewModel.kinematics.kind === 'tree'
                  ? text({ ja: '全ヒンジ', en: 'All hinges' })
                  : text({ ja: '指定折り量', en: 'Target fold' })}
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
                        ? text({
                            ja: '全ヒンジの指定折り量（度）',
                            en: 'Target fold for all hinges (degrees)',
                          })
                        : text({
                            ja: '指定折り量（度）',
                            en: 'Target fold (degrees)',
                          })
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
                    <strong id="hinge-angle-title">
                      {text({
                        ja: 'ヒンジ別の折り量',
                        en: 'Fold amount by hinge',
                      })}
                    </strong>
                    <span>
                      {text({
                        ja: '橙枠=従属面・衝突未検証',
                        en: 'Orange outline = dependent face; collision unchecked',
                      })}
                    </span>
                  </div>
                  {foldPreviewModel.kinematics.joints.map((joint, index) => {
                    const hingeAngle = foldTreeHingeAngles[index]?.angleDegrees ?? foldAngle
                    const label = joint.hinge.assignment === 'mountain'
                      ? text({ ja: '山折り', en: 'mountain fold' })
                      : text({ ja: '谷折り', en: 'valley fold' })
                    const inputId = `hinge-angle-${joint.hinge.edgeId}`
                    const selected = selectedLineId === joint.hinge.edgeId
                    return (
                      <div className="hinge-angle-row" key={joint.hinge.edgeId}>
                        <button
                          type="button"
                          className="hinge-select-button"
                          aria-pressed={benchmarkRun ? false : selected}
                          aria-label={formattedText({
                            ja: '{index}番目の{label}を2D・3Dで{action}',
                            en: '{action} {label} {index} in 2D and 3D',
                          }, {
                            index: index + 1,
                            label,
                            action: selected
                              ? text({ ja: '選択解除', en: 'Deselect' })
                              : text({ ja: '選択', en: 'Select' }),
                          })}
                          disabled={Boolean(benchmarkRun)}
                          title={formattedText({
                            ja: '2D・3Dで選択: {edgeId}',
                            en: 'Select in 2D and 3D: {edgeId}',
                          }, { edgeId: joint.hinge.edgeId })}
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
                          aria-label={formattedText({
                            ja: '{index}番目の{label}の折り量',
                            en: 'Fold amount for {label} {index}',
                          }, { index: index + 1, label })}
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
                            aria-label={formattedText({
                              ja: '{index}番目の{label}の角度',
                              en: 'Angle for {label} {index}',
                            }, { index: index + 1, label })}
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

        <WorkspaceLayoutSeparator kind="inspector" />

        <aside id="workspace-inspector-panel" className="inspector panel">
          <div className="panel-heading">
            {text({ ja: 'プロパティ', en: 'Properties' })}
          </div>
          <section>
            <h2>{text({ ja: '選択要素', en: 'Selection' })}</h2>
            {selectedLine ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedLine.id}</dd></div>
                  <div>
                    <dt>{text({ ja: '種類', en: 'Type' })}</dt>
                    <dd>{lineKindLabel(selectedLine.kind, locale)}</dd>
                  </div>
                  <div>
                    <dt>{text({ ja: '始点', en: 'Start' })}</dt>
                    <dd>{formatLengthPoint(
                      selectedLine.x1,
                      selectedLine.y1,
                      displayedLengthUnit,
                      locale,
                    )}</dd>
                  </div>
                  <div>
                    <dt>{text({ ja: '終点', en: 'End' })}</dt>
                    <dd>{formatLengthPoint(
                      selectedLine.x2,
                      selectedLine.y2,
                      displayedLengthUnit,
                      locale,
                    )}</dd>
                  </div>
                  <div><dt>ΔX</dt><dd>{formatLength(selectedLineMeasurement?.deltaX, displayedLengthUnit, locale)}</dd></div>
                  <div><dt>ΔY</dt><dd>{formatLength(selectedLineMeasurement?.deltaY, displayedLengthUnit, locale)}</dd></div>
                  <div>
                    <dt>{text({ ja: '長さ', en: 'Length' })}</dt>
                    <dd>{formatLength(selectedLineMeasurement?.length, displayedLengthUnit, locale)}</dd>
                  </div>
                  <div>
                    <dt>{text({ ja: '角度', en: 'Angle' })}</dt>
                    <dd>{formatMeasurementValue(
                      selectedLineMeasurement?.angleDegrees,
                      '°',
                      2,
                      locale,
                    )}</dd>
                  </div>
                </dl>
                {benchmarkRun ? (
                  <p className="muted">
                    {text({
                      ja: '性能テストの図は選択・計測のみ可能です。',
                      en: 'The benchmark pattern supports selection and measurement only.',
                    })}
                  </p>
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
                        ? text({
                            ja: '方向参照を解除',
                            en: 'Clear direction reference',
                          })
                        : text({
                            ja: '方向参照に設定',
                            en: 'Set as direction reference',
                          })}
                    </button>
                    {selectedLine.kind === 'boundary' ? (
                      <button
                        type="button"
                        disabled={coreBusy}
                        onClick={() => void splitSelectedBoundaryEdge()}
                      >
                        {text({
                          ja: '輪郭辺を中点で分割',
                          en: 'Split boundary edge at midpoint',
                        })}
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="danger"
                        disabled={coreBusy}
                        onClick={() => void deleteSelection()}
                      >
                        {text({ ja: '線を削除', en: 'Delete line' })}
                      </button>
                    )}
                  </div>
                )}
                {selectedLine.kind === 'boundary' && (
                  <p className="muted">
                    {text({
                      ja: '分割後に選択される新しい頂点を移動して、紙の輪郭を編集できます。',
                      en: 'Move the newly selected vertex after splitting to edit the paper boundary.',
                    })}
                  </p>
                )}
              </>
            ) : selectedBenchmarkVertex ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedBenchmarkVertex.id}</dd></div>
                  <div>
                    <dt>{text({ ja: '種類', en: 'Type' })}</dt>
                    <dd>{text({
                      ja: '性能テスト頂点',
                      en: 'Benchmark vertex',
                    })}</dd>
                  </div>
                  <div><dt>X</dt><dd>{selectedBenchmarkVertex.x}</dd></div>
                  <div><dt>Y</dt><dd>{selectedBenchmarkVertex.y}</dd></div>
                </dl>
                <p className="muted">
                  {text({
                    ja: '性能テストの図は選択・計測のみ可能です。',
                    en: 'The benchmark pattern supports selection and measurement only.',
                  })}
                </p>
              </>
            ) : selectedVertex ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedVertex.id}</dd></div>
                  <div>
                    <dt>{text({ ja: '種類', en: 'Type' })}</dt>
                    <dd>{text({ ja: '頂点', en: 'Vertex' })}</dd>
                  </div>
                </dl>
                <form
                  key={`${selectedVertex.id}:${selectedVertex.position.x}:${selectedVertex.position.y}:${lengthDisplayUnit.key}`}
                  className="coordinate-form"
                  onSubmit={submitVertexPosition}
                >
                  <label className="field">
                    {`X (${lengthDisplayUnitLabelText})`}
                    <LengthValueInput
                      name="x_display"
                      disabled={coreBusy}
                      initialMillimetres={selectedVertex.position.x}
                      unit={lengthDisplayUnit}
                      ariaLabel={formattedText({
                        ja: '頂点のX座標 ({unit})',
                        en: 'Vertex X coordinate ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <label className="field">
                    {`Y (${lengthDisplayUnitLabelText})`}
                    <LengthValueInput
                      name="y_display"
                      disabled={coreBusy}
                      initialMillimetres={selectedVertex.position.y}
                      unit={lengthDisplayUnit}
                      ariaLabel={formattedText({
                        ja: '頂点のY座標 ({unit})',
                        en: 'Vertex Y coordinate ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <div className="property-actions">
                    <button type="submit" disabled={coreBusy}>
                      {text({ ja: '座標を更新', en: 'Update coordinates' })}
                    </button>
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
                        ? text({
                            ja: '輪郭頂点を削除して辺を統合',
                            en: 'Delete boundary vertex and merge edges',
                          })
                        : text({ ja: '頂点を削除', en: 'Delete vertex' })}
                    </button>
                  </div>
                  <p className="muted">
                    {selectedVertexIsBoundary
                      ? formattedText({
                          ja: '輪郭は最低3点必要です（現在{count}点）。この操作は元に戻せます。接続線がある場合など、安全に統合できない削除は拒否されます。',
                          en: 'A boundary needs at least three points ({count} currently). This action can be undone. Unsafe deletion, such as a vertex with connected lines, is rejected.',
                        }, { count: paperBoundaryVertexCount })
                      : text({
                          ja: '接続線がある頂点は、線を削除してから削除します。',
                          en: 'Delete connected lines before deleting their vertex.',
                        })}
                  </p>
                </form>
              </>
            ) : (
              <p className="muted">
                {text({
                  ja: '線または頂点を選択してください',
                  en: 'Select a line or vertex',
                })}
              </p>
            )}
          </section>
          {nativeSnapshot && !benchmarkRun && (
            <ProjectLayerPanel
              document={nativeSnapshot.project_layers}
              bindingKey={[
                nativeSnapshot.project_instance_id,
                nativeSnapshot.project_id,
                nativeSnapshot.revision,
              ].join(':')}
              selectedEdgeId={selectedLine?.id ?? null}
              disabled={coreBusy || recoveryBlocking}
              documentInvalid={projectLayerDocumentInvalid}
              onCreate={createLayerFromPanel}
              onRename={renameLayerFromPanel}
              onMove={moveLayerFromPanel}
              onDelete={deleteLayerFromPanel}
              onAssignSelectedEdge={assignSelectedEdgeToLayer}
            />
          )}
          {nativeSnapshot && !benchmarkRun && (
            <GeometricConstraintPanel
              document={nativeSnapshot.geometric_constraints ?? {
                schema_version: 1,
                constraints: [],
              }}
              preflight={geometricConstraintPreflight?.result ?? null}
              analyzing={geometricConstraintAnalysisBusy}
              analysisFailed={
                geometricConstraintAnalysisFailed || geometricConstraintDocumentInvalid
              }
              selectedEdgeId={selectedLine?.id ?? null}
              disabled={coreBusy || geometricConstraintDocumentInvalid}
              onAddOrientation={addSelectedEdgeOrientationConstraint}
              onRemove={removeConstraint}
              onSelectEdge={(edgeId) => {
                if (!nativeLines.some((line) => line.id === edgeId)) return
                setSelectedLineId(edgeId)
                setSelectedVertexId(null)
              }}
              onRetryAnalysis={retryGeometricConstraintAnalysis}
            />
          )}
          {validation && (
            <section className={validation.is_valid ? 'validation-report valid' : 'validation-report invalid'}>
              <h2>{text({ ja: '幾何検証', en: 'Geometry validation' })}</h2>
              {validation.is_valid ? (
                <p>
                  {text({
                    ja: '問題は見つかりませんでした。',
                    en: 'No issues were found.',
                  })}
                </p>
              ) : (
                <>
                  <p>
                    {formattedText({
                      ja: '{count}件の問題が見つかりました。',
                      en: '{count} issues were found.',
                    }, { count: validation.issues.length })}
                  </p>
                  <ul>
                    {validation.issues.slice(0, 20).map((issue, index) => {
                      const edgeId = issue.edges.find((id) =>
                        nativeLines.some((line) => line.id === id))
                      const vertexId = issue.vertices.find((id) =>
                        nativeSnapshot?.crease_pattern.vertices.some((vertex) => vertex.id === id))
                      const label = validationIssueLabel(issue.code, locale)
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
              <h2>
                {text({
                  ja: '局所平坦折り条件',
                  en: 'Local flat-foldability conditions',
                })}
              </h2>
              <p
                id="local-flat-foldability-summary"
                className="local-flat-foldability-summary"
                role="status"
                aria-live="polite"
                aria-atomic="true"
              >
                {localizedLocalFlatFoldabilitySummary(
                  localFlatFoldabilityPresentation,
                  locale,
                )}
              </p>
              {localFlatFoldabilityPresentation.maxExactFoldDegree !== null && (
                <p className="local-flat-foldability-coverage">
                  {formattedText({
                    ja: '対応範囲: 紙内部の単一頂点・ゼロ厚モデル、折り線次数{degree}以下',
                    en: 'Coverage: a single interior vertex, zero-thickness model, fold degree {degree} or less',
                  }, {
                    degree: localFlatFoldabilityPresentation.maxExactFoldDegree,
                  })}
                </p>
              )}
              {localFlatFoldabilityPresentation.kind === 'ready' && (
                <>
                  <ul
                    className="local-flat-foldability-counts"
                    aria-label={text({
                      ja: '局所平坦折り条件の頂点別件数',
                      en: 'Vertex counts by local flat-foldability result',
                    })}
                  >
                    {([
                      [
                        'satisfied',
                        { ja: '成立', en: 'Satisfied' },
                        localFlatFoldabilityPresentation.counts.satisfied,
                      ],
                      [
                        'violated',
                        { ja: '不成立', en: 'Violated' },
                        localFlatFoldabilityPresentation.counts.violated,
                      ],
                      [
                        'not-applicable',
                        { ja: '対象外', en: 'Not applicable' },
                        localFlatFoldabilityPresentation.counts.notApplicable,
                      ],
                      [
                        'indeterminate',
                        { ja: '判定不能', en: 'Indeterminate' },
                        localFlatFoldabilityPresentation.counts.indeterminate,
                      ],
                    ] as const).map(([kind, label, count]) => (
                      <li key={kind} className={`is-${kind}`}>
                        <span>{text(label)}</span>
                        <strong>{count.toLocaleString(locale)}</strong>
                      </li>
                    ))}
                  </ul>
                  {selectedLocalFlatFoldability && (
                    <div className="selected-local-flat-foldability">
                      <h3>
                        {text({
                          ja: '選択頂点の局所条件',
                          en: 'Local conditions for selected vertex',
                        })}
                      </h3>
                      <dl>
                        <div>
                          <dt>{text({ ja: '総合', en: 'Overall' })}</dt>
                          <dd>
                            {localizedLocalFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.verdict,
                              locale,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>
                            {text({
                              ja: '川崎条件',
                              en: 'Kawasaki condition',
                            })}
                          </dt>
                          <dd>
                            {localizedLocalFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.kawasaki,
                              locale,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>
                            {text({
                              ja: '前川条件',
                              en: 'Maekawa condition',
                            })}
                          </dt>
                          <dd>
                            {localizedLocalFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.maekawa,
                              locale,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>{text({ ja: '折り線次数', en: 'Fold degree' })}</dt>
                          <dd>{selectedLocalFlatFoldability.foldDegree}</dd>
                        </div>
                        <div>
                          <dt>
                            {text({
                              ja: '山折り / 谷折り',
                              en: 'Mountain / valley',
                            })}
                          </dt>
                          <dd>
                            {selectedLocalFlatFoldability.mountainCount}
                            {' / '}
                            {selectedLocalFlatFoldability.valleyCount}
                          </dd>
                        </div>
                      </dl>
                      {selectedLocalFlatFoldability.reason && (
                        <p className="local-flat-foldability-reason">
                          {localizedLocalFlatFoldabilityReasonLabel(
                            selectedLocalFlatFoldability.reason,
                            localFlatFoldabilityPresentation.maxExactFoldDegree,
                            locale,
                          )}
                        </p>
                      )}
                    </div>
                  )}
                  {localFlatFoldabilityPresentation.visibleItems.length > 0 && (
                    <>
                      <h3>
                        {text({
                          ja: '確認が必要な頂点',
                          en: 'Vertices requiring review',
                        })}
                      </h3>
                      <ul className="local-flat-foldability-items">
                        {localFlatFoldabilityPresentation.visibleItems.map((item) => {
                          const verdictLabel =
                            localizedLocalFlatFoldabilityConditionLabel(
                              item.verdict,
                              locale,
                            )
                          const reasonLabel = localizedLocalFlatFoldabilityReasonLabel(
                            item.reason,
                            localFlatFoldabilityPresentation.maxExactFoldDegree,
                            locale,
                          )
                          return (
                            <li key={item.vertexId}>
                              <button
                                type="button"
                                aria-pressed={selectedVertexId === item.vertexId}
                                aria-label={formattedText({
                                  ja: '頂点{ordinal}、局所必要条件{verdict}。川崎条件{kawasaki}、前川条件{maekawa}。{reason}',
                                  en: 'Vertex {ordinal}: local necessary condition {verdict}. Kawasaki condition {kawasaki}; Maekawa condition {maekawa}. {reason}',
                                }, {
                                  ordinal: item.ordinal,
                                  verdict: verdictLabel,
                                  kawasaki:
                                    localizedLocalFlatFoldabilityConditionLabel(
                                      item.kawasaki,
                                      locale,
                                    ),
                                  maekawa:
                                    localizedLocalFlatFoldabilityConditionLabel(
                                      item.maekawa,
                                      locale,
                                    ),
                                  reason: reasonLabel,
                                })}
                                onClick={() => {
                                  setSelectedVertexId(item.vertexId)
                                  setSelectedLineId(null)
                                }}
                              >
                                <span className={`local-verdict is-${item.verdict}`}>
                                  {verdictLabel}
                                </span>
                                <span>
                                  {formattedText({
                                    ja: '頂点 {ordinal}',
                                    en: 'Vertex {ordinal}',
                                  }, { ordinal: item.ordinal })}
                                </span>
                                <span className="local-flat-foldability-item-detail">
                                  {reasonLabel || (
                                    formattedText({
                                      ja: '川崎 {kawasaki}・前川 {maekawa}',
                                      en: 'Kawasaki {kawasaki} · Maekawa {maekawa}',
                                    }, {
                                      kawasaki:
                                        localizedLocalFlatFoldabilityConditionLabel(
                                          item.kawasaki,
                                          locale,
                                        ),
                                      maekawa:
                                        localizedLocalFlatFoldabilityConditionLabel(
                                          item.maekawa,
                                          locale,
                                        ),
                                    })
                                  )}
                                </span>
                              </button>
                            </li>
                          )
                        })}
                      </ul>
                      {localFlatFoldabilityPresentation.hiddenItemCount > 0 && (
                        <p className="muted">
                          {formattedText({
                            ja: 'ほか{count}頂点。頂点を選択すると個別結果を確認できます。',
                            en: '{count} more vertices. Select a vertex to review its result.',
                          }, {
                            count:
                              localFlatFoldabilityPresentation.hiddenItemCount
                                .toLocaleString(locale),
                          })}
                        </p>
                      )}
                    </>
                  )}
                </>
              )}
              <p className="local-flat-foldability-disclaimer">
                {text({
                  ja: '成立はこのモデルで確認した局所必要条件だけを表します。展開図全体が平坦に折り畳めることや、実際の折り経路は保証しません。',
                  en: 'Satisfied means only that the local necessary conditions were verified by this model. It does not guarantee that the entire pattern can fold flat or that a physical folding path exists.',
                })}
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
            <h2>{text({ ja: '紙', en: 'Paper' })}</h2>
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
                <label htmlFor="paper-thickness-mm">
                  {text({ ja: '厚さ', en: 'Thickness' })}
                </label>
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
                <span>{lengthDisplayUnitLabelText}</span>
              </div>
              <div className="paper-color-fields">
                <label className="paper-color-field">
                  <span>{text({ ja: '表色', en: 'Front color' })}</span>
                  <input
                    name="front_color"
                    type="color"
                    defaultValue={rgbaToHex(nativeSnapshot?.paper.front.color, '#ffffff')}
                    disabled={coreBusy || !nativeSnapshot}
                  />
                </label>
                <label className="paper-color-field">
                  <span>{text({ ja: '裏色', en: 'Back color' })}</span>
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
                {text({ ja: '切断を許可', en: 'Allow cutting' })}
              </label>
              <div className="property-actions">
                <button type="submit" disabled={coreBusy || !nativeSnapshot}>
                  {text({
                    ja: '紙設定を更新',
                    en: 'Update paper settings',
                  })}
                </button>
              </div>
            </form>
            <div className="paper-size-editor">
              <h3>{text({ ja: '用紙サイズ', en: 'Paper size' })}</h3>
              <form
                key={paperResizeFormKey}
                className="paper-size-form"
                onSubmit={submitPaperResize}
                noValidate
              >
                <div className="paper-size-fields">
                  <label className="field">
                    <span>{text({ ja: '幅', en: 'Width' })}</span>
                    <LengthValueInput
                      name="width_display"
                      minimumMillimetres={0}
                      initialMillimetres={rectangularPaperSize?.width ?? 0}
                      unit={lengthDisplayUnit}
                      readOnly={rectangularRatioReferenceAxis === 'width'}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      ariaLabel={formattedText({
                        ja: '用紙の幅 ({unit})',
                        en: 'Paper width ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                    <span>{lengthDisplayUnitLabelText}</span>
                  </label>
                  <label className="field">
                    <span>{text({ ja: '高さ', en: 'Height' })}</span>
                    <LengthValueInput
                      name="height_display"
                      minimumMillimetres={0}
                      initialMillimetres={rectangularPaperSize?.height ?? 0}
                      unit={lengthDisplayUnit}
                      readOnly={rectangularRatioReferenceAxis === 'height'}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      ariaLabel={formattedText({
                        ja: '用紙の高さ ({unit})',
                        en: 'Paper height ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                    <span>{lengthDisplayUnitLabelText}</span>
                  </label>
                </div>
                {!rectangularPaperSize && (
                  <p className="paper-size-note">
                    {text({
                      ja: '軸平行な長方形として判定できない紙は、この画面ではサイズ変更できません。',
                      en: 'Paper that is not recognized as an axis-aligned rectangle cannot be resized here.',
                    })}
                  </p>
                )}
                <p className="paper-size-note">
                  {text({
                    ja: 'サイズ変更時は、折り線を含むすべての頂点を左上基準で比例変換します。',
                    en: 'Resizing proportionally transforms every vertex, including fold lines, from the top-left origin.',
                  })}
                </p>
                <CreationDimensionExpressionSummary
                  key={nativeSnapshot?.project_id ?? 'no-project'}
                  binding={creationDimensionExpression}
                />
                {rectangularRatioReferenceAxis && (
                  <p className="paper-size-note">
                    {formattedText({
                      ja: '紙辺比では基準辺と平行な{axis}は 1 のまま読み取り専用です。直交する寸法だけを変更し、基準辺の物理長は維持します。',
                      en: 'For a paper-edge ratio, {axis} remains read-only at 1. Only the perpendicular dimension changes, preserving the physical length of the reference edge.',
                    }, {
                      axis: rectangularRatioReferenceAxis === 'width'
                        ? text({ ja: '幅', en: 'width' })
                        : text({ ja: '高さ', en: 'height' }),
                    })}
                  </p>
                )}
                <div className="property-actions">
                  <button
                    type="submit"
                    disabled={coreBusy || !nativeSnapshot || !rectangularPaperSize}
                  >
                    {text({
                      ja: '用紙サイズを変更',
                      en: 'Resize paper',
                    })}
                  </button>
                </div>
              </form>
            </div>
          </section>
          <section>
            <h2>{text({ ja: '編集履歴', en: 'Edit history' })}</h2>
            {boundHistoryLimitSettings && nativeSnapshot ? (
              <HistoryLimitControl
                settings={boundHistoryLimitSettings}
                expectedProjectInstanceId={nativeSnapshot.project_instance_id}
                expectedProjectId={nativeSnapshot.project_id}
                expectedRevision={nativeSnapshot.revision}
                disabled={coreBusy || recoveryBlocking}
                onApplied={acceptAppliedHistoryLimit}
              />
            ) : historyLimitLoadState.kind === 'failed' ? (
              <div role="alert">
                <p>
                  {text({
                    ja: 'Undo・Redo履歴の上限を確認できませんでした。',
                    en: 'The undo/redo history limit could not be checked.',
                  })}
                </p>
                <button
                  type="button"
                  disabled={coreBusy || recoveryBlocking}
                  onClick={() => setHistoryLimitRetrySequence(
                    (sequence) => sequence + 1,
                  )}
                >
                  {text({ ja: '再試行', en: 'Retry' })}
                </button>
              </div>
            ) : historyLimitLoadState.kind === 'unavailable' ? (
              <p className="muted">
                {text({
                  ja: '履歴上限の設定はデスクトップ版で利用できます。',
                  en: 'History limit settings are available in the desktop app.',
                })}
              </p>
            ) : (
              <p className="muted" role="status" aria-live="polite">
                {text({
                  ja: '履歴上限を確認しています…',
                  en: 'Checking history limit…',
                })}
              </p>
            )}
          </section>
          <section>
            <h2>{text({ ja: 'スナップ', en: 'Snap' })}</h2>
            <div
              className="chip-row"
              aria-label={text({ ja: 'スナップ設定', en: 'Snap settings' })}
            >
              {SNAP_OPTIONS.map(({ kind, label }) => (
                <button
                  key={kind}
                  type="button"
                  className={`chip${snapSettings[kind] ? ' active' : ''}`}
                  aria-pressed={snapSettings[kind]}
                  disabled={coreBusy}
                  onClick={() => setSnapSettings((current) => toggleSnapSetting(current, kind))}
                >
                  {text(label)}
                </button>
              ))}
            </div>
            <div className="angle-snap-settings">
              <h3>{text({ ja: '角度スナップ', en: 'Angle snap' })}</h3>
              <label className="angle-snap-field">
                <span>{text({ ja: 'プリセット', en: 'Preset' })}</span>
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
                  <option value="custom">
                    {text({ ja: '任意角', en: 'Custom angle' })}
                  </option>
                </select>
              </label>
              <label className="angle-snap-field">
                <span>{text({ ja: '角度', en: 'Angle' })}</span>
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
                  {text({
                    ja: '角度は0より大きく90以下で入力してください。最後の正常値を使用します。',
                    en: 'Enter an angle greater than 0 and no more than 90. The last valid value will be used.',
                  })}
                </p>
              )}
              <div className="angle-reference-setting">
                <span>{text({ ja: '基準', en: 'Reference' })}</span>
                <div
                  className="chip-row"
                  role="group"
                  aria-label={text({
                    ja: '角度スナップの基準',
                    en: 'Angle snap reference',
                  })}
                >
                  <button
                    type="button"
                    className={`chip${angleReferenceKind === 'global-horizontal' ? ' active' : ''}`}
                    aria-pressed={angleReferenceKind === 'global-horizontal'}
                    disabled={coreBusy}
                    onClick={() => setAngleReferenceKind('global-horizontal')}
                  >
                    {text({ ja: '水平', en: 'Horizontal' })}
                  </button>
                  <button
                    type="button"
                    className={`chip${angleReferenceKind === 'edge' ? ' active' : ''}`}
                    aria-pressed={angleReferenceKind === 'edge'}
                    disabled={coreBusy}
                    onClick={() => setAngleReferenceKind('edge')}
                  >
                    {text({
                      ja: '方向参照辺',
                      en: 'Direction reference edge',
                    })}
                  </button>
                </div>
              </div>
              <p className="muted">
                {formattedText({
                  ja: '現在: {angle}°・{reference}',
                  en: 'Current: {angle}° · {reference}',
                }, {
                  angle: formatAngleDegrees(angleDegrees),
                  reference: angleReferenceKind === 'global-horizontal'
                    ? text({ ja: '水平基準', en: 'horizontal reference' })
                    : text({
                        ja: '方向参照辺基準',
                        en: 'direction edge reference',
                      }),
                })}
              </p>
              {snapSettings.angle && angleReferenceKind === 'edge' && !parallelReferenceLine && (
                <p className="field-error" role="status">
                  {text({
                    ja: '線を選択して方向参照に設定してください。暗黙に水平基準へは切り替えません。',
                    en: 'Select a line and set it as the direction reference. The app will not silently switch to horizontal.',
                  })}
                </p>
              )}
            </div>
            {parallelReferenceLine ? (
              <div className="property-actions">
                <span className="muted" title={parallelReferenceLine.id}>
                  {formattedText({
                    ja: '方向参照（平行・角度）: {kind}',
                    en: 'Direction reference (parallel and angle): {kind}',
                  }, {
                    kind: lineKindLabel(parallelReferenceLine.kind, locale),
                  })}
                </span>
                <button
                  type="button"
                  disabled={coreBusy}
                  onClick={() => setParallelReferenceEdgeId(null)}
                >
                  {text({ ja: '参照を解除', en: 'Clear reference' })}
                </button>
              </div>
            ) : (
              <p className="muted">
                {text({
                  ja: '線を選択して「方向参照に設定」を押すと、平行・角度スナップの基準にできます。',
                  en: 'Select a line and choose “Set as direction reference” to use it for parallel and angle snapping.',
                })}
              </p>
            )}
          </section>
        </aside>
      </section>

      <div className="workspace-timeline-separator" inert={modalOpen}>
        <WorkspaceLayoutSeparator kind="timeline" />
      </div>

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

      {(recoveryStartup.kind === 'checking'
        || recoveryStartup.kind === 'failed') && (
        <RecoveryStartupOverlay
          phase={recoveryStartup.kind}
          busy={recoveryActionBusy}
          onRetry={retryRecoveryStartup}
        />
      )}

      {recoveryStartup.kind === 'candidate' && (
        <RecoveryDialog
          key={`${recoveryStartup.candidate.status}:${recoveryStartup.candidate.recovery_id}`}
          candidate={recoveryStartup.candidate}
          busy={recoveryActionBusy}
          error={recoveryActionError}
          onRestore={restoreStartupRecovery}
          onDiscard={discardStartupRecovery}
          onRetry={retryRecoveryStartup}
        />
      )}

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
                <span className="dialog-eyebrow">
                  {text({
                    ja: '一枚紙から開始',
                    en: 'Start from one sheet',
                  })}
                </span>
                <h2 id="new-project-title">
                  {text({
                    ja: '新しいプロジェクト',
                    en: 'New project',
                  })}
                </h2>
              </div>
              <button
                type="button"
                className="dialog-close"
                disabled={coreBusy}
                onClick={() => {
                  setNewProjectOpen(false)
                  setNewProjectError(null)
                }}
                aria-label={text({ ja: '閉じる', en: 'Close' })}
              >
                ×
              </button>
            </header>
            <form onSubmit={submitNewProject} noValidate>
              <label className="dialog-field dialog-field-wide">
                <span>{text({ ja: '作品名', en: 'Project name' })}</span>
                <input
                  name="name"
                  defaultValue={text({
                    ja: '無題の作品',
                    en: 'Untitled work',
                  })}
                  maxLength={120}
                  required
                  autoFocus
                  disabled={coreBusy}
                />
              </label>

              <fieldset>
                <legend>{text({ ja: '用紙サイズ', en: 'Paper size' })}</legend>
                <div className="dialog-grid two-columns">
                  <label className="dialog-field">
                    <span>{text({ ja: '幅', en: 'Width' })}</span>
                    <NumericExpressionInput
                      id="new-project-width-expression"
                      name="width_expression"
                      defaultSource="400"
                      disabled={coreBusy}
                      ariaLabel={text({
                        ja: '用紙の幅の式 (mm)',
                        en: 'Paper width expression (mm)',
                      })}
                    />
                  </label>
                  <label className="dialog-field">
                    <span>{text({ ja: '高さ', en: 'Height' })}</span>
                    <NumericExpressionInput
                      id="new-project-height-expression"
                      name="height_expression"
                      defaultSource="400"
                      disabled={coreBusy}
                      ariaLabel={text({
                        ja: '用紙の高さの式 (mm)',
                        en: 'Paper height expression (mm)',
                      })}
                    />
                  </label>
                </div>
              </fieldset>

              <fieldset>
                <legend>
                  {text({ ja: '材料設定', en: 'Material settings' })}
                </legend>
                <div className="dialog-grid three-columns">
                  <div className="dialog-field">
                    <label htmlFor="new-project-paper-thickness-mm">
                      {text({ ja: '紙厚', en: 'Paper thickness' })}
                    </label>
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
                    <span>{text({ ja: '表色', en: 'Front color' })}</span>
                    <input
                      name="front_color"
                      type="color"
                      defaultValue="#ffffff"
                      disabled={coreBusy}
                    />
                  </label>
                  <label className="dialog-field color-field">
                    <span>{text({ ja: '裏色', en: 'Back color' })}</span>
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
                  {text({
                    ja: 'この作品で切断線の作成を許可する',
                    en: 'Allow cut lines in this project',
                  })}
                </label>
              </fieldset>

              <p className="dialog-note">
                {text({
                  ja: '左上を (0, 0) mm とする長方形の用紙と、4本の輪郭線を作成します。',
                  en: 'Creates rectangular paper with its top-left at (0, 0) mm and four boundary edges.',
                })}
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
                  {text({ ja: 'キャンセル', en: 'Cancel' })}
                </button>
                <button type="submit" className="primary" disabled={coreBusy}>
                  {coreBusy
                    ? text({ ja: '作成中…', en: 'Creating…' })
                    : text({ ja: '作成', en: 'Create' })}
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
        <span>
          {formattedText({
            ja: 'ツール: {tool}',
            en: 'Tool: {tool}',
          }, {
            tool: benchmarkRun
              ? text({
                  ja: '性能テスト選択',
                  en: 'Benchmark selection',
                })
              : toolLabel(activeTool, locale),
          })}
        </span>
        <span>{coreStatus}</span>
        <span>
          {formattedText({
            ja: 'スナップ: {status}',
            en: 'Snap: {status}',
          }, { status: snapStatusLabel })}
        </span>
        <span className="status-spacer" />
        <KeyboardShortcutControl />
        <WorkspaceLayoutControl />
        <ThemeControl />
        <LanguageControl />
        {isDiagnosticsShareAvailable() && (
          <button
            ref={diagnosticsButtonRef}
            type="button"
            className="diagnostics-button"
            aria-haspopup="dialog"
            onClick={() => setDiagnosticsDialogOpen(true)}
          >
            {text({ ja: '診断情報', en: 'Diagnostics' })}
          </button>
        )}
        <button
          type="button"
          className="benchmark-button"
          disabled={coreBusy || benchmarkLoading}
          onClick={() => void toggleBenchmark()}
        >
          {benchmarkLoading
            ? text({ ja: '読込中…', en: 'Loading…' })
            : benchmarkRun
              ? text({
                  ja: '通常図へ戻る',
                  en: 'Return to normal pattern',
                })
              : text({
                  ja: '10,000本テスト',
                  en: '10,000-edge test',
                })}
        </button>
        <span className="benchmark-status" aria-live="polite" title={benchmarkStatus}>
          {benchmarkStatus}
        </span>
      </footer>
    </main>
  )
}

function sameRecoveryCandidate(
  state: RecoveryStartupState,
  candidate: RecoveryCandidateAvailable | RecoveryCandidateInvalid,
): boolean {
  if (
    state.kind !== 'candidate'
    || state.candidate.status !== candidate.status
    || state.candidate.recovery_id !== candidate.recovery_id
  ) return false
  if (
    state.candidate.status === 'available'
    && candidate.status === 'available'
  ) {
    return state.candidate.project_id === candidate.project_id
      && state.candidate.updated_at_unix_ms === candidate.updated_at_unix_ms
  }
  return state.candidate.status === 'invalid'
    && candidate.status === 'invalid'
}

function lineKindLabel(kind: CreaseLine['kind'], locale: Locale) {
  const labels: Readonly<Record<CreaseLine['kind'], LocalizedText>> = {
    mountain: { ja: '山折り', en: 'Mountain fold' },
    valley: { ja: '谷折り', en: 'Valley fold' },
    auxiliary: { ja: '補助線', en: 'Auxiliary line' },
    boundary: { ja: '輪郭線', en: 'Boundary edge' },
    cut: { ja: '切断線', en: 'Cut line' },
  }
  return selectLocalizedText(locale, labels[kind])
}

function normalizeFoldAngle(value: number) {
  if (!Number.isFinite(value)) return null
  return Math.min(180, Math.max(0, value))
}

function formatBytes(bytes: number, locale: Locale) {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return selectLocalizedText(locale, {
      ja: 'サイズ不明',
      en: 'Unknown size',
    })
  }
  if (bytes < 1_000) return `${bytes} B`
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`
  return `${(bytes / 1_000_000).toFixed(2)} MB`
}

function toolLabel(tool: string, locale: Locale) {
  const labels: Readonly<Record<string, LocalizedText>> = {
    select: { ja: '選択', en: 'Select' },
    vertex: { ja: '頂点', en: 'Vertex' },
    mountain: { ja: '山折り', en: 'Mountain fold' },
    valley: { ja: '谷折り', en: 'Valley fold' },
    auxiliary: { ja: '補助線', en: 'Auxiliary line' },
    cut: { ja: '切断', en: 'Cut' },
    measure: { ja: '計測', en: 'Measure' },
  }
  const label = labels[tool]
  return label
    ? selectLocalizedText(locale, label)
    : selectLocalizedText(locale, {
        ja: '不明なツール',
        en: 'Unknown tool',
      })
}

function validationIssueLabel(code: string, locale: Locale) {
  const labels: Readonly<Record<string, LocalizedText>> = {
    non_finite_vertex: {
      ja: '有限でない頂点座標',
      en: 'Non-finite vertex coordinates',
    },
    duplicate_vertex: {
      ja: '同じ位置の重複頂点',
      en: 'Duplicate vertices at the same position',
    },
    missing_endpoint: {
      ja: '存在しない端点を参照する線',
      en: 'Line references a missing endpoint',
    },
    zero_length_edge: { ja: '長さ0の線', en: 'Zero-length line' },
    unsplit_intersection: {
      ja: '分割されていない交差・重なり',
      en: 'Unsplit intersection or overlap',
    },
    intersection_calculation_failed: {
      ja: '交差計算に失敗',
      en: 'Intersection calculation failed',
    },
    non_finite_thickness: {
      ja: '紙の厚さが有限値ではありません',
      en: 'Paper thickness is not finite',
    },
    negative_thickness: {
      ja: '紙の厚さは0 mm以上にする必要があります',
      en: 'Paper thickness must be at least 0 mm',
    },
    too_few_boundary_vertices: {
      ja: '紙の輪郭には3つ以上の頂点が必要です',
      en: 'Paper boundary needs at least three vertices',
    },
    duplicate_boundary_vertex: {
      ja: '紙の輪郭に同じ頂点が重複しています',
      en: 'Paper boundary contains a duplicate vertex',
    },
    missing_boundary_vertex: {
      ja: '紙の輪郭が存在しない頂点を参照しています',
      en: 'Paper boundary references a missing vertex',
    },
    non_finite_boundary_vertex: {
      ja: '紙の輪郭頂点の座標が有限値ではありません',
      en: 'Paper boundary vertex coordinates are not finite',
    },
    missing_boundary_edge: {
      ja: '紙の輪郭線が不足しています',
      en: 'Paper boundary edges are missing',
    },
    duplicate_boundary_edge: {
      ja: '紙の輪郭線が重複しています',
      en: 'Paper boundary contains a duplicate edge',
    },
    unexpected_boundary_edge: {
      ja: '紙の輪郭に余分な輪郭線があります',
      en: 'Paper boundary contains an unexpected edge',
    },
    zero_length_boundary_edge: {
      ja: '紙の輪郭に長さ0の辺があります',
      en: 'Paper boundary contains a zero-length edge',
    },
    boundary_self_intersection: {
      ja: '紙の輪郭が自己交差しています',
      en: 'Paper boundary intersects itself',
    },
    boundary_intersection_calculation_failed: {
      ja: '紙の輪郭の交差判定に失敗しました',
      en: 'Paper boundary intersection test failed',
    },
    zero_area_boundary: {
      ja: '紙の輪郭の面積が0です',
      en: 'Paper boundary has zero area',
    },
    boundary_area_calculation_failed: {
      ja: '紙の輪郭の面積計算に失敗しました',
      en: 'Paper boundary area calculation failed',
    },
  }
  const label = labels[code]
  return label
    ? selectLocalizedText(locale, label)
    : selectLocalizedText(locale, {
        ja: '不明な幾何検証問題',
        en: 'Unknown geometry validation issue',
      })
}

function localFlatFoldabilityCoreStatus(
  presentation: LocalFlatFoldabilityPresentation,
  locale: Locale,
) {
  if (presentation.kind === 'invalid') {
    return selectLocalizedText(locale, {
      ja: '局所判定結果を確認不能',
      en: 'Local result unavailable',
    })
  }
  if (presentation.kind === 'blocked') {
    return selectLocalizedText(locale, {
      ja: '局所判定を前段の幾何問題で遮断',
      en: 'Local analysis blocked by geometry issues',
    })
  }
  if (presentation.reportStatus === 'necessary_conditions_satisfied') {
    return formatLocalizedText(locale, {
      ja: '局所必要条件が{count}頂点で成立',
      en: 'Local necessary conditions satisfied at {count} vertices',
    }, { count: presentation.counts.satisfied })
  }
  if (presentation.reportStatus === 'not_applicable') {
    return selectLocalizedText(locale, {
      ja: '局所判定の対象頂点なし',
      en: 'No vertices eligible for local analysis',
    })
  }
  if (presentation.reportStatus === 'violated') {
    return formatLocalizedText(locale, {
      ja: '局所必要条件に不成立{count}頂点',
      en: 'Local necessary conditions violated at {count} vertices',
    }, { count: presentation.counts.violated })
  }
  return formatLocalizedText(locale, {
    ja: '局所判定不能{count}頂点',
    en: 'Local result indeterminate at {count} vertices',
  }, { count: presentation.counts.indeterminate })
}

function localizedLocalFlatFoldabilityConditionLabel(
  condition: Parameters<typeof localFlatFoldabilityConditionLabel>[0],
  locale: Locale,
) {
  if (locale === 'ja') return localFlatFoldabilityConditionLabel(condition)
  return {
    satisfied: 'Satisfied',
    violated: 'Violated',
    not_applicable: 'Not applicable',
    indeterminate: 'Indeterminate',
  }[condition]
}

function localizedLocalFlatFoldabilityReasonLabel(
  reason: Parameters<typeof localFlatFoldabilityReasonLabel>[0],
  maxExactFoldDegree: number,
  locale: Locale,
) {
  if (locale === 'ja') {
    return localFlatFoldabilityReasonLabel(reason, maxExactFoldDegree)
  }
  switch (reason) {
    case 'paper_boundary':
      return 'Paper boundary vertices are outside the current local model.'
    case 'cut_incident':
      return 'Vertices incident to a cut line are outside the current local model.'
    case 'fold_degree_limit':
      return formatLocalizedText(locale, {
        ja: '',
        en: 'Indeterminate because the fold degree exceeds the exact limit ({limit}).',
      }, { limit: maxExactFoldDegree })
    case 'no_incident_fold_edges':
      return 'Not applicable because there are no incident mountain or valley folds.'
    case null:
      return ''
  }
}

function localizedLocalFlatFoldabilitySummary(
  presentation: LocalFlatFoldabilityPresentation,
  locale: Locale,
) {
  if (presentation.kind === 'invalid') {
    return selectLocalizedText(locale, {
      ja: '局所平坦折り条件の結果を確認できませんでした。成立とは扱いません。',
      en: 'The local flat-foldability result could not be verified and is not treated as satisfied.',
    })
  }
  if (presentation.kind === 'blocked') {
    return selectLocalizedText(locale, {
      ja: '前段の幾何構造に問題があるため、局所平坦折り条件は判定していません。',
      en: 'Local flat-foldability was not evaluated because the preceding geometry has issues.',
    })
  }
  const detail = formatLocalizedText(locale, {
    ja: '成立{satisfied}、不成立{violated}、対象外{notApplicable}、判定不能{indeterminate}',
    en: 'satisfied {satisfied}, violated {violated}, not applicable {notApplicable}, indeterminate {indeterminate}',
  }, {
    satisfied: presentation.counts.satisfied,
    violated: presentation.counts.violated,
    notApplicable: presentation.counts.notApplicable,
    indeterminate: presentation.counts.indeterminate,
  })
  switch (presentation.reportStatus) {
    case 'necessary_conditions_satisfied':
      return formatLocalizedText(locale, {
        ja: '対応範囲内の局所必要条件が成立しました（{detail}）。',
        en: 'Local necessary conditions are satisfied within the supported scope ({detail}).',
      }, { detail })
    case 'not_applicable':
      return formatLocalizedText(locale, {
        ja: '現在の局所条件を適用できる頂点がありません（{detail}）。',
        en: 'No vertices are eligible for the current local conditions ({detail}).',
      }, { detail })
    case 'violated':
      return formatLocalizedText(locale, {
        ja: '局所必要条件に不成立の頂点があります（{detail}）。',
        en: 'Some vertices violate the local necessary conditions ({detail}).',
      }, { detail })
    case 'indeterminate':
      return formatLocalizedText(locale, {
        ja: '局所必要条件を判定できない頂点があります（{detail}）。',
        en: 'Some vertices have indeterminate local necessary conditions ({detail}).',
      }, { detail })
  }
}

function localizedCreaseExportFormatLabel(
  format: CreasePatternExportFormat,
  locale: Locale,
) {
  if (locale === 'ja') return creasePatternExportFormatLabel(format)
  return format === 'dxf'
    ? 'DXF (AutoCAD 2007)'
    : creasePatternExportFormatLabel(format)
}

function localizedInstructionExportFormatLabel(
  format: InstructionExportFormat,
  locale: Locale,
) {
  if (locale === 'ja') return instructionExportFormatLabel(format)
  return format === 'svg_zip'
    ? 'SVG images ZIP'
    : instructionExportFormatLabel(format)
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
  locale: Locale = 'ja',
) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return selectLocalizedText(locale, {
      ja: '計測不可',
      en: 'Unavailable',
    })
  }
  const normalized = Object.is(value, -0) ? 0 : value
  return `${normalized.toLocaleString(
    locale === 'ja' ? 'ja-JP' : 'en-US',
    { maximumFractionDigits },
  )}${unit}`
}

function formatAngleDegrees(value: number) {
  if (!Number.isFinite(value)) return '—'
  if (value !== 0 && Math.abs(value) < 0.000001) return value.toExponential(3)
  return String(Number(value.toFixed(6)))
}

function formatLineMeasurementLabel(
  measurement: LineMeasurement | null,
  unit: ReturnType<typeof resolveLengthDisplayUnit>,
  locale: Locale,
) {
  if (!measurement) {
    return selectLocalizedText(locale, {
      ja: '計測不可',
      en: 'Unavailable',
    })
  }
  return `${formatLength(measurement.length, unit, locale)} / ${
    formatMeasurementValue(measurement.angleDegrees, '°', 2, locale)
  }`
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

function newProjectExpressionErrorMessage(
  error: unknown,
  locale: Locale,
) {
  const category = numericExpressionNativeErrorCategory(error)
  if (!category) return null
  switch (category) {
    case 'invalid_request':
      return selectLocalizedText(locale, {
        ja: '幅または高さの式が空か、入力上限を超えています。',
        en: 'The width or height expression is empty or exceeds an input limit.',
      })
    case 'invalid_expression':
      return selectLocalizedText(locale, {
        ja: '幅または高さの式を解釈できません。',
        en: 'The width or height expression could not be parsed.',
      })
    case 'resource_limit':
      return selectLocalizedText(locale, {
        ja: '幅または高さの式が複雑すぎるため評価を中止しました。',
        en: 'Evaluation stopped because the width or height expression is too complex.',
      })
    case 'result_out_of_range':
      return selectLocalizedText(locale, {
        ja: '幅または高さを正のmm値として安全に採用できません。',
        en: 'The width or height cannot be safely used as a positive millimetre value.',
      })
    case 'native_unavailable':
      return selectLocalizedText(locale, {
        ja: '式を使った新規作成はデスクトップ版で利用できます。',
        en: 'Creating a project from expressions is available in the desktop app.',
      })
    case 'invalid_response':
    case 'stale_response':
    case 'internal_failure':
      return selectLocalizedText(locale, {
        ja: '幅または高さの評価結果を採用できませんでした。',
        en: 'The evaluated width or height result could not be used.',
      })
  }
}

function isEditingText(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.matches('input, textarea')) return true
  return target.isContentEditable || Boolean(target.closest('[contenteditable="true"]'))
}

export default App
