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
  type CreaseCanvasFace,
  type CreaseCanvasRenderMetrics,
  type CreaseLine,
  type PaperBounds,
  type PaperPolygonPoint,
} from './components/CreaseCanvas'
import { CreaseExportDialog } from './components/CreaseExportDialog'
import { CreationDimensionExpressionSummary } from './components/CreationDimensionExpressionSummary'
import { DiagnosticsDialog } from './components/DiagnosticsDialog'
import { FoldImportDialog } from './components/FoldImportDialog'
import { Fold3dFramesLauncher } from './components/Fold3dFramesLauncher'
import { FoldPreview } from './components/FoldPreview'
import { FoldTechniqueEditorDialog } from './components/FoldTechniqueEditorDialog'
import { FoldTechniqueTimelinePreviewDialog } from './components/FoldTechniqueTimelinePreviewDialog'
import { GeometricConstraintPanel } from './components/GeometricConstraintPanel'
import { GlobalFlatFoldabilityPanel } from './components/GlobalFlatFoldabilityPanel'
import { HistoryLimitControl } from './components/HistoryLimitControl'
import { InstructionExportDialog } from './components/InstructionExportDialog'
import { InstructionTimelinePanel } from './components/InstructionTimelinePanel'
import { KeyboardShortcutControl } from './components/KeyboardShortcutControl'
import { LanguageControl } from './components/LanguageControl'
import { LengthUnitControl } from './components/LengthUnitControl'
import { MeshAnimationExportDialog } from './components/MeshAnimationExportDialog'
import { NumericExpressionInput } from './components/NumericExpressionInput'
import { ProjectLayerPanel } from './components/ProjectLayerPanel'
import { RecoveryAutosaveStatusBanner } from './components/RecoveryAutosaveStatusBanner'
import { RecoveryDialog } from './components/RecoveryDialog'
import { RecoveryStartupOverlay } from './components/RecoveryStartupOverlay'
import { StaticMeshExportDialog } from './components/StaticMeshExportDialog'
import { StackedFoldPanel } from './components/StackedFoldPanel'
import { SvgImportDialog } from './components/SvgImportDialog'
import { ThemeControl } from './components/ThemeControl'
import { UpdateCheckPopover } from './components/UpdateCheckControl'
import { WorkspaceLayoutControl } from './components/WorkspaceLayoutControl'
import { WorkspaceLayoutSeparator } from './components/WorkspaceLayoutSeparator'
import {
  addEdge,
  addGeometricConstraint,
  addEdgeOrientationConstraint,
  addConnectedVertex,
  addInstructionStep,
  addVertex,
  appendNamedTechniqueInstructionSteps,
  analyzeGeometricConstraints,
  analyzeProjectTopology,
  applyFoldImport,
  applySvgImport,
  assignEdgeToProjectLayer,
  beginInstructionExportGeneration,
  cancelCreasePatternExport,
  cancelFoldImport,
  cancelInstructionExport,
  cancelInstructionMeshAnimation,
  cancelStaticMeshExport,
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
  moveEdge,
  mirrorEdgeLeftRight,
  rotateEdgeAboutPoint,
  moveProjectLayer,
  moveVertices,
  moveVertex,
  newProject,
  openProject,
  previewCreasePatternExport,
  previewFoldImport,
  previewInstructionExport,
  previewInstructionMeshAnimation,
  previewStaticMeshExport,
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
  saveInstructionMeshAnimation,
  saveStaticMeshExport,
  setLengthDisplayUnit,
  setElementMetadata,
  splitBoundaryEdge,
  splitEdge,
  undo,
  updateProjectLayerPresentation,
  updateProjectMemo,
  updatePaperProperties,
  importFrontPaperTexture,
  importBackPaperTexture,
  type ProjectSnapshot,
  type GeometricConstraintKind,
  type ProjectTopologyResponse,
  type InstructionVisual,
  type RgbaColor,
  type ElementMetadata,
  type ElementMetadataTarget,
  type ValidationSnapshot,
  validateSvgImportSettings,
  validateProject,
} from './lib/coreClient'
import {
  isNativeProjectFolderAvailable,
  openProjectFolder,
  projectFolderClientErrorMessage,
  saveProjectFolderAs,
} from './lib/projectFolderClient'
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
import {
  type StaticMeshExportFormat,
  type StaticMeshExportPreview,
} from './lib/staticMeshExport'
import type { MeshAnimationPreviewResponse } from './lib/meshAnimationExport'
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
import {
  createProjectLayerCanvasView,
  placementTouchesLockedLayer,
} from './lib/projectLayerCanvasView'
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
import {
  builtinPaperPatternAsset,
  builtinPaperPatternFromAsset,
} from './lib/paperPatterns'
import {
  foldPreviewAppliedPoseKey,
  type FoldPreviewAppliedPoseSnapshot,
} from './lib/foldPreviewAppliedPose'
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
import { planInstructionAutoRecord } from './lib/instructionAutoRecord'
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
  type ResolvedLengthDisplayUnit,
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
  evaluateFiniteNumericExpression,
  evaluatePositiveMillimetreExpression,
  MAX_NUMERIC_EXPRESSION_SOURCE_BYTES,
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
import {
  createInitialFoldTechniqueDocumentV1,
  foldTechniqueLocalizedTextV1,
  type FoldTechniqueFileDocumentV1,
} from './lib/foldTechniqueEditor'
import {
  createFoldTechniqueTimelineProposalV1,
  type FoldTechniqueTimelineProposalPreview,
} from './lib/foldTechniqueTimelineProposal'
import {
  completeOwnedRequest,
  createOwnedRequestGate,
  ownedRequestActive,
  tryBeginOwnedRequest,
} from './lib/ownedRequestGate'
import {
  foldTechniqueFileClientErrorCode,
  isNativeFoldTechniqueFileAvailable,
  openFoldTechniqueFileV1,
  saveFoldTechniqueFileAsV1,
} from './lib/foldTechniqueFileClient'
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

type FoldTechniqueWorkspace = Readonly<{
  document: FoldTechniqueFileDocumentV1
  dirty: boolean
}>

type FoldTechniqueEditorState = Readonly<{
  mode: 'create' | 'edit'
  initialDocument: FoldTechniqueFileDocumentV1
  techniqueIndex: number
}>

type FoldTechniqueTimelinePreviewState = Readonly<{
  preview: Extract<FoldTechniqueTimelineProposalPreview, { ok: true }>
  sourceDocument: FoldTechniqueFileDocumentV1
  techniqueIndex: number
  expectedProjectInstanceId: string
  expectedProjectId: string
  expectedRevision: number
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

function foldTechniqueFileErrorAppMessage(
  error: unknown,
): AppMessage {
  switch (foldTechniqueFileClientErrorCode(error)) {
    case 'native_unavailable':
      return appMessage({
        ja: '折り技法ファイルの操作はデスクトップ版で利用できます。',
        en: 'Fold-technique file operations are available in the desktop app.',
      })
    case 'busy':
      return appMessage({
        ja: '別の折り技法ファイル操作が進行中です。完了後にもう一度お試しください。',
        en: 'Another fold-technique file operation is in progress. Try again after it finishes.',
      })
    case 'not_regular_file':
      return appMessage({
        ja: '通常ファイルではないため、安全のため処理しませんでした。',
        en: 'The selection was not processed because it is not a regular file.',
      })
    case 'too_large':
      return appMessage({
        ja: '折り技法ファイルが1 MiBの上限を超えています。',
        en: 'The fold-technique file exceeds the 1 MiB limit.',
      })
    case 'invalid_document':
      return appMessage({
        ja: '折り技法ファイルが厳格なV1形式を満たしていません。',
        en: 'The fold-technique file does not satisfy the strict V1 format.',
      })
    case 'open_failed':
    case 'read_failed':
      return appMessage({
        ja: '折り技法ファイルを安全に読み込めませんでした。',
        en: 'The fold-technique file could not be read safely.',
      })
    case 'save_failed':
      return appMessage({
        ja: '折り技法ファイルを原子的に保存できませんでした。',
        en: 'The fold-technique file could not be saved atomically.',
      })
    case 'invalid_response':
      return appMessage({
        ja: '折り技法ファイル操作の応答を検証できませんでした。',
        en: 'The fold-technique file operation response could not be verified.',
      })
  }
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
  const [selectedFaceId, setSelectedFaceId] = useState<string | null>(null)
  const [compassCircles, setCompassCircles] = useState<readonly {
    centerX: number
    centerY: number
    radius: number
  }[]>([])
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
  const [instructionVisual, setInstructionVisual] =
    useState<InstructionVisual | null>(null)
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
  const [autoRecordInstructions, setAutoRecordInstructions] = useState(false)
  const lastAutoRecordedPoseSequenceRef = useRef(0)
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
    | 'folder_open'
    | 'folder_save'
    | 'fold_import'
    | 'svg_import'
    | 'crease_export'
    | 'mesh_export'
    | 'instruction_export'
    | 'mesh_animation_export'
    | null
  >(null)
  const [coreBusy, setCoreBusy] = useState(false)
  const [newProjectOpen, setNewProjectOpen] = useState(false)
  const [newProjectErrorMessage, setNewProjectError] =
    useState<AppMessage | null>(null)
  const [diagnosticsDialogOpen, setDiagnosticsDialogOpen] = useState(false)
  const [foldTechniqueWorkspace, setFoldTechniqueWorkspace] =
    useState<FoldTechniqueWorkspace | null>(null)
  const [foldTechniqueEditor, setFoldTechniqueEditor] =
    useState<FoldTechniqueEditorState | null>(null)
  const [foldTechniqueBusy, setFoldTechniqueBusy] = useState(false)
  const [foldTechniqueSaveFailed, setFoldTechniqueSaveFailed] = useState(false)
  const [foldTechniqueSelectedIndex, setFoldTechniqueSelectedIndex] = useState(0)
  const [foldTechniqueTimelinePreview, setFoldTechniqueTimelinePreview] =
    useState<FoldTechniqueTimelinePreviewState | null>(null)
  const [foldTechniqueTimelineBusy, setFoldTechniqueTimelineBusy] =
    useState(false)
  const [foldTechniqueTimelineError, setFoldTechniqueTimelineError] =
    useState<AppMessage | null>(null)
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
  const [meshExportOpen, setMeshExportOpen] = useState(false)
  const [meshExportFormat, setMeshExportFormat] =
    useState<StaticMeshExportFormat>('obj')
  const [meshExportPreview, setMeshExportPreview] =
    useState<StaticMeshExportPreview | null>(null)
  const [meshExportErrorMessage, setMeshExportError] =
    useState<AppMessage | null>(null)
  const [meshExportNoticeMessage, setMeshExportNotice] =
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
  const [meshAnimationExportOpen, setMeshAnimationExportOpen] = useState(false)
  const [meshAnimationExportPreview, setMeshAnimationExportPreview] =
    useState<MeshAnimationPreviewResponse | null>(null)
  const [meshAnimationExportError, setMeshAnimationExportError] =
    useState<AppMessage | null>(null)
  const [meshAnimationExportNotice, setMeshAnimationExportNotice] =
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
  const meshExportError = appMessageText(locale, meshExportErrorMessage)
  const meshExportNotice = appMessageText(locale, meshExportNoticeMessage)
  const instructionExportError = appMessageText(
    locale,
    instructionExportErrorState,
  )
  const instructionExportNotice = appMessageText(
    locale,
    instructionExportNoticeMessage,
  )
  const foldTechniqueTimelineErrorText = appMessageText(
    locale,
    foldTechniqueTimelineError,
  )
  const recoveryBlocking = recoveryStartup.kind !== 'ready'
  const coreOperationRef = useRef(false)
  const latestSnapshotRef = useRef<ProjectSnapshot | null>(null)
  const appliedFoldPoseRef = useRef<FoldPreviewAppliedPoseSnapshot | null>(
    appliedFoldPose,
  )
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
  const foldTechniqueWorkspaceRef = useRef<FoldTechniqueWorkspace | null>(
    foldTechniqueWorkspace,
  )
  const foldTechniqueBusyRef = useRef(foldTechniqueBusy)
  const foldTechniqueEditorDirtyRef = useRef(false)
  const foldTechniqueEditorOpenerRef = useRef<HTMLButtonElement | null>(null)
  const foldTechniqueRequestIdRef = useRef(0)
  const foldTechniqueTimelineOpenerRef = useRef<HTMLButtonElement | null>(null)
  const foldTechniqueTimelineRequestGateRef = useRef(createOwnedRequestGate())
  const foldImportButtonRef = useRef<HTMLButtonElement>(null)
  const svgImportButtonRef = useRef<HTMLButtonElement>(null)
  const creaseExportButtonRef = useRef<HTMLButtonElement>(null)
  const creaseExportRequestIdRef = useRef(0)
  const meshExportButtonRef = useRef<HTMLButtonElement>(null)
  const meshExportRequestIdRef = useRef(0)
  const instructionExportButtonRef = useRef<HTMLButtonElement>(null)
  const meshAnimationExportButtonRef = useRef<HTMLButtonElement>(null)
  const meshAnimationExportRequestIdRef = useRef(0)
  const instructionExportRequestIdRef = useRef(0)
  const instructionExportGenerationIdRef = useRef<string | null>(null)
  recoveryStartupRef.current = recoveryStartup
  recoveryBlockingRef.current = recoveryBlocking
  appliedFoldPoseRef.current = appliedFoldPose
  foldTechniqueWorkspaceRef.current = foldTechniqueWorkspace
  foldTechniqueBusyRef.current = foldTechniqueBusy
  const replaceFoldTechniqueWorkspace = useCallback((
    workspace: FoldTechniqueWorkspace,
  ) => {
    foldTechniqueWorkspaceRef.current = workspace
    setFoldTechniqueWorkspace(workspace)
    setFoldTechniqueSelectedIndex(0)
  }, [])
  const setFoldTechniqueOperationBusy = useCallback((busy: boolean) => {
    foldTechniqueBusyRef.current = busy
    setFoldTechniqueBusy(busy)
  }, [])
  const noteFoldTechniqueEditorDirty = useCallback((dirty: boolean) => {
    foldTechniqueEditorDirtyRef.current = dirty
  }, [])
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
  const foldTechniqueTimelinePreviewStale = Boolean(
    foldTechniqueTimelinePreview
    && (
      !nativeSnapshot
      || nativeSnapshot.project_instance_id
        !== foldTechniqueTimelinePreview.expectedProjectInstanceId
      || nativeSnapshot.project_id
        !== foldTechniqueTimelinePreview.expectedProjectId
      || nativeSnapshot.revision
        !== foldTechniqueTimelinePreview.expectedRevision
      || foldTechniqueWorkspace?.document
        !== foldTechniqueTimelinePreview.sourceDocument
      || foldTechniqueSelectedIndex
        !== foldTechniqueTimelinePreview.techniqueIndex
    ),
  )
  const modalOpen = newProjectOpen
    || diagnosticsDialogOpen
    || foldTechniqueEditor !== null
    || foldTechniqueBusy
    || foldTechniqueTimelinePreview !== null
    || foldTechniqueTimelineBusy
    || foldImportPreview !== null
    || svgImportPreview !== null
    || creaseExportOpen
    || meshExportOpen
    || instructionExportOpen
    || meshAnimationExportOpen
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
    if (forceReplacement) setCompassCircles([])
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
    setSelectedFaceId(null)
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
  const nativeLayerView = useMemo(
    () => createProjectLayerCanvasView(
      nativeSnapshot?.project_layers,
      nativeSnapshot?.crease_pattern,
    ),
    [nativeSnapshot],
  )
  const nativeLines = nativeLayerView.lines
  const nativeVertices = nativeLayerView.vertices
  useEffect(() => {
    const visibleLineIds = new Set(nativeLines.map(({ id }) => id))
    const visibleVertexIds = new Set(nativeVertices.map(({ id }) => id))
    setSelectedLineId((current) =>
      current === null || visibleLineIds.has(current) ? current : null)
    setSelectedVertexId((current) =>
      current === null || visibleVertexIds.has(current) ? current : null)
    setPendingEdgeStart((current) =>
      current === null || visibleVertexIds.has(current) ? current : null)
  }, [nativeLines, nativeVertices])
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
    () => nativeLayerView.vertices.some(({ id }) => id === selectedVertexId)
      ? nativeSnapshot?.crease_pattern.vertices.find(
          (vertex) => vertex.id === selectedVertexId,
        )
      : undefined,
    [nativeLayerView.vertices, nativeSnapshot, selectedVertexId],
  )
  const selectedVertexLocked = selectedVertexId !== null
    && nativeLayerView.lockedVertexIds.has(selectedVertexId)
  const selectedVertexExpression = selectedVertex
    ? nativeSnapshot?.numeric_expressions?.vertex_coordinates?.find(
        (binding) =>
          binding.vertex === selectedVertex.id
          && Object.is(binding.adopted_x_mm, selectedVertex.position.x)
          && Object.is(binding.adopted_y_mm, selectedVertex.position.y),
      )
    : undefined
  useEffect(() => {
    if (
      !nativeLayerView.defaultLayerLocked
      || activeTool === 'select'
      || activeTool === 'measure'
    ) return
    setActiveTool('select')
    setPendingEdgeStart(null)
    setCancelInteractionToken((token) => token + 1)
  }, [activeTool, nativeLayerView.defaultLayerLocked])
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
  const canvasFaces = useMemo<readonly CreaseCanvasFace[]>(() => {
    const topology = topologyResponse?.snapshot
    if (
      !nativeSnapshot
      || !topology
      || topologyResponse.project_id !== nativeSnapshot.project_id
      || topologyResponse.revision !== nativeSnapshot.revision
      || topology.source_revision !== nativeSnapshot.revision
    ) return []
    const positions = new Map<string, Array<{ x: number; y: number }>>()
    for (const vertex of nativeSnapshot.crease_pattern.vertices) {
      const matches = positions.get(vertex.id)
      if (matches) matches.push(vertex.position)
      else positions.set(vertex.id, [vertex.position])
    }
    const faces: CreaseCanvasFace[] = []
    for (const face of topology.faces) {
      const polygon: Array<{ x: number; y: number }> = []
      let valid = face.outer.half_edges.length >= 3
      for (const halfEdge of face.outer.half_edges) {
        const matches = positions.get(halfEdge.origin)
        if (matches?.length !== 1) {
          valid = false
          break
        }
        polygon.push({ x: matches[0].x, y: matches[0].y })
      }
      if (valid) faces.push(Object.freeze({
        id: face.id,
        vertexIds: Object.freeze(
          face.outer.half_edges.map((halfEdge) => halfEdge.origin),
        ),
        edgeIds: Object.freeze(
          face.outer.half_edges.map((halfEdge) => halfEdge.edge),
        ),
        polygon: Object.freeze(polygon),
      }))
    }
    return Object.freeze(faces)
  }, [nativeSnapshot, topologyResponse])
  const selectedFace = selectedFaceId
    ? canvasFaces.find((face) => face.id === selectedFaceId)
    : undefined
  const selectedFaceLocked = selectedFace?.edgeIds.some((edgeId) =>
    nativeLines.find((line) => line.id === edgeId)?.locked ?? true) ?? false
  const selectedFaceRemovableEdges = selectedFace?.edgeIds.flatMap((edgeId) => {
    const line = nativeLines.find((candidate) => candidate.id === edgeId)
    return line && line.kind !== 'boundary' && !line.locked ? [line] : []
  }) ?? []
  const selectedElementTarget: ElementMetadataTarget | null = selectedLine
    ? { kind: 'edge', id: selectedLine.id }
    : selectedFace
      ? { kind: 'face', id: selectedFace.id }
      : selectedVertex
        ? { kind: 'vertex', id: selectedVertex.id }
        : null
  const selectedElementMetadata = selectedElementTarget && nativeSnapshot
    ? findElementMetadata(nativeSnapshot.element_metadata, selectedElementTarget)
    : null
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
      || step.declarativeOnly
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
      setInstructionVisual(step.visual)
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
      setInstructionVisual(step.visual)
      return true
    }
    if (preview.kinematics.kind !== 'tree') return false
    setFoldAngleOverrides({
      projectId: preview.projectId,
      values: angles,
    })
    setInstructionVisual(step.visual)
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
          if (foldTechniqueBusyRef.current) return 'core'
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
            is_dirty: current.is_dirty
              || foldTechniqueWorkspaceRef.current?.dirty === true
              || foldTechniqueEditorDirtyRef.current,
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

  useEffect(() => {
    const current = latestSnapshotRef.current
    const plan = planInstructionAutoRecord({
      enabled: autoRecordInstructions,
      sequence: manualPoseChangeSequence,
      lastRecordedSequence: lastAutoRecordedPoseSequenceRef.current,
      snapshot: current,
      appliedPose: appliedFoldPose,
      locale,
    })
    if (!plan) return
    lastAutoRecordedPoseSequenceRef.current = plan.sequence
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      addInstructionStep(
        projectId,
        revision,
        projectInstanceId,
        plan.title,
        '',
        '',
        1_500,
        plan.pose.fixedFace,
        plan.pose.hingeAngles,
      ))
  }, [
    appliedFoldPose,
    autoRecordInstructions,
    locale,
    manualPoseChangeSequence,
    runNativeEdit,
  ])

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

  const updateLayerPresentationFromPanel = useCallback((
    layerId: string,
    visible: boolean,
    locked: boolean,
    opacity: number,
  ) => runProjectLayerEdit((
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
  ) => updateProjectLayerPresentation(
    projectId,
    revision,
    projectInstanceId,
    baseSnapshot,
    layerId,
    visible,
    locked,
    opacity,
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
    if (!selectedLine || selectedLine.locked || benchmarkRun) {
      return Promise.resolve(false)
    }
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

  const addConstraint = useCallback((constraint: GeometricConstraintKind) => {
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      addGeometricConstraint(
        projectId,
        revision,
        projectInstanceId,
        constraint,
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
    if (selectedLine?.locked || selectedVertexLocked) {
      setCoreStatus(appMessage({
        ja: 'ロック中のレイヤーに属する図形は編集できません。レイヤーの編集ロックを解除してください。',
        en: 'This geometry belongs to a locked layer. Unlock the layer before editing it.',
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
    selectedVertexLocked,
    selectedVertexIsBoundary,
  ])

  async function splitSelectedBoundaryEdge() {
    const current = latestSnapshotRef.current
    if (
      !current
      || selectedLine?.kind !== 'boundary'
      || selectedLine.locked
      || coreOperationRef.current
    ) return
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
    if (
      !current
      || coreOperationRef.current
    ) return
    if (placementTouchesLockedLayer(placement, nativeLayerView)) {
      setCoreStatus(appMessage({
        ja: 'ロック中のレイヤーにある折り線または頂点は編集できません。',
        en: 'Creases and vertices on a locked layer cannot be edited.',
      }))
      return
    }
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
    if (nativeLayerView.defaultLayerLocked) {
      setCoreStatus(appMessage({
        ja: '既定レイヤーがロックされているため、新しい線を追加できません。',
        en: 'The default layer is locked, so a new line cannot be added.',
      }))
      return
    }
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
      setSelectedFaceId(null)
      return
    }
    selectVertexForEdge(vertexId)
  }

  async function submitVertexPosition(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedVertex || selectedVertexLocked) return
    const currentVertices = current.crease_pattern.vertices.filter(
      (vertex) => vertex.id === selectedVertex.id,
    )
    if (currentVertices.length !== 1) return
    const currentVertex = currentVertices[0]
    const currentUnit = resolveLengthDisplayUnit(current)
    const form = new FormData(event.currentTarget)
    if (form.get('vertex_action') === 'polar_endpoint') {
      const lengthDisplayExpression = String(
        form.get('polar_length_display') ?? '',
      )
      const angleDegreesExpression = String(
        form.get('polar_angle_degrees') ?? '',
      )
      let length: number
      let angleDegrees: number
      try {
        length = await evaluateDisplayLengthExpression(
          lengthDisplayExpression,
          currentUnit,
        )
        angleDegrees = (
          await evaluateFiniteNumericExpression(
            angleDegreesExpression,
          )
        ).value
      } catch (error) {
        setCoreStatus(editExpressionErrorMessage(error))
        return
      }
      const edgeKind = form.get('polar_edge_kind')
      if (
        length === null
        || length <= 0
        || !Number.isFinite(angleDegrees)
        || Math.abs(angleDegrees) > 360_000
        || (
          edgeKind !== 'mountain'
          && edgeKind !== 'valley'
          && edgeKind !== 'auxiliary'
          && edgeKind !== 'cut'
        )
        || (edgeKind === 'cut' && !current.cutting_allowed)
      ) {
        setCoreStatus(appMessage({
          ja: '正の有限な長さ、有限な角度、利用可能な線種を入力してください。',
          en: 'Enter a positive finite length, a finite angle, and an available line type.',
        }))
        return
      }
      const angleRadians = angleDegrees * Math.PI / 180
      const x = currentVertex.position.x + length * Math.cos(angleRadians)
      const y = currentVertex.position.y + length * Math.sin(angleRadians)
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        setCoreStatus(appMessage({
          ja: '指定した長さと角度から有限な座標を作成できません。',
          en: 'The specified length and angle do not produce finite coordinates.',
        }))
        return
      }
      const previousVertexIds = new Set(
        current.crease_pattern.vertices.map(({ id }) => id),
      )
      const result: { snapshot: ProjectSnapshot | null } = { snapshot: null }
      const succeeded = await runNativeEdit(async (
        projectId,
        revision,
        projectInstanceId,
      ) => {
        const snapshot = await addConnectedVertex(
          projectId,
          revision,
          projectInstanceId,
          selectedVertex.id,
          x,
          y,
          millimetreExpressionSource(
            lengthDisplayExpression,
            currentUnit.millimetresPerUnit,
          ),
          angleDegreesExpression,
          length,
          angleDegrees,
          edgeKind,
        )
        result.snapshot = snapshot
        return snapshot
      })
      if (!succeeded || !result.snapshot) return
      const added = result.snapshot.crease_pattern.vertices.find(
        ({ id }) => !previousVertexIds.has(id),
      )
      setSelectedLineId(null)
      setPendingEdgeStart(null)
      setSelectedVertexId(added?.id ?? null)
      setActiveTool('select')
      setCoreStatus(appMessage({
        ja: '指定した長さと角度から終点と線を追加しました。',
        en: 'Added an endpoint and line from the specified length and angle.',
      }))
      return
    }
    const xDisplayExpression = String(form.get('x_display') ?? '')
    const yDisplayExpression = String(form.get('y_display') ?? '')
    let x: number | null = null
    let y: number | null = null
    try {
      x = await evaluateDisplayLengthExpression(
        xDisplayExpression,
        currentUnit,
      )
      y = await evaluateDisplayLengthExpression(
        yDisplayExpression,
        currentUnit,
      )
    } catch (error) {
      setCoreStatus(editExpressionErrorMessage(error))
      return
    }
    if (x === null || y === null) {
      setCoreStatus(appMessage({
        ja: '座標には有限の数値を入力してください',
        en: 'Enter finite numeric coordinates.',
      }))
      return
    }
    await runNativeEdit((projectId, revision, projectInstanceId) =>
      moveVertex(
        projectId,
        revision,
        projectInstanceId,
        selectedVertex.id,
        x,
        y,
        millimetreExpressionSource(xDisplayExpression, currentUnit.millimetresPerUnit),
        millimetreExpressionSource(yDisplayExpression, currentUnit.millimetresPerUnit),
      ))
  }

  async function submitDirectVertex(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || benchmarkRun || nativeLayerView.defaultLayerLocked) return
    const currentUnit = resolveLengthDisplayUnit(current)
    const form = new FormData(event.currentTarget)
    const xDisplayExpression = String(form.get('direct_x_display') ?? '')
    const yDisplayExpression = String(form.get('direct_y_display') ?? '')
    let x: number | null = null
    let y: number | null = null
    try {
      x = await evaluateDisplayLengthExpression(
        xDisplayExpression,
        currentUnit,
      )
      y = await evaluateDisplayLengthExpression(
        yDisplayExpression,
        currentUnit,
      )
    } catch (error) {
      setCoreStatus(editExpressionErrorMessage(error))
      return
    }
    if (x === null || y === null) {
      setCoreStatus(appMessage({
        ja: '有限な数値座標を入力してください。',
        en: 'Enter finite numeric coordinates.',
      }))
      return
    }

    const previousVertexIds = new Set(
      current.crease_pattern.vertices.map(({ id }) => id),
    )
    const result: { snapshot: ProjectSnapshot | null } = { snapshot: null }
    const succeeded = await runNativeEdit(async (
      projectId,
      revision,
      projectInstanceId,
    ) => {
      const snapshot = await addVertex(
        projectId,
        revision,
        projectInstanceId,
        x,
        y,
        millimetreExpressionSource(xDisplayExpression, currentUnit.millimetresPerUnit),
        millimetreExpressionSource(yDisplayExpression, currentUnit.millimetresPerUnit),
      )
      result.snapshot = snapshot
      return snapshot
    })
    if (!succeeded || !result.snapshot) return
    const added = result.snapshot.crease_pattern.vertices.find(
      ({ id }) => !previousVertexIds.has(id),
    )
    setPendingEdgeStart(null)
    setSelectedLineId(null)
    setSelectedVertexId(added?.id ?? null)
    setActiveTool('select')
    setCoreStatus(appMessage({
      ja: '指定座標に頂点を追加しました。',
      en: 'Added a vertex at the specified coordinates.',
    }))
  }

  async function submitMoveSelectedEdge(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedLine || benchmarkRun || selectedLine.locked) return
    const currentUnit = resolveLengthDisplayUnit(current)
    const form = new FormData(event.currentTarget)
    const deltaXDisplayExpression = String(form.get('edge_delta_x_display') ?? '')
    const deltaYDisplayExpression = String(form.get('edge_delta_y_display') ?? '')
    let deltaX: number | null = null
    let deltaY: number | null = null
    try {
      deltaX = await evaluateDisplayLengthExpression(deltaXDisplayExpression, currentUnit)
      deltaY = await evaluateDisplayLengthExpression(deltaYDisplayExpression, currentUnit)
    } catch (error) {
      setCoreStatus(editExpressionErrorMessage(error))
      return
    }
    if (deltaX === null || deltaY === null) {
      setCoreStatus(appMessage({
        ja: '線の移動量には有限な数式を入力してください。',
        en: 'Enter finite expressions for the line translation.',
      }))
      return
    }
    await runNativeEdit((projectId, revision, projectInstanceId) =>
      moveEdge(
        projectId,
        revision,
        projectInstanceId,
        selectedLine.id,
        millimetreExpressionSource(
          deltaXDisplayExpression,
          currentUnit.millimetresPerUnit,
        ),
        millimetreExpressionSource(
          deltaYDisplayExpression,
          currentUnit.millimetresPerUnit,
        ),
        deltaX,
        deltaY,
      ))
  }

  async function submitMirrorSelectedEdge(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedLine || benchmarkRun || selectedLine.locked) return
    const currentUnit = resolveLengthDisplayUnit(current)
    const form = new FormData(event.currentTarget)
    const source = String(form.get('symmetry_axis_x_display') ?? '')
    try {
      const axisX = await evaluateDisplayLengthExpression(source, currentUnit)
      await runNativeEdit((projectId, revision, projectInstanceId) =>
        mirrorEdgeLeftRight(
          projectId,
          revision,
          projectInstanceId,
          selectedLine.id,
          millimetreExpressionSource(source, currentUnit.millimetresPerUnit),
          axisX,
        ))
    } catch (error) {
      setCoreStatus(editExpressionErrorMessage(error))
    }
  }

  async function submitRotateSelectedEdge(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedLine || benchmarkRun || selectedLine.locked) return
    const currentUnit = resolveLengthDisplayUnit(current)
    const form = new FormData(event.currentTarget)
    const xSource = String(form.get('rotation_center_x_display') ?? '')
    const ySource = String(form.get('rotation_center_y_display') ?? '')
    const angleSource = String(form.get('rotation_angle_degrees') ?? '')
    try {
      const [centerX, centerY, angle] = await Promise.all([
        evaluateDisplayLengthExpression(xSource, currentUnit),
        evaluateDisplayLengthExpression(ySource, currentUnit),
        evaluateFiniteNumericExpression(angleSource).then(({ value }) => value),
      ])
      await runNativeEdit((projectId, revision, projectInstanceId) =>
        rotateEdgeAboutPoint(
          projectId,
          revision,
          projectInstanceId,
          selectedLine.id,
          millimetreExpressionSource(xSource, currentUnit.millimetresPerUnit),
          millimetreExpressionSource(ySource, currentUnit.millimetresPerUnit),
          angleSource,
          centerX,
          centerY,
          angle,
        ))
    } catch (error) {
      setCoreStatus(editExpressionErrorMessage(error))
    }
  }

  async function submitMoveSelectedFace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedFace || benchmarkRun || selectedFaceLocked) return
    const currentUnit = resolveLengthDisplayUnit(current)
    const form = new FormData(event.currentTarget)
    const deltaXDisplayExpression = String(form.get('face_delta_x_display') ?? '')
    const deltaYDisplayExpression = String(form.get('face_delta_y_display') ?? '')
    let deltaX: number | null = null
    let deltaY: number | null = null
    try {
      deltaX = await evaluateDisplayLengthExpression(deltaXDisplayExpression, currentUnit)
      deltaY = await evaluateDisplayLengthExpression(deltaYDisplayExpression, currentUnit)
    } catch (error) {
      setCoreStatus(editExpressionErrorMessage(error))
      return
    }
    if (deltaX === null || deltaY === null) {
      setCoreStatus(appMessage({
        ja: '面の移動量には有限な数式を入力してください。',
        en: 'Enter finite expressions for the face translation.',
      }))
      return
    }
    await runNativeEdit((projectId, revision, projectInstanceId) =>
      moveVertices(
        projectId,
        revision,
        projectInstanceId,
        [...selectedFace.vertexIds],
        millimetreExpressionSource(
          deltaXDisplayExpression,
          currentUnit.millimetresPerUnit,
        ),
        millimetreExpressionSource(
          deltaYDisplayExpression,
          currentUnit.millimetresPerUnit,
        ),
        deltaX,
        deltaY,
      ))
  }

  async function submitSplitSelectedFace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedFace || selectedFaceLocked) return
    const form = new FormData(event.currentTarget)
    const start = String(form.get('face_split_start') ?? '')
    const end = String(form.get('face_split_end') ?? '')
    const kind = form.get('face_split_kind')
    const startIndex = selectedFace.vertexIds.indexOf(start)
    const endIndex = selectedFace.vertexIds.indexOf(end)
    const boundaryCount = selectedFace.vertexIds.length
    const adjacent = startIndex >= 0 && endIndex >= 0 && (
      Math.abs(startIndex - endIndex) === 1
      || Math.abs(startIndex - endIndex) === boundaryCount - 1
    )
    if (
      startIndex < 0
      || endIndex < 0
      || start === end
      || adjacent
      || current.crease_pattern.edges.some((edge) =>
        (edge.start === start && edge.end === end)
        || (edge.start === end && edge.end === start))
      || (
        kind !== 'mountain'
        && kind !== 'valley'
        && kind !== 'auxiliary'
        && kind !== 'cut'
      )
      || (kind === 'cut' && !current.cutting_allowed)
    ) {
      setCoreStatus(appMessage({
        ja: '面を分割する非隣接の2頂点と利用可能な線種を選択してください。',
        en: 'Choose two non-adjacent face vertices and an available line type.',
      }))
      return
    }
    await runNativeEdit((projectId, revision, projectInstanceId) =>
      addEdge(projectId, revision, projectInstanceId, start, end, kind))
    setSelectedFaceId(null)
  }

  async function submitMergeSelectedFace(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const form = new FormData(event.currentTarget)
    const edgeId = String(form.get('face_merge_edge') ?? '')
    const edge = nativeLines.find((line) =>
      line.id === edgeId
      && selectedFace?.edgeIds.includes(line.id)
      && line.kind !== 'boundary')
    if (!edge || edge.locked) return
    await runNativeEdit((projectId, revision, projectInstanceId) =>
      removeEdge(projectId, revision, projectInstanceId, edge.id))
    setSelectedFaceId(null)
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
    const frontPattern = form.get('front_pattern')
    const frontTextureAsset = frontPattern === 'custom'
      ? current.paper.front.texture_asset
      : builtinPaperPatternAsset(frontPattern)
    const backPattern = form.get('back_pattern')
    const backTextureAsset = backPattern === 'custom'
      ? current.paper.back.texture_asset
      : builtinPaperPatternAsset(backPattern)
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
        frontTextureAsset,
        backTextureAsset,
        cuttingAllowed: form.get('cutting_allowed') === 'on',
      }))
  }

  function chooseFrontPaperTexture() {
    if (coreOperationRef.current) return
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      importFrontPaperTexture(projectId, revision, projectInstanceId))
  }

  function chooseBackPaperTexture() {
    if (coreOperationRef.current) return
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      importBackPaperTexture(projectId, revision, projectInstanceId))
  }

  function submitElementMetadata(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedElementTarget || coreOperationRef.current) return
    const form = new FormData(event.currentTarget)
    const name = String(form.get('element_name') ?? '').trim()
    const memo = String(form.get('element_memo') ?? '')
    const parsedColor = parseHexColor(String(form.get('element_color') ?? ''))
    const color = form.get('element_use_color') === 'on' ? parsedColor : null
    if (name.length > 120 || memo.length > 4_000 || (color === null
      && form.get('element_use_color') === 'on')) {
      setCoreStatus(appMessage({
        ja: '要素の名前、色、メモを確認してください。',
        en: 'Review the element name, color, and memo.',
      }))
      return
    }
    const metadata: ElementMetadata | null = name || memo || color
      ? { name, memo, color }
      : null
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      setElementMetadata(
        projectId,
        revision,
        projectInstanceId,
        selectedElementTarget,
        metadata,
      ))
  }

  function submitProjectMemo(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current || recoveryBlockingRef.current) return
    const memo = String(new FormData(event.currentTarget).get('project_memo') ?? '')
    if (memo.length > 16_000) {
      setCoreStatus(appMessage({
        ja: 'プロジェクトメモは16000文字以内で入力してください。',
        en: 'Keep the project memo within 16,000 characters.',
      }))
      return
    }
    void runNativeEdit((projectId, revision, projectInstanceId) =>
      updateProjectMemo(projectId, revision, projectInstanceId, memo))
  }

  async function submitPaperResize(event: FormEvent<HTMLFormElement>) {
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
    const form = new FormData(event.currentTarget)
    const widthExpression = referenceAxis === 'width'
      ? finiteNumberExpressionSource(currentSize.width)
      : String(form.get('width_display') ?? '')
    const heightExpression = referenceAxis === 'height'
      ? finiteNumberExpressionSource(currentSize.height)
      : String(form.get('height_display') ?? '')
    let widthMm: number | null = currentSize.width
    let heightMm: number | null = currentSize.height
    try {
      if (referenceAxis !== 'width') {
        widthMm = await evaluateDisplayLengthExpression(
          widthExpression,
          currentUnit,
        )
      }
      if (referenceAxis !== 'height') {
        heightMm = await evaluateDisplayLengthExpression(
          heightExpression,
          currentUnit,
        )
      }
    } catch (error) {
      setCoreStatus(editExpressionErrorMessage(error))
      return
    }
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
    const widthMillimetreExpression = referenceAxis === 'width'
      ? widthExpression
      : millimetreExpressionSource(widthExpression, currentUnit.millimetresPerUnit)
    const heightMillimetreExpression = referenceAxis === 'height'
      ? heightExpression
      : millimetreExpressionSource(heightExpression, currentUnit.millimetresPerUnit)

    void runNativeEdit((projectId, revision, projectInstanceId) =>
      resizeRectangularPaper(
        projectId,
        revision,
        projectInstanceId,
        widthMillimetreExpression,
        heightMillimetreExpression,
        widthMm,
        heightMm,
      ))
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

  async function runProjectFolderOperation(
    operation: 'folder_open' | 'folder_save',
  ) {
    const current = latestSnapshotRef.current
    if (
      !current
      || coreOperationRef.current
      || recoveryBlockingRef.current
    ) return
    if (
      operation === 'folder_open'
      && current.is_dirty
      && !window.confirm(appConfirmationText(locale, 'openProject'))
    ) return

    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation(operation)
    setCancelInteractionToken((token) => token + 1)
    try {
      const response = operation === 'folder_open'
        ? await openProjectFolder(locale)
        : await saveProjectFolderAs(locale)
      if (response.canceled) {
        setCoreStatus(appMessage({
          ja: '展開フォルダー操作をキャンセルしました',
          en: 'Expanded-folder operation cancelled',
        }))
        return
      }
      applySnapshot(response.project, operation === 'folder_open')
      if (operation === 'folder_open') {
        setValidation(null)
        setSelectedLineId(null)
        setSelectedVertexId(null)
        setPendingEdgeStart(null)
        setParallelReferenceEdgeId(null)
      }
      setCoreStatus(operation === 'folder_open'
        ? appMessage({
            ja: '展開フォルダーから「{name}」を開きました',
            en: 'Opened “{name}” from an expanded folder',
          }, { name: response.project.name })
        : appMessage({
            ja: '「{name}」を新しい展開フォルダーへ保存しました',
            en: 'Saved “{name}” to a new expanded folder',
          }, { name: response.project.name }))
    } catch (error) {
      setCoreStatus(appMessage({
        ja: projectFolderClientErrorMessage(error, 'ja'),
        en: projectFolderClientErrorMessage(error, 'en'),
      }))
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  function openNewFoldTechniqueEditor(opener: HTMLButtonElement) {
    if (
      foldTechniqueBusy
      || coreBusy
      || !isNativeFoldTechniqueFileAvailable()
    ) return
    if (
      foldTechniqueWorkspaceRef.current?.dirty
      && !window.confirm(
        appConfirmationText(locale, 'replaceFoldTechnique'),
      )
    ) return
    foldTechniqueEditorOpenerRef.current = opener
    foldTechniqueEditorDirtyRef.current = true
    setFoldTechniqueSaveFailed(false)
    setFoldTechniqueEditor({
      mode: 'create',
      initialDocument: createInitialFoldTechniqueDocumentV1(),
      techniqueIndex: 0,
    })
  }

  function openCurrentFoldTechniqueEditor(opener: HTMLButtonElement) {
    if (foldTechniqueBusy || coreBusy || !foldTechniqueWorkspace) return
    foldTechniqueEditorOpenerRef.current = opener
    foldTechniqueEditorDirtyRef.current = false
    setFoldTechniqueSaveFailed(false)
    setFoldTechniqueEditor({
      mode: 'edit',
      initialDocument: foldTechniqueWorkspace.document,
      techniqueIndex: 0,
    })
  }

  function closeFoldTechniqueEditor() {
    if (foldTechniqueBusy || coreBusy) return
    if (
      foldTechniqueEditorDirtyRef.current
      && !window.confirm(
        appConfirmationText(locale, 'discardFoldTechniqueDraft'),
      )
    ) return
    foldTechniqueEditorDirtyRef.current = false
    foldTechniqueEditorOpenerRef.current = null
    setFoldTechniqueEditor(null)
    setFoldTechniqueSaveFailed(false)
  }

  async function importFoldTechniqueFile(opener: HTMLButtonElement) {
    if (
      foldTechniqueBusy
      || coreBusy
      || !isNativeFoldTechniqueFileAvailable()
    ) return
    if (
      foldTechniqueWorkspaceRef.current?.dirty
      && !window.confirm(
        appConfirmationText(locale, 'replaceFoldTechnique'),
      )
    ) return
    foldTechniqueEditorOpenerRef.current = opener
    const requestId = nextFoldTechniqueRequestId(foldTechniqueRequestIdRef)
    setFoldTechniqueOperationBusy(true)
    setFoldTechniqueSaveFailed(false)
    try {
      const response = await openFoldTechniqueFileV1(requestId, locale)
      if (foldTechniqueRequestIdRef.current !== requestId) return
      if (response.canceled) {
        setCoreStatus(appMessage({
          ja: '折り技法ファイルの取込をキャンセルしました。',
          en: 'Fold-technique file import was cancelled.',
        }))
        return
      }
      if (!response.document) throw new Error('missing admitted document')
      replaceFoldTechniqueWorkspace({
        document: response.document,
        dirty: false,
      })
      foldTechniqueEditorDirtyRef.current = false
      setFoldTechniqueEditor({
        mode: 'edit',
        initialDocument: response.document,
        techniqueIndex: 0,
      })
      setCoreStatus(appMessage({
        ja: '折り技法ファイルを取り込みました。内容を確認して編集できます。',
        en: 'Imported the fold-technique file. You can review and edit it.',
      }))
    } catch (error) {
      if (foldTechniqueRequestIdRef.current !== requestId) return
      setCoreStatus(foldTechniqueFileErrorAppMessage(error))
    } finally {
      if (foldTechniqueRequestIdRef.current === requestId) {
        setFoldTechniqueOperationBusy(false)
      }
    }
  }

  async function confirmFoldTechniqueEditor(
    document: FoldTechniqueFileDocumentV1,
  ) {
    const editor = foldTechniqueEditor
    if (!editor || foldTechniqueBusy || coreBusy) return
    if (editor.mode === 'edit') {
      replaceFoldTechniqueWorkspace({ document, dirty: true })
      foldTechniqueEditorDirtyRef.current = false
      foldTechniqueEditorOpenerRef.current = null
      setFoldTechniqueEditor(null)
      setFoldTechniqueSaveFailed(false)
      setCoreStatus(appMessage({
        ja: '折り技法の変更を保持しました。共有するには「別名保存」を実行してください。',
        en: 'Kept the fold-technique changes. Choose “Save as” to share them.',
      }))
      return
    }
    await saveCreatedFoldTechnique(document)
  }

  async function saveCreatedFoldTechnique(
    document: FoldTechniqueFileDocumentV1,
  ) {
    const requestId = nextFoldTechniqueRequestId(foldTechniqueRequestIdRef)
    setFoldTechniqueOperationBusy(true)
    setFoldTechniqueSaveFailed(false)
    try {
      const response = await saveFoldTechniqueFileAsV1(
        requestId,
        locale,
        document,
      )
      if (foldTechniqueRequestIdRef.current !== requestId) return
      if (response.canceled) {
        setCoreStatus(appMessage({
          ja: '新しい折り技法の保存をキャンセルしました。編集内容は画面に残っています。',
          en: 'Saving the new fold technique was cancelled. The edited content remains open.',
        }))
        return
      }
      if (!response.document) throw new Error('missing admitted document')
      replaceFoldTechniqueWorkspace({
        document: response.document,
        dirty: false,
      })
      foldTechniqueEditorDirtyRef.current = false
      foldTechniqueEditorOpenerRef.current = null
      setFoldTechniqueEditor(null)
      setCoreStatus(appMessage({
        ja: '新しい折り技法を作成し、共有ファイルへ保存しました。',
        en: 'Created the fold technique and saved it to a shared file.',
      }))
    } catch (error) {
      if (foldTechniqueRequestIdRef.current !== requestId) return
      setFoldTechniqueSaveFailed(true)
      setCoreStatus(foldTechniqueFileErrorAppMessage(error))
    } finally {
      if (foldTechniqueRequestIdRef.current === requestId) {
        setFoldTechniqueOperationBusy(false)
      }
    }
  }

  async function saveCurrentFoldTechniqueAs() {
    const workspace = foldTechniqueWorkspace
    if (
      !workspace
      || foldTechniqueBusy
      || coreBusy
      || !isNativeFoldTechniqueFileAvailable()
    ) return
    const requestId = nextFoldTechniqueRequestId(foldTechniqueRequestIdRef)
    setFoldTechniqueOperationBusy(true)
    try {
      const response = await saveFoldTechniqueFileAsV1(
        requestId,
        locale,
        workspace.document,
      )
      if (foldTechniqueRequestIdRef.current !== requestId) return
      if (response.canceled) {
        setCoreStatus(appMessage({
          ja: '折り技法ファイルの別名保存をキャンセルしました。内容は変更していません。',
          en: 'Saving the fold-technique file as another file was cancelled. No content changed.',
        }))
        return
      }
      if (!response.document) throw new Error('missing admitted document')
      replaceFoldTechniqueWorkspace({
        document: response.document,
        dirty: false,
      })
      setCoreStatus(appMessage({
        ja: '折り技法を別名の共有ファイルへ保存しました。',
        en: 'Saved the fold technique to another shared file.',
      }))
    } catch (error) {
      if (foldTechniqueRequestIdRef.current !== requestId) return
      setCoreStatus(foldTechniqueFileErrorAppMessage(error))
    } finally {
      if (foldTechniqueRequestIdRef.current === requestId) {
        setFoldTechniqueOperationBusy(false)
      }
    }
  }

  function previewSelectedFoldTechniqueTimeline(opener: HTMLButtonElement) {
    const workspace = foldTechniqueWorkspaceRef.current
    const current = latestSnapshotRef.current
    if (
      !workspace
      || !current
      || coreOperationRef.current
      || foldTechniqueBusyRef.current
      || ownedRequestActive(foldTechniqueTimelineRequestGateRef.current)
      || !isNativeCoreAvailable()
    ) return
    const proposal = createFoldTechniqueTimelineProposalV1(
      workspace.document,
      foldTechniqueSelectedIndex,
      locale,
      current.instruction_timeline.steps.length,
    )
    if (!proposal.ok) {
      const message = proposal.error === 'timeline_capacity'
        ? appMessage({
            ja: '折り手順の上限内に追加できません（必要 {required}、空き {available}）。',
            en: 'The proposal does not fit in the instruction limit (requires {required}, {available} available).',
          }, {
            required: proposal.requiredSteps,
            available: proposal.availableSteps,
          })
        : proposal.error === 'proposal_size'
          ? appMessage({
              ja: '折り技法の説明案が安全な入力サイズ上限を超えています。',
              en: 'The fold-technique proposal exceeds the safe input-size limit.',
            })
          : appMessage({
              ja: '選択中の折り技法から説明案を作成できませんでした。',
              en: 'Could not build a proposal from the selected fold technique.',
            })
      setCoreStatus(message)
      return
    }
    foldTechniqueTimelineOpenerRef.current = opener
    setFoldTechniqueTimelineError(null)
    setFoldTechniqueTimelinePreview({
      preview: proposal,
      sourceDocument: workspace.document,
      techniqueIndex: foldTechniqueSelectedIndex,
      expectedProjectInstanceId: current.project_instance_id,
      expectedProjectId: current.project_id,
      expectedRevision: current.revision,
    })
  }

  function closeFoldTechniqueTimelinePreview() {
    if (ownedRequestActive(foldTechniqueTimelineRequestGateRef.current)) return
    const opener = foldTechniqueTimelineOpenerRef.current
    foldTechniqueTimelineOpenerRef.current = null
    setFoldTechniqueTimelinePreview(null)
    setFoldTechniqueTimelineError(null)
    requestAnimationFrame(() => opener?.focus())
  }

  async function confirmFoldTechniqueTimelineProposal() {
    const pending = foldTechniqueTimelinePreview
    const current = latestSnapshotRef.current
    if (
      !pending
      || ownedRequestActive(foldTechniqueTimelineRequestGateRef.current)
    ) return
    if (
      !current
      || current.project_instance_id !== pending.expectedProjectInstanceId
      || current.project_id !== pending.expectedProjectId
      || current.revision !== pending.expectedRevision
      || foldTechniqueWorkspaceRef.current?.document !== pending.sourceDocument
      || foldTechniqueSelectedIndex !== pending.techniqueIndex
    ) {
      setFoldTechniqueTimelineError(appMessage({
        ja: 'プロジェクトまたは選択中の技法が変わりました。案を閉じて作り直してください。',
        en: 'The project or selected technique changed. Close and rebuild the proposal.',
      }))
      return
    }

    const requestId = tryBeginOwnedRequest(
      foldTechniqueTimelineRequestGateRef.current,
    )
    if (requestId === null) return
    setFoldTechniqueTimelineBusy(true)
    setFoldTechniqueTimelineError(null)
    let succeeded = false
    try {
      succeeded = await runNativeEdit((
        projectId,
        revision,
        projectInstanceId,
      ) => {
        if (
          projectInstanceId !== pending.expectedProjectInstanceId
          || projectId !== pending.expectedProjectId
          || revision !== pending.expectedRevision
        ) return Promise.reject(new Error('stale named-technique proposal'))
        return appendNamedTechniqueInstructionSteps(
          projectId,
          revision,
          projectInstanceId,
          pending.preview.proposal,
        )
      })
    } catch {
      succeeded = false
    }
    if (!completeOwnedRequest(
      foldTechniqueTimelineRequestGateRef.current,
      requestId,
    )) return
    setFoldTechniqueTimelineBusy(false)
    if (!succeeded) {
      setFoldTechniqueTimelineError(appMessage({
        ja: '説明ステップを追加できませんでした。プロジェクトは変更されていません。',
        en: 'Could not append the description steps. The project was not changed.',
      }))
      return
    }
    const opener = foldTechniqueTimelineOpenerRef.current
    foldTechniqueTimelineOpenerRef.current = null
    setFoldTechniqueTimelinePreview(null)
    setCoreStatus(appMessage({
      ja: '「{technique}」から説明専用の折り手順を追加しました。1回のUndoで戻せます。',
      en: 'Added description-only steps from “{technique}”. One Undo removes the complete addition.',
    }, { technique: pending.preview.techniqueName }))
    requestAnimationFrame(() => opener?.focus())
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

  async function prepareStaticMeshExport(format: StaticMeshExportFormat) {
    const current = latestSnapshotRef.current
    const pose = appliedFoldPoseRef.current
    const sourcePoseKey = foldPreviewAppliedPoseKey(pose)
    if (
      !current
      || !pose
      || pose.state === 'running'
      || !sourcePoseKey
      || pose.projectId !== current.project_id
      || pose.revision !== current.revision
      || coreOperationRef.current
    ) return

    const requestId = ++meshExportRequestIdRef.current
    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('mesh_export')
    setMeshExportPreview(null)
    setMeshExportError(null)
    setMeshExportNotice(null)
    setCancelInteractionToken((token) => token + 1)
    try {
      const response = await previewStaticMeshExport(
        current.project_instance_id,
        current.project_id,
        current.revision,
        format,
      )
      if (requestId !== meshExportRequestIdRef.current) {
        await cancelStaticMeshExport(response.preview.exportId).catch(() => undefined)
        return
      }
      const latest = latestSnapshotRef.current
      const latestPose = appliedFoldPoseRef.current
      const preview = response.preview
      if (
        !latest
        || preview.format !== format
        || preview.projectInstanceId !== current.project_instance_id
        || preview.projectId !== current.project_id
        || preview.revision !== current.revision
        || latest.project_instance_id !== current.project_instance_id
        || latest.project_id !== current.project_id
        || latest.revision !== current.revision
        || foldPreviewAppliedPoseKey(latestPose) !== sourcePoseKey
        || latestPose?.state === 'running'
      ) {
        await cancelStaticMeshExport(preview.exportId).catch(() => undefined)
        throw new Error('stale static-mesh preview')
      }
      setMeshExportPreview(preview)
      setCoreStatus(appMessage({
        ja: '現在の3D姿勢の中央面メッシュと情報損失を確認してください',
        en: 'Review the current-pose mid-surface mesh and information loss.',
      }))
    } catch {
      if (requestId !== meshExportRequestIdRef.current) return
      const safeError = appMessage({
        ja: '現在表示中の認証済み3D姿勢からメッシュを生成できませんでした。3D表示の更新完了後に再試行してください。',
        en: 'Could not generate a mesh from the authenticated pose currently displayed. Wait for the 3D view to finish updating, then retry.',
      })
      setMeshExportError(safeError)
      setCoreStatus(safeError)
    } finally {
      if (requestId === meshExportRequestIdRef.current) {
        setFileOperation(null)
        coreOperationRef.current = false
        setCoreBusy(false)
      }
    }
  }

  function beginStaticMeshExport() {
    const current = latestSnapshotRef.current
    const pose = appliedFoldPoseRef.current
    if (
      !current
      || !pose
      || pose.state === 'running'
      || pose.projectId !== current.project_id
      || pose.revision !== current.revision
      || coreOperationRef.current
    ) return
    setMeshExportOpen(true)
    setMeshExportFormat('obj')
    setMeshExportPreview(null)
    setMeshExportError(null)
    setMeshExportNotice(null)
    void prepareStaticMeshExport('obj')
  }

  function changeStaticMeshExportFormat(format: StaticMeshExportFormat) {
    if (format === meshExportFormat || coreOperationRef.current) return
    setMeshExportFormat(format)
    void prepareStaticMeshExport(format)
  }

  async function closeStaticMeshExportDialog() {
    if (coreOperationRef.current) return
    const preview = meshExportPreview
    meshExportRequestIdRef.current += 1
    if (!preview) {
      setMeshExportOpen(false)
      setMeshExportError(null)
      setMeshExportNotice(null)
      requestAnimationFrame(() => meshExportButtonRef.current?.focus())
      return
    }

    coreOperationRef.current = true
    setCoreBusy(true)
    try {
      await cancelStaticMeshExport(preview.exportId)
      setMeshExportOpen(false)
      setMeshExportPreview(null)
      setMeshExportError(null)
      setMeshExportNotice(null)
      setCoreStatus(appMessage({
        ja: '現在姿勢の3Dメッシュ書き出しをキャンセルしました',
        en: 'Current-pose 3D mesh export cancelled.',
      }))
      requestAnimationFrame(() => meshExportButtonRef.current?.focus())
    } catch {
      const safeError = appMessage({
        ja: '3Dメッシュの書き出しプレビューを破棄できませんでした。',
        en: 'Could not discard the 3D mesh export preview.',
      })
      setMeshExportError(safeError)
      setCoreStatus(safeError)
    } finally {
      coreOperationRef.current = false
      setCoreBusy(false)
    }
  }

  async function saveCurrentStaticMeshExport(warningsAcknowledged: boolean) {
    const current = latestSnapshotRef.current
    const preview = meshExportPreview
    if (!current || !preview || coreOperationRef.current) return
    if (
      current.project_instance_id !== preview.projectInstanceId
      || current.project_id !== preview.projectId
      || current.revision !== preview.revision
    ) {
      setMeshExportError(appMessage({
        ja: '編集内容が変わったため、現在姿勢から書き出しデータを作り直してください。',
        en: 'The project changed. Rebuild the export from the current pose.',
      }))
      return
    }

    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('mesh_export')
    setMeshExportError(null)
    setMeshExportNotice(null)
    try {
      const response = await saveStaticMeshExport(preview, warningsAcknowledged)
      if (response.canceled) {
        setMeshExportNotice(appMessage({
          ja: '保存先の選択をキャンセルしました。同じ不変データで再試行できます。',
          en: 'Save location selection was cancelled. You can retry with the same immutable data.',
        }))
        setCoreStatus(appMessage({
          ja: '3Dメッシュの保存先選択をキャンセルしました',
          en: '3D mesh save location selection cancelled.',
        }))
        return
      }
      setMeshExportOpen(false)
      setMeshExportPreview(null)
      setMeshExportNotice(null)
      setCoreStatus(appMessage({
        ja: '{fileName}を書き出しました',
        en: 'Exported {fileName}',
      }, { fileName: preview.suggestedFileName }))
      requestAnimationFrame(() => meshExportButtonRef.current?.focus())
    } catch {
      const safeError = appMessage({
        ja: '3D姿勢または編集内容が変わったか、ファイルを保存できませんでした。現在姿勢から作り直して再試行してください。',
        en: 'The 3D pose or project changed, or the file could not be saved. Rebuild from the current pose and retry.',
      })
      setMeshExportError(safeError)
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

  async function prepareMeshAnimationExport() {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    const requestId = ++meshAnimationExportRequestIdRef.current
    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('mesh_animation_export')
    setMeshAnimationExportPreview(null)
    setMeshAnimationExportError(null)
    setMeshAnimationExportNotice(null)
    try {
      const preview = await previewInstructionMeshAnimation({
        expectedProjectInstanceId: current.project_instance_id,
        expectedProjectId: current.project_id,
        expectedRevision: current.revision,
      })
      if (requestId !== meshAnimationExportRequestIdRef.current) {
        await cancelInstructionMeshAnimation(preview.exportId).catch(() => undefined)
        return
      }
      const latest = latestSnapshotRef.current
      if (
        !latest
        || latest.project_instance_id !== preview.projectInstanceId
        || latest.project_id !== preview.projectId
        || latest.revision !== preview.revision
      ) {
        await cancelInstructionMeshAnimation(preview.exportId).catch(() => undefined)
        throw new Error('stale animation preview')
      }
      setMeshAnimationExportPreview(preview)
    } catch {
      if (requestId !== meshAnimationExportRequestIdRef.current) return
      const error = appMessage({
        ja: '現在の手順からアニメーションを作成できませんでした。手順を確認して再試行してください。',
        en: 'Could not build an animation from the current instructions. Review them and retry.',
      })
      setMeshAnimationExportError(error)
      setCoreStatus(error)
    } finally {
      if (requestId === meshAnimationExportRequestIdRef.current) {
        setFileOperation(null)
        coreOperationRef.current = false
        setCoreBusy(false)
      }
    }
  }

  function beginMeshAnimationExport() {
    if (!latestSnapshotRef.current || coreOperationRef.current) return
    setMeshAnimationExportOpen(true)
    void prepareMeshAnimationExport()
  }

  async function closeMeshAnimationExport() {
    if (coreOperationRef.current) return
    const preview = meshAnimationExportPreview
    meshAnimationExportRequestIdRef.current += 1
    if (preview) {
      coreOperationRef.current = true
      setCoreBusy(true)
      try {
        await cancelInstructionMeshAnimation(preview.exportId)
      } catch {
        setMeshAnimationExportError(appMessage({
          ja: 'アニメーション書き出しを安全に破棄できませんでした。',
          en: 'Could not safely discard the animation export.',
        }))
        coreOperationRef.current = false
        setCoreBusy(false)
        return
      }
      coreOperationRef.current = false
      setCoreBusy(false)
    }
    setMeshAnimationExportOpen(false)
    setMeshAnimationExportPreview(null)
    setMeshAnimationExportError(null)
    setMeshAnimationExportNotice(null)
    requestAnimationFrame(() => meshAnimationExportButtonRef.current?.focus())
  }

  async function saveCurrentMeshAnimationExport() {
    const preview = meshAnimationExportPreview
    const current = latestSnapshotRef.current
    if (!preview || !current || coreOperationRef.current) return
    if (
      current.project_instance_id !== preview.projectInstanceId
      || current.project_id !== preview.projectId
      || current.revision !== preview.revision
    ) {
      setMeshAnimationExportError(appMessage({
        ja: 'プロジェクトが変更されました。現在の手順から再作成してください。',
        en: 'The project changed. Rebuild from the current instructions.',
      }))
      return
    }
    coreOperationRef.current = true
    setCoreBusy(true)
    setFileOperation('mesh_animation_export')
    setMeshAnimationExportError(null)
    setMeshAnimationExportNotice(null)
    try {
      const response = await saveInstructionMeshAnimation({
        exportId: preview.exportId,
        expectedProjectInstanceId: preview.projectInstanceId,
        expectedProjectId: preview.projectId,
        expectedRevision: preview.revision,
        expectedSourceFingerprint: preview.sourceFingerprint,
      })
      if (response.canceled) {
        setMeshAnimationExportNotice(appMessage({
          ja: '保存先の選択をキャンセルしました。同じ生成データで再試行できます。',
          en: 'Save location selection was cancelled. You can retry with the same generated data.',
        }))
        return
      }
      setMeshAnimationExportOpen(false)
      setMeshAnimationExportPreview(null)
      setCoreStatus(appMessage({
        ja: '{fileName} を保存しました',
        en: 'Exported {fileName}',
      }, { fileName: preview.suggestedFileName }))
      requestAnimationFrame(() => meshAnimationExportButtonRef.current?.focus())
    } catch {
      const error = appMessage({
        ja: '手順が変更されたか、ファイルを保存できませんでした。再作成してから再試行してください。',
        en: 'The instructions changed or the file could not be saved. Rebuild and retry.',
      })
      setMeshAnimationExportError(error)
      setCoreStatus(error)
    } finally {
      setFileOperation(null)
      coreOperationRef.current = false
      setCoreBusy(false)
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

  function moveBenchmarkVertex(vertexId: string, x: number, y: number) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) return
    setBenchmarkRun((current) => {
      if (!current || !current.vertices.some(({ id }) => id === vertexId)) {
        return current
      }
      return {
        ...current,
        vertices: current.vertices.map((vertex) =>
          vertex.id === vertexId ? { ...vertex, x, y } : vertex),
        lines: current.lines.map((line) => ({
          ...line,
          x1: line.startVertexId === vertexId ? x : line.x1,
          y1: line.startVertexId === vertexId ? y : line.y1,
          x2: line.endVertexId === vertexId ? x : line.x2,
          y2: line.endVertexId === vertexId ? y : line.y2,
        })),
      }
    })
  }

  function deleteBenchmarkLine(lineId: string) {
    setBenchmarkRun((current) => {
      if (!current || !current.lines.some(({ id }) => id === lineId)) return current
      return { ...current, lines: current.lines.filter(({ id }) => id !== lineId) }
    })
    setSelectedLineId(null)
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
            type="button"
            disabled={
              coreBusy
              || !nativeSnapshot
              || !isNativeProjectFolderAvailable()
            }
            title={text({
              ja: 'manifestとハッシュを検証して、展開済みプロジェクトフォルダーを開きます',
              en: 'Open an expanded project folder after validating its manifest and hashes',
            })}
            onClick={() => void runProjectFolderOperation('folder_open')}
          >
            {fileOperation === 'folder_open'
              ? text({ ja: 'フォルダー確認中…', en: 'Checking folder…' })
              : text({ ja: '展開フォルダーを開く', en: 'Open expanded folder' })}
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
          <Fold3dFramesLauncher
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onApplied={async () => applySnapshot(await getProjectSnapshot())}
          />
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
            ref={meshExportButtonRef}
            type="button"
            disabled={
              coreBusy
              || benchmarkLoading
              || Boolean(benchmarkRun)
              || !nativeSnapshot
              || !appliedFoldPose
              || appliedFoldPose.state === 'running'
              || appliedFoldPose.projectId !== nativeSnapshot.project_id
              || appliedFoldPose.revision !== nativeSnapshot.revision
            }
            title={text({
              ja: '現在表示中の3D姿勢を中央面メッシュとして書き出します',
              en: 'Export the currently displayed 3D pose as a mid-surface mesh',
            })}
            onClick={beginStaticMeshExport}
            aria-haspopup="dialog"
          >
            {fileOperation === 'mesh_export'
              ? text({ ja: '3D生成中…', en: 'Generating 3D…' })
              : text({ ja: '3D書出し', en: 'Export 3D' })}
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
            disabled={
              coreBusy
              || !nativeSnapshot
              || !isNativeProjectFolderAvailable()
            }
            title={text({
              ja: '選択した親フォルダーへ展開形式で保存します。ローカルNTFS/ReFSでは同じプロジェクトの既存フォルダーを安全に置き換え、それ以外の保存先では新規保存だけを行います。別のプロジェクトは上書きしません',
              en: 'Save an expanded folder inside the selected parent. On local NTFS/ReFS, an existing folder for the same project is replaced safely; other destinations allow only a new save. A different project is never overwritten',
            })}
            onClick={() => void runProjectFolderOperation('folder_save')}
          >
            {fileOperation === 'folder_save'
              ? text({ ja: '展開保存中…', en: 'Saving folder…' })
              : text({ ja: '展開フォルダー保存', en: 'Save expanded folder' })}
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
              disabled={
                coreBusy
                || (id === 'cut' && !nativeSnapshot?.cutting_allowed)
                || (
                  id !== 'select'
                  && id !== 'measure'
                  && nativeLayerView.defaultLayerLocked
                )
              }
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
              paperPattern={builtinPaperPatternFromAsset(
                nativeSnapshot?.paper.front.texture_asset,
              )}
              vertices={displayedVertices}
              faces={benchmarkRun ? [] : canvasFaces}
              tool={benchmarkRun ? 'select' : activeTool}
              selectedVertexId={selectedVertexId}
              selectedFaceId={selectedFaceId}
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
              compassCircles={benchmarkRun ? [] : compassCircles}
              validationVertexHighlights={canvasLocalFlatFoldabilityHighlights}
              lockedVertexIds={
                benchmarkRun ? undefined : nativeLayerView.lockedVertexIds
              }
              ariaDescribedBy={localFlatFoldabilitySummaryId}
              cancelInteractionToken={cancelInteractionToken}
              disabled={coreBusy || benchmarkLoading}
              renderMetricsRequestId={benchmarkRun?.requestId ?? null}
              onRenderMetrics={recordBenchmarkRenderMetrics}
              onSelectLine={(lineId) => {
                setSelectedLineId(lineId)
                if (lineId) {
                  setSelectedVertexId(null)
                  setSelectedFaceId(null)
                }
              }}
              onSelectFace={benchmarkRun
                ? undefined
                : (faceId) => {
                    setSelectedFaceId(faceId)
                    if (faceId) {
                      setSelectedLineId(null)
                      setSelectedVertexId(null)
                    }
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
                    setSelectedFaceId(null)
                  }
                : selectCanvasVertex}
              onMoveVertex={benchmarkRun
                ? moveBenchmarkVertex
                : (vertexId, x, y) => {
                    if (nativeLayerView.lockedVertexIds.has(vertexId)) return
                    void runNativeEdit((projectId, revision, projectInstanceId) =>
                      moveVertex(projectId, revision, projectInstanceId, vertexId, x, y))
                  }}
            />
          </article>

          <WorkspaceLayoutSeparator kind="editor" />

          <article id="fold-preview-panel" className="panel preview-panel">
            <div className="panel-heading">
              <span>{text({ ja: '3D プレビュー', en: '3D preview' })}</span>
              <label>
                <input
                  type="checkbox"
                  checked={autoRecordInstructions}
                  disabled={coreBusy || Boolean(benchmarkRun) || !nativeSnapshot}
                  onChange={(event) => {
                    lastAutoRecordedPoseSequenceRef.current = manualPoseChangeSequence
                    setAutoRecordInstructions(event.currentTarget.checked)
                  }}
                />
                {text({ ja: '3D操作を自動記録', en: 'Auto-record 3D edits' })}
              </label>
              <span className={foldPreviewStatusClass}>{foldPreviewStatus}</span>
            </div>
            <FoldPreview
              angle={foldAngle}
              hingeAngles={foldTreeHingeAngles}
              selectedHingeId={selectedPreviewHingeId}
              selectedFaceId={selectedFaceId}
              selectedVertexId={selectedVertexId}
              fixedFaceId={effectiveFixedFaceId}
              instructionVisual={instructionVisual}
              onSelectHinge={benchmarkRun || foldPreviewHingeIds.size === 0
                ? undefined
                : (edgeId) => {
                    if (!nativeLines.some(({ id }) => id === edgeId)) return
                    setSelectedLineId(edgeId)
                    if (edgeId) {
                      setSelectedVertexId(null)
                      setSelectedFaceId(null)
                    }
                  }}
              onSelectFace={benchmarkRun
                ? undefined
                : (faceId) => {
                    if (
                      faceId
                      && !foldPreviewModel?.faces.some((face) => face.id === faceId)
                    ) return
                    setSelectedFaceId(faceId)
                    if (faceId) {
                      setSelectedLineId(null)
                      setSelectedVertexId(null)
                    }
                  }}
              onSelectVertex={benchmarkRun
                ? undefined
                  : (vertexId) => {
                    if (
                      vertexId
                      && !nativeSnapshot?.crease_pattern.vertices.some(
                        (vertex) => vertex.id === vertexId,
                      )
                    ) return
                    setSelectedVertexId(vertexId)
                    if (vertexId) {
                      setSelectedLineId(null)
                      setSelectedFaceId(null)
                    }
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
              frontTextureAsset={nativeSnapshot?.paper.front.texture_asset}
              backTextureAsset={nativeSnapshot?.paper.back.texture_asset}
              thicknessMm={nativeSnapshot?.paper.thickness_mm}
              lengthDisplayUnit={lengthDisplayUnit}
            />
            {topologyResponse && !topologyResponse.simulation_ready && (
              <section className="validation-report invalid topology-blockers">
                <h2>{text({
                  ja: '3D移行を妨げている問題',
                  en: 'Issues blocking 3D',
                })}</h2>
                <p>{formattedText({
                  ja: '{count}件の問題を解消するまで3D折り操作へ移行できません。',
                  en: 'Resolve these {count} issues before entering 3D folding.',
                }, { count: topologyResponse.issues.length })}</p>
                <ul>
                  {topologyResponse.issues.map((issue, index) => {
                    const locations = topologyIssueLocations(issue.kind)
                    return (
                      <li key={`${issue.kind.kind}:${index}`}>
                        <span className="topology-issue-reason">
                          {topologyIssueLabel(issue.kind, locale)}
                        </span>
                        {locations.length > 0 && (
                          <div className="topology-issue-locations">
                            {locations.map((location) => (
                              <button
                                type="button"
                                key={`${location.kind}:${location.id}`}
                                onClick={() => {
                                  if (location.kind === 'edge') {
                                    if (!nativeLines.some((line) => line.id === location.id)) return
                                    setSelectedLineId(location.id)
                                    setSelectedVertexId(null)
                                    setSelectedFaceId(null)
                                  } else {
                                    if (!nativeVertices.some((vertex) => vertex.id === location.id)) return
                                    setSelectedVertexId(location.id)
                                    setSelectedLineId(null)
                                    setSelectedFaceId(null)
                                  }
                                }}
                              >
                                {location.kind === 'edge'
                                  ? text({ ja: '線', en: 'Line' })
                                  : text({ ja: '頂点', en: 'Vertex' })}
                                {' '}
                                {location.id}
                              </button>
                            ))}
                          </div>
                        )}
                      </li>
                    )
                  })}
                </ul>
              </section>
            )}
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
            {selectedElementTarget && (
              <form
                key={`${selectedElementTarget.kind}:${selectedElementTarget.id}:${nativeSnapshot?.revision ?? 0}`}
                className="element-metadata-form"
                onSubmit={submitElementMetadata}
              >
                <label className="field">
                  <span>{text({ ja: '名前', en: 'Name' })}</span>
                  <input
                    name="element_name"
                    type="text"
                    maxLength={120}
                    defaultValue={selectedElementMetadata?.name ?? ''}
                    disabled={coreBusy}
                  />
                </label>
                <label className="field">
                  <span>{text({ ja: 'メモ', en: 'Memo' })}</span>
                  <textarea
                    name="element_memo"
                    maxLength={4_000}
                    defaultValue={selectedElementMetadata?.memo ?? ''}
                    disabled={coreBusy}
                  />
                </label>
                <label className="check">
                  <input
                    name="element_use_color"
                    type="checkbox"
                    defaultChecked={Boolean(selectedElementMetadata?.color)}
                    disabled={coreBusy}
                  />{' '}
                  {text({ ja: '個別色を使用', en: 'Use custom color' })}
                </label>
                <label className="paper-color-field">
                  <span>{text({ ja: '色', en: 'Color' })}</span>
                  <input
                    name="element_color"
                    type="color"
                    defaultValue={rgbaToHex(
                      selectedElementMetadata?.color ?? undefined,
                      '#4b82c3',
                    )}
                    disabled={coreBusy}
                  />
                </label>
                <button type="submit" disabled={coreBusy}>
                  {text({ ja: '要素情報を保存', en: 'Save element details' })}
                </button>
              </form>
            )}
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
                  <>
                    <button
                      type="button"
                      className="danger"
                      onClick={() => deleteBenchmarkLine(selectedLine.id)}
                    >
                      {text({ ja: '性能データの線を削除', en: 'Delete benchmark line' })}
                    </button>
                    <p className="muted">
                      {text({
                        ja: '1万本データ上で選択・計測・頂点移動・線削除を検証できます。',
                        en: 'Selection, measurement, vertex movement, and line deletion are available on the 10,000-edge data.',
                      })}
                    </p>
                  </>
                ) : (
                  <>
                  <form onSubmit={(event) => void submitMoveSelectedEdge(event)}>
                    <fieldset disabled={coreBusy || selectedLine.locked}>
                      <legend>{text({ ja: '線全体を移動', en: 'Move entire line' })}</legend>
                      <label className="field">
                        {formattedText({
                          ja: '横移動量 ({unit})',
                          en: 'Horizontal offset ({unit})',
                        }, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="edge_delta_x_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <label className="field">
                        {formattedText({
                          ja: '縦移動量 ({unit})',
                          en: 'Vertical offset ({unit})',
                        }, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="edge_delta_y_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <div className="property-actions">
                        <button type="submit">
                          {text({ ja: '線全体を移動', en: 'Move entire line' })}
                        </button>
                      </div>
                    </fieldset>
                  </form>
                  <form onSubmit={(event) => void submitMirrorSelectedEdge(event)}>
                    <fieldset disabled={coreBusy || selectedLine.locked}>
                      <legend>{text({ ja: '左右対称編集', en: 'Left-right symmetry' })}</legend>
                      <label className="field">
                        {formattedText({
                          ja: '対称軸 X ({unit})',
                          en: 'Mirror axis X ({unit})',
                        }, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="symmetry_axis_x_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <button type="submit">
                        {text({ ja: '左右反転を適用', en: 'Apply left-right reflection' })}
                      </button>
                    </fieldset>
                  </form>
                  <form onSubmit={(event) => void submitRotateSelectedEdge(event)}>
                    <fieldset disabled={coreBusy || selectedLine.locked}>
                      <legend>{text({ ja: '回転対称編集', en: 'Rotational symmetry' })}</legend>
                      <label className="field">
                        {formattedText({
                          ja: '中心 X ({unit})',
                          en: 'Center X ({unit})',
                        }, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="rotation_center_x_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <label className="field">
                        {formattedText({
                          ja: '中心 Y ({unit})',
                          en: 'Center Y ({unit})',
                        }, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="rotation_center_y_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <label className="field">
                        {text({ ja: '回転角度 (°)', en: 'Rotation angle (°)' })}
                        <input
                          name="rotation_angle_degrees"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="180"
                        />
                      </label>
                      <button type="submit">
                        {text({ ja: '回転を適用', en: 'Apply rotation' })}
                      </button>
                    </fieldset>
                  </form>
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
                        disabled={coreBusy || selectedLine.locked}
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
                        disabled={coreBusy || selectedLine.locked}
                        onClick={() => void deleteSelection()}
                      >
                        {text({ ja: '線を削除', en: 'Delete line' })}
                      </button>
                    )}
                  </div>
                  </>
                )}
                {selectedLine.locked && (
                  <p className="muted">
                    {text({
                      ja: 'この線のレイヤーは編集ロック中です。選択・計測・参照はできますが、図形は変更できません。',
                      en: 'This line layer is locked. Selection, measurement, and references remain available, but geometry cannot be changed.',
                    })}
                  </p>
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
            ) : selectedFace ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedFace.id}</dd></div>
                  <div>
                    <dt>{text({ ja: '境界頂点', en: 'Boundary vertices' })}</dt>
                    <dd>{selectedFace.vertexIds.length}</dd>
                  </div>
                  <div>
                    <dt>{text({ ja: '境界線', en: 'Boundary lines' })}</dt>
                    <dd>{selectedFace.edgeIds.length}</dd>
                  </div>
                </dl>
                <form onSubmit={(event) => void submitMoveSelectedFace(event)}>
                  <fieldset disabled={coreBusy || selectedFaceLocked}>
                    <legend>{text({ ja: '面全体を移動', en: 'Move entire face' })}</legend>
                    <label className="field">
                      {formattedText({
                        ja: '横移動量 ({unit})',
                        en: 'Horizontal offset ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                      <input
                        name="face_delta_x_display"
                        type="text"
                        inputMode="text"
                        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                        defaultValue="0"
                      />
                    </label>
                    <label className="field">
                      {formattedText({
                        ja: '縦移動量 ({unit})',
                        en: 'Vertical offset ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                      <input
                        name="face_delta_y_display"
                        type="text"
                        inputMode="text"
                        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                        defaultValue="0"
                      />
                    </label>
                    <div className="property-actions">
                      <button type="submit">
                        {text({ ja: '面全体を移動', en: 'Move entire face' })}
                      </button>
                    </div>
                  </fieldset>
                </form>
                <form onSubmit={(event) => void submitSplitSelectedFace(event)}>
                  <fieldset disabled={
                    coreBusy || selectedFaceLocked || selectedFace.vertexIds.length < 4
                  }>
                    <legend>{text({
                      ja: '面を追加・分割',
                      en: 'Add or split a face',
                    })}</legend>
                    <label className="field">
                      {text({ ja: '始点', en: 'Start vertex' })}
                      <select
                        name="face_split_start"
                        defaultValue={selectedFace.vertexIds[0]}
                      >
                        {selectedFace.vertexIds.map((vertexId, index) => (
                          <option value={vertexId} key={vertexId}>
                            {formattedText({
                              ja: '頂点 {index}: {id}',
                              en: 'Vertex {index}: {id}',
                            }, { index: index + 1, id: vertexId })}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="field">
                      {text({ ja: '終点', en: 'End vertex' })}
                      <select
                        name="face_split_end"
                        defaultValue={selectedFace.vertexIds[2]}
                      >
                        {selectedFace.vertexIds.map((vertexId, index) => (
                          <option value={vertexId} key={vertexId}>
                            {formattedText({
                              ja: '頂点 {index}: {id}',
                              en: 'Vertex {index}: {id}',
                            }, { index: index + 1, id: vertexId })}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="field">
                      {text({ ja: '分割線種', en: 'Split line type' })}
                      <select name="face_split_kind" defaultValue="mountain">
                        <option value="mountain">
                          {text({ ja: '山折り', en: 'Mountain fold' })}
                        </option>
                        <option value="valley">
                          {text({ ja: '谷折り', en: 'Valley fold' })}
                        </option>
                        <option value="auxiliary">
                          {text({ ja: '補助線', en: 'Auxiliary line' })}
                        </option>
                        {nativeSnapshot?.cutting_allowed && (
                          <option value="cut">
                            {text({ ja: '切断線', en: 'Cut' })}
                          </option>
                        )}
                      </select>
                    </label>
                    <div className="property-actions">
                      <button type="submit">
                        {text({ ja: '分割して面を追加', en: 'Split and add face' })}
                      </button>
                    </div>
                  </fieldset>
                </form>
                <form onSubmit={(event) => void submitMergeSelectedFace(event)}>
                  <fieldset disabled={
                    coreBusy || selectedFaceLocked || selectedFaceRemovableEdges.length === 0
                  }>
                    <legend>{text({
                      ja: '面を削除・統合',
                      en: 'Delete or merge face',
                    })}</legend>
                    <label className="field">
                      {text({ ja: '削除する共有線', en: 'Shared line to remove' })}
                      <select name="face_merge_edge">
                        {selectedFaceRemovableEdges.map((line) => (
                          <option value={line.id} key={line.id}>
                            {lineKindLabel(line.kind, locale)}: {line.id}
                          </option>
                        ))}
                      </select>
                    </label>
                    <div className="property-actions">
                      <button type="submit" className="danger">
                        {text({
                          ja: '共有線を削除して面を統合',
                          en: 'Remove line and merge face',
                        })}
                      </button>
                    </div>
                  </fieldset>
                </form>
                {selectedFaceLocked && (
                  <p className="muted">
                    {text({
                      ja: '面の境界にロック中のレイヤーが含まれるため移動できません。',
                      en: 'This face cannot move because its boundary includes a locked layer.',
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
                    ja: '性能データの頂点は2D上でドラッグして移動できます。',
                    en: 'Drag the benchmark vertex in 2D to move it and its incident lines.',
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
                  key={`${selectedVertex.id}:${selectedVertex.position.x}:${selectedVertex.position.y}:${lengthDisplayUnit.key}:${selectedVertexExpression?.x_source ?? ''}:${selectedVertexExpression?.y_source ?? ''}`}
                  className="coordinate-form"
                  onSubmit={submitVertexPosition}
                >
                  <label className="field">
                    {`X (${lengthDisplayUnitLabelText})`}
                    <input
                      name="x_display"
                      type="text"
                      inputMode="text"
                      maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                      defaultValue={lengthDisplayUnit.millimetresPerUnit === 1
                        && selectedVertexExpression
                        ? selectedVertexExpression.x_source
                        : formatLengthInput(
                            selectedVertex.position.x,
                            lengthDisplayUnit,
                          )}
                      disabled={coreBusy || selectedVertexLocked}
                      aria-label={formattedText({
                        ja: '頂点のX座標 ({unit})',
                        en: 'Vertex X coordinate ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <label className="field">
                    {`Y (${lengthDisplayUnitLabelText})`}
                    <input
                      name="y_display"
                      type="text"
                      inputMode="text"
                      maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                      defaultValue={lengthDisplayUnit.millimetresPerUnit === 1
                        && selectedVertexExpression
                        ? selectedVertexExpression.y_source
                        : formatLengthInput(
                            selectedVertex.position.y,
                            lengthDisplayUnit,
                          )}
                      disabled={coreBusy || selectedVertexLocked}
                      aria-label={formattedText({
                        ja: '頂点のY座標 ({unit})',
                        en: 'Vertex Y coordinate ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <div className="property-actions">
                    <button
                      type="submit"
                      name="vertex_action"
                      value="update_coordinates"
                      disabled={coreBusy || selectedVertexLocked}
                    >
                      {text({ ja: '座標を更新', en: 'Update coordinates' })}
                    </button>
                    <button
                      type="button"
                      className="danger"
                      disabled={
                        coreBusy ||
                        selectedVertexLocked ||
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
                  {selectedVertexExpression?.polar_construction ? (
                    <p className="muted" data-vertex-polar-expression>
                      {formattedText({
                        ja: '作図式: 長さ {length} mm / 角度 {angle}°（評価値 {lengthValue} mm / {angleValue}°）',
                        en: 'Construction expression: length {length} mm / angle {angle}° (evaluated {lengthValue} mm / {angleValue}°)',
                      }, {
                        length: selectedVertexExpression.polar_construction.length_source,
                        angle: selectedVertexExpression.polar_construction.angle_degrees_source,
                        lengthValue: selectedVertexExpression.polar_construction.adopted_length_mm,
                        angleValue: selectedVertexExpression.polar_construction.adopted_angle_degrees,
                      })}
                    </p>
                  ) : null}
                  <fieldset>
                    <legend>
                      {text({ ja: '長さ・角度指定の終点', en: 'Endpoint by length and angle' })}
                    </legend>
                    <label className="field">
                      {`${text({ ja: '長さ', en: 'Length' })} (${lengthDisplayUnitLabelText})`}
                      <input
                        name="polar_length_display"
                        type="text"
                        inputMode="text"
                        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                        defaultValue={formatLengthInput(10, lengthDisplayUnit)}
                        disabled={coreBusy || selectedVertexLocked}
                        aria-label={formattedText({
                          ja: '始点からの長さ ({unit})',
                          en: 'Length from the start vertex ({unit})',
                        }, { unit: lengthDisplayUnitLabelText })}
                      />
                    </label>
                    <label className="field">
                      {text({ ja: '角度 (度)', en: 'Angle (degrees)' })}
                      <input
                        name="polar_angle_degrees"
                        type="text"
                        inputMode="text"
                        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                        defaultValue="0"
                        disabled={coreBusy || selectedVertexLocked}
                        aria-label={text({
                          ja: '始点からの角度 (度)',
                          en: 'Angle from the start vertex (degrees)',
                        })}
                      />
                    </label>
                    <label className="field">
                      {text({ ja: '線種', en: 'Line type' })}
                      <select
                        name="polar_edge_kind"
                        defaultValue="mountain"
                        disabled={coreBusy || selectedVertexLocked}
                        aria-label={text({
                          ja: '長さ・角度指定作図の線種',
                          en: 'Line type for length and angle drawing',
                        })}
                      >
                        <option value="mountain">
                          {text({ ja: '山折り', en: 'Mountain fold' })}
                        </option>
                        <option value="valley">
                          {text({ ja: '谷折り', en: 'Valley fold' })}
                        </option>
                        <option value="auxiliary">
                          {text({ ja: '補助線', en: 'Auxiliary line' })}
                        </option>
                        {nativeSnapshot?.cutting_allowed && (
                          <option value="cut">
                            {text({ ja: '切断線', en: 'Cut' })}
                          </option>
                        )}
                      </select>
                    </label>
                    <div className="property-actions">
                      <button
                        type="submit"
                        name="vertex_action"
                        value="polar_endpoint"
                        disabled={coreBusy || selectedVertexLocked}
                      >
                        {text({
                          ja: '長さと角度から線を作図',
                          en: 'Draw line by length and angle',
                        })}
                      </button>
                    </div>
                  </fieldset>
                  <fieldset>
                    <legend>
                      {text({ ja: 'コンパス円', en: 'Compass circle' })}
                    </legend>
                    <label className="field">
                      {`${text({ ja: '半径', en: 'Radius' })} (${lengthDisplayUnitLabelText})`}
                      <input
                        name="compass_radius_display"
                        type="number"
                        inputMode="decimal"
                        min="0.000001"
                        step="any"
                        defaultValue="10"
                        disabled={coreBusy}
                      />
                    </label>
                    <div className="property-actions">
                      <button
                        type="button"
                        disabled={coreBusy}
                        onClick={(event) => {
                          const form = event.currentTarget.form
                          const input = form?.elements.namedItem('compass_radius_display')
                          if (!(input instanceof HTMLInputElement)) return
                          const displayRadius = Number(input.value)
                          const radius = displayRadius
                            * lengthDisplayUnit.millimetresPerUnit
                          if (!Number.isFinite(radius) || radius <= 0) return
                          setCompassCircles((current) => [
                            ...current,
                            {
                              centerX: selectedVertex.position.x,
                              centerY: selectedVertex.position.y,
                              radius,
                            },
                          ].slice(-64))
                        }}
                      >
                        {text({ ja: '選択頂点を中心に円を追加', en: 'Add circle at selected vertex' })}
                      </button>
                      <button
                        type="button"
                        disabled={coreBusy || compassCircles.length === 0}
                        onClick={() => setCompassCircles([])}
                      >
                        {text({ ja: 'コンパス円を消去', en: 'Clear compass circles' })}
                      </button>
                    </div>
                    <p className="muted">
                      {formattedText({
                        ja: '補助円 {count} 個。交点を見ながら定規相当の線作図を行えます。',
                        en: '{count} construction circles. Use their visible intersections with the ruler-equivalent line tools.',
                      }, { count: compassCircles.length })}
                    </p>
                  </fieldset>
                  {selectedVertexLocked && (
                    <p className="muted">
                      {text({
                        ja: 'この頂点にはロック中のレイヤーの線が接続されているため、移動・削除できません。',
                        en: 'This vertex is connected to a line on a locked layer and cannot be moved or deleted.',
                      })}
                    </p>
                  )}
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
            ) : nativeSnapshot && !benchmarkRun ? (
              <>
                <p className="muted">
                  {text({
                    ja: '線または頂点を選択するか、座標を指定して頂点を追加します。',
                    en: 'Select a line or vertex, or add a vertex by coordinates.',
                  })}
                </p>
                <form
                  key={`${nativeSnapshot.project_instance_id}:${lengthDisplayUnit.key}`}
                  className="coordinate-form"
                  onSubmit={(event) => void submitDirectVertex(event)}
                >
                  <label className="field">
                    {`X (${lengthDisplayUnitLabelText})`}
                    <input
                      name="direct_x_display"
                      type="text"
                      inputMode="text"
                      maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                      defaultValue="0"
                      disabled={coreBusy || nativeLayerView.defaultLayerLocked}
                      aria-label={formattedText({
                        ja: '新しい頂点のX座標 ({unit})',
                        en: 'New vertex X coordinate ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <label className="field">
                    {`Y (${lengthDisplayUnitLabelText})`}
                    <input
                      name="direct_y_display"
                      type="text"
                      inputMode="text"
                      maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                      defaultValue="0"
                      disabled={coreBusy || nativeLayerView.defaultLayerLocked}
                      aria-label={formattedText({
                        ja: '新しい頂点のY座標 ({unit})',
                        en: 'New vertex Y coordinate ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <div className="property-actions">
                    <button
                      type="submit"
                      disabled={coreBusy || nativeLayerView.defaultLayerLocked}
                    >
                      {text({ ja: '座標から頂点を追加', en: 'Add vertex by coordinates' })}
                    </button>
                  </div>
                  {nativeLayerView.defaultLayerLocked && (
                    <p className="muted">
                      {text({
                        ja: '既定レイヤーがロックされているため頂点を追加できません。',
                        en: 'Unlock the default layer before adding a vertex.',
                      })}
                    </p>
                  )}
                </form>
              </>
            ) : (
              <p className="muted">
                {text({
                  ja: '線または頂点を選択してください。',
                  en: 'Select a line or vertex.',
                })}
              </p>
            )}
          </section>
          {nativeSnapshot && !benchmarkRun && (
            <section className="property-section">
              <h2>{text({ ja: 'プロジェクトメモ', en: 'Project memo' })}</h2>
              <form
                key={`${nativeSnapshot.project_instance_id}:${nativeSnapshot.memo}`}
                onSubmit={(event) => void submitProjectMemo(event)}
              >
                <label>
                  <span>{text({ ja: 'メモ', en: 'Notes' })}</span>
                  <textarea
                    name="project_memo"
                    maxLength={16_000}
                    rows={5}
                    defaultValue={nativeSnapshot.memo}
                    disabled={coreBusy || recoveryBlocking}
                  />
                </label>
                <div className="property-actions">
                  <button type="submit" disabled={coreBusy || recoveryBlocking}>
                    {text({ ja: 'メモを保存', en: 'Save memo' })}
                  </button>
                </div>
              </form>
            </section>
          )}
          {nativeSnapshot && !benchmarkRun && (
            <StackedFoldPanel
              locale={locale}
              snapshot={nativeSnapshot}
              selectedLine={selectedLine ? {
                id: selectedLine.id,
                start: { x: selectedLine.x1, y: selectedLine.y1 },
                end: { x: selectedLine.x2, y: selectedLine.y2 },
              } : null}
              disabled={coreBusy || recoveryBlocking}
              refreshSnapshot={requestProjectSnapshot}
              onApplied={(snapshot) => {
                applySnapshot(snapshot)
                setSelectedLineId(null)
                setSelectedVertexId(null)
                setSelectedFaceId(null)
                setCoreStatus(appMessage({
                  ja: '折り重ねを原子的に適用しました。Undoで全体を戻せます。',
                  en: 'The stacked fold was applied atomically. Undo restores the whole change.',
                }))
              }}
            />
          )}
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
              onUpdatePresentation={updateLayerPresentationFromPanel}
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
              onAddConstraint={addConstraint}
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
                        nativeVertices.some((vertex) => vertex.id === id))
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
                                  if (!nativeVertices.some(
                                    ({ id }) => id === item.vertexId,
                                  )) return
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
              <div className="paper-color-fields">
                <label className="paper-color-field">
                  <span>{text({ ja: '表の模様', en: 'Front pattern' })}</span>
                  <select
                    name="front_pattern"
                    defaultValue={builtinPaperPatternFromAsset(
                      nativeSnapshot?.paper.front.texture_asset,
                    ) ?? (nativeSnapshot?.paper.front.texture_asset ? 'custom' : 'none')}
                    disabled={coreBusy || !nativeSnapshot}
                  >
                    <option value="none">{text({ ja: 'なし（単色）', en: 'None (solid)' })}</option>
                    <option value="dots">{text({ ja: 'ドット', en: 'Dots' })}</option>
                    <option value="grid">{text({ ja: '格子', en: 'Grid' })}</option>
                    <option value="stripes">{text({ ja: '縞', en: 'Stripes' })}</option>
                    {nativeSnapshot?.paper.front.texture_asset
                      && !builtinPaperPatternFromAsset(nativeSnapshot.paper.front.texture_asset)
                      ? <option value="custom">{text({ ja: '読み込んだ画像', en: 'Imported image' })}</option>
                      : null}
                  </select>
                  <button
                    type="button"
                    disabled={coreBusy || !nativeSnapshot}
                    onClick={chooseFrontPaperTexture}
                  >
                    {text({ ja: '画像を読み込む…', en: 'Import image…' })}
                  </button>
                </label>
                <label className="paper-color-field">
                  <span>{text({ ja: '裏の模様', en: 'Back pattern' })}</span>
                  <select
                    name="back_pattern"
                    defaultValue={builtinPaperPatternFromAsset(
                      nativeSnapshot?.paper.back.texture_asset,
                    ) ?? (nativeSnapshot?.paper.back.texture_asset ? 'custom' : 'none')}
                    disabled={coreBusy || !nativeSnapshot}
                  >
                    <option value="none">{text({ ja: 'なし（単色）', en: 'None (solid)' })}</option>
                    <option value="dots">{text({ ja: 'ドット', en: 'Dots' })}</option>
                    <option value="grid">{text({ ja: '格子', en: 'Grid' })}</option>
                    <option value="stripes">{text({ ja: '縞', en: 'Stripes' })}</option>
                    {nativeSnapshot?.paper.back.texture_asset
                      && !builtinPaperPatternFromAsset(nativeSnapshot.paper.back.texture_asset)
                      ? <option value="custom">{text({ ja: '読み込んだ画像', en: 'Imported image' })}</option>
                      : null}
                  </select>
                  <button
                    type="button"
                    disabled={coreBusy || !nativeSnapshot}
                    onClick={chooseBackPaperTexture}
                  >
                    {text({ ja: '画像を読み込む…', en: 'Import image…' })}
                  </button>
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
                    <input
                      name="width_display"
                      type="text"
                      inputMode="text"
                      maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                      defaultValue={formatLengthInput(
                        rectangularPaperSize?.width ?? 0,
                        lengthDisplayUnit,
                      )}
                      readOnly={rectangularRatioReferenceAxis === 'width'}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      aria-label={formattedText({
                        ja: '用紙の幅 ({unit})',
                        en: 'Paper width ({unit})',
                      }, { unit: lengthDisplayUnitLabelText })}
                    />
                    <span>{lengthDisplayUnitLabelText}</span>
                  </label>
                  <label className="field">
                    <span>{text({ ja: '高さ', en: 'Height' })}</span>
                    <input
                      name="height_display"
                      type="text"
                      inputMode="text"
                      maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                      defaultValue={formatLengthInput(
                        rectangularPaperSize?.height ?? 0,
                        lengthDisplayUnit,
                      )}
                      readOnly={rectangularRatioReferenceAxis === 'height'}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      aria-label={formattedText({
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
          <section className="fold-technique-workspace">
            <h2>
              {text({
                ja: '名前付き折り技法',
                en: 'Named fold techniques',
              })}
            </h2>
            <p className="muted">
              {text({
                ja: '複数の説明手順を宣言データとして作成・共有します。折り操作、プロジェクト変更、外部取得は自動実行しません。中割り・かぶせ・沈め折り・層選択運動は現在未対応の物理操作としてファイル内に明示します。',
                en: 'Create and share multiple instruction steps as declarative data. This never auto-runs folds, changes the project, or fetches external resources. Inside reverse, outside reverse, sink, and layer-selective motions are explicitly stored as unsupported physical operations.',
              })}
            </p>
            {foldTechniqueWorkspace && (
              <>
                <dl>
                  <div>
                    <dt>{text({ ja: 'パッケージID', en: 'Package ID' })}</dt>
                    <dd>{foldTechniqueWorkspace.document.package_id}</dd>
                  </div>
                  <div>
                    <dt>{text({ ja: '技法数', en: 'Techniques' })}</dt>
                    <dd>
                      {foldTechniqueWorkspace.document.techniques.length
                        .toLocaleString(locale)}
                    </dd>
                  </div>
                  <div>
                    <dt>{text({ ja: '共有状態', en: 'Share state' })}</dt>
                    <dd>
                      {foldTechniqueWorkspace.dirty
                        ? text({
                            ja: '変更あり・別名保存が必要',
                            en: 'Changed · Save as required',
                          })
                        : text({
                            ja: '保存済み',
                            en: 'Saved',
                          })}
                    </dd>
                  </div>
                </dl>
                <label className="dialog-field">
                  <span>
                    {text({
                      ja: 'タイムラインへ追加する技法',
                      en: 'Technique to add to timeline',
                    })}
                  </span>
                  <select
                    value={foldTechniqueSelectedIndex}
                    disabled={
                      coreBusy
                      || foldTechniqueBusy
                      || foldTechniqueTimelineBusy
                    }
                    onChange={(event) => {
                      const nextIndex = Number(event.currentTarget.value)
                      if (
                        Number.isSafeInteger(nextIndex)
                        && nextIndex >= 0
                        && nextIndex
                          < foldTechniqueWorkspace.document.techniques.length
                      ) setFoldTechniqueSelectedIndex(nextIndex)
                    }}
                  >
                    {foldTechniqueWorkspace.document.techniques.map(
                      (technique, techniqueIndex) => (
                        <option
                          key={`${technique.id}:${technique.version}`}
                          value={techniqueIndex}
                        >
                          {foldTechniqueLocalizedTextV1(
                            technique.names,
                            locale,
                          ) || foldTechniqueLocalizedTextV1(
                            technique.names,
                            locale === 'ja' ? 'en' : 'ja',
                          ) || technique.id}
                        </option>
                      ),
                    )}
                  </select>
                </label>
              </>
            )}
            <div className="property-actions fold-technique-actions">
              <button
                type="button"
                disabled={
                  coreBusy
                  || foldTechniqueBusy
                  || !isNativeFoldTechniqueFileAvailable()
                }
                aria-haspopup="dialog"
                onClick={(event) =>
                  openNewFoldTechniqueEditor(event.currentTarget)}
              >
                {text({ ja: '新規作成', en: 'Create' })}
              </button>
              <button
                type="button"
                disabled={
                  coreBusy
                  || foldTechniqueBusy
                  || !isNativeFoldTechniqueFileAvailable()
                }
                aria-haspopup="dialog"
                onClick={(event) =>
                  void importFoldTechniqueFile(event.currentTarget)}
              >
                {text({ ja: 'ファイル取込', en: 'Import file' })}
              </button>
              <button
                type="button"
                disabled={
                  coreBusy
                  || foldTechniqueBusy
                  || !foldTechniqueWorkspace
                }
                aria-haspopup="dialog"
                onClick={(event) =>
                  openCurrentFoldTechniqueEditor(event.currentTarget)}
              >
                {text({ ja: '編集', en: 'Edit' })}
              </button>
              <button
                type="button"
                disabled={
                  coreBusy
                  || foldTechniqueBusy
                  || !foldTechniqueWorkspace
                  || !isNativeFoldTechniqueFileAvailable()
                }
                onClick={() => void saveCurrentFoldTechniqueAs()}
              >
                {text({ ja: '別名保存', en: 'Save as' })}
              </button>
              <button
                type="button"
                disabled={
                  coreBusy
                  || foldTechniqueBusy
                  || foldTechniqueTimelineBusy
                  || !foldTechniqueWorkspace
                  || !nativeSnapshot
                  || !isNativeCoreAvailable()
                }
                aria-haspopup="dialog"
                onClick={(event) =>
                  previewSelectedFoldTechniqueTimeline(event.currentTarget)}
              >
                {text({
                  ja: '折り手順案を作成',
                  en: 'Build timeline proposal',
                })}
              </button>
            </div>
            {foldTechniqueBusy && (
              <p role="status" aria-live="polite">
                {text({
                  ja: '折り技法ファイルを処理しています…',
                  en: 'Processing the fold-technique file…',
                })}
              </p>
            )}
            {!isNativeFoldTechniqueFileAvailable() && (
              <p className="muted">
                {text({
                  ja: '安全なファイル選択と原子的保存はデスクトップ版で利用できます。',
                  en: 'Safe file selection and atomic saving are available in the desktop app.',
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
        animationExportButtonRef={meshAnimationExportButtonRef}
        inert={modalOpen}
        runNativeEdit={runNativeEdit}
        applyStepPose={applyInstructionStepPose}
        onExport={beginInstructionExport}
        onAnimationExport={beginMeshAnimationExport}
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

      {foldTechniqueEditor && (
        <FoldTechniqueEditorDialog
          key={`${foldTechniqueEditor.mode}:${foldTechniqueEditor.initialDocument.package_id}`}
          mode={foldTechniqueEditor.mode}
          initialDocument={foldTechniqueEditor.initialDocument}
          techniqueIndex={foldTechniqueEditor.techniqueIndex}
          busy={foldTechniqueBusy || coreBusy}
          saveFailed={foldTechniqueSaveFailed}
          onConfirm={(document) => {
            void confirmFoldTechniqueEditor(document)
          }}
          onCancel={closeFoldTechniqueEditor}
          onDirtyChange={noteFoldTechniqueEditorDirty}
          returnFocusTo={foldTechniqueEditorOpenerRef.current}
        />
      )}

      {foldTechniqueTimelinePreview && (
        <FoldTechniqueTimelinePreviewDialog
          preview={foldTechniqueTimelinePreview.preview}
          busy={foldTechniqueTimelineBusy}
          stale={foldTechniqueTimelinePreviewStale}
          error={foldTechniqueTimelineErrorText}
          onConfirm={() => void confirmFoldTechniqueTimelineProposal()}
          onCancel={closeFoldTechniqueTimelinePreview}
        />
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

      {meshExportOpen && (
        <StaticMeshExportDialog
          format={meshExportFormat}
          preview={meshExportPreview}
          busy={coreBusy}
          error={meshExportError}
          notice={meshExportNotice}
          onFormatChange={changeStaticMeshExportFormat}
          onRetry={() => void prepareStaticMeshExport(meshExportFormat)}
          onSave={(warningsAcknowledged) => {
            void saveCurrentStaticMeshExport(warningsAcknowledged)
          }}
          onCancel={() => void closeStaticMeshExportDialog()}
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

      {meshAnimationExportOpen && (
        <MeshAnimationExportDialog
          preview={meshAnimationExportPreview}
          busy={coreBusy}
          error={appMessageText(locale, meshAnimationExportError)}
          notice={appMessageText(locale, meshAnimationExportNotice)}
          onRetry={() => void prepareMeshAnimationExport()}
          onSave={() => void saveCurrentMeshAnimationExport()}
          onCancel={() => void closeMeshAnimationExport()}
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
        <UpdateCheckPopover />
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

function topologyIssueLocations(
  issue: ProjectTopologyResponse['issues'][number]['kind'],
) {
  switch (issue.kind) {
    case 'duplicate_vertex_id':
      return [{ kind: 'vertex' as const, id: issue.vertex }]
    case 'duplicate_edge_id':
    case 'unsupported_active_edge':
    case 'active_edge_outside_paper':
    case 'disconnected_fold_graph':
    case 'non_separating_fold':
    case 'unsupported_fold_graph':
    case 'invalid_edge_incidence':
    case 'unsupported_adjacent_boundary_fold':
    case 'degenerate_fold_face':
      return [{ kind: 'edge' as const, id: issue.edge }]
    case 'fold_endpoint_not_on_boundary':
    case 'unsupported_non_convex_fold_sheet':
      return [
        { kind: 'edge' as const, id: issue.edge },
        { kind: 'vertex' as const, id: issue.vertex },
      ]
    case 'too_many_active_fold_edges':
      return issue.edges.map((id) => ({ kind: 'edge' as const, id }))
    default:
      return []
  }
}

function topologyIssueLabel(
  issue: ProjectTopologyResponse['issues'][number]['kind'],
  locale: Locale,
) {
  const labels: Record<typeof issue.kind, LocalizedText> = {
    duplicate_vertex_id: { ja: '頂点IDが重複しています。', en: 'A vertex ID is duplicated.' },
    duplicate_edge_id: { ja: '線IDが重複しています。', en: 'A line ID is duplicated.' },
    invalid_paper: { ja: '用紙の輪郭または属性が不正です。', en: 'The paper boundary or properties are invalid.' },
    invalid_crease_pattern: { ja: '展開図の幾何が不正です。', en: 'The crease-pattern geometry is invalid.' },
    unsupported_active_edge: { ja: '3D化できない線種が含まれています。', en: 'A line kind cannot be converted to 3D.' },
    too_many_active_fold_edges: { ja: '有効な折り線が処理上限を超えています。', en: 'The active fold count exceeds the supported limit.' },
    active_edge_outside_paper: { ja: '折り線が用紙の外側にあります。', en: 'A fold line lies outside the paper.' },
    disconnected_fold_graph: { ja: '折り構造が分断されています。', en: 'The fold graph is disconnected.' },
    non_separating_fold: { ja: '折り線が面を2つに分離していません。', en: 'A fold line does not separate two faces.' },
    unsupported_fold_graph: { ja: '現在の3Dモデルで扱えない折り構造です。', en: 'The fold graph is unsupported by the current 3D model.' },
    invalid_edge_incidence: { ja: '線に接する面の構成が不正です。', en: 'A line has invalid face incidence.' },
    fold_endpoint_not_on_boundary: { ja: '折り線の端点が用紙輪郭上にありません。', en: 'A fold endpoint is not on the paper boundary.' },
    unsupported_adjacent_boundary_fold: { ja: '輪郭に隣接する折り線を3D化できません。', en: 'A boundary-adjacent fold is unsupported.' },
    unsupported_non_convex_fold_sheet: { ja: '非凸用紙上のこの折り線を3D化できません。', en: 'This fold on a non-convex sheet is unsupported.' },
    degenerate_fold_face: { ja: '折り線から面を構成できません。', en: 'A fold line produces a degenerate face.' },
    unrepresentable_face_area: { ja: '面積を安全に表現できません。', en: 'A face area cannot be represented safely.' },
    internal_boundary_resolution: { ja: '用紙輪郭から面を確定できません。', en: 'Faces could not be resolved from the paper boundary.' },
  }
  return selectLocalizedText(locale, labels[issue.kind])
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

function findElementMetadata(
  document: ProjectSnapshot['element_metadata'],
  target: ElementMetadataTarget,
): ElementMetadata | null {
  if (target.kind === 'vertex') {
    return document.vertices.find((record) => record.vertex === target.id)?.metadata ?? null
  }
  if (target.kind === 'edge') {
    return document.edges.find((record) => record.edge === target.id)?.metadata ?? null
  }
  return document.faces.find((record) => record.face === target.id)?.metadata ?? null
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

async function evaluateDisplayLengthExpression(
  source: string,
  unit: ResolvedLengthDisplayUnit,
) {
  const adopted = await evaluateFiniteNumericExpression(source)
  const millimetres = adopted.value * unit.millimetresPerUnit
  if (!Number.isFinite(millimetres)) {
    throw new Error('display length expression overflow')
  }
  return millimetres === 0 ? 0 : millimetres
}

function millimetreExpressionSource(source: string, millimetresPerUnit: number) {
  if (millimetresPerUnit === 1) return source
  return `(${source}) * ${finiteNumberExpressionSource(millimetresPerUnit)}`
}

function finiteNumberExpressionSource(value: number) {
  if (!Number.isFinite(value)) throw new Error('non-finite expression source')
  return String(value === 0 ? 0 : value)
}

function editExpressionErrorMessage(error: unknown) {
  const category = numericExpressionNativeErrorCategory(error)
  if (category === 'native_unavailable') {
    return appMessage({
      ja: '数式入力はデスクトップ版で利用できます。',
      en: 'Expression input is available in the desktop app.',
    })
  }
  if (category === 'resource_limit') {
    return appMessage({
      ja: '数式が複雑すぎるため評価を中止しました。',
      en: 'Evaluation stopped because the expression is too complex.',
    })
  }
  return appMessage({
    ja: '小数・分数・平方根・π・四則演算・括弧を使った有限の数式を入力してください。',
    en: 'Enter a finite expression using decimals, fractions, square roots, pi, operators, or parentheses.',
  })
}

function isEditingText(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.matches('input, textarea')) return true
  return target.isContentEditable || Boolean(target.closest('[contenteditable="true"]'))
}

function nextFoldTechniqueRequestId(reference: { current: number }): number {
  const next = reference.current >= 0xffff_ffff
    ? 1
    : reference.current + 1
  reference.current = next
  return next
}

export default App
