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
} from './components/CreaseCanvas'
import { CreaseExportDialog } from './components/CreaseExportDialog'
import { AnnotationPanel } from './components/AnnotationPanel'
import { UnderlayPanel } from './components/UnderlayPanel'
import { CreationDimensionExpressionSummary } from './components/CreationDimensionExpressionSummary'
import { DiagnosticsDialog } from './components/DiagnosticsDialog'
import { FoldImportDialog } from './components/FoldImportDialog'
import { Fold3dFramesLauncher } from './components/Fold3dFramesLauncher'
import { FoldPreview } from './components/FoldPreview'
import { EffectiveCutDiagnosticPanel } from './components/EffectiveCutDiagnosticPanel'
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
import { BulkIntersectionRepairControl } from './components/BulkIntersectionRepairControl'
import { PairMeasurementStatus } from './components/PairMeasurementStatus'
import type { InstructionOnionSkinRequest } from './lib/instructionOnionSkin'
import { useCreasePairMeasurement } from './lib/useCreasePairMeasurement'
import {
  addEdge,
  addAnnotation,
  addGeometricConstraint,
  addEdgeOrientationConstraint,
  addConnectedVertex,
  addRayToFirstTarget,
  addInstructionStep,
  addVertex,
  analyzeGeometricConstraints,
  analyzeProjectTopology,
  applyGeometricConstraintSolve,
  applyMirrorSelection,
  confirmLinearArray,
  confirmRadialArray,
  assignEdgeToProjectLayer,
  connectEdgeIntersection,
  connectIntersectionCluster,
  repairAllUnsplitIntersections,
  connectTJunction,
  createProjectLayer,
  deleteProjectLayer,
  generateBenchmarkPattern,
  getProjectSnapshot as requestProjectSnapshot,
  isNativeCoreAvailable,
  matchesProjectOccGuard,
  moveEdge,
  mirrorEdgeLeftRight,
  rotateEdgeAboutPoint,
  moveProjectLayer,
  moveVertices,
  moveVertex,
  newProject,
  previewGeometricConstraintSolve,
  previewGeometricConstraintEdgeSolve,
  previewGeometricConstraintExpressionSolve,
  preflightMirrorSelection,
  previewLinearArray,
  previewRadialArray,
  redo,
  removeAnnotation,
  removeUnderlay,
  renameProjectLayer,
  removeBoundaryVertex,
  removeEdge,
  removeGeometricConstraint,
  removeVertex,
  resizeRectangularPaper,
  setLengthDisplayUnit,
  setElementMetadata,
  splitBoundaryEdge,
  splitEdge,
  undo,
  updateAnnotation,
  updateUnderlay,
  importUnderlayImage,
  updateProjectLayerPresentation,
  updateProjectMemo,
  updatePaperProperties,
  importFrontPaperTexture,
  importBackPaperTexture,
  type ProjectSnapshot,
  type MirrorSelectionPreflight,
  type MirrorSelectionRequest,
  type LinearArrayPreview,
  type LinearArrayRequest,
  type RadialArrayPreview,
  type RadialArrayRequest,
  type GeometricConstraintKind,
  type ProjectTopologyResponse,
  type InstructionVisual,
  type ElementMetadata,
  type ElementMetadataTarget,
  type ValidationSnapshot,
  validateProject,
  proveCurrentAssignedLocalSufficiencyV1,
  type AssignedLocalSufficiencyResponseV1,
  type AssignedLocalSufficiencySummaryResponseV1,
} from './lib/coreClient'
import { runProjectFileOperation } from './lib/projectFileClient'
import { createAssignedLocalSufficiencySummaryCoordinator } from './lib/assignedLocalSufficiencySummaryCoordinator'
import { createProofScopePresentation } from './lib/proofScopePresentation'
import {
  isNativeProjectFolderAvailable,
  openProjectFolder,
  projectFolderClientErrorMessage,
  saveProjectFolderAs,
} from './lib/projectFolderClient'
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
import { APP_TEXT } from './lib/appText.ts'
import { updateGridPreferenceInput } from './lib/gridPreference'
import { useCanvasUnderlays } from './lib/useCanvasUnderlays'
import { useGridDivisionPreference } from './lib/useGridDivisionPreference'
import { useProjectCanvasProjection } from './lib/useProjectCanvasProjection'
import { useCreaseExportWorkflow } from './lib/useCreaseExportWorkflow'
import { useStaticMeshExportWorkflow } from './lib/useStaticMeshExportWorkflow'
import { useMeshAnimationExportWorkflow } from './lib/useMeshAnimationExportWorkflow'
import { useInstructionExportWorkflow } from './lib/useInstructionExportWorkflow'
import { useFoldImportWorkflow } from './lib/useFoldImportWorkflow'
import { useSvgImportWorkflow } from './lib/useSvgImportWorkflow'
import { useBeginnerEditorState } from './lib/useBeginnerEditorState'
import { useBeginnerProfileWorkflow } from './lib/useBeginnerProfileWorkflow'
import { useBeginnerCandidateWorkflow } from './lib/useBeginnerCandidateWorkflow'
import { useBeginnerParameterGridWorkflow } from './lib/useBeginnerParameterGridWorkflow'
import { useBeginnerReferenceWorkflow } from './lib/useBeginnerReferenceWorkflow'
import { useBeginnerRecognitionWorkflow } from './lib/useBeginnerRecognitionWorkflow'
import type { BeginnerNativeEditRunner } from './lib/beginnerWorkflowSupport'
import {
  appConfirmationText,
  appErrorLocalizedText,
} from './lib/appMessages'
import {
  createInitialFoldTechniqueDocumentV1,
  foldTechniqueLocalizedTextV1,
  type FoldTechniqueFileDocumentV1,
} from './lib/foldTechniqueEditor'
import { useFoldTechniqueTimelineProposal } from './lib/useFoldTechniqueTimelineProposal'
import {
  foldTechniqueFileClientErrorCode,
  isNativeFoldTechniqueFileAvailable,
  openFoldTechniqueFileV1,
  saveFoldTechniqueFileAsV1,
} from './lib/foldTechniqueFileClient'
import './App.css'
import { ProtrusionDimensionEditor } from './components/ProtrusionDimensionEditor'
import { GenericBodyOutlineEditor } from './components/GenericBodyOutlineEditor'
import { BeginnerShapeCanvasPreview } from './components/BeginnerShapeCanvasPreview'
import { RecognitionContourCopyAction } from './components/RecognitionContourCopyAction'
import { BeginnerCandidateControls } from './components/BeginnerCandidateControls'
import { BeginnerCandidateResults } from './components/BeginnerCandidateResults'
import { BeginnerRecognitionPanel } from './components/BeginnerRecognitionPanel'
import {
  formatBytes,
  lineKindLabel,
  localFlatFoldabilityCoreStatus,
  localizedLocalFlatFoldabilityConditionLabel,
  localizedLocalFlatFoldabilityReasonLabel,
  localizedLocalFlatFoldabilitySummary,
  normalizeFoldAngle,
  toolLabel,
  validationIssueLabel,
} from './lib/appPresentation'
import {
  formatAngleDegrees,
  formatLineMeasurementLabel,
  formatMeasurementValue,
  measureCreaseLine,
  resolveRectangularPaperSize,
  resolveUniqueParallelReference,
} from './lib/appGeometry'
import {
  findElementMetadata,
  hasControlCharacter,
  parseHexColor,
  rgbaToCss,
  rgbaToHex,
} from './lib/appElementMetadata'
import {
  evaluateDisplayLengthExpression,
  finiteNumberExpressionSource,
  millimetreExpressionSource,
  newProjectExpressionErrorMessage,
} from './lib/appNumericExpression'
import {
  isEditingText,
  namedBookFoldPalette,
  nextFoldTechniqueRequestId,
  selectedNamedBookFold,
} from './lib/appFoldTechnique'

const SNAP_OPTIONS: ReadonlyArray<{
  kind: keyof SnapSettings
  label: LocalizedText
}> = [
  { kind: 'grid', label: APP_TEXT.grid },
  { kind: 'vertex', label: APP_TEXT.vertex },
  { kind: 'intersection', label: APP_TEXT.intersection },
  { kind: 'edge', label: APP_TEXT.edge },
  { kind: 'midpoint', label: APP_TEXT.midpoint },
  { kind: 'horizontal', label: APP_TEXT.horizontal },
  { kind: 'vertical', label: APP_TEXT.vertical },
  { kind: 'parallel', label: APP_TEXT.parallel },
  { kind: 'angle', label: APP_TEXT.angle },
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
      return appMessage(APP_TEXT.foldTechniqueFileOperationsAreAvailableInTheDesktopApp)
    case 'busy':
      return appMessage(APP_TEXT.anotherFoldTechniqueFileOperationIsInProgressTryAgain)
    case 'not_regular_file':
      return appMessage(APP_TEXT.theSelectionWasNotProcessedBecauseItIsNotA)
    case 'too_large':
      return appMessage(APP_TEXT.theFoldTechniqueFileExceedsThe1MiBLimit)
    case 'invalid_document':
      return appMessage(APP_TEXT.theFoldTechniqueFileDoesNotSatisfyTheStrictV1)
    case 'open_failed':
    case 'read_failed':
      return appMessage(APP_TEXT.theFoldTechniqueFileCouldNotBeReadSafely)
    case 'save_failed':
      return appMessage(APP_TEXT.theFoldTechniqueFileCouldNotBeSavedAtomically)
    case 'invalid_response':
      return appMessage(APP_TEXT.theFoldTechniqueFileOperationResponseCouldNotBeVerified)
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
  const [instructionOnionSkin, setInstructionOnionSkin] =
    useState<InstructionOnionSkinRequest | null>(null)
  const [instructionOnionSkinStatus, setInstructionOnionSkinStatus] = useState<Readonly<{
    request: InstructionOnionSkinRequest
    state: 'available' | 'unavailable'
  }> | null>(null)
  const [assignedLocalSufficiency, setAssignedLocalSufficiency] =
    useState<AssignedLocalSufficiencyResponseV1 | null>(null)
  const [assignedLocalSummary, setAssignedLocalSummary] =
    useState<AssignedLocalSufficiencySummaryResponseV1 | null>(null)
  const [assignedLocalSummaryStatus, setAssignedLocalSummaryStatus] =
    useState<'idle' | 'loading' | 'retrying' | 'ready' | 'failed'>('idle')
  const [selectedFaceId, setSelectedFaceId] = useState<string | null>(null)
  const [hoveredLayerFaceId, setHoveredLayerFaceId] = useState<string | null>(null)
  const [mirrorVertexIds, setMirrorVertexIds] = useState<string[]>([])
  const [mirrorEdgeIds, setMirrorEdgeIds] = useState<string[]>([])
  const [mirrorMode, setMirrorMode] = useState<'move' | 'duplicate'>('duplicate')
  const [mirrorAxis, setMirrorAxis] = useState({
    x1: '0', y1: '0', x2: '0', y2: '100',
  })
  const [mirrorPreview, setMirrorPreview] = useState<{
    binding: string
    request: MirrorSelectionRequest
    result: MirrorSelectionPreflight
  } | null>(null)
  const [mirrorBusy, setMirrorBusy] = useState(false)
  const [linearArrayPreview, setLinearArrayPreview] = useState<{
    request: LinearArrayRequest
    result: LinearArrayPreview
  } | null>(null)
  const linearArrayRequestSequenceRef = useRef(0)
  const [radialArrayPreview,setRadialArrayPreview]=useState<{request:RadialArrayRequest;result:RadialArrayPreview}|null>(null)
  const radialArrayRequestSequenceRef=useRef(0)
  const mirrorRequestSequenceRef = useRef(0)
  const mirrorOperationRef = useRef(false)
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
  const [foldPreviewCamera, setFoldPreviewCamera] = useState<Readonly<{
    poseModelKey: string
    camera: NonNullable<InstructionVisual['camera']>
  }> | null>(null)
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
    () => appMessage(APP_TEXT.notRun),
  )
  const [benchmarkRun, setBenchmarkRun] = useState<BenchmarkRun | null>(null)
  const [benchmarkLoading, setBenchmarkLoading] = useState(false)
  const [nativeSnapshot, setNativeSnapshot] = useState<ProjectSnapshot | null>(null)
  useEffect(() => {
    linearArrayRequestSequenceRef.current += 1
    setLinearArrayPreview(null)
    radialArrayRequestSequenceRef.current += 1
    setRadialArrayPreview(null)
  }, [selectedLineId, nativeSnapshot?.project_instance_id, nativeSnapshot?.project_id, nativeSnapshot?.revision])
  const canvasUnderlays = useCanvasUnderlays(nativeSnapshot)
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
      ? appMessage(APP_TEXT.waitingForFaceAndHingeAnalysis)
      : appMessage(APP_TEXT.text3dAnalysisIsAvailableInTheDesktopApp),
  )
  const [validation, setValidation] = useState<ValidationSnapshot | null>(null)
  const unsplitIntersectionCount = useMemo(
    () => validation?.issues.filter((issue) => issue.code === 'unsplit_intersection').length ?? 0,
    [validation],
  )
  const [globalFlatFoldabilityJob, setGlobalFlatFoldabilityJob] =
    useState<GlobalFlatFoldabilityJobDto | null>(null)
  const [globalFlatFoldabilityTimeLimit, setGlobalFlatFoldabilityTimeLimit] =
    useState<GlobalFlatFoldabilityTimePreset>(
      DEFAULT_GLOBAL_FLAT_FOLDABILITY_TIME_PRESET,
    )
  const [coreStatusMessage, setCoreStatus] = useState<AppMessage>(
    () => isNativeCoreAvailable()
      ? appMessage(APP_TEXT.connectingToCore)
      : appMessage(APP_TEXT.browserPrototypeMode),
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
  const [bulkIntersectionRepairPending, setBulkIntersectionRepairPending] = useState(false)
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
  const {
    gridDivisionsInput,
    setGridDivisionsInput,
    gridDiagonals,
    setGridDiagonals,
    gridDivisions,
    gridDivisionsValid,
  } = useGridDivisionPreference()
  const benchmarkStatus = appMessageText(
    locale,
    benchmarkStatusMessage,
  ) ?? ''
  const topologyStatus = appMessageText(locale, topologyStatusMessage) ?? ''
  const coreStatus = appMessageText(locale, coreStatusMessage) ?? ''
  const newProjectError = appMessageText(locale, newProjectErrorMessage)
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
  const beginnerNativeEditRef = useRef<BeginnerNativeEditRunner>(
    async () => false,
  )
  const invalidateBeginnerGridRef = useRef<() => void>(() => undefined)
  const foldTechniqueWorkspaceRef = useRef<FoldTechniqueWorkspace | null>(
    foldTechniqueWorkspace,
  )
  const foldTechniqueBusyRef = useRef(foldTechniqueBusy)
  const foldTechniqueEditorDirtyRef = useRef(false)
  const foldTechniqueEditorOpenerRef = useRef<HTMLButtonElement | null>(null)
  const foldTechniqueRequestIdRef = useRef(0)
  const {
    open: creaseExportOpen,
    format: creaseExportFormat,
    preview: creaseExportPreview,
    error: creaseExportErrorMessage,
    notice: creaseExportNoticeMessage,
    buttonRef: creaseExportButtonRef,
    prepare: prepareCreaseExport,
    begin: beginCreaseExport,
    changeFormat: changeCreaseExportFormat,
    close: closeCreaseExportDialog,
    save: saveCurrentCreaseExport,
  } = useCreaseExportWorkflow({
    locale,
    copy: {
      previewRejected: APP_TEXT.rejectedAnExportPreviewThatDoesNotMatchTheCurrent,
      previewReadyJapanese: APP_TEXT.message0145,
      previewReadyEnglish: APP_TEXT.reviewInformationLossForTheFormatExport,
      cancelled: APP_TEXT.creasePatternExportCancelled,
      projectChanged: APP_TEXT.theProjectChangedRebuildTheExportData,
      saveCancelledNotice: APP_TEXT.saveLocationSelectionWasCancelledYouCanRetryFromThe,
      saveCancelledStatus: APP_TEXT.creasePatternSaveLocationSelectionCancelled,
    },
    getCurrentSnapshot: () => latestSnapshotRef.current,
    operationActive: () => coreOperationRef.current,
    setOperationBusy: (busy) => {
      coreOperationRef.current = busy
      setCoreBusy(busy)
    },
    setFileOperation,
    cancelInteraction: () => setCancelInteractionToken((token) => token + 1),
    onStatus: setCoreStatus,
    prepareFailedMessage: appMessage(
      appErrorLocalizedText('crease_export_prepare_failed'),
    ),
    cleanupFailedMessage: appMessage(
      appErrorLocalizedText('crease_export_cleanup_failed'),
    ),
    saveFailedMessage: appMessage(
      appErrorLocalizedText('crease_export_save_failed'),
    ),
    savedMessage: (preview) => appMessage(
      APP_TEXT.exportedFileName,
      { fileName: preview.suggested_file_name },
    ),
  })
  const {
    open: meshExportOpen,
    format: meshExportFormat,
    preview: meshExportPreview,
    error: meshExportErrorMessage,
    notice: meshExportNoticeMessage,
    buttonRef: meshExportButtonRef,
    prepare: prepareStaticMeshExport,
    begin: beginStaticMeshExport,
    changeFormat: changeStaticMeshExportFormat,
    close: closeStaticMeshExportDialog,
    save: saveCurrentStaticMeshExport,
  } = useStaticMeshExportWorkflow({
    copy: {
      previewReady: APP_TEXT.reviewTheCurrentPoseMidSurfaceMeshAndInformationLoss,
      prepareFailed: APP_TEXT.couldNotGenerateAMeshFromTheAuthenticatedPoseCurrently,
      cancelled: APP_TEXT.currentPose3DMeshExportCancelled,
      cleanupFailed: APP_TEXT.couldNotDiscardThe3DMeshExportPreview,
      projectChanged: APP_TEXT.theProjectChangedRebuildTheExportFromTheCurrentPose,
      saveCancelledNotice: APP_TEXT.saveLocationSelectionWasCancelledYouCanRetryWithThe,
      saveCancelledStatus: APP_TEXT.text3dMeshSaveLocationSelectionCancelled,
      saved: APP_TEXT.exportedFileName,
      saveFailed: APP_TEXT.the3DPoseOrProjectChangedOrTheFileCould,
    },
    getCurrentSnapshot: () => latestSnapshotRef.current,
    getCurrentPose: () => appliedFoldPoseRef.current,
    operationActive: () => coreOperationRef.current,
    setOperationBusy: (busy) => {
      coreOperationRef.current = busy
      setCoreBusy(busy)
    },
    setFileOperation,
    cancelInteraction: () => setCancelInteractionToken((token) => token + 1),
    onStatus: setCoreStatus,
  })
  const {
    open: meshAnimationExportOpen,
    preview: meshAnimationExportPreview,
    error: meshAnimationExportError,
    notice: meshAnimationExportNotice,
    buttonRef: meshAnimationExportButtonRef,
    prepare: prepareMeshAnimationExport,
    begin: beginMeshAnimationExport,
    close: closeMeshAnimationExport,
    save: saveCurrentMeshAnimationExport,
  } = useMeshAnimationExportWorkflow({
    copy: {
      prepareFailed: APP_TEXT.couldNotBuildAnAnimationFromTheCurrentInstructionsReview,
      cleanupFailed: APP_TEXT.couldNotSafelyDiscardTheAnimationExport,
      projectChanged: APP_TEXT.theProjectChangedRebuildFromTheCurrentInstructions,
      saveCancelledNotice: APP_TEXT.saveLocationSelectionWasCancelledYouCanRetryWithThe2,
      saved: APP_TEXT.exportedFileName2,
      saveFailed: APP_TEXT.theInstructionsChangedOrTheFileCouldNotBeSaved,
    },
    getCurrentSnapshot: () => latestSnapshotRef.current,
    operationActive: () => coreOperationRef.current,
    setOperationBusy: (busy) => {
      coreOperationRef.current = busy
      setCoreBusy(busy)
    },
    setFileOperation,
    onStatus: setCoreStatus,
  })
  const creaseExportError = appMessageText(locale, creaseExportErrorMessage)
  const creaseExportNotice = appMessageText(locale, creaseExportNoticeMessage)
  const meshExportError = appMessageText(locale, meshExportErrorMessage)
  const meshExportNotice = appMessageText(locale, meshExportNoticeMessage)
  recoveryStartupRef.current = recoveryStartup
  recoveryBlockingRef.current = recoveryBlocking
  appliedFoldPoseRef.current = appliedFoldPose
  foldTechniqueWorkspaceRef.current = foldTechniqueWorkspace
  foldTechniqueBusyRef.current = foldTechniqueBusy
  const runBeginnerNativeEdit = useCallback<BeginnerNativeEditRunner>(
    (action) => beginnerNativeEditRef.current(action),
    [],
  )
  const {
    beginnerDesignFormRef,
    beginnerPartTotal,
    setBeginnerPartTotal,
    beginnerSkeletonSegments,
    setBeginnerSkeletonSegments,
    beginnerSkeletonTree,
    beginnerComponentBridgeOverride,
    setBeginnerComponentBridgeOverride,
    beginnerProtrusions,
    setBeginnerProtrusions,
    beginnerBodyOutline,
    setBeginnerBodyOutline,
    beginnerBodySize,
    setBeginnerBodySize,
    beginnerBodyOutlineMode,
    setBeginnerBodyOutlineMode,
    beginnerProtrusionKinds,
    setBeginnerProtrusionKinds,
    beginnerBulgeTargets,
    setBeginnerBulgeTargets,
    addBeginnerSkeletonSegment,
    addBeginnerProtrusion,
    createEmptyGenericTarget,
    addBeginnerBulgeTarget,
  } = useBeginnerEditorState({
    snapshot: nativeSnapshot,
    getCurrentSnapshot: () => latestSnapshotRef.current,
    getSelectedFaceId: () => selectedFaceId,
  })
  const beginnerCandidateWorkflow = useBeginnerCandidateWorkflow({
    snapshot: nativeSnapshot,
    getCurrentSnapshot: () => latestSnapshotRef.current,
    runNativeEdit: runBeginnerNativeEdit,
    confirm: (message) => window.confirm(text(message)),
    copy: {
      applyPlan: APP_TEXT.applyThisCandidateToTheCreasePatternAndInstructionsYou,
      saveSymmetric:
        APP_TEXT.saveTheAdjustedSymmetricParametersThisDoesNotStartGeneration,
      appendInstructions:
        APP_TEXT.appendThisReviewedReadOnlyProposalToTheInstructionsIt,
    },
    consensusProgressEnabled: isNativeCoreAvailable(),
  })
  const {
    beginnerCandidateBusy,
    consensusSelectionDraft,
    cancelBeginnerCandidates,
    toggleConsensusReference,
    saveConsensusReferences,
    confirmAndAppendGenericTreeInstructions,
  } = beginnerCandidateWorkflow
  const beginnerGridWorkflow = useBeginnerParameterGridWorkflow({
    getCurrentSnapshot: () => latestSnapshotRef.current,
    skeletonTreeStatus: beginnerSkeletonTree.status,
    runNativeEdit: runBeginnerNativeEdit,
    confirm: (message) => window.confirm(text(message)),
    applyConfirmation:
      APP_TEXT.revalidateThisDesignSGridGeometryAndGlobalProofThen,
  })
  const { invalidateBeginnerGridForProjectReplacement } =
    beginnerGridWorkflow
  invalidateBeginnerGridRef.current =
    invalidateBeginnerGridForProjectReplacement
  const {
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
  } = useBeginnerReferenceWorkflow({
    snapshot: nativeSnapshot,
    getCurrentSnapshot: () => latestSnapshotRef.current,
    runNativeEdit: runBeginnerNativeEdit,
    confirm: (message) => window.confirm(text(message)),
    copy: {
      applySuggestion:
        APP_TEXT.applyThisMeasuredCandidateBoundingBoxAreaAndNormalsProvide,
      copyEstimatedBridges:
        APP_TEXT.bridgesBetweenDisconnected3DComponentsAreEstimatedCopyToA,
    },
    editor: {
      beginnerDesignFormRef,
      setBeginnerBodyOutline,
      setBeginnerBodyOutlineMode,
      setBeginnerProtrusions,
      setBeginnerSkeletonSegments,
      setBeginnerComponentBridgeOverride,
    },
  })
  const beginnerRecognitionWorkflow = useBeginnerRecognitionWorkflow({
    snapshot: nativeSnapshot,
    getCurrentSnapshot: () => latestSnapshotRef.current,
    operationBlocked: () => (
      coreOperationRef.current || recoveryBlockingRef.current
    ),
    runNativeEdit: runBeginnerNativeEdit,
    confirm: (message) => window.confirm(text(message)),
    copy: {
      copyOutline:
        APP_TEXT.copyThisOutlineIntoTheEditableTargetSkeletonThisDoes,
      applyParts:
        APP_TEXT.applyTheExplicitPartAssignmentsToTargetPartsThisDoes,
      copyProposal:
        APP_TEXT.copyThisRecognitionProposalIntoTheEditorTheProjectStays,
      overrideLowConfidence:
        APP_TEXT.thisContourProposalHasLowConfidenceOverrideAfterReviewingIts,
    },
    editor: {
      beginnerDesignFormRef,
      setBeginnerPartTotal,
      setBeginnerSkeletonSegments,
      setBeginnerBodyOutline,
      setBeginnerBodyOutlineMode,
      setBeginnerProtrusions,
    },
    onMissingReference: () => {
      setCoreStatus(appMessage(APP_TEXT.selectAReferenceImageToRecognize))
    },
    onRecognitionReady: (mode) => {
      setCoreStatus(appMessage({
        ja: mode === 'silhouette'
          ? '\u8f2a\u90ed\u753b\u50cf\u306e\u8a8d\u8b58\u6848\u3092\u4f5c\u6210\u3057\u307e\u3057\u305f\u3002\u307e\u3060\u4fdd\u5b58\u3055\u308c\u3066\u3044\u307e\u305b\u3093\u3002'
          : '\u30de\u30fc\u30ab\u30fcPNG\u306e\u8a8d\u8b58\u6848\u3092\u4f5c\u6210\u3057\u307e\u3057\u305f\u3002\u307e\u3060\u4fdd\u5b58\u3055\u308c\u3066\u3044\u307e\u305b\u3093\u3002',
        en: mode === 'silhouette'
          ? 'Created a silhouette proposal. It has not been saved.'
          : 'Created a marker PNG proposal. It has not been saved.',
      }))
    },
    onRecognitionFailure: (reason) => {
      setCoreStatus(appMessage({
        ja: reason === 'ambiguous_silhouette'
          ? '\u8f2a\u90ed\u304c\u8907\u6570\u307e\u305f\u306f\u4e0d\u660e\u77ad\u306a\u305f\u3081\u8a8d\u8b58\u3092\u62d2\u5426\u3057\u307e\u3057\u305f\u3002'
          : reason === 'resource_limit'
            ? '\u753b\u50cf\u304c\u8a8d\u8b58\u306e\u8cc7\u6e90\u4e0a\u9650\u3092\u8d85\u3048\u3066\u3044\u307e\u3059\u3002'
            : reason === 'unsupported_silhouette'
              ? '\u8f2a\u90ed\u753b\u50cf\u306f\u900f\u660e\u80cc\u666f\u3068\u5b8c\u5168\u306a\u9ed2\u306e\u5358\u4e00\u5f62\u72b6\u306b\u3057\u3066\u304f\u3060\u3055\u3044\u3002'
              : '\u753b\u50cf\u3092\u5b89\u5168\u306b\u8a8d\u8b58\u3067\u304d\u307e\u305b\u3093\u3067\u3057\u305f\u3002',
        en: reason === 'ambiguous_silhouette'
          ? 'Recognition was rejected because the silhouette is ambiguous or disconnected.'
          : reason === 'resource_limit'
            ? 'The image exceeds the recognition resource limit.'
            : reason === 'unsupported_silhouette'
              ? 'Use one solid black silhouette on a transparent background.'
              : 'The image could not be recognized safely.',
      }))
    },
    onProposalCopied: () => {
      setCoreStatus(appMessage(
        APP_TEXT.copiedTheProposalIntoTheEditorSaveItToAdd,
      ))
    },
  })
  const {
    beginnerRecognitionProposal,
    beginnerRecognitionBusy,
    beginnerSilhouetteThresholds,
    beginnerSilhouetteCropRoi,
    beginnerSilhouetteOrientation,
    beginnerSilhouetteMirror,
    invalidateBeginnerRecognition,
  } = beginnerRecognitionWorkflow
  const { submitBeginnerDesignProfile } = useBeginnerProfileWorkflow({
    getCurrentSnapshot: () => latestSnapshotRef.current,
    runNativeEdit: runBeginnerNativeEdit,
    editor: {
      beginnerBodyOutline,
      beginnerBodyOutlineMode,
      beginnerSkeletonSegments,
      beginnerComponentBridgeOverride,
      beginnerProtrusions,
      beginnerProtrusionKinds,
      beginnerBulgeTargets,
    },
    recognitionProposal: beginnerRecognitionProposal,
    silhouetteThresholds: beginnerSilhouetteThresholds,
    silhouetteCropRoi: beginnerSilhouetteCropRoi,
    silhouetteOrientation: beginnerSilhouetteOrientation,
    silhouetteMirror: beginnerSilhouetteMirror,
  })
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
    requestGenerationId: string,
  ) => {
    const response = await analyzeGeometricConstraints(
      expectedProjectInstanceId,
      expectedProjectId,
      expectedRevision,
      requestGenerationId,
    )
    const current = latestSnapshotRef.current
    if (
      !current
      || !matchesProjectOccGuard({
        expectedProjectInstanceId: response.project_instance_id,
        expectedProjectId: response.project_id,
        expectedRevision: response.revision,
      }, current)
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
    const priorSnapshot = latestSnapshotRef.current
    if (priorSnapshot && !matchesProjectOccGuard({
      expectedProjectInstanceId: admittedSnapshot.project_instance_id,
      expectedProjectId: admittedSnapshot.project_id,
      expectedRevision: admittedSnapshot.revision,
    }, priorSnapshot)) {
      invalidateBeginnerGridRef.current()
    }
    latestSnapshotRef.current = admittedSnapshot
    globalFlatFoldabilityCoordinatorRef.current?.invalidate({
      projectInstanceId: admittedSnapshot.project_instance_id,
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
    setTopologyStatus(appMessage(APP_TEXT.waitingForFaceAndHingeAnalysis))
  }, [])
  const acceptImportedProjectSnapshot = useCallback((
    snapshot: ProjectSnapshot,
    source: 'fold' | 'svg',
  ) => {
    applySnapshot(snapshot, true)
    setBenchmarkRun(null)
    setBenchmarkStatus(appMessage(
      source === 'fold'
        ? APP_TEXT.returnedToTheNormalCreasePatternAfterFOLDImport
        : APP_TEXT.returnedToTheNormalCreasePatternAfterSVGImport,
    ))
    setSelectedLineId(null)
    setSelectedVertexId(null)
    setPendingEdgeStart(null)
    setParallelReferenceEdgeId(null)
    setAppliedFoldPose(null)
    setFoldAngleOverrides({ projectId: null, values: new Map() })
    setFixedFaceChoice({ projectId: null, faceId: null })
    setActiveTool('select')
  }, [applySnapshot])
  const {
    preview: foldImportPreview,
    error: foldImportErrorMessage,
    buttonRef: foldImportButtonRef,
    begin: beginFoldImport,
    close: closeFoldImportDialog,
    apply: confirmFoldImport,
  } = useFoldImportWorkflow({
    locale,
    copy: {
      missingPreview: APP_TEXT.noImportPreviewWasReturned,
      cancelled: APP_TEXT.foldImportCancelled,
      reviewReady: APP_TEXT.reviewTheFOLDLineTypesAndScale,
      imported: APP_TEXT.importedNameFromFOLDASaveLocationHasNotBeen,
    },
    getCurrentSnapshot: () => latestSnapshotRef.current,
    operationActive: () => coreOperationRef.current,
    setOperationBusy: (busy) => {
      coreOperationRef.current = busy
      setCoreBusy(busy)
    },
    setFileOperation,
    cancelInteraction: () => setCancelInteractionToken((token) => token + 1),
    onStatus: setCoreStatus,
    onApplied: (snapshot) => acceptImportedProjectSnapshot(snapshot, 'fold'),
  })
  const {
    preview: svgImportPreview,
    validation: svgImportValidation,
    error: svgImportErrorMessage,
    buttonRef: svgImportButtonRef,
    begin: beginSvgImport,
    invalidateValidation: invalidateSvgImportValidation,
    validate: validateSvgImportDraft,
    close: closeSvgImportDialog,
    apply: confirmSvgImport,
  } = useSvgImportWorkflow({
    locale,
    copy: {
      missingPreview: APP_TEXT.noImportPreviewWasReturned,
      cancelled: APP_TEXT.svgImportCancelled,
      reviewReady: APP_TEXT.reviewTheSVGBoundaryLineTypesAndScale,
      validationReadyJapanese: APP_TEXT.message0140,
      validationReadyEnglish: APP_TEXT.validatedSVGBoundaryWidthHeightMm,
      imported: APP_TEXT.importedNameFromSVGASaveLocationHasNotBeen,
    },
    getCurrentSnapshot: () => latestSnapshotRef.current,
    operationActive: () => coreOperationRef.current,
    setOperationBusy: (busy) => {
      coreOperationRef.current = busy
      setCoreBusy(busy)
    },
    setFileOperation,
    cancelInteraction: () => setCancelInteractionToken((token) => token + 1),
    onStatus: setCoreStatus,
    onApplied: (snapshot) => acceptImportedProjectSnapshot(snapshot, 'svg'),
  })
  const foldImportError = appMessageText(locale, foldImportErrorMessage)
  const svgImportError = appMessageText(locale, svgImportErrorMessage)
  const acceptAppliedHistoryLimit = useCallback(async (
    settings: HistoryLimitSettings,
  ) => {
    const current = latestSnapshotRef.current
    if (
      !current
      || !matchesProjectOccGuard({
        expectedProjectInstanceId: settings.projectInstanceId,
        expectedProjectId: settings.projectId,
        expectedRevision: settings.revision,
      }, current)
    ) return

    const refreshed = await requestProjectSnapshot()
    const latest = latestSnapshotRef.current
    if (
      latest !== current
      || !matchesProjectOccGuard({
        expectedProjectInstanceId: settings.projectInstanceId,
        expectedProjectId: settings.projectId,
        expectedRevision: settings.revision,
      }, refreshed)
    ) return

    applySnapshot(refreshed)
    setHistoryLimitLoadState({ kind: 'ready', settings })
    setCoreStatus(appMessage(APP_TEXT.undoRedoHistoryLimitChangedToLimit, { limit: settings.historyEntryLimit }))
  }, [applySnapshot])

  const resetRecoveredProjectUi = useCallback(() => {
    benchmarkRequestIdRef.current += 1
    setBenchmarkLoading(false)
    setBenchmarkRun(null)
    setBenchmarkStatus(appMessage(APP_TEXT.showingRestoredEdits))
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
    setCoreStatus(appMessage(APP_TEXT.checkingRecoveryData))
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
        setCoreStatus(appMessage(APP_TEXT.rustCoreRevisionRevision, { revision: snapshot.revision }))
      } else {
        setRecoveryStartup({ kind: 'candidate', candidate })
        setCoreStatus(appMessage(APP_TEXT.chooseHowToHandleTheUnsavedRecoveryData))
      }
    } catch {
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
      ) return
      reportUnexpected('app.project_snapshot')
      setRecoveryStartup({ kind: 'failed' })
      setCoreStatus(appMessage(APP_TEXT.recoveryDataCouldNotBeCheckedPleaseTryAgain))
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
      setCoreStatus(appMessage(APP_TEXT.unsavedEditsWereRestoredChooseALocationAndSaveThem))
    } catch {
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
        || !sameRecoveryCandidate(recoveryStartupRef.current, candidate)
      ) return
      setRecoveryActionError(true)
      setCoreStatus(appMessage(APP_TEXT.recoveryDataCouldNotBeRestoredPleaseTryAgain))
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
      setCoreStatus(appMessage(APP_TEXT.recoveryDataWasDiscarded))
    } catch {
      if (
        !recoveryMountedRef.current
        || requestId !== recoveryRequestSequenceRef.current
        || !sameRecoveryCandidate(recoveryStartupRef.current, candidate)
      ) return
      setRecoveryActionError(true)
      setCoreStatus(appMessage(APP_TEXT.recoveryDataCouldNotBeDiscardedPleaseTryAgain))
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
    setMirrorEdgeIds((current) => current.filter((id) => visibleLineIds.has(id)))
    setMirrorVertexIds((current) => current.filter((id) => visibleVertexIds.has(id)))
  }, [nativeLines, nativeVertices])
  useEffect(() => {
    setMirrorPreview(null)
  }, [
    nativeSnapshot?.project_instance_id,
    nativeSnapshot?.project_id,
    nativeSnapshot?.revision,
  ])
  const displayedLines = benchmarkRun?.lines ?? nativeLines
  const displayedVertices = benchmarkRun?.vertices ?? nativeVertices
  const {
    measurementVertexIds,
    measurementLineIds,
    pairMeasurement,
    selectMeasurementLine,
    selectMeasurementVertex,
  } = useCreasePairMeasurement({
    active: activeTool === 'measure',
    lines: displayedLines,
    vertices: displayedVertices,
  })
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
  useEffect(() => {
    setAssignedLocalSummary(null)
    if (!nativeSnapshot) {
      setAssignedLocalSummaryStatus('idle')
      return
    }
    const coordinator = createAssignedLocalSufficiencySummaryCoordinator({
      onState(state) {
        if (state.status === 'running' || state.status === 'retrying') {
          setAssignedLocalSummaryStatus(state.status === 'retrying' ? 'retrying' : 'loading')
        } else if (state.status === 'ready') {
          setAssignedLocalSummary(state.response)
          setAssignedLocalSummaryStatus('ready')
        } else if (state.status === 'failed') {
          setAssignedLocalSummaryStatus('failed')
        }
      },
    })
    coordinator.start({
      expectedProjectInstanceId: nativeSnapshot.project_instance_id,
      expectedProjectId: nativeSnapshot.project_id,
      expectedRevision: nativeSnapshot.revision,
      expectedFoldModelFingerprint: nativeSnapshot.fold_model_fingerprint,
    })
    return () => coordinator.dispose()
  }, [nativeSnapshot])
  useEffect(() => {
    let current = true
    if (!selectedVertexId || !nativeSnapshot
      || assignedLocalSummaryStatus === 'loading'
      || assignedLocalSummaryStatus === 'retrying') {
      setAssignedLocalSufficiency(null)
      return () => {
        current = false
      }
    }
    void proveCurrentAssignedLocalSufficiencyV1({
      expectedProjectInstanceId: nativeSnapshot.project_instance_id,
      expectedProjectId: nativeSnapshot.project_id,
      expectedRevision: nativeSnapshot.revision,
      vertex: selectedVertexId,
    }).then((response) => {
      if (current) setAssignedLocalSufficiency(response)
    }).catch(() => {
      if (current) setAssignedLocalSufficiency(null)
    })
    return () => {
      current = false
    }
  }, [assignedLocalSummaryStatus, nativeSnapshot, selectedVertexId])
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
  const {
    boundaryVertexIds,
    paperBounds,
    paperPolygon,
    boundaryLengthReferences,
    lengthDisplayUnit,
    rectangularPaperSize,
    foldPreviewModel,
    canvasFaces,
    canvasAnnotations,
  } = useProjectCanvasProjection(nativeSnapshot, topologyResponse)
  const {
    open: instructionExportOpen,
    format: instructionExportFormat,
    preview: instructionExportPreview,
    generationActive: instructionExportGenerationActive,
    phase: instructionExportPhase,
    error: instructionExportErrorState,
    notice: instructionExportNoticeMessage,
    buttonRef: instructionExportButtonRef,
    prepare: prepareInstructionExport,
    begin: beginInstructionExport,
    changeFormat: changeInstructionExportFormat,
    close: closeInstructionExportDialog,
    save: saveCurrentInstructionExport,
  } = useInstructionExportWorkflow({
    copy: {
      previewReadyJapanese: APP_TEXT.message0160,
      previewReadyEnglish: APP_TEXT.reviewTheFormatContentAndNotices,
      prepareFailed: APP_TEXT.couldNotPrepareTheInstructionsError,
      prepareStatusFailed: APP_TEXT.instructionExportErrorError,
      progressFailed: APP_TEXT.progressCouldNotBeUpdatedErrorWaitingForTheGenerated,
      stopping: APP_TEXT.stoppingInstructionGeneration,
      stopped: APP_TEXT.instructionGenerationStopped,
      alreadyFinished: APP_TEXT.instructionGenerationHasAlreadyFinished,
      cancelled: APP_TEXT.instructionExportCancelled,
      cancelFailed: APP_TEXT.couldNotCancelError,
      cancelStatusFailed: APP_TEXT.instructionCancellationErrorError,
      projectChanged: APP_TEXT.theProjectChangedRebuildTheInstructionData,
      saveCancelledNotice: APP_TEXT.saveLocationSelectionWasCancelledYouCanSaveAgainFrom,
      saveCancelledStatus: APP_TEXT.instructionSaveLocationSelectionCancelled,
      saved: APP_TEXT.exportedFileName3,
      saveFailed: APP_TEXT.couldNotExportTheInstructionsError,
      saveStatusFailed: APP_TEXT.instructionExportErrorError,
    },
    getCurrentSnapshot: () => latestSnapshotRef.current,
    exportAvailable: () => foldPreviewModel !== null,
    operationActive: () => coreOperationRef.current,
    setOperationBusy: (busy) => {
      coreOperationRef.current = busy
      setCoreBusy(busy)
    },
    setFileOperation,
    cancelInteraction: () => setCancelInteractionToken((token) => token + 1),
    onStatus: setCoreStatus,
  })
  const instructionExportError = appMessageText(
    locale,
    instructionExportErrorState,
  )
  const instructionExportNotice = appMessageText(
    locale,
    instructionExportNoticeMessage,
  )
  const paperBoundaryVertexCount = boundaryVertexIds.size
  const selectedVertexIsBoundary = selectedVertex
    ? boundaryVertexIds.has(selectedVertex.id)
    : false
  const displayedLengthUnit = benchmarkRun
    ? MILLIMETRE_LENGTH_DISPLAY_UNIT
    : lengthDisplayUnit
  const pairMeasurementFormattedValue = pairMeasurement?.kind === 'vertex'
    ? formatLength(pairMeasurement.value, displayedLengthUnit, locale)
    : pairMeasurement?.kind === 'line'
      ? formatMeasurementValue(pairMeasurement.value, '°', 2, locale)
      : undefined
  const creationDimensionExpression =
    nativeSnapshot?.numeric_expressions?.rectangular_paper_creation
  const rectangularRatioReferenceAxis = ratioReferenceAxis(lengthDisplayUnit)
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
    ? formattedText(APP_TEXT.faceIndex, { index: effectiveFixedFaceIndex + 1 })
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
    ? text(APP_TEXT.blockedBy3DInputConsistencyValidation)
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
    : text(APP_TEXT.unknownDimensions)
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
    .join(text(APP_TEXT.message0036))
    || text(APP_TEXT.none)

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
        || !matchesProjectOccGuard({
          expectedProjectInstanceId: settings.projectInstanceId,
          expectedProjectId: settings.projectId,
          expectedRevision: settings.revision,
        }, current)
      ) return
      setHistoryLimitLoadState({ kind: 'ready', settings })
    }).catch(() => {
      const current = latestSnapshotRef.current
      if (
        disposed
        || requestId !== historyLimitRequestSequenceRef.current
        || !current
        || !matchesProjectOccGuard(expected, current)
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
    setTopologyStatus(appMessage(APP_TEXT.analyzingFacesAndHinges))

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
          setTopologyStatus(appMessage(APP_TEXT.facesFacesHingesHinges, {
            faces: response.snapshot.faces.length,
            hinges: response.snapshot.hinge_adjacency.length,
          }))
        } else {
          setTopologyStatus(appMessage(APP_TEXT.text3dAnalysisBlockedCountIssues, { count: response.issues.length }))
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
        setCoreStatus(appMessage(APP_TEXT.theQuitCheckCouldNotStartKeepTheAppOpen))
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
        setCoreStatus(appMessage(APP_TEXT.coreErrorTheEditResultCouldNotBeMergedInto))
        return false
      }
      applySnapshot(snapshot)
      setValidation(null)
      setCoreStatus(appMessage(APP_TEXT.rustCoreRevisionRevision, { revision: snapshot.revision }))
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
  beginnerNativeEditRef.current = runNativeEdit

  const {
    preview: foldTechniqueTimelinePreview,
    busy: foldTechniqueTimelineBusy,
    error: foldTechniqueTimelineError,
    stale: foldTechniqueTimelinePreviewStale,
    previewSelected: previewSelectedFoldTechniqueTimeline,
    closePreview: closeFoldTechniqueTimelinePreview,
    confirmProposal: confirmFoldTechniqueTimelineProposal,
  } = useFoldTechniqueTimelineProposal({
    locale,
    snapshot: nativeSnapshot,
    workspace: foldTechniqueWorkspace,
    selectedIndex: foldTechniqueSelectedIndex,
    nativeCoreAvailable: isNativeCoreAvailable,
    getCurrentSnapshot: () => latestSnapshotRef.current,
    getCurrentWorkspace: () => foldTechniqueWorkspaceRef.current,
    coreOperationActive: () => coreOperationRef.current,
    foldTechniqueBusy: () => foldTechniqueBusyRef.current,
    runNativeEdit,
    onStatus: setCoreStatus,
  })
  const foldTechniqueTimelineErrorText = appMessageText(
    locale,
    foldTechniqueTimelineError,
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

  const repairAllIntersections = useCallback(async () => {
    if (unsplitIntersectionCount === 0 || bulkIntersectionRepairPending) return
    setBulkIntersectionRepairPending(true)
    try {
      const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
        repairAllUnsplitIntersections(projectId, revision, projectInstanceId))
      if (succeeded) setCoreStatus(appMessage(APP_TEXT.intersectionsRepairedUndoAndRedoAreAvailable))
    } finally {
      setBulkIntersectionRepairPending(false)
    }
  }, [bulkIntersectionRepairPending, runNativeEdit, unsplitIntersectionCount])

  function addCurrentToMirrorSelection() {
    setMirrorPreview(null)
    if (selectedVertex) {
      setMirrorVertexIds((current) =>
        [...new Set([...current, selectedVertex.id])].sort())
      return
    }
    if (selectedLine) {
      setMirrorEdgeIds((current) =>
        [...new Set([...current, selectedLine.id])].sort())
      setMirrorVertexIds((current) =>
        [...new Set([
          ...current,
          selectedLine.startVertexId,
          selectedLine.endVertexId,
        ])].sort())
    }
  }

  function createMirrorRequest(): MirrorSelectionRequest | null {
    const values = [
      mirrorAxis.x1, mirrorAxis.y1, mirrorAxis.x2, mirrorAxis.y2,
    ].map(Number)
    if (values.some((value) => !Number.isFinite(value))) return null
    const [x1, y1, x2, y2] = values as [number, number, number, number]
    if (x1 === x2 && y1 === y2) return null
    const vertices = [...mirrorVertexIds].sort()
    const edges = [...mirrorEdgeIds].sort()
    return {
      vertices,
      edges,
      axis: { start: { x: x1, y: y1 }, end: { x: x2, y: y2 } },
      mode: mirrorMode,
      new_vertices: mirrorMode === 'duplicate'
        ? vertices.map(() => crypto.randomUUID()).sort()
        : [],
      new_edges: mirrorMode === 'duplicate'
        ? edges.map(() => crypto.randomUUID()).sort()
        : [],
    }
  }

  async function previewCurrentMirrorSelection() {
    const current = latestSnapshotRef.current
    const request = createMirrorRequest()
    if (
      !current || !request || mirrorOperationRef.current
      || coreOperationRef.current
    ) {
      setMirrorPreview(null)
      setCoreStatus(appMessage(APP_TEXT.chooseAMirrorSelectionAndAFiniteTwoPointAxis))
      return
    }
    const sequence = ++mirrorRequestSequenceRef.current
    mirrorOperationRef.current = true
    setMirrorBusy(true)
    try {
      const result = await preflightMirrorSelection(
        current.project_id,
        current.revision,
        current.project_instance_id,
        request,
      )
      const latest = latestSnapshotRef.current
      if (
        sequence !== mirrorRequestSequenceRef.current
        || latest !== current
      ) return
      const binding = [
        current.project_instance_id,
        current.project_id,
        current.revision,
      ].join(':')
      setMirrorPreview({ binding, request, result })
    } catch {
      if (sequence === mirrorRequestSequenceRef.current) {
        setMirrorPreview(null)
        setCoreStatus(appMessage(APP_TEXT.mirrorPreflightFailed))
      }
    } finally {
      if (sequence === mirrorRequestSequenceRef.current) {
        mirrorOperationRef.current = false
        setMirrorBusy(false)
      }
    }
  }

  async function applyCurrentMirrorSelection() {
    const preview = mirrorPreview
    const current = latestSnapshotRef.current
    if (
      !preview || !preview.result.allowed || !current
      || mirrorOperationRef.current
    ) return
    const binding = [
      current.project_instance_id,
      current.project_id,
      current.revision,
    ].join(':')
    if (binding !== preview.binding) {
      setMirrorPreview(null)
      return
    }
    mirrorOperationRef.current = true
    setMirrorBusy(true)
    const applied = await runNativeEdit((projectId, revision, projectInstanceId) => {
      if (
        projectId !== current.project_id
        || revision !== current.revision
        || projectInstanceId !== current.project_instance_id
      ) return Promise.reject(new Error('stale mirror preview'))
      return applyMirrorSelection(
        projectId,
        revision,
        projectInstanceId,
        preview.request,
      )
    })
    mirrorOperationRef.current = false
    setMirrorBusy(false)
    if (applied) {
      setMirrorPreview(null)
      setMirrorVertexIds([])
      setMirrorEdgeIds([])
    }
  }

  function cancelMirrorSelection() {
    mirrorRequestSequenceRef.current += 1
    mirrorOperationRef.current = false
    setMirrorBusy(false)
    setMirrorPreview(null)
    setMirrorVertexIds([])
    setMirrorEdgeIds([])
  }

  async function submitLinearArrayPreview(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedLine || selectedLine.locked || coreBusy) return
    const sequence = ++linearArrayRequestSequenceRef.current
    const form = new FormData(event.currentTarget)
    const copies = Number(form.get('linear_array_copies'))
    const dx = Number(form.get('linear_array_dx'))
    const dy = Number(form.get('linear_array_dy'))
    if (!Number.isInteger(copies) || copies < 1 || copies > 16
      || !Number.isFinite(dx) || !Number.isFinite(dy) || (dx === 0 && dy === 0)) {
      setLinearArrayPreview(null)
      setCoreStatus(appMessage(APP_TEXT.choose116CopiesAndAFiniteNonZeroOffset))
      return
    }
    const request: LinearArrayRequest = {
      vertices: [selectedLine.startVertexId, selectedLine.endVertexId].sort(),
      edges: [selectedLine.id],
      additional_copies: copies,
      delta: { x: dx, y: dy },
    }
    try {
      const result = await previewLinearArray(
        current.project_id, current.revision, current.project_instance_id, request,
      )
      if (sequence !== linearArrayRequestSequenceRef.current
        || latestSnapshotRef.current !== current || result.authorizes_project_mutation) return
      setLinearArrayPreview({ request, result })
    } catch {
      if (sequence !== linearArrayRequestSequenceRef.current) return
      setLinearArrayPreview(null)
      setCoreStatus(appMessage(APP_TEXT.theLinearArrayPreviewCouldNotBeCreated))
    }
  }

  async function confirmCurrentLinearArray() {
    const preview = linearArrayPreview
    const current = latestSnapshotRef.current
    if (!preview || !current) return
    const result = preview.result
    if (!matchesProjectOccGuard({
      expectedProjectInstanceId: result.project_instance_id,
      expectedProjectId: result.project_id,
      expectedRevision: result.revision,
    }, current)) {
      setLinearArrayPreview(null)
      return
    }
    const applied = await runNativeEdit((projectId, revision, projectInstanceId) =>
      confirmLinearArray(projectId, revision, projectInstanceId, preview.request, result.request_sha256))
    if (applied) {
      linearArrayRequestSequenceRef.current += 1
      setLinearArrayPreview(null)
    }
  }
  async function submitRadialArrayPreview(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const current = latestSnapshotRef.current
    if (!current || !selectedLine || selectedLine.locked || coreBusy) return
    const sequence = ++radialArrayRequestSequenceRef.current
    const form = new FormData(event.currentTarget)
    const copies = Number(form.get('radial_array_copies'))
    const angle = Number(form.get('radial_array_angle'))
    if (!Number.isInteger(copies) || copies < 1 || copies > 3
      || ![90, 180, 270].includes(angle) || (angle === 180 && copies !== 1)) {
      setRadialArrayPreview(null)
      return
    }
    const request: RadialArrayRequest = {
      center: selectedLine.startVertexId,
      vertices: [selectedLine.startVertexId, selectedLine.endVertexId].sort(),
      edges: [selectedLine.id], additional_copies: copies,
      angle_microdegrees: angle * 1_000_000,
    }
    try {
      const result = await previewRadialArray(current.project_id, current.revision, current.project_instance_id, request)
      if (sequence !== radialArrayRequestSequenceRef.current
        || latestSnapshotRef.current !== current || result.authorizes_project_mutation
        || !matchesProjectOccGuard({
          expectedProjectInstanceId: result.project_instance_id,
          expectedProjectId: result.project_id,
          expectedRevision: result.revision,
        }, current)
        || result.additional_copies !== request.additional_copies
        || result.angle_microdegrees !== request.angle_microdegrees
        || result.source_vertex_count !== request.vertices.length
        || result.source_edge_count !== request.edges.length) return
      setRadialArrayPreview({ request, result })
    } catch { if (sequence === radialArrayRequestSequenceRef.current) setRadialArrayPreview(null) }
  }

  async function confirmCurrentRadialArray() {
    const preview = radialArrayPreview
    const current = latestSnapshotRef.current
    if (!preview || !current) return
    const result = preview.result
    if (!matchesProjectOccGuard({
      expectedProjectInstanceId: result.project_instance_id,
      expectedProjectId: result.project_id,
      expectedRevision: result.revision,
    }, current)) {
      setRadialArrayPreview(null); return
    }
    const applied = await runNativeEdit((id, revision, instance) =>
      confirmRadialArray(id, revision, instance, preview.request, result.request_sha256))
    if (applied) { radialArrayRequestSequenceRef.current += 1; setRadialArrayPreview(null) }
  }

  function mirrorPreflightIssueText(issue: string | null) {
    switch (issue) {
      case 'invalid_axis':
        return text(APP_TEXT.theMirrorAxisIsInvalid)
      case 'empty_selection':
        return text(APP_TEXT.theSelectionIsEmpty)
      case 'noncanonical_selection':
      case 'invalid_new_ids':
      case 'core_rejected':
        return text(APP_TEXT.thisEditIsUnsafeForTheCurrentGeometryOrLayers)
      default:
        return text(APP_TEXT.theMirrorEditCannotBeApplied)
    }
  }

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
      || !matchesProjectOccGuard({
        expectedProjectInstanceId: projectInstanceId,
        expectedProjectId: projectId,
        expectedRevision: revision,
      }, baseSnapshot)
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

  const previewConstraintSolve = useCallback((
    vertexId: string,
    x: number,
    y: number,
  ) => {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current || recoveryBlockingRef.current) {
      return Promise.reject(new Error('project unavailable'))
    }
    return previewGeometricConstraintSolve(
      current.project_id,
      current.revision,
      current.project_instance_id,
      vertexId,
      x,
      y,
    )
  }, [])

  const applyConstraintSolve = useCallback((token: string) =>
    runNativeEdit((projectId, revision, projectInstanceId) =>
      applyGeometricConstraintSolve(
        projectId,
        revision,
        projectInstanceId,
        token,
      )), [runNativeEdit])

  const previewConstraintEdgeSolve = useCallback((
    edgeId: string,
    startX: number,
    startY: number,
    endX: number,
    endY: number,
  ) => {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current || recoveryBlockingRef.current) {
      return Promise.reject(new Error('project unavailable'))
    }
    return previewGeometricConstraintEdgeSolve(
      current.project_id,
      current.revision,
      current.project_instance_id,
      edgeId,
      startX,
      startY,
      endX,
      endY,
    )
  }, [])

  const previewConstraintExpressionSolve = useCallback(() => {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current || recoveryBlockingRef.current) {
      return Promise.reject(new Error('project unavailable'))
    }
    return previewGeometricConstraintExpressionSolve(
      current.project_id,
      current.revision,
      current.project_instance_id,
    )
  }, [])

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
        projectInstanceId: current.project_instance_id,
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
      setCoreStatus(appMessage(APP_TEXT.theBenchmarkPatternIsReadOnlyReturnToTheNormal))
      return
    }
    if (selectedLine?.locked || selectedVertexLocked) {
      setCoreStatus(appMessage(APP_TEXT.thisGeometryBelongsToALockedLayerUnlockTheLayer))
      return
    }
    if (selectedLine) {
      if (selectedLine.kind === 'boundary') {
        setCoreStatus(appMessage(APP_TEXT.addOrRemoveBoundaryEdgesThroughPaperShapeEditing))
        return
      }
      const removed = await runNativeEdit((projectId, revision, projectInstanceId) =>
        removeEdge(projectId, revision, projectInstanceId, selectedLine.id))
      if (removed) setSelectedLineId(null)
      return
    }
    if (selectedVertex) {
      if (selectedVertexIsBoundary && paperBoundaryVertexCount <= 3) {
        setCoreStatus(appMessage(APP_TEXT.thisBoundaryVertexCannotBeDeletedBecauseABoundaryNeeds))
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
        ? appMessage(APP_TEXT.deletedTheBoundaryVertexAndMergedItsAdjacentEdgesUndo)
        : appMessage(APP_TEXT.deletedTheVertexUndoCanRestoreIt))
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
      setCoreStatus(appMessage(APP_TEXT.theBoundaryEdgeWasSplitButTheNewVertexCould))
      return
    }
    setSelectedVertexId(addedVertex.id)
    setActiveTool('select')
    setCoreStatus(appMessage(APP_TEXT.splitTheBoundaryEdgeAtItsMidpointAndSelectedThe))
  }

  async function placeCanvasVertex(placement: VertexPlacement) {
    const current = latestSnapshotRef.current
    if (
      !current
      || coreOperationRef.current
    ) return
    if (placementTouchesLockedLayer(placement, nativeLayerView)) {
      setCoreStatus(appMessage(APP_TEXT.creasesAndVerticesOnALockedLayerCannotBeEdited))
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
          throw new Error(formattedText(APP_TEXT.theEdgeToSplitWasNotFoundEdgeId, { edgeId: placement.edgeId }))
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
          throw new Error(text(APP_TEXT.theEdgesSelectedForIntersectionConnectionAreInvalid))
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
        setCoreStatus(appMessage(APP_TEXT.theIntersectionWasConnectedButTheConnectedVertexCouldNot))
        return
      }
      setSelectedLineId(null)
      setPendingEdgeStart(null)
      setSelectedVertexId(result.connectedVertexId)
      setCoreStatus(placement.operation === 'connect-t-junction'
        ? appMessage(APP_TEXT.connectedTheTJunctionOneUndoRestoresIt)
        : placement.operation === 'connect-intersection-cluster'
          ? appMessage(APP_TEXT.connectedCountEdgesAsAnIntersectionClusterOneUndoRestores, { count: placement.targets.length })
          : appMessage(APP_TEXT.atomicallySplitTwoEdgesAtTheirIntersectionOneUndoRestores))
      return
    }

    const addedVertices = result.snapshot.crease_pattern.vertices.filter(
      ({ id }) => !previousVertexIds.has(id),
    )
    setSelectedLineId(null)
    setPendingEdgeStart(null)
    if (addedVertices.length !== 1) {
      setSelectedVertexId(null)
      setCoreStatus(appMessage(APP_TEXT.aVertexWasCreatedButItCouldNotBeUniquely))
      return
    }
    setSelectedVertexId(addedVertices[0].id)
    setCoreStatus(placement.operation === 'split-edge'
      ? appMessage(APP_TEXT.splitTheEdgeAndSelectedTheNewVertexUndoCan)
      : appMessage(APP_TEXT.addedAndSelectedAVertexUndoCanRestoreIt))
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
      setCoreStatus(appMessage(APP_TEXT.theDefaultLayerIsLockedSoANewLineCannot))
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
      setCoreStatus(appMessage(APP_TEXT.selectTheLineEndpoint))
      return
    }
    if (pendingEdgeStart === vertexId) {
      setCoreStatus(appMessage(APP_TEXT.selectAVertexDifferentFromTheStartPoint))
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
    const submitter = (event.nativeEvent as SubmitEvent).submitter
    if (submitter instanceof HTMLButtonElement && submitter.name) {
      form.set(submitter.name, submitter.value)
    }
    if (form.get('vertex_action') === 'ray_to_target') {
      const angleSource = String(form.get('polar_angle_degrees') ?? '')
      const edgeKind = form.get('polar_edge_kind')
      let angleDegrees: number
      try {
        angleDegrees = (await evaluateFiniteNumericExpression(angleSource)).value
      } catch (error) {
        setCoreStatus(editExpressionErrorMessage(error))
        return
      }
      const angleMicrodegrees = angleDegrees * 1_000_000
      if (!Number.isSafeInteger(angleMicrodegrees) || angleMicrodegrees < 0
        || angleMicrodegrees >= 360_000_000
        || (edgeKind !== 'mountain' && edgeKind !== 'valley'
          && edgeKind !== 'auxiliary' && edgeKind !== 'cut')
        || (edgeKind === 'cut' && !current.cutting_allowed)) {
        setCoreStatus(appMessage(APP_TEXT.enterAnAngleFrom0UpTo360ExclusiveWith))
        return
      }
      const succeeded = await runNativeEdit((projectId, revision, projectInstanceId) =>
        addRayToFirstTarget(projectId, revision, projectInstanceId, selectedVertex.id,
          angleMicrodegrees, edgeKind))
      if (!succeeded) return
      setSelectedLineId(null)
      setPendingEdgeStart(null)
      setCoreStatus(appMessage(APP_TEXT.drewALineToTheFirstTargetIntersectedAtThe))
      return
    }
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
        setCoreStatus(appMessage(APP_TEXT.enterAPositiveFiniteLengthAFiniteAngleAndAn))
        return
      }
      const angleRadians = angleDegrees * Math.PI / 180
      const x = currentVertex.position.x + length * Math.cos(angleRadians)
      const y = currentVertex.position.y + length * Math.sin(angleRadians)
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        setCoreStatus(appMessage(APP_TEXT.theSpecifiedLengthAndAngleDoNotProduceFiniteCoordinates))
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
      setCoreStatus(appMessage(APP_TEXT.addedAnEndpointAndLineFromTheSpecifiedLengthAnd))
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
      setCoreStatus(appMessage(APP_TEXT.enterFiniteNumericCoordinates))
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
      setCoreStatus(appMessage(APP_TEXT.enterFiniteNumericCoordinates2))
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
    setCoreStatus(appMessage(APP_TEXT.addedAVertexAtTheSpecifiedCoordinates))
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
      setCoreStatus(appMessage(APP_TEXT.enterFiniteExpressionsForTheLineTranslation))
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
      setCoreStatus(appMessage(APP_TEXT.enterFiniteExpressionsForTheFaceTranslation))
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
      setCoreStatus(appMessage(APP_TEXT.chooseTwoNonAdjacentFaceVerticesAndAnAvailableLine))
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
      setCoreStatus(appMessage(APP_TEXT.enterAFinitePaperThicknessOf0OrGreater))
      return
    }
    if (!frontColor || !backColor) {
      setCoreStatus(appMessage(APP_TEXT.chooseValidFrontAndBackColors))
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
      setCoreStatus(appMessage(APP_TEXT.reviewTheElementNameColorAndMemo))
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
      setCoreStatus(appMessage(APP_TEXT.keepTheProjectMemoWithin16000Characters))
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
      setCoreStatus(appMessage(APP_TEXT.theCurrentPaperIsNotAnAxisAlignedRectangleSo))
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
      setCoreStatus(appMessage(APP_TEXT.enterAFinitePaperWidthGreaterThan0))
      return
    }
    if (heightMm === null || heightMm <= 0) {
      setCoreStatus(appMessage(APP_TEXT.enterAFinitePaperHeightGreaterThan0))
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
    setCoreStatus(appMessage(APP_TEXT.revisionRevisionValidating, { revision: current.revision }))
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
        setCoreStatus(appMessage(APP_TEXT.theProjectChangedDuringValidationPleaseValidateAgain))
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
        ja: formatLocalizedText('ja', APP_TEXT.message0105, {
          revision: result.revision,
          geometry: result.is_valid
            ? '幾何検証に合格'
            : formatLocalizedText('ja', APP_TEXT.message0106, { count: result.issues.length }),
          local: localFlatFoldabilityCoreStatus(localPresentation, 'ja'),
        }),
        en: formatLocalizedText('en', APP_TEXT.revisionRevisionGeometryLocal, {
          revision: result.revision,
          geometry: result.is_valid
            ? 'Geometry passed'
            : formatLocalizedText('en', APP_TEXT.countGeometryIssues, { count: result.issues.length }),
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
      setNewProjectError(appMessage(APP_TEXT.enterAProjectName))
      return
    }
    if ([...name].length > 120 || hasControlCharacter(name)) {
      setNewProjectError(appMessage(APP_TEXT.useAtMost120CharactersAndDoNotIncludeControl))
      return
    }
    if (!widthExpression.trim()) {
      setNewProjectError(appMessage(APP_TEXT.enterAWidthExpression))
      return
    }
    if (!heightExpression.trim()) {
      setNewProjectError(appMessage(APP_TEXT.enterAHeightExpression))
      return
    }
    if (!thicknessInput || !Number.isFinite(thicknessMm) || thicknessMm < 0) {
      setNewProjectError(appMessage(APP_TEXT.enterAFinitePaperThicknessOf0OrGreater2))
      return
    }
    if (!frontColor || !backColor) {
      setNewProjectError(appMessage(APP_TEXT.chooseFrontAndBackColors))
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
      setCoreStatus(appMessage(APP_TEXT.createdNameASaveLocationHasNotBeenSetYet, { name: snapshot.name }))
    } catch (error) {
      const japaneseMessage = newProjectExpressionErrorMessage(error, 'ja')
        ?? '新しいプロジェクトを作成できませんでした。'
      const englishMessage = newProjectExpressionErrorMessage(error, 'en')
        ?? 'The new project could not be created.'
      setNewProjectError(appMessage({
        ja: formatLocalizedText('ja', APP_TEXT.message0116, { message: japaneseMessage }),
        en: formatLocalizedText('en', APP_TEXT.couldNotCreateTheProjectMessage, { message: englishMessage }),
      }))
      setCoreStatus(appMessage({
        ja: formatLocalizedText('ja', APP_TEXT.message0118, { message: japaneseMessage }),
        en: formatLocalizedText('en', APP_TEXT.newProjectErrorMessage, { message: englishMessage }),
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
      const response = await runProjectFileOperation(operation)
      applySnapshot(
        response.project,
        operation === 'open' && !response.canceled,
      )
      if (response.canceled) {
        setCoreStatus(appMessage(APP_TEXT.fileOperationCancelled))
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
        ? appMessage(APP_TEXT.openedName, { name: response.project.name })
        : appMessage(APP_TEXT.savedName, { name: response.project.name }))
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
        setCoreStatus(appMessage(APP_TEXT.expandedFolderOperationCancelled))
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
        ? appMessage(APP_TEXT.openedNameFromAnExpandedFolder, { name: response.project.name })
        : appMessage(APP_TEXT.savedNameToANewExpandedFolder, { name: response.project.name }))
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
        setCoreStatus(appMessage(APP_TEXT.foldTechniqueFileImportWasCancelled))
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
      setCoreStatus(appMessage(APP_TEXT.importedTheFoldTechniqueFileYouCanReviewAndEdit))
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
      setCoreStatus(appMessage(APP_TEXT.keptTheFoldTechniqueChangesChooseSaveAsToShare))
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
        setCoreStatus(appMessage(APP_TEXT.savingTheNewFoldTechniqueWasCancelledTheEditedContent))
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
      setCoreStatus(appMessage(APP_TEXT.createdTheFoldTechniqueAndSavedItToAShared))
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
        setCoreStatus(appMessage(APP_TEXT.savingTheFoldTechniqueFileAsAnotherFileWasCancelled))
        return
      }
      if (!response.document) throw new Error('missing admitted document')
      replaceFoldTechniqueWorkspace({
        document: response.document,
        dirty: false,
      })
      setCoreStatus(appMessage(APP_TEXT.savedTheFoldTechniqueToAnotherSharedFile))
    } catch (error) {
      if (foldTechniqueRequestIdRef.current !== requestId) return
      setCoreStatus(foldTechniqueFileErrorAppMessage(error))
    } finally {
      if (foldTechniqueRequestIdRef.current === requestId) {
        setFoldTechniqueOperationBusy(false)
      }
    }
  }

  async function toggleBenchmark() {
    if (benchmarkRun) {
      setBenchmarkRun(null)
      setBenchmarkStatus(appMessage(APP_TEXT.returnedToTheNormalCreasePattern))
      setSelectedLineId(null)
      setSelectedVertexId(null)
      return
    }
    if (benchmarkLoading) return

    setBenchmarkLoading(true)
    setBenchmarkStatus(appMessage(APP_TEXT.generatingAndTransferring10000RealEdges))
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
      setBenchmarkStatus(appMessageWithLocalizedVariables(APP_TEXT.countEdgesBytesGenerationTransferResponseMsMsMeasuringCanvas, (locale) => ({
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
    setBenchmarkStatus(appMessageWithLocalizedVariables(APP_TEXT.countEdgesBytesGenerationTransferResponseMsMsConversionPreparationMsMs, (locale) => ({
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
          {nativeSnapshot?.name ?? text(APP_TEXT.untitledProject)}
          {nativeSnapshot?.is_dirty ? ' *' : ''}
        </span>
        <nav
          className="top-actions"
          aria-label={text(APP_TEXT.projectActions)}
        >
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText(APP_TEXT.newShortcut, {
              shortcut: keyboardShortcutDisplayValue('new', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('new', keyboardShortcuts)}
            onClick={() => {
              setNewProjectError(null)
              setNewProjectOpen(true)
            }}
          >
            {text(APP_TEXT.new)}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot?.can_undo}
            onClick={() => runNativeEdit(undo)}
            title={formattedText(APP_TEXT.undoShortcut, {
              shortcut: keyboardShortcutDisplayValue('undo', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('undo', keyboardShortcuts)}
          >
            {text(APP_TEXT.undo)}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot?.can_redo}
            onClick={() => runNativeEdit(redo)}
            title={formattedText(APP_TEXT.redoShortcut, {
              shortcut: keyboardShortcutDisplayValue('redo', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('redo', keyboardShortcuts)}
          >
            {text(APP_TEXT.redo)}
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
            {text(APP_TEXT.vertexAtCenter)}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText(APP_TEXT.openShortcut, {
              shortcut: keyboardShortcutDisplayValue('open', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('open', keyboardShortcuts)}
            onClick={() => void runFileOperation('open')}
          >
            {fileOperation === 'open'
              ? text(APP_TEXT.opening)
              : text(APP_TEXT.open)}
          </button>
          <button
            type="button"
            disabled={
              coreBusy
              || !nativeSnapshot
              || !isNativeProjectFolderAvailable()
            }
            title={text(APP_TEXT.openAnExpandedProjectFolderAfterValidatingItsManifestAnd)}
            onClick={() => void runProjectFolderOperation('folder_open')}
          >
            {fileOperation === 'folder_open'
              ? text(APP_TEXT.checkingFolder)
              : text(APP_TEXT.openExpandedFolder)}
          </button>
          <button
            ref={foldImportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void beginFoldImport()}
            aria-haspopup="dialog"
          >
            {fileOperation === 'fold_import'
              ? text(APP_TEXT.analyzing)
              : text(APP_TEXT.importFOLD)}
          </button>
          <button
            ref={svgImportButtonRef}
            type="button"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void beginSvgImport()}
            aria-haspopup="dialog"
          >
            {fileOperation === 'svg_import'
              ? text(APP_TEXT.analyzing)
              : text(APP_TEXT.importSVG)}
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
              ? text(APP_TEXT.generating)
              : text(APP_TEXT.export)}
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
            title={text(APP_TEXT.exportTheCurrentlyDisplayed3DPoseAsAMidSurface)}
            onClick={beginStaticMeshExport}
            aria-haspopup="dialog"
          >
            {fileOperation === 'mesh_export'
              ? text(APP_TEXT.generating3D)
              : text(APP_TEXT.export3D)}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText(APP_TEXT.saveShortcut, {
              shortcut: keyboardShortcutDisplayValue('save', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('save', keyboardShortcuts)}
            onClick={() => void runFileOperation('save')}
          >
            {fileOperation === 'save'
              ? text(APP_TEXT.saving)
              : text(APP_TEXT.save)}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            title={formattedText(APP_TEXT.saveAsShortcut, {
              shortcut: keyboardShortcutDisplayValue('save_as', keyboardShortcuts),
            })}
            aria-keyshortcuts={keyboardShortcutAriaValue('save_as', keyboardShortcuts)}
            onClick={() => void runFileOperation('save_as')}
          >
            {fileOperation === 'save_as'
              ? text(APP_TEXT.saving)
              : text(APP_TEXT.saveAs)}
          </button>
          <button
            type="button"
            disabled={
              coreBusy
              || !nativeSnapshot
              || !isNativeProjectFolderAvailable()
            }
            title={text(APP_TEXT.saveAnExpandedFolderInsideTheSelectedParentOnLocal)}
            onClick={() => void runProjectFolderOperation('folder_save')}
          >
            {fileOperation === 'folder_save'
              ? text(APP_TEXT.savingFolder)
              : text(APP_TEXT.saveExpandedFolder)}
          </button>
          <button
            type="button"
            className="primary"
            disabled={coreBusy || benchmarkLoading || Boolean(benchmarkRun) || !nativeSnapshot}
            onClick={() => void runValidation()}
          >
            {text(APP_TEXT.validate)}
          </button>
        </nav>
      </header>

      <section className="workspace" inert={modalOpen} id="workspace-main" data-inspector-side={workspaceLayout.inspectorSide}>
        <aside
          className="tool-rail"
          aria-label={text(APP_TEXT.drawingTools)}
        >
          {([
            { id: 'select', icon: '↖', label: APP_TEXT.select },
            { id: 'vertex', icon: '＋', label: APP_TEXT.vertex },
            { id: 'mountain', icon: '━', label: APP_TEXT.mountainFold },
            { id: 'valley', icon: '┅', label: APP_TEXT.valleyFold },
            { id: 'auxiliary', icon: '┈', label: APP_TEXT.auxiliaryLine },
            { id: 'cut', icon: '✂', label: APP_TEXT.cut },
            { id: 'measure', icon: '∠', label: APP_TEXT.measure },
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
              <span>{text(APP_TEXT.text2dCreasePattern)}</span>
              <span className="panel-meta">
                {benchmarkRun
                  ? formattedText(APP_TEXT.benchmarkCountEdges, { count: displayedLines.length.toLocaleString(locale) })
                  : formattedText(APP_TEXT.sizeCountEdges, {
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
              tool={activeTool === 'measure' ? 'measure' : benchmarkRun ? 'select' : activeTool}
              selectedVertexId={selectedVertexId}
              selectedFaceId={selectedFaceId}
              highlightedFaceId={hoveredLayerFaceId}
              mirrorSelectedVertexIds={mirrorVertexIds}
              mirrorSelectedLineIds={mirrorEdgeIds}
              pendingVertexId={pendingEdgeStart}
              selectedLineId={selectedLineId}
              measurementLabel={formatLineMeasurementLabel(
                selectedLineMeasurement,
                displayedLengthUnit,
                locale,
              )}
              snapSettings={snapSettings}
              gridDivisions={gridDivisions}
              gridDiagonals={gridDiagonals}
              parallelReference={benchmarkRun ? null : parallelReferenceLine}
              angleConfig={angleSnapConfig}
              compassCircles={benchmarkRun ? [] : compassCircles}
              annotations={benchmarkRun ? [] : canvasAnnotations}
              underlays={benchmarkRun ? [] : canvasUnderlays}
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
                  if (activeTool === 'measure') {
                    selectMeasurementLine(lineId)
                  }
                } else if (activeTool === 'measure') {
                  selectMeasurementLine(null)
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
                      setCoreStatus(appMessage(APP_TEXT.tooManyIntersectionCandidatesZoomInAndTryAgain))
                    } else if (reason === 'intersection-blocked') {
                      setCoreStatus(appMessage(APP_TEXT.thisIntersectionClusterIsUnsupportedOrAmbiguousCheckForOverlapping))
                    }
                  }}
              onSelectVertex={activeTool === 'measure'
                ? (vertexId) => {
                    setSelectedVertexId(vertexId)
                    setSelectedLineId(null)
                    setSelectedFaceId(null)
                    selectMeasurementVertex(vertexId)
                  }
                : benchmarkRun
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
            {activeTool === 'measure' && (
              <PairMeasurementStatus
                locale={locale}
                kind={pairMeasurement?.kind ?? 'pending'}
                formattedValue={pairMeasurementFormattedValue}
                vertexCount={measurementVertexIds.length}
                lineCount={measurementLineIds.length}
              />
            )}
          </article>

          <WorkspaceLayoutSeparator kind="editor" />

          <article id="fold-preview-panel" className="panel preview-panel">
            <div className="panel-heading">
              <span>{text(APP_TEXT.text3dPreview)}</span>
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
                {text(APP_TEXT.autoRecord3DEdits)}
              </label>
              <span className={foldPreviewStatusClass}>{foldPreviewStatus}</span>
            </div>
            <FoldPreview
              angle={foldAngle}
              disabled={coreBusy || recoveryBlocking || Boolean(benchmarkRun)}
              projectInstanceId={nativeSnapshot?.project_instance_id ?? null}
              foldModelFingerprint={nativeSnapshot?.fold_model_fingerprint ?? null}
              onionSkinRequest={instructionOnionSkin}
              onOnionSkinStatusChange={setInstructionOnionSkinStatus}
              hingeAngles={foldTreeHingeAngles}
              selectedHingeId={selectedPreviewHingeId}
              selectedFaceId={selectedFaceId}
              highlightedFaceId={hoveredLayerFaceId}
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
              onCameraChange={(camera) => {
                if (!foldPreviewPoseModelKey) return
                setFoldPreviewCamera({ poseModelKey: foldPreviewPoseModelKey, camera })
              }}
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
            {nativeSnapshot?.crease_pattern.edges.some((edge) => edge.kind === 'cut') && (
              <EffectiveCutDiagnosticPanel
                key={`${nativeSnapshot.project_instance_id}:${nativeSnapshot.project_id}:${nativeSnapshot.revision}:${nativeSnapshot.fold_model_fingerprint}`}
                snapshot={nativeSnapshot}
              />
            )}
            {topologyResponse && !topologyResponse.simulation_ready && (
              <section className="validation-report invalid topology-blockers">
                <h2>{text(APP_TEXT.issuesBlocking3D)}</h2>
                <p>{formattedText(APP_TEXT.resolveTheseCountIssuesBeforeEntering3DFolding, { count: topologyResponse.issues.length })}</p>
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
                                  ? text(APP_TEXT.line)
                                  : text(APP_TEXT.vertex)}
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
                {text(APP_TEXT.fixedFace)}
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
                        {formattedText(APP_TEXT.faceIndex, { index: index + 1 })}
                      </option>
                    ))
                  : (
                      <option value="">
                        {text(APP_TEXT.unavailable)}
                      </option>
                    )}
              </select>
              <span>
                {fixedFaceEnabled
                  ? text(APP_TEXT.blueOutlineFixed)
                  : '—'}
              </span>
            </div>
            <div className="fold-control">
              <label htmlFor="fold-angle">
                {foldPreviewModel?.kind === 'fold_graph'
                  && foldPreviewModel.kinematics.kind === 'tree'
                  ? text(APP_TEXT.allHinges)
                  : text(APP_TEXT.targetFold)}
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
                        ? text(APP_TEXT.targetFoldForAllHingesDegrees)
                        : text(APP_TEXT.targetFoldDegrees)
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
                      {text(APP_TEXT.foldAmountByHinge)}
                    </strong>
                    <span>
                      {text(APP_TEXT.orangeOutlineDependentFaceCollisionUnchecked)}
                    </span>
                  </div>
                  {foldPreviewModel.kinematics.joints.map((joint, index) => {
                    const hingeAngle = foldTreeHingeAngles[index]?.angleDegrees ?? foldAngle
                    const label = joint.hinge.assignment === 'mountain'
                      ? text(APP_TEXT.mountainFold2)
                      : text(APP_TEXT.valleyFold2)
                    const inputId = `hinge-angle-${joint.hinge.edgeId}`
                    const selected = selectedLineId === joint.hinge.edgeId
                    return (
                      <div className="hinge-angle-row" key={joint.hinge.edgeId}>
                        <button
                          type="button"
                          className="hinge-select-button"
                          aria-pressed={benchmarkRun ? false : selected}
                          aria-label={formattedText(APP_TEXT.actionLabelIndexIn2DAnd3D, {
                            index: index + 1,
                            label,
                            action: selected
                              ? text(APP_TEXT.deselect)
                              : text(APP_TEXT.select),
                          })}
                          disabled={Boolean(benchmarkRun)}
                          title={formattedText(APP_TEXT.selectIn2DAnd3DEdgeId, { edgeId: joint.hinge.edgeId })}
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
                          aria-label={formattedText(APP_TEXT.foldAmountForLabelIndex, { index: index + 1, label })}
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
                            aria-label={formattedText(APP_TEXT.angleForLabelIndex, { index: index + 1, label })}
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
            {text(APP_TEXT.properties)}
          </div>
          <section>
            <h2>{text(APP_TEXT.selection)}</h2>
            <section className="mirror-selection-panel" aria-labelledby="mirror-selection-heading">
              <h3 id="mirror-selection-heading">
                {text(APP_TEXT.mirrorSelection)}
              </h3>
              <p aria-live="polite">
                {formattedText(APP_TEXT.verticesVerticesEdgesEdges, { vertices: mirrorVertexIds.length, edges: mirrorEdgeIds.length })}
              </p>
              <div className="button-row">
                <button
                  type="button"
                  disabled={coreBusy || mirrorBusy || (!selectedVertex && !selectedLine)}
                  onClick={addCurrentToMirrorSelection}
                >
                  {text(APP_TEXT.addCurrentSelection)}
                </button>
                {beginnerCandidateBusy && (
                  <button type="button" onClick={cancelBeginnerCandidates}>
                    {text(APP_TEXT.cancelCandidateGeneration)}
                  </button>
                )}
                <button
                  type="button"
                  disabled={coreBusy || (
                    mirrorVertexIds.length === 0 && mirrorEdgeIds.length === 0
                  )}
                  onClick={cancelMirrorSelection}
                >
                  {text(APP_TEXT.cancel)}
                </button>
              </div>
              <fieldset disabled={coreBusy || mirrorBusy}>
                <legend>{text(APP_TEXT.operation)}</legend>
                <label>
                  <input
                    type="radio"
                    name="mirror_mode"
                    checked={mirrorMode === 'duplicate'}
                    onChange={() => {
                      setMirrorMode('duplicate')
                      setMirrorPreview(null)
                    }}
                  />
                  {text(APP_TEXT.duplicate)}
                </label>
                <label>
                  <input
                    type="radio"
                    name="mirror_mode"
                    checked={mirrorMode === 'move'}
                    onChange={() => {
                      setMirrorMode('move')
                      setMirrorPreview(null)
                    }}
                  />
                  {text(APP_TEXT.move)}
                </label>
              </fieldset>
              <fieldset disabled={coreBusy || mirrorBusy}>
                <legend>{text(APP_TEXT.twoPointMirrorAxis)}</legend>
                {([
                  ['x1', '始点 X', 'Start X'],
                  ['y1', '始点 Y', 'Start Y'],
                  ['x2', '終点 X', 'End X'],
                  ['y2', '終点 Y', 'End Y'],
                ] as const).map(([key, ja, en]) => (
                  <label className="field" key={key}>
                    <span>{text({ ja, en })}</span>
                    <input
                      aria-label={text({ ja, en })}
                      inputMode="decimal"
                      value={mirrorAxis[key]}
                      onChange={(event) => {
                        setMirrorAxis((current) => ({
                          ...current,
                          [key]: event.currentTarget.value,
                        }))
                        setMirrorPreview(null)
                      }}
                    />
                  </label>
                ))}
              </fieldset>
              <div className="button-row">
                <button
                  type="button"
                  disabled={
                    coreBusy || mirrorBusy
                    || (mirrorVertexIds.length === 0 && mirrorEdgeIds.length === 0)
                  }
                  onClick={() => void previewCurrentMirrorSelection()}
                >
                  {mirrorBusy
                    ? text(APP_TEXT.checking)
                    : text(APP_TEXT.preflight)}
                </button>
                <button
                  type="button"
                  disabled={coreBusy || mirrorBusy || !mirrorPreview?.result.allowed}
                  onClick={() => void applyCurrentMirrorSelection()}
                >
                  {text(APP_TEXT.applyMirrorEdit)}
                </button>
              </div>
              {mirrorPreview && (
                <p
                  role="status"
                  data-testid="mirror-selection-preflight"
                  className={mirrorPreview.result.allowed ? 'status-good' : 'status-bad'}
                >
                  {mirrorPreview.result.allowed
                    ? text(APP_TEXT.readyReviewAndExplicitlyApplyTheEdit)
                    : mirrorPreflightIssueText(mirrorPreview.result.issue)}
                </p>
              )}
            </section>
            {selectedElementTarget && (
              <form
                key={`${selectedElementTarget.kind}:${selectedElementTarget.id}:${nativeSnapshot?.revision ?? 0}`}
                className="element-metadata-form"
                onSubmit={submitElementMetadata}
              >
                <label className="field">
                  <span>{text(APP_TEXT.name)}</span>
                  <input
                    name="element_name"
                    type="text"
                    maxLength={120}
                    defaultValue={selectedElementMetadata?.name ?? ''}
                    disabled={coreBusy}
                  />
                </label>
                <label className="field">
                  <span>{text(APP_TEXT.memo)}</span>
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
                  {text(APP_TEXT.useCustomColor)}
                </label>
                <label className="paper-color-field">
                  <span>{text(APP_TEXT.color)}</span>
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
                  {text(APP_TEXT.saveElementDetails)}
                </button>
              </form>
            )}
            {selectedLine ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedLine.id}</dd></div>
                  <div>
                    <dt>{text(APP_TEXT.type)}</dt>
                    <dd>{lineKindLabel(selectedLine.kind, locale)}</dd>
                  </div>
                  <div>
                    <dt>{text(APP_TEXT.start)}</dt>
                    <dd>{formatLengthPoint(
                      selectedLine.x1,
                      selectedLine.y1,
                      displayedLengthUnit,
                      locale,
                    )}</dd>
                  </div>
                  <div>
                    <dt>{text(APP_TEXT.end)}</dt>
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
                    <dt>{text(APP_TEXT.length)}</dt>
                    <dd>{formatLength(selectedLineMeasurement?.length, displayedLengthUnit, locale)}</dd>
                  </div>
                  <div>
                    <dt>{text(APP_TEXT.angle)}</dt>
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
                      {text(APP_TEXT.deleteBenchmarkLine)}
                    </button>
                    <p className="muted">
                      {text(APP_TEXT.selectionMeasurementVertexMovementAndLineDeletionAreAvailableOn)}
                    </p>
                  </>
                ) : (
                  <>
                  <form onSubmit={(event) => void submitMoveSelectedEdge(event)}>
                    <fieldset disabled={coreBusy || selectedLine.locked}>
                      <legend>{text(APP_TEXT.moveEntireLine)}</legend>
                      <label className="field">
                        {formattedText(APP_TEXT.horizontalOffsetUnit, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="edge_delta_x_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <label className="field">
                        {formattedText(APP_TEXT.verticalOffsetUnit, { unit: lengthDisplayUnitLabelText })}
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
                          {text(APP_TEXT.moveEntireLine)}
                        </button>
                      </div>
                    </fieldset>
                  </form>
                  <form onSubmit={(event) => void submitMirrorSelectedEdge(event)}>
                    <fieldset disabled={coreBusy || selectedLine.locked}>
                      <legend>{text(APP_TEXT.leftRightSymmetry)}</legend>
                      <label className="field">
                        {formattedText(APP_TEXT.mirrorAxisXUnit, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="symmetry_axis_x_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <button type="submit">
                        {text(APP_TEXT.applyLeftRightReflection)}
                      </button>
                    </fieldset>
                  </form>
                  <form onSubmit={(event) => void submitRotateSelectedEdge(event)}>
                    <fieldset disabled={coreBusy || selectedLine.locked}>
                      <legend>{text(APP_TEXT.rotationalSymmetry)}</legend>
                      <label className="field">
                        {formattedText(APP_TEXT.centerXUnit, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="rotation_center_x_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <label className="field">
                        {formattedText(APP_TEXT.centerYUnit, { unit: lengthDisplayUnitLabelText })}
                        <input
                          name="rotation_center_y_display"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="0"
                        />
                      </label>
                      <label className="field">
                        {text(APP_TEXT.rotationAngle)}
                        <input
                          name="rotation_angle_degrees"
                          type="text"
                          inputMode="text"
                          maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                          defaultValue="180"
                        />
                      </label>
                      <button type="submit">
                        {text(APP_TEXT.applyRotation)}
                      </button>
                    </fieldset>
                  </form>
                  {selectedLine.kind !== 'boundary' && (
                    <form
                      onSubmit={(event) => void submitLinearArrayPreview(event)}
                      onInput={() => {
                        linearArrayRequestSequenceRef.current += 1
                        setLinearArrayPreview(null)
                      }}
                      data-testid="linear-array-panel"
                    >
                      <fieldset disabled={coreBusy || selectedLine.locked}>
                        <legend>{text(APP_TEXT.linearArray)}</legend>
                        <label className="field">
                          {text(APP_TEXT.additionalCopies)}
                          <input name="linear_array_copies" type="number" min="1" max="16" step="1" defaultValue="1" />
                        </label>
                        <label className="field">
                          {text(APP_TEXT.xOffsetMm)}
                          <input name="linear_array_dx" type="number" step="any" defaultValue="10" />
                        </label>
                        <label className="field">
                          {text(APP_TEXT.yOffsetMm)}
                          <input name="linear_array_dy" type="number" step="any" defaultValue="0" />
                        </label>
                        <button type="submit" data-testid="preview-linear-array">
                          {text(APP_TEXT.previewArray)}
                        </button>
                      </fieldset>
                      {linearArrayPreview?.request.edges[0] === selectedLine.id && (
                        <div data-testid="linear-array-preview" aria-live="polite">
                          <p>{formattedText(APP_TEXT.verticesVerticesAndEdgesEdgeSeedsWillBeAddedThe, {
                            vertices: linearArrayPreview.result.generated_vertex_count,
                            edges: linearArrayPreview.result.generated_edge_seed_count,
                          })}</p>
                          <button type="button" onClick={() => void confirmCurrentLinearArray()} data-testid="confirm-linear-array">
                            {text(APP_TEXT.confirmArray)}
                          </button>
                          <button type="button" onClick={() => {
                            linearArrayRequestSequenceRef.current += 1
                            setLinearArrayPreview(null)
                          }}>
                            {text(APP_TEXT.cancel2)}
                          </button>
                        </div>
                      )}
                    </form>
                  )}
                  {selectedLine.kind !== 'boundary' && (
                    <form onSubmit={(event)=>void submitRadialArrayPreview(event)} onInput={()=>{radialArrayRequestSequenceRef.current+=1;setRadialArrayPreview(null)}} data-testid="radial-array-panel">
                      <fieldset disabled={coreBusy||selectedLine.locked}><legend>{text(APP_TEXT.radialArray)}</legend>
                        <p className="muted">{text(APP_TEXT.usesTheStartVertexOfTheSelectedLineAsThe)}</p>
                        <label className="field">{text(APP_TEXT.additionalCopies)}<input name="radial_array_copies" type="number" min="1" max="3" step="1" defaultValue="1"/></label>
                        <label className="field">{text(APP_TEXT.rotationAngle2)}<select name="radial_array_angle" defaultValue="90"><option value="90">90°</option><option value="180">180°</option><option value="270">270°</option></select></label>
                        <button type="submit" data-testid="preview-radial-array">{text(APP_TEXT.previewRadialArray)}</button>
                      </fieldset>
                      {radialArrayPreview?.request.edges[0]===selectedLine.id&&<div data-testid="radial-array-preview" aria-live="polite"><p>{formattedText(APP_TEXT.copiesCopiesWillBeAddedAfterConfirmation,{copies:radialArrayPreview.result.additional_copies})}</p><button type="button" data-testid="confirm-radial-array" onClick={()=>void confirmCurrentRadialArray()}>{text(APP_TEXT.confirmRadialArray)}</button><button type="button" onClick={()=>{radialArrayRequestSequenceRef.current+=1;setRadialArrayPreview(null)}}>{text(APP_TEXT.cancel2)}</button></div>}
                    </form>
                  )}
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
                        ? text(APP_TEXT.clearDirectionReference)
                        : text(APP_TEXT.setAsDirectionReference)}
                    </button>
                    {selectedLine.kind === 'boundary' ? (
                      <button
                        type="button"
                        disabled={coreBusy || selectedLine.locked}
                        onClick={() => void splitSelectedBoundaryEdge()}
                      >
                        {text(APP_TEXT.splitBoundaryEdgeAtMidpoint)}
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="danger"
                        disabled={coreBusy || selectedLine.locked}
                        onClick={() => void deleteSelection()}
                      >
                        {text(APP_TEXT.deleteLine)}
                      </button>
                    )}
                  </div>
                  </>
                )}
                {selectedLine.locked && (
                  <p className="muted">
                    {text(APP_TEXT.thisLineLayerIsLockedSelectionMeasurementAndReferencesRemain)}
                  </p>
                )}
                {selectedLine.kind === 'boundary' && (
                  <p className="muted">
                    {text(APP_TEXT.moveTheNewlySelectedVertexAfterSplittingToEditThe)}
                  </p>
                )}
              </>
            ) : selectedFace ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedFace.id}</dd></div>
                  <div>
                    <dt>{text(APP_TEXT.boundaryVertices)}</dt>
                    <dd>{selectedFace.vertexIds.length}</dd>
                  </div>
                  <div>
                    <dt>{text(APP_TEXT.boundaryLines)}</dt>
                    <dd>{selectedFace.edgeIds.length}</dd>
                  </div>
                </dl>
                <form onSubmit={(event) => void submitMoveSelectedFace(event)}>
                  <fieldset disabled={coreBusy || selectedFaceLocked}>
                    <legend>{text(APP_TEXT.moveEntireFace)}</legend>
                    <label className="field">
                      {formattedText(APP_TEXT.horizontalOffsetUnit, { unit: lengthDisplayUnitLabelText })}
                      <input
                        name="face_delta_x_display"
                        type="text"
                        inputMode="text"
                        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                        defaultValue="0"
                      />
                    </label>
                    <label className="field">
                      {formattedText(APP_TEXT.verticalOffsetUnit, { unit: lengthDisplayUnitLabelText })}
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
                        {text(APP_TEXT.moveEntireFace)}
                      </button>
                    </div>
                  </fieldset>
                </form>
                <form onSubmit={(event) => void submitSplitSelectedFace(event)}>
                  <fieldset disabled={
                    coreBusy || selectedFaceLocked || selectedFace.vertexIds.length < 4
                  }>
                    <legend>{text(APP_TEXT.addOrSplitAFace)}</legend>
                    <label className="field">
                      {text(APP_TEXT.startVertex)}
                      <select
                        name="face_split_start"
                        defaultValue={selectedFace.vertexIds[0]}
                      >
                        {selectedFace.vertexIds.map((vertexId, index) => (
                          <option value={vertexId} key={vertexId}>
                            {formattedText(APP_TEXT.vertexIndexId, { index: index + 1, id: vertexId })}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="field">
                      {text(APP_TEXT.endVertex)}
                      <select
                        name="face_split_end"
                        defaultValue={selectedFace.vertexIds[2]}
                      >
                        {selectedFace.vertexIds.map((vertexId, index) => (
                          <option value={vertexId} key={vertexId}>
                            {formattedText(APP_TEXT.vertexIndexId, { index: index + 1, id: vertexId })}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="field">
                      {text(APP_TEXT.splitLineType)}
                      <select name="face_split_kind" defaultValue="mountain">
                        <option value="mountain">
                          {text(APP_TEXT.mountainFold)}
                        </option>
                        <option value="valley">
                          {text(APP_TEXT.valleyFold)}
                        </option>
                        <option value="auxiliary">
                          {text(APP_TEXT.auxiliaryLine)}
                        </option>
                        {nativeSnapshot?.cutting_allowed && (
                          <option value="cut">
                            {text(APP_TEXT.cut2)}
                          </option>
                        )}
                      </select>
                    </label>
                    <div className="property-actions">
                      <button type="submit">
                        {text(APP_TEXT.splitAndAddFace)}
                      </button>
                    </div>
                  </fieldset>
                </form>
                <form onSubmit={(event) => void submitMergeSelectedFace(event)}>
                  <fieldset disabled={
                    coreBusy || selectedFaceLocked || selectedFaceRemovableEdges.length === 0
                  }>
                    <legend>{text(APP_TEXT.deleteOrMergeFace)}</legend>
                    <label className="field">
                      {text(APP_TEXT.sharedLineToRemove)}
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
                        {text(APP_TEXT.removeLineAndMergeFace)}
                      </button>
                    </div>
                  </fieldset>
                </form>
                {selectedFaceLocked && (
                  <p className="muted">
                    {text(APP_TEXT.thisFaceCannotMoveBecauseItsBoundaryIncludesALocked)}
                  </p>
                )}
              </>
            ) : selectedBenchmarkVertex ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedBenchmarkVertex.id}</dd></div>
                  <div>
                    <dt>{text(APP_TEXT.type)}</dt>
                    <dd>{text(APP_TEXT.benchmarkVertex)}</dd>
                  </div>
                  <div><dt>X</dt><dd>{selectedBenchmarkVertex.x}</dd></div>
                  <div><dt>Y</dt><dd>{selectedBenchmarkVertex.y}</dd></div>
                </dl>
                <p className="muted">
                  {text(APP_TEXT.dragTheBenchmarkVertexIn2DToMoveItAnd)}
                </p>
              </>
            ) : selectedVertex ? (
              <>
                <dl>
                  <div><dt>ID</dt><dd>{selectedVertex.id}</dd></div>
                  <div>
                    <dt>{text(APP_TEXT.type)}</dt>
                    <dd>{text(APP_TEXT.vertex)}</dd>
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
                      aria-label={formattedText(APP_TEXT.vertexXCoordinateUnit, { unit: lengthDisplayUnitLabelText })}
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
                      aria-label={formattedText(APP_TEXT.vertexYCoordinateUnit, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <div className="property-actions">
                    <button
                      type="submit"
                      name="vertex_action"
                      value="update_coordinates"
                      disabled={coreBusy || selectedVertexLocked}
                    >
                      {text(APP_TEXT.updateCoordinates)}
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
                        ? text(APP_TEXT.deleteBoundaryVertexAndMergeEdges)
                        : text(APP_TEXT.deleteVertex)}
                    </button>
                  </div>
                  {selectedVertexExpression?.polar_construction ? (
                    <p className="muted" data-vertex-polar-expression>
                      {formattedText(APP_TEXT.constructionExpressionLengthLengthMmAngleAngleEvaluatedLengthValueMm, {
                        length: selectedVertexExpression.polar_construction.length_source,
                        angle: selectedVertexExpression.polar_construction.angle_degrees_source,
                        lengthValue: selectedVertexExpression.polar_construction.adopted_length_mm,
                        angleValue: selectedVertexExpression.polar_construction.adopted_angle_degrees,
                      })}
                    </p>
                  ) : null}
                  <fieldset>
                    <legend>
                      {text(APP_TEXT.endpointByLengthAndAngle)}
                    </legend>
                    <label className="field">
                      {`${text(APP_TEXT.length)} (${lengthDisplayUnitLabelText})`}
                      <input
                        name="polar_length_display"
                        type="text"
                        inputMode="text"
                        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                        defaultValue={formatLengthInput(10, lengthDisplayUnit)}
                        disabled={coreBusy || selectedVertexLocked}
                        aria-label={formattedText(APP_TEXT.lengthFromTheStartVertexUnit, { unit: lengthDisplayUnitLabelText })}
                      />
                    </label>
                    <label className="field">
                      {text(APP_TEXT.angleDegrees)}
                      <input
                        name="polar_angle_degrees"
                        type="text"
                        inputMode="text"
                        maxLength={MAX_NUMERIC_EXPRESSION_SOURCE_BYTES}
                        defaultValue="0"
                        disabled={coreBusy || selectedVertexLocked}
                        aria-label={text(APP_TEXT.angleFromTheStartVertexDegrees)}
                      />
                    </label>
                    <label className="field">
                      {text(APP_TEXT.lineType)}
                      <select
                        name="polar_edge_kind"
                        defaultValue="mountain"
                        disabled={coreBusy || selectedVertexLocked}
                        aria-label={text(APP_TEXT.lineTypeForLengthAndAngleDrawing)}
                      >
                        <option value="mountain">
                          {text(APP_TEXT.mountainFold)}
                        </option>
                        <option value="valley">
                          {text(APP_TEXT.valleyFold)}
                        </option>
                        <option value="auxiliary">
                          {text(APP_TEXT.auxiliaryLine)}
                        </option>
                        {nativeSnapshot?.cutting_allowed && (
                          <option value="cut">
                            {text(APP_TEXT.cut2)}
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
                        {text(APP_TEXT.drawLineByLengthAndAngle)}
                      </button>
                      <button
                        type="submit"
                        name="vertex_action"
                        value="ray_to_target"
                        data-testid="draw-ray-to-first-target"
                        disabled={coreBusy || selectedVertexLocked}
                      >
                        {text(APP_TEXT.drawToFirstTargetByAngle)}
                      </button>
                    </div>
                  </fieldset>
                  <fieldset>
                    <legend>
                      {text(APP_TEXT.compassCircle)}
                    </legend>
                    <label className="field">
                      {`${text(APP_TEXT.radius)} (${lengthDisplayUnitLabelText})`}
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
                        {text(APP_TEXT.addCircleAtSelectedVertex)}
                      </button>
                      <button
                        type="button"
                        disabled={coreBusy || compassCircles.length === 0}
                        onClick={() => setCompassCircles([])}
                      >
                        {text(APP_TEXT.clearCompassCircles)}
                      </button>
                    </div>
                    <p className="muted">
                      {formattedText(APP_TEXT.countConstructionCirclesTheVertexToolSnapsToCircleLine, { count: compassCircles.length })}
                    </p>
                  </fieldset>
                  {selectedVertexLocked && (
                    <p className="muted">
                      {text(APP_TEXT.thisVertexIsConnectedToALineOnALocked)}
                    </p>
                  )}
                  <p className="muted">
                    {selectedVertexIsBoundary
                      ? formattedText(APP_TEXT.aBoundaryNeedsAtLeastThreePointsCountCurrentlyThis, { count: paperBoundaryVertexCount })
                      : text(APP_TEXT.deleteConnectedLinesBeforeDeletingTheirVertex)}
                  </p>
                </form>
              </>
            ) : nativeSnapshot && !benchmarkRun ? (
              <>
                <p className="muted">
                  {text(APP_TEXT.selectALineOrVertexOrAddAVertexBy)}
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
                      aria-label={formattedText(APP_TEXT.newVertexXCoordinateUnit, { unit: lengthDisplayUnitLabelText })}
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
                      aria-label={formattedText(APP_TEXT.newVertexYCoordinateUnit, { unit: lengthDisplayUnitLabelText })}
                    />
                  </label>
                  <div className="property-actions">
                    <button
                      type="submit"
                      disabled={coreBusy || nativeLayerView.defaultLayerLocked}
                    >
                      {text(APP_TEXT.addVertexByCoordinates)}
                    </button>
                  </div>
                  {nativeLayerView.defaultLayerLocked && (
                    <p className="muted">
                      {text(APP_TEXT.unlockTheDefaultLayerBeforeAddingAVertex)}
                    </p>
                  )}
                </form>
              </>
            ) : (
              <p className="muted">
                {text(APP_TEXT.selectALineOrVertex)}
              </p>
            )}
          </section>
          {nativeSnapshot && !benchmarkRun && (
            <section className="property-section">
              <h2>{text(APP_TEXT.projectMemo)}</h2>
              <form
                key={`${nativeSnapshot.project_instance_id}:${nativeSnapshot.memo}`}
                onSubmit={(event) => void submitProjectMemo(event)}
              >
                <label>
                  <span>{text(APP_TEXT.notes)}</span>
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
                    {text(APP_TEXT.saveMemo)}
                  </button>
                </div>
              </form>
              <div aria-labelledby="beginner-candidate-heading">
                <BeginnerCandidateControls
                  locale={locale}
                  coreBusy={coreBusy}
                  recoveryBlocking={recoveryBlocking}
                  skeletonTreeStatus={beginnerSkeletonTree.status}
                  candidateWorkflow={beginnerCandidateWorkflow}
                  gridWorkflow={beginnerGridWorkflow}
                />
                <BeginnerCandidateResults
                  locale={locale}
                  snapshot={nativeSnapshot}
                  coreBusy={coreBusy}
                  recoveryBlocking={recoveryBlocking}
                  candidateWorkflow={beginnerCandidateWorkflow}
                />
              </div>
            </section>
          )}
          {nativeSnapshot && !benchmarkRun && (
            <section className="property-section" aria-labelledby="beginner-design-heading">
              <h2 id="beginner-design-heading">
                {text(APP_TEXT.beginnerDesignPriorities)}
              </h2>
              <p className="muted">
                {text(APP_TEXT.setsHowFutureOnDeviceDesignCandidatesAreScoredIt)}
              </p>
              <form
                ref={beginnerDesignFormRef}
                key={[
                  nativeSnapshot.project_instance_id,
                  nativeSnapshot.beginner_design_profile.preset,
                  nativeSnapshot.beginner_design_profile.generation_constraints.maximum_steps,
                  nativeSnapshot.beginner_design_profile.generation_constraints.detail_level,
                  JSON.stringify(nativeSnapshot.beginner_design_profile.generation_constraints.generic_body_size_tenths_mm),
                  JSON.stringify(nativeSnapshot.beginner_design_profile.generation_constraints.generic_body_outline_tenths_mm),
                  nativeSnapshot.beginner_design_profile.generation_constraints.generic_body_outline_mode ?? 'symmetric',
                  nativeSnapshot.beginner_design_profile.generation_constraints.target_category ?? 'unset',
                  JSON.stringify(nativeSnapshot.beginner_design_profile.generation_constraints.target_parts),
                  JSON.stringify(nativeSnapshot.beginner_design_profile.generation_constraints.skeleton_segments),
                  JSON.stringify(nativeSnapshot.beginner_design_profile.generation_constraints.protrusions),
                  JSON.stringify(nativeSnapshot.beginner_design_profile.generation_constraints.bulge_targets),
                  JSON.stringify(nativeSnapshot.beginner_design_profile.generation_constraints.target_asset),
                  nativeSnapshot.beginner_design_profile.generation_constraints.allowed_techniques.join(','),
                ].join(':')}
                onSubmit={submitBeginnerDesignProfile}
              >
                {nativeSnapshot.beginner_design_profile.outline_edit_authority && (
                  <p role="status">{formattedText(APP_TEXT.savedOutlineEditAuthorityCountEditsImageDigestDigest, {
                    count: nativeSnapshot.beginner_design_profile.outline_edit_authority.edits.length,
                    digest: nativeSnapshot.beginner_design_profile.outline_edit_authority.source_sha256
                      .slice(0, 4).map((byte) => byte.toString(16).padStart(2, '0')).join(''),
                  })}</p>
                )}
                {nativeSnapshot.beginner_design_profile.generation_provenance?.generic_tree && (
                  <div role="status"><p>{formattedText(APP_TEXT.savedGenericTreeNameOriginSourceOrientationOrientationGeneratorV, {
                    name: nativeSnapshot.beginner_design_profile.generation_constraints.custom_object_display_name ?? 'Custom object',
                    source: nativeSnapshot.beginner_design_profile.generation_provenance.generic_tree.source,
                    orientation: nativeSnapshot.beginner_design_profile.generation_provenance.generic_tree.orientation,
                    version: nativeSnapshot.beginner_design_profile.generation_provenance.generic_tree.generator_version,
                  })}</p>
                    {nativeSnapshot.beginner_design_profile.generation_provenance.generic_tree.instruction_proposal && (
                      <ol aria-label={text(APP_TEXT.readOnlyFoldingInstructionProposal)}>
                        {nativeSnapshot.beginner_design_profile.generation_provenance.generic_tree.instruction_proposal.steps.map((step) => (
                          <li key={step.canonical_crease_id}>
                            {step.canonical_crease_id} · depth {step.tree_depth} · {step.assignment} · {step.target_branch} · fixed {step.fixed_side}
                            <br />{step.caution}
                          </li>
                        ))}
                      </ol>
                    )}
                    {nativeSnapshot.beginner_design_profile.generation_provenance.generic_tree.instruction_proposal && (
                      <button type="button" onClick={confirmAndAppendGenericTreeInstructions}>
                        {text(APP_TEXT.confirmAndAppendToInstructions)}
                      </button>
                    )}
                  </div>
                )}
                <label className="field">
                  <span>{text(APP_TEXT.evaluationPreset)}</span>
                  <select
                    name="design_preset"
                    defaultValue={nativeSnapshot.beginner_design_profile.preset}
                    disabled={coreBusy || recoveryBlocking}
                    aria-describedby="beginner-design-weights"
                  >
                    <option value="balanced">
                      {text(APP_TEXT.balanced)}
                    </option>
                    <option value="shape_priority">
                      {text(APP_TEXT.shapeFidelityPriority)}
                    </option>
                    <option value="foldability_priority">
                      {text(APP_TEXT.foldabilityPriority)}
                    </option>
                  </select>
                </label>
                <p id="beginner-design-weights" className="muted">
                  {formattedText(APP_TEXT.currentWeightsShapeShapeFoldabilityFoldabilityStepsStepsPaperEfficiency, {
                    shape: nativeSnapshot.beginner_design_profile.shape_fidelity_weight,
                    foldability: nativeSnapshot.beginner_design_profile.foldability_weight,
                    steps: nativeSnapshot.beginner_design_profile.step_count_weight,
                    paper: nativeSnapshot.beginner_design_profile.paper_efficiency_weight,
                  })}
                </p>
                <label className="field">
                  <span>{text(APP_TEXT.targetShapeCategory)}</span>
                  <select
                    name="target_category"
                    required
                    defaultValue={nativeSnapshot.beginner_design_profile.generation_constraints.target_category ?? ''}
                    disabled={coreBusy || recoveryBlocking}
                    aria-describedby="beginner-target-category-help"
                  >
                    <option value="" disabled>
                      {text(APP_TEXT.selectACategory)}
                    </option>
                    <option value="animal">{text(APP_TEXT.animal)}</option>
                    <option value="insect">{text(APP_TEXT.insect)}</option>
                    <option value="custom_object">{text(APP_TEXT.customObject)}</option>
                  </select>
                </label>
                <label className="field">
                  <span>{text(APP_TEXT.customObjectDisplayName)}</span>
                  <input
                    name="custom_object_display_name"
                    type="text"
                    maxLength={64}
                    defaultValue={nativeSnapshot.beginner_design_profile.generation_constraints.custom_object_display_name ?? 'Custom object'}
                    disabled={coreBusy || recoveryBlocking}
                    aria-describedby="beginner-custom-object-name-help"
                  />
                </label>
                <p id="beginner-custom-object-name-help" className="muted">
                  {text(APP_TEXT.displayMetadataOnlyItDoesNotAffectGeneratorAuthorityOr)}
                </p>
                <p id="beginner-target-category-help" className="muted">
                  {text(APP_TEXT.animalAndInsectUseNamedTemplatesCustomObjectIsRouted)}
                </p>
                <label className="field">
                  <span>{text(APP_TEXT.referenceImage)}</span>
                  <select
                    name="target_reference_underlay"
                    defaultValue={
                      nativeSnapshot.beginner_design_profile.generation_constraints.target_asset
                        ?.kind === 'reference_image'
                        ? nativeSnapshot.beginner_design_profile.generation_constraints.target_asset
                            .underlay_id
                        : ''
                    }
                    disabled={coreBusy || recoveryBlocking}
                    aria-describedby="beginner-target-asset-help"
                  >
                    <option value="">{text(APP_TEXT.none2)}</option>
                    {(nativeSnapshot.underlays?.underlays ?? []).map((underlay, index) => (
                      <option key={underlay.id} value={underlay.id}>
                        {formattedText(APP_TEXT.underlayImageIndex, { index: index + 1 })}
                      </option>
                    ))}
                  </select>
                </label>
                <p id="beginner-target-asset-help" className="muted">
                  {text(APP_TEXT.onlyPNGJPEGImagesAlreadyPlacedInThisProjectCan)}
                </p>
                <div aria-live="polite">
                  <button
                    type="button"
                    onClick={requestBeginnerReferenceModelImport}
                    disabled={coreBusy || recoveryBlocking}
                    aria-describedby="beginner-reference-model-help"
                  >
                    {text(APP_TEXT.import3DReferenceModel)}
                  </button>
                  {beginnerRecognitionBusy && <button type="button"
                    onClick={invalidateBeginnerRecognition}>
                    {text(APP_TEXT.cancelImageRecognition)}
                  </button>}
                  <p id="beginner-reference-model-help" className="muted">
                    {text(APP_TEXT.aGLB20ModelIsAReadOnlyVisual)}
                  </p>
                  <fieldset aria-describedby="reference-consensus-selection-help">
                    <legend>References for consensus</legend>
                    <p id="reference-consensus-selection-help" className="muted">Select two to four project references. Content hashes are read only by the native core.</p>
                    {[...(nativeSnapshot.underlays?.underlays ?? []).map((underlay, index) => ({
                      kind: 'image' as const, asset_id: underlay.asset, label: `Underlay image ${index + 1} (image)`,
                    })), ...(nativeSnapshot.reference_model_assets ?? []).map((asset, index) => ({
                      kind: 'reference_model' as const, asset_id: asset.asset_id, label: `3D reference ${index + 1} (GLB)`,
                    }))].filter((asset, index, all) => all.findIndex((candidate) => candidate.asset_id === asset.asset_id) === index)
                      .map((asset) => {
                        const checked = consensusSelectionDraft.some((selection) => selection.asset_id === asset.asset_id)
                        return <label key={asset.asset_id}><input type="checkbox" checked={checked}
                          disabled={!checked && consensusSelectionDraft.length >= 4}
                          onChange={() => toggleConsensusReference(asset.kind, asset.asset_id)} />{asset.label}</label>
                      })}
                    <p role="status" aria-live="polite">{`${consensusSelectionDraft.length} of 2–4 references selected.`}</p>
                    <button type="button" disabled={consensusSelectionDraft.length < 2 || consensusSelectionDraft.length > 4 || coreBusy || recoveryBlocking}
                      onClick={saveConsensusReferences}>Save consensus references</button>
                  </fieldset>
                  {(nativeSnapshot.reference_model_assets ?? []).length > 0 && <ul aria-label={text(APP_TEXT.project3DReferenceAssets)}>
                    {(nativeSnapshot.reference_model_assets ?? []).map((asset, index) => {
                      const active = nativeSnapshot.beginner_design_profile.generation_constraints.target_asset
                        ?.kind === 'reference_model'
                        && nativeSnapshot.beginner_design_profile.generation_constraints.target_asset.asset_id === asset.asset_id
                      const archived = nativeSnapshot.beginner_design_profile.archived_reference_model_asset_ids
                        ?.includes(asset.asset_id) ?? false
                      return <li key={asset.asset_id}>
                        {`GLB ${index + 1} · SHA-256 ${asset.sha256.slice(0, 4)
                          .map((byte) => byte.toString(16).padStart(2, '0')).join('')}`}
                        {active ? <span> · Active reference</span> : !archived && <button type="button"
                          onClick={() => activateBeginnerReferenceAsset(asset.asset_id)}>
                          Activate this reference
                        </button>}
                        <button type="button" onClick={() => archiveBeginnerReferenceAsset(asset.asset_id, !archived)}>
                          {archived ? 'Restore archived reference' : 'Archive reference without deleting bytes'}
                        </button>
                      </li>
                    })}
                  </ul>}
                  {nativeSnapshot.beginner_design_profile.generation_constraints.target_asset?.kind
                    === 'reference_model' && (
                    <>
                      <p role="status">
                        {text(APP_TEXT.aValidated3DReferenceModelIsAttached)}
                      </p>
                      <button type="button" onClick={toggleBeginnerReferenceModelPreview}>
                        {beginnerReferenceGeometry
                          ? text(APP_TEXT.hide3DReferencePreview)
                          : text(APP_TEXT.show3DReferencePreview)}
                      </button>
                      <button type="button" onClick={requestBeginnerReferenceSuggestion}
                        disabled={coreBusy || recoveryBlocking}>
                        {text(APP_TEXT.suggestRangesFromSafeGeometryFeatures)}
                      </button>
                      {beginnerReferenceSuggestion && (
                        <div role="status">
                          <p>{text(APP_TEXT.thisIsNot3DRecognitionItIsAReadOnly)}</p>
                          <p>{formattedText(APP_TEXT.countProtrusionsLengthLengthMmThicknessThicknessMm, {
                            count: beginnerReferenceSuggestion.protrusions.reduce((sum, target) => sum + target.count, 0),
                            length: beginnerReferenceSuggestion.protrusions[0]?.length_tenths_mm ? beginnerReferenceSuggestion.protrusions[0].length_tenths_mm / 10 : 0,
                            thickness: beginnerReferenceSuggestion.protrusions[0]?.thickness_tenths_mm ? beginnerReferenceSuggestion.protrusions[0].thickness_tenths_mm / 10 : 0,
                          })}</p>
                          <p>{formattedText(APP_TEXT.general3DProposalQualityScore100PrincipalExtentsXY, {
                            score: beginnerReferenceSuggestion.quality_score,
                            x: beginnerReferenceSuggestion.principal_axis_extents_tenths_mm[0],
                            y: beginnerReferenceSuggestion.principal_axis_extents_tenths_mm[1],
                            z: beginnerReferenceSuggestion.principal_axis_extents_tenths_mm[2],
                            protrusions: beginnerReferenceSuggestion.general_protrusion_candidates.length,
                            bars: beginnerReferenceSuggestion.stick_bars.length,
                          })}</p>
                          {beginnerReferenceSuggestion.insufficiency_reasons.length > 0 && <p>{formattedText(APP_TEXT.general3DProposalInsufficiencyReasons, { reasons: beginnerReferenceSuggestion.insufficiency_reasons.join(', ') })}</p>}
                          <fieldset>
                            <legend>{text(APP_TEXT.explicitlyAssignMeasuredSurfaceRangesTo28Parts)}</legend>
                            {beginnerReferenceSuggestion.surface_ranges.map((range, index) => {
                              const target = beginnerReferenceSuggestion.protrusions[index]
                              if (!target) return null
                              return <div key={range.id}>
                                <input type="checkbox"
                                  aria-label={formattedText(APP_TEXT.assignSurfaceRangeRangeIdToPartPartId, { rangeId: range.id, partId: target.id })}
                                  checked={beginnerSurfaceAssignments.some(
                                    (item) => item.range_id === range.id)}
                                  onChange={(event) => setBeginnerSurfaceAssignments((current) => {
                                    if (event.currentTarget.checked) return [...current, {
                                      range_id: range.id, protrusion_id: target.id,
                                    }]
                                    return current.filter((item) => item.range_id !== range.id)
                                  })} />
                                {formattedText(APP_TEXT.surfaceRangeIdCenterXYZLengthLengthMm, {
                                  id: range.id,
                                  x: target.position_tenths_mm[0] / 10,
                                  y: target.position_tenths_mm[1] / 10,
                                  z: target.position_tenths_mm[2] / 10,
                                  length: target.length_tenths_mm / 10,
                                })}
                                <span>{formattedText(APP_TEXT.partId, { id: target.id })}</span>
                                <span>{text(APP_TEXT.triangleIndicesAddRemoveAdjacentFacesOnly)}</span>
                                <input type="text"
                                  aria-label={formattedText(APP_TEXT.surfaceRangeRangeIdTriangleIndices, { rangeId: range.id })}
                                  value={beginnerSurfaceEdits.find(
                                    (edit) => edit.range_id === range.id)?.triangle_indices.join(',') ?? ''}
                                  onChange={(event) => {
                                    const indices = event.currentTarget.value.split(',')
                                      .map((value) => Number(value.trim()))
                                      .filter((value) => Number.isInteger(value) && value >= 0)
                                    setBeginnerSurfaceEdits((current) => current.map((edit) =>
                                      edit.range_id === range.id
                                        ? { ...edit, triangle_indices: [...new Set(indices)] }
                                        : edit))
                                  }} />
                                {(['X', 'Y', 'Z'] as const).map((axis, axisIndex) => <label key={axis}>
                                  <span>{`Bulge direction ${axis}`}</span>
                                  <input type="number" min="-1" max="1" step="0.001"
                                    value={(beginnerSurfaceEdits.find(
                                      (edit) => edit.range_id === range.id)?.bulge_direction_milli[axisIndex] ?? 0) / 1000}
                                    onChange={(event) => setBeginnerSurfaceEdits((current) => current.map((edit) => {
                                      if (edit.range_id !== range.id) return edit
                                      const direction = [...edit.bulge_direction_milli] as [number, number, number]
                                      direction[axisIndex] = Math.round(Number(event.currentTarget.value) * 1000)
                                      return { ...edit, bulge_direction_milli: direction }
                                    }))} />
                                </label>)}
                                <label><span>{text(APP_TEXT.bulgeAmountMm)}</span>
                                  <input type="number" min="0.1" max="100000" step="0.1"
                                    value={(beginnerSurfaceEdits.find(
                                      (edit) => edit.range_id === range.id)?.bulge_amount_tenths_mm ?? 1) / 10}
                                    onChange={(event) => setBeginnerSurfaceEdits((current) => current.map((edit) =>
                                      edit.range_id === range.id ? { ...edit,
                                        bulge_amount_tenths_mm: Math.round(Number(event.currentTarget.value) * 10) } : edit))} />
                                </label>
                              </div>
                            })}
                            <p>{text(APP_TEXT.onlyGLBMeasuredRangesAreShownDuplicateUnconfirmedOrTampered)}</p>
                          </fieldset>
                          <button type="button" onClick={confirmBeginnerReferenceSuggestion}
                            disabled={beginnerSurfaceAssignments.length < 2}>
                            {text(APP_TEXT.confirmAndApplySuggestedRanges)}
                          </button>
                          {(beginnerReferenceSuggestion.generic_body_outline_tenths_mm
                            || beginnerReferenceSuggestion.protrusions.some(
                              (target) => target.local_outline_tenths_mm)) && <>
                            <p>{formattedText(APP_TEXT.editableBodyContourBodyPointsLocalContoursLocal, {
                              body: beginnerReferenceSuggestion.generic_body_outline_tenths_mm?.length ?? 0,
                              local: beginnerReferenceSuggestion.protrusions.filter(
                                (target) => target.local_outline_tenths_mm).length,
                            })}</p>
                            <button type="button" hidden onClick={copyBeginnerReferenceContours}>
                              {text(APP_TEXT.reviewAndCopyContoursToEditor)}
                            </button>
                          </>}
                          <RecognitionContourCopyAction locale={locale}
                            bodyPointCount={beginnerReferenceSuggestion
                              .generic_body_outline_tenths_mm?.length ?? 0}
                            localContourCount={beginnerReferenceSuggestion.protrusions.filter(
                              (target) => target.local_outline_tenths_mm).length}
                            onCopy={copyBeginnerReferenceContours} />
                          <button type="button" onClick={copyBeginnerGeneralReferenceTarget}>
                            {text(APP_TEXT.reviewAndCopyGeneral3DProposalToEditor)}
                          </button>
                          {beginnerComponentBridgeOverride && (
                            <fieldset aria-label={text(APP_TEXT.reviewedComponentBridgeOverrides)}>
                              <legend>Component bridges (reviewed, maximum 7)</legend>
                              {beginnerComponentBridgeOverride.bridges.map((bridge, index) => (
                                <label key={bridge.id}>
                                  <input type="checkbox" checked={bridge.accepted} onChange={(event) => {
                                    setBeginnerComponentBridgeOverride({ ...beginnerComponentBridgeOverride,
                                      bridges: beginnerComponentBridgeOverride.bridges.map((item, itemIndex) =>
                                        itemIndex === index ? { ...item, accepted: event.target.checked } : item),
                                    })
                                  }} />
                                  {`Bridge ${bridge.id}: component `}
                                  <select value={bridge.start_component_id} onChange={(event) => setBeginnerComponentBridgeOverride({
                                    ...beginnerComponentBridgeOverride, bridges: beginnerComponentBridgeOverride.bridges.map((item, itemIndex) => itemIndex === index ? { ...item, start_component_id: Number(event.target.value) } : item),
                                  })}>{Array.from({ length: beginnerComponentBridgeOverride.component_count }, (_, id) => <option key={id} value={id}>{id}</option>)}</select>
                                  {' to '}
                                  <select value={bridge.end_component_id} onChange={(event) => setBeginnerComponentBridgeOverride({
                                    ...beginnerComponentBridgeOverride, bridges: beginnerComponentBridgeOverride.bridges.map((item, itemIndex) => itemIndex === index ? { ...item, end_component_id: Number(event.target.value) } : item),
                                  })}>{Array.from({ length: beginnerComponentBridgeOverride.component_count }, (_, id) => <option key={id} value={id}>{id}</option>)}</select>
                                </label>
                              ))}
                            </fieldset>
                          )}
                        </div>
                      )}
                      {beginnerReferenceGeometry && (
                        <svg
                          viewBox="-100 -100 200 200"
                          role="img"
                          aria-label={text(APP_TEXT.readOnly3DReferenceModel)}
                        >
                          {beginnerReferenceGeometry.triangle_indices.map((triangle, index) => {
                            const points = triangle.map((vertex) => {
                              const position = beginnerReferenceGeometry.positions[vertex]
                              return `${position[0]},${-position[1]}`
                            }).join(' ')
                            return <polygon key={index} points={points} fill="none" stroke="currentColor" />
                          })}
                        </svg>
                      )}
                    </>
                  )}
                </div>
                <BeginnerRecognitionPanel
                  locale={locale}
                  coreBusy={coreBusy}
                  recoveryBlocking={recoveryBlocking}
                  workflow={beginnerRecognitionWorkflow}
                />
                <fieldset
                  aria-describedby="beginner-target-parts-help beginner-target-parts-total"
                  onInput={(event) => {
                    const inputs = event.currentTarget.querySelectorAll<HTMLInputElement>(
                      'input[name^="target_part_"]',
                    )
                    setBeginnerPartTotal(Array.from(inputs).reduce(
                      (sum, input) => sum + Math.max(0, Number(input.value) || 0),
                      0,
                    ))
                  }}
                >
                  <legend>{text(APP_TEXT.targetShapeParts)}</legend>
                  {([
                    ['head', APP_TEXT.head2],
                    ['torso', APP_TEXT.torso2],
                    ['leg', APP_TEXT.legs],
                    ['horn', APP_TEXT.horns],
                    ['ear', APP_TEXT.ears],
                    ['wing', APP_TEXT.wings],
                    ['tail', APP_TEXT.tails],
                  ] as const).map(([kind, label]) => (
                    <label className="field" key={kind}>
                      <span>{text(label)}</span>
                      <input
                        name={`target_part_${kind}`}
                        type="number"
                        min={kind === 'head' || kind === 'torso' ? 1 : 0}
                        max={8}
                        required={kind === 'head' || kind === 'torso'}
                        defaultValue={
                          nativeSnapshot.beginner_design_profile.generation_constraints.target_parts
                            .find((part) => part.kind === kind)?.count
                            ?? (kind === 'head' || kind === 'torso' ? 1 : 0)
                        }
                        disabled={coreBusy || recoveryBlocking}
                      />
                    </label>
                  ))}
                </fieldset>
                <fieldset aria-describedby="beginner-body-size-help">
                  <legend>{text(APP_TEXT.targetBodySizeOptional)}</legend>
                  <label className="field">
                    <span>{text(APP_TEXT.bodyWidthMm)}</span>
                    <input name="generic_body_width_mm" type="number" min={0.1} max={100000} step={0.1}
                      value={beginnerBodySize?.[0] === undefined ? '' : beginnerBodySize[0] / 10}
                      onChange={(event) => { const value = Number(event.currentTarget.value)
                        setBeginnerBodySize((current) => event.currentTarget.value === '' ? undefined
                          : [Math.round(value * 10), current?.[1] ?? Math.round(value * 10)]) }} />
                  </label>
                  <label className="field">
                    <span>{text(APP_TEXT.bodyHeightMm)}</span>
                    <input name="generic_body_height_mm" type="number" min={0.1} max={100000} step={0.1}
                      value={beginnerBodySize?.[1] === undefined ? '' : beginnerBodySize[1] / 10}
                      onChange={(event) => { const value = Number(event.currentTarget.value)
                        setBeginnerBodySize((current) => event.currentTarget.value === '' ? undefined
                          : [current?.[0] ?? Math.round(value * 10), Math.round(value * 10)]) }} />
                  </label>
                  <p id="beginner-body-size-help" className="muted">{text(APP_TEXT.leaveBothFieldsBlankForNoBodySizeTargetA)}</p>
                </fieldset>
                <GenericBodyOutlineEditor locale={locale} points={beginnerBodyOutline}
                  mode={beginnerBodyOutlineMode} onModeChange={(mode) => {
                    setBeginnerBodyOutlineMode(mode)
                    setBeginnerBodyOutline([])
                  }} onChange={setBeginnerBodyOutline} />
                <BeginnerShapeCanvasPreview locale={locale} bodySize={beginnerBodySize}
                  bodyOutline={beginnerBodyOutline} bodyMode={beginnerBodyOutlineMode}
                  protrusions={beginnerProtrusions} onBodyOutlineChange={setBeginnerBodyOutline}
                  onProtrusionChange={(changed) => setBeginnerProtrusions((targets) => targets.map(
                    (target) => target.id === changed.id ? changed : target,
                  ))} />
                <output id="beginner-target-parts-total" aria-live="polite">
                  {formattedText(APP_TEXT.totalPartsTotal32, { total: beginnerPartTotal })}
                </output>
                <p id="beginner-target-parts-help" className="muted">
                  {text(APP_TEXT.oneHeadAndOneTorsoAreRequiredEachPartIs)}
                </p>
                <fieldset aria-describedby="beginner-skeleton-help">
                  <legend>{text(APP_TEXT.stickSkeleton)}</legend>
                  <label className="field">
                    <span>{text(APP_TEXT.startXMm)}</span>
                    <input name="skeleton_start_x_mm" type="number" min={-10000} max={10000} step={0.1} defaultValue={0} />
                  </label>
                  <label className="field">
                    <span>{text(APP_TEXT.startYMm)}</span>
                    <input name="skeleton_start_y_mm" type="number" min={-10000} max={10000} step={0.1} defaultValue={0} />
                  </label>
                  <label className="field">
                    <span>{text(APP_TEXT.lengthMm)}</span>
                    <input name="skeleton_length_mm" type="number" min={0.1} max={10000} step={0.1} defaultValue={10} required />
                  </label>
                  <label className="field">
                    <span>{text(APP_TEXT.angleDegrees)}</span>
                    <input name="skeleton_angle_degrees" type="number" min={-360} max={360} step={0.1} defaultValue={0} required />
                  </label>
                  <label className="field">
                    <span>{text(APP_TEXT.thicknessMm)}</span>
                    <input name="skeleton_thickness_mm" type="number" min={0.1} max={1000} step={0.1} defaultValue={1} required />
                  </label>
                  <button
                    type="button"
                    disabled={beginnerSkeletonSegments.length >= 64 || coreBusy || recoveryBlocking}
                    onClick={(event) => {
                      if (event.currentTarget.form) addBeginnerSkeletonSegment(event.currentTarget.form)
                    }}
                  >
                    {text(APP_TEXT.addSkeletonBar)}
                  </button>
                  <svg viewBox="-110 -110 220 220" role="img"
                    aria-label={text(APP_TEXT.stickSkeletonPreview)}>
                    {beginnerSkeletonSegments.map((segment) => (
                      <line
                        key={segment.id}
                        x1={segment.start.x_tenths_mm / 10}
                        y1={segment.start.y_tenths_mm / 10}
                        x2={segment.end.x_tenths_mm / 10}
                        y2={segment.end.y_tenths_mm / 10}
                        stroke="currentColor"
                        strokeWidth={Math.max(0.5, segment.thickness_tenths_mm / 10)}
                      />
                    ))}
                  </svg>
                  <ul aria-label={text(APP_TEXT.skeletonBarList)}>
                    {beginnerSkeletonSegments.map((segment) => (
                      <li key={segment.id}>
                        #{segment.id}: {formattedText(APP_TEXT.thicknessThicknessMm, { thickness: segment.thickness_tenths_mm / 10 })}
                        {([
                          ['start.x_tenths_mm', text(APP_TEXT.startX), segment.start.x_tenths_mm],
                          ['start.y_tenths_mm', text(APP_TEXT.startY), segment.start.y_tenths_mm],
                          ['end.x_tenths_mm', text(APP_TEXT.endX), segment.end.x_tenths_mm],
                          ['end.y_tenths_mm', text(APP_TEXT.endY), segment.end.y_tenths_mm],
                          ['thickness_tenths_mm', text(APP_TEXT.thickness), segment.thickness_tenths_mm],
                        ] as const).map(([field, label, tenths]) => <label key={field}>
                          <span>{label} (mm)</span>
                          <input type="number" step="0.1" defaultValue={tenths / 10}
                            min={field === 'thickness_tenths_mm' ? 0.1 : -10000}
                            max={field === 'thickness_tenths_mm' ? 1000 : 10000}
                            aria-label={formattedText(APP_TEXT.skeletonBarSegmentIdLabelMm, { segmentId: segment.id, label })}
                            onBlur={(event) => {
                              const next = Math.round(Number(event.currentTarget.value) * 10)
                              const valid = Number.isSafeInteger(next) && (field === 'thickness_tenths_mm'
                                ? next >= 1 && next <= 10_000 : Math.abs(next) <= 100_000)
                              if (!valid) { event.currentTarget.value = String(tenths / 10); return }
                              setBeginnerSkeletonSegments((segments) => segments.map((item) => {
                                if (item.id !== segment.id) return item
                                if (field === 'thickness_tenths_mm') return { ...item, thickness_tenths_mm: next }
                                const [endpoint, axis] = field.split('.') as ['start' | 'end', 'x_tenths_mm' | 'y_tenths_mm']
                                const changed = { ...item, [endpoint]: { ...item[endpoint], [axis]: next } }
                                return changed.start.x_tenths_mm === changed.end.x_tenths_mm
                                  && changed.start.y_tenths_mm === changed.end.y_tenths_mm ? item : changed
                              }))
                            }} />
                        </label>)}
                        <button type="button" onClick={() => setBeginnerSkeletonSegments(
                          (segments) => segments.filter((item) => item.id !== segment.id),
                        )}>
                          {text(APP_TEXT.remove)}
                        </button>
                      </li>
                    ))}
                  </ul>
                </fieldset>
                <p id="beginner-skeleton-help" className="muted">
                  {text(APP_TEXT.upTo64BarsAreStoredAt01Mm)}
                </p>
                <p role="status">{beginnerSkeletonTree.status === 'tree'
                  ? formattedText(APP_TEXT.skeletonTreeConfirmedPointsJointsAndEdgesBranchesCandidateGeneration, { points: beginnerSkeletonTree.pointCount, edges: beginnerSkeletonTree.edgeCount })
                  : formattedText(APP_TEXT.skeletonTreeUnconfirmedReasonCyclesDuplicateEdgesAndDisconnectedSkeleton, { reason: beginnerSkeletonTree.status })}</p>
                <fieldset aria-describedby="beginner-protrusion-help">
                  <legend>{text(APP_TEXT.protrusionTargets)}</legend>
                  {([
                    ['protrusion_count', text(APP_TEXT.count), 2, 1, 8, 1],
                    ['protrusion_length_mm', text(APP_TEXT.lengthMm), 20, 0.1, 100000, 0.1],
                    ['protrusion_thickness_mm', text(APP_TEXT.thicknessMm), 2, 0.1, 1000, 0.1],
                    ['protrusion_position_x_mm', text(APP_TEXT.finalPositionXMm), 0, -10000, 10000, 0.1],
                    ['protrusion_position_y_mm', text(APP_TEXT.finalPositionYMm), 0, -10000, 10000, 0.1],
                    ['protrusion_position_z_mm', text(APP_TEXT.finalPositionZMm), 0, -10000, 10000, 0.1],
                    ['protrusion_direction_x', text(APP_TEXT.directionX), 1, -1, 1, 0.001],
                    ['protrusion_direction_y', text(APP_TEXT.directionY), 0, -1, 1, 0.001],
                    ['protrusion_direction_z', text(APP_TEXT.directionZ), 0, -1, 1, 0.001],
                    ['protrusion_curvature_degrees', text(APP_TEXT.curvatureDegrees), 0, -360, 360, 1],
                    ['protrusion_motion_min', text(APP_TEXT.motionMinimumDegrees), 0, -360, 360, 1],
                    ['protrusion_motion_max', text(APP_TEXT.motionMaximumDegrees), 0, -360, 360, 1],
                    ['protrusion_priority', text(APP_TEXT.priority), 50, 1, 100, 1],
                  ] as const).map(([name, label, initial, min, max, step]) => (
                    <label className="field" key={name}>
                      <span>{label}</span>
                      <input name={name} type="number" defaultValue={initial}
                        min={min} max={max} step={step} required />
                    </label>
                  ))}
                  <label className="field">
                    <span>{text(APP_TEXT.rootWidthMmOptional)}</span>
                    <input name="protrusion_root_width_mm" type="number" min={0.1} max={1000} step={0.1} />
                  </label>
                  <label className="field">
                    <span>{text(APP_TEXT.tipWidthMmOptional)}</span>
                    <input name="protrusion_tip_width_mm" type="number" min={0.1} max={1000} step={0.1} />
                  </label>
                  <label className="field"><span>{text(APP_TEXT.symmetry)}</span>
                    <select name="protrusion_symmetry" defaultValue="none">
                      <option value="none">{text(APP_TEXT.none)}</option>
                      <option value="bilateral">{text(APP_TEXT.bilateral)}</option>
                      <option value="radial">{text(APP_TEXT.radial)}</option>
                    </select>
                  </label>
                  <label className="field"><span>{text(APP_TEXT.joint)}</span>
                    <select name="protrusion_joint" defaultValue="fixed">
                      <option value="fixed">{text(APP_TEXT.fixed)}</option>
                      <option value="hinge">{text(APP_TEXT.hinge)}</option>
                      <option value="ball">{text(APP_TEXT.ball)}</option>
                    </select>
                  </label>
                  <label className="field"><span>{text(APP_TEXT.side)}</span>
                    <select name="protrusion_side" defaultValue="either">
                      <option value="front">{text(APP_TEXT.front)}</option>
                      <option value="back">{text(APP_TEXT.back)}</option>
                      <option value="either">{text(APP_TEXT.either)}</option>
                    </select>
                  </label>
                  <button type="button" disabled={beginnerProtrusions.length >= 8 || coreBusy}
                    onClick={(event) => event.currentTarget.form
                      && addBeginnerProtrusion(event.currentTarget.form)}>
                    {text(APP_TEXT.addProtrusionTarget)}
                  </button>
                  {beginnerProtrusions.length === 0 && <button type="button" disabled={coreBusy}
                    onClick={createEmptyGenericTarget}>
                    {text(APP_TEXT.createEmptyGenericTarget)}
                  </button>}
                  {beginnerProtrusions.length > 0 && <table aria-label={text(APP_TEXT.featureConstraintComparison)}>
                    <thead><tr><th>{text(APP_TEXT.feature)}</th>
                      <th>{text(APP_TEXT.length)}</th>
                      <th>{text(APP_TEXT.thickness)}</th>
                      <th>{text(APP_TEXT.joint)}</th>
                      <th>{text(APP_TEXT.motion)}</th>
                      <th>{text(APP_TEXT.side2)}</th>
                      <th>{text(APP_TEXT.priority)}</th></tr></thead>
                    <tbody>{beginnerProtrusions.map((target, index) => <tr key={target.id}>
                      <td>{beginnerProtrusionKinds[index] ?? 'tail'} #{target.id}</td>
                      <td>{target.length_tenths_mm / 10} mm</td><td>{target.thickness_tenths_mm / 10} mm</td>
                      <td>{target.joint}</td><td>{target.motion_degrees.join('..')}°</td>
                      <td>{target.side}</td><td>{target.priority}/100</td>
                    </tr>)}</tbody>
                  </table>}
                  <ul aria-label={text(APP_TEXT.protrusionTargetList)}>
                    {beginnerProtrusions.map((target, index) => (
                      <ProtrusionDimensionEditor key={target.id} locale={locale} target={target}
                        kind={beginnerProtrusionKinds[index] ?? 'tail'}
                        onKindChange={(kind) => setBeginnerProtrusionKinds((kinds) =>
                          kinds.length === beginnerProtrusions.length
                            ? kinds.map((item, kindIndex) => kindIndex === index ? kind : item)
                            : beginnerProtrusions.map((_, kindIndex) => kindIndex === index ? kind : 'tail'))}
                        onChange={(changed) => setBeginnerProtrusions((targets) => targets.map(
                          (item) => item.id === changed.id ? changed : item,
                        ))}
                        onRemove={() => {
                          setBeginnerProtrusions((targets) => targets.filter((item) => item.id !== target.id)
                            .map((item, canonicalIndex) => ({ ...item, id: canonicalIndex + 1 })))
                          setBeginnerProtrusionKinds((kinds) => kinds.filter((_, kindIndex) => kindIndex !== index))
                        }}
                        canRemove={beginnerProtrusions.length !== 2}
                        canMoveUp={index > 0} canMoveDown={index + 1 < beginnerProtrusions.length}
                        onMoveUp={() => {
                          setBeginnerProtrusions((targets) => {
                            if (index === 0) return targets
                            const moved = [...targets]
                            ;[moved[index - 1], moved[index]] = [moved[index]!, moved[index - 1]!]
                            return moved.map((item, canonicalIndex) => ({ ...item, id: canonicalIndex + 1 }))
                          })
                          setBeginnerProtrusionKinds((kinds) => {
                            if (index === 0) return kinds
                            const moved = [...kinds]
                            ;[moved[index - 1], moved[index]] = [moved[index]!, moved[index - 1]!]
                            return moved
                          })
                        }}
                        onMoveDown={() => {
                          setBeginnerProtrusions((targets) => {
                            if (index + 1 >= targets.length) return targets
                            const moved = [...targets]
                            ;[moved[index], moved[index + 1]] = [moved[index + 1]!, moved[index]!]
                            return moved.map((item, canonicalIndex) => ({ ...item, id: canonicalIndex + 1 }))
                          })
                          setBeginnerProtrusionKinds((kinds) => {
                            if (index + 1 >= kinds.length) return kinds
                            const moved = [...kinds]
                            ;[moved[index], moved[index + 1]] = [moved[index + 1]!, moved[index]!]
                            return moved
                          })
                        }} />
                    ))}
                  </ul>
                </fieldset>
                <p id="beginner-protrusion-help" className="muted">
                  {text(APP_TEXT.explicitlySetsCountDimensionsFinalPositionDirectionSymmetryCurvatureJoin)}
                </p>
                <fieldset aria-describedby="beginner-bulge-help">
                  <legend>{text(APP_TEXT.text3dBulgeTargets)}</legend>
                  <p>{selectedFaceId
                    ? formattedText(APP_TEXT.selectedFaceId, { id: selectedFaceId })
                    : text(APP_TEXT.selectATargetFaceInThe2DOr3DView)}</p>
                  {([
                    ['bulge_min_x', 'Range minimum X (mm)', -5],
                    ['bulge_min_y', 'Range minimum Y (mm)', -5],
                    ['bulge_min_z', 'Range minimum Z (mm)', -5],
                    ['bulge_max_x', 'Range maximum X (mm)', 5],
                    ['bulge_max_y', 'Range maximum Y (mm)', 5],
                    ['bulge_max_z', 'Range maximum Z (mm)', 5],
                    ['bulge_direction_x', 'Bulge direction X', 0],
                    ['bulge_direction_y', 'Bulge direction Y', 0],
                    ['bulge_direction_z', 'Bulge direction Z', 1],
                    ['bulge_amount_mm', 'Bulge amount (mm)', 5],
                  ] as const).map(([name, label, initial]) => (
                    <label className="field" key={name}><span>{label}</span>
                      <input name={name} type="number" step={name.includes('direction') ? 0.001 : 0.1}
                        min={name === 'bulge_amount_mm' ? 0.1 : name.includes('direction') ? -1 : -10000}
                        max={name === 'bulge_amount_mm' ? 100000 : name.includes('direction') ? 1 : 10000}
                        defaultValue={initial} required />
                    </label>
                  ))}
                  <button type="button"
                    disabled={!selectedFaceId || beginnerBulgeTargets.length >= 32 || coreBusy}
                    onClick={(event) => event.currentTarget.form
                      && addBeginnerBulgeTarget(event.currentTarget.form)}>
                    {text(APP_TEXT.addBulgeTargetForSelectedFace)}
                  </button>
                  <ul aria-label={text(APP_TEXT.text3dBulgeTargetList)}>
                    {beginnerBulgeTargets.map((target) => (
                      <li key={target.id}>
                        {formattedText(APP_TEXT.faceFaceAmountAmountMm, { face: target.face_ids[0], amount: target.amount_tenths_mm / 10 })}
                        <button type="button" onClick={() => setBeginnerBulgeTargets(
                          (targets) => targets.filter((item) => item.id !== target.id),
                        )}>{text(APP_TEXT.remove)}</button>
                      </li>
                    ))}
                  </ul>
                </fieldset>
                <p id="beginner-bulge-help" className="muted">
                  {text(APP_TEXT.storesOnlyTheBoundedRangeDirectionAndAmountBoundTo)}
                </p>
                <label className="field">
                  <span>{text(APP_TEXT.maximumSteps)}</span>
                  <input
                    name="maximum_steps"
                    type="number"
                    min={1}
                    max={500}
                    required
                    defaultValue={nativeSnapshot.beginner_design_profile.generation_constraints.maximum_steps}
                    disabled={coreBusy || recoveryBlocking}
                  />
                </label>
                <label className="field">
                  <span>{text(APP_TEXT.partDetail)}</span>
                  <select
                    name="detail_level"
                    defaultValue={nativeSnapshot.beginner_design_profile.generation_constraints.detail_level}
                    disabled={coreBusy || recoveryBlocking}
                  >
                    <option value="simple">{text(APP_TEXT.simple)}</option>
                    <option value="standard">{text(APP_TEXT.standard)}</option>
                    <option value="detailed">{text(APP_TEXT.detailed)}</option>
                  </select>
                </label>
                <label className="field">
                  <span>{text(APP_TEXT.allowedFoldTechniques)}</span>
                  <select
                    name="allowed_techniques"
                    multiple
                    size={8}
                    required
                    defaultValue={nativeSnapshot.beginner_design_profile.generation_constraints.allowed_techniques}
                    disabled={coreBusy || recoveryBlocking}
                    aria-describedby="beginner-technique-help"
                  >
                    <option value="valley_fold">{text(APP_TEXT.valleyFold)}</option>
                    <option value="mountain_fold">{text(APP_TEXT.mountainFold)}</option>
                    <option value="inside_reverse_fold">{text(APP_TEXT.insideReverseFold)}</option>
                    <option value="outside_reverse_fold">{text(APP_TEXT.outsideReverseFold)}</option>
                    <option value="squash_fold">{text(APP_TEXT.squashFold)}</option>
                    <option value="petal_fold">{text(APP_TEXT.petalFold)}</option>
                    <option value="sink_fold">{text(APP_TEXT.sinkFold)}</option>
                    <option value="crimp_fold">{text(APP_TEXT.crimpFold)}</option>
                  </select>
                </label>
                <p id="beginner-technique-help" className="muted">
                  {text(APP_TEXT.holdCtrlOrCommandToSelectMultipleTechniquesSelectAt)}
                </p>
                <p className="muted" data-testid="petal-fold-certification-scope">
                  {text(APP_TEXT.petalFoldIsADesignPreferenceOnlyItsPhysicalMotion)}
                </p>
                <button type="submit" disabled={coreBusy || recoveryBlocking}>
                  {text(APP_TEXT.saveDesignPriorities)}
                </button>
              </form>
            </section>
          )}
          {nativeSnapshot && !benchmarkRun && (
            <StackedFoldPanel
              locale={locale}
              snapshot={nativeSnapshot}
              appliedPose={appliedFoldPose}
              selectedLine={selectedLine ? {
                id: selectedLine.id,
                start: { x: selectedLine.x1, y: selectedLine.y1 },
                end: { x: selectedLine.x2, y: selectedLine.y2 },
              } : null}
              disabled={coreBusy || recoveryBlocking}
              namedBookFold={selectedNamedBookFold(
                foldTechniqueWorkspace?.document ?? null,
                foldTechniqueSelectedIndex,
                locale,
                selectedLine?.kind,
              )}
              namedTechniquePalette={namedBookFoldPalette(
                foldTechniqueWorkspace?.document ?? null,
                locale,
                selectedLine?.kind,
              )}
              onSelectNamedTechnique={(techniqueId) => {
                const techniques = foldTechniqueWorkspace?.document.techniques ?? []
                const index = techniques.findIndex((item) => item.id === techniqueId)
                if (index >= 0 && techniques[index]?.id === techniqueId) {
                  setFoldTechniqueSelectedIndex(index)
                }
              }}
              refreshSnapshot={requestProjectSnapshot}
              onApplied={(snapshot) => {
                applySnapshot(snapshot)
                setSelectedLineId(null)
                setSelectedVertexId(null)
                setSelectedFaceId(null)
                setCoreStatus(appMessage(APP_TEXT.theStackedFoldWasAppliedAtomicallyUndoRestoresTheWhole))
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
            <AnnotationPanel
              locale={locale}
              annotations={nativeSnapshot.annotations?.annotations ?? []}
              layers={nativeSnapshot.project_layers.layers}
              vertices={nativeVertices}
              disabled={coreBusy || recoveryBlocking}
              onAdd={(record) => void runNativeEdit(
                (projectId, revision, projectInstanceId) =>
                  addAnnotation(projectId, revision, projectInstanceId, record),
              )}
              onUpdate={(record) => void runNativeEdit(
                (projectId, revision, projectInstanceId) =>
                  updateAnnotation(projectId, revision, projectInstanceId, record),
              )}
              onRemove={(id) => void runNativeEdit(
                (projectId, revision, projectInstanceId) =>
                  removeAnnotation(projectId, revision, projectInstanceId, id),
              )}
            />
          )}
          {nativeSnapshot && !benchmarkRun && (
            <UnderlayPanel
              locale={locale}
              underlays={nativeSnapshot.underlays?.underlays ?? []}
              layers={nativeSnapshot.project_layers.layers}
              disabled={coreBusy || recoveryBlocking}
              onImport={(draft) => void runNativeEdit(
                (projectId, revision, projectInstanceId) =>
                  importUnderlayImage(projectId, revision, projectInstanceId, draft),
              )}
              onUpdate={(record) => void runNativeEdit(
                (projectId, revision, projectInstanceId) =>
                  updateUnderlay(projectId, revision, projectInstanceId, record),
              )}
              onRemove={(id) => void runNativeEdit(
                (projectId, revision, projectInstanceId) =>
                  removeUnderlay(projectId, revision, projectInstanceId, id),
              )}
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
              selectedVertexId={selectedVertexId}
              selectedVertexPosition={
                nativeVertices.find(({ id }) => id === selectedVertexId) ?? null
              }
              selectedEdgeGeometry={selectedLine}
              edges={nativeLines}
              vertices={nativeVertices}
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
              onPreviewSolve={previewConstraintSolve}
              onPreviewEdgeSolve={previewConstraintEdgeSolve}
              onPreviewExpressionSolve={previewConstraintExpressionSolve}
              onApplySolve={applyConstraintSolve}
            />
          )}
          {validation && (
            <section className={validation.is_valid ? 'validation-report valid' : 'validation-report invalid'}>
              <h2>{text(APP_TEXT.geometryValidation)}</h2>
              {validation.is_valid ? (
                <p>
                  {text(APP_TEXT.noIssuesWereFound)}
                </p>
              ) : (
                <>
                  <p>
                    {formattedText(APP_TEXT.countIssuesWereFound, { count: validation.issues.length })}
                  </p>
                  <BulkIntersectionRepairControl
                    count={unsplitIntersectionCount}
                    pending={bulkIntersectionRepairPending}
                    disabled={coreBusy || fileOperation !== null}
                    locale={locale}
                    onConfirm={() => void repairAllIntersections()}
                  />
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
                {text(APP_TEXT.localFlatFoldabilityConditions)}
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
                  {formattedText(APP_TEXT.coverageASingleInteriorVertexZeroThicknessModelFoldDegree, {
                    degree: localFlatFoldabilityPresentation.maxExactFoldDegree,
                  })}
                </p>
              )}
              {localFlatFoldabilityPresentation.kind === 'ready' && (
                <>
                  <ul
                    className="local-flat-foldability-counts"
                    aria-label={text(APP_TEXT.vertexCountsByLocalFlatFoldabilityResult)}
                  >
                    {([
                      [
                        'satisfied',
                        APP_TEXT.satisfied,
                        localFlatFoldabilityPresentation.counts.satisfied,
                      ],
                      [
                        'violated',
                        APP_TEXT.violated,
                        localFlatFoldabilityPresentation.counts.violated,
                      ],
                      [
                        'not-applicable',
                        APP_TEXT.notApplicable,
                        localFlatFoldabilityPresentation.counts.notApplicable,
                      ],
                      [
                        'indeterminate',
                        APP_TEXT.indeterminate2,
                        localFlatFoldabilityPresentation.counts.indeterminate,
                      ],
                    ] as const).map(([kind, label, count]) => (
                      <li key={kind} className={`is-${kind}`}>
                        <span>{text(label)}</span>
                        <strong>{count.toLocaleString(locale)}</strong>
                      </li>
                    ))}
                  </ul>
                  {(assignedLocalSummaryStatus === 'loading'
                    || assignedLocalSummaryStatus === 'retrying') && (
                    <p role="status">{text({
                      ja: assignedLocalSummaryStatus === 'retrying'
                        ? '旧解析の終了を待って局所十分性summaryを再試行しています…'
                        : '全頂点の指定M/V局所十分性を有界解析しています…',
                      en: assignedLocalSummaryStatus === 'retrying'
                        ? 'Waiting for the previous worker to exit, then retrying the summary…'
                        : 'Running the bounded assigned M/V local-sufficiency summary…',
                    })}</p>
                  )}
                  {assignedLocalSummaryStatus === 'failed' && (
                    <p role="alert">{text(APP_TEXT.theAllVertexLocalSufficiencySummaryIsUnavailable)}</p>
                  )}
                  {assignedLocalSummary && (
                    <section aria-label={text(APP_TEXT.allVertexLocalSufficiencySummary)}>
                      <p>{text(APP_TEXT.necessaryConditionFailureProvenSufficiencyAndIndeterminateAreSeparatePas)}</p>
                      <ul>
                        {assignedLocalSummary.vertices.map((item) => (
                          <li key={item.vertex}>
                            <button type="button" onClick={() => setSelectedVertexId(item.vertex)}>
                              {item.vertex.slice(0, 8)} · {item.status === 'necessary_failed'
                                ? text(APP_TEXT.necessaryFailed)
                                : item.status === 'sufficient_proven'
                                  ? text(APP_TEXT.sufficiencyProven)
                                  : text(APP_TEXT.indeterminate2)}
                            </button>
                          </li>
                        ))}
                      </ul>
                    </section>
                  )}
                  {selectedLocalFlatFoldability && (
                    <div className="selected-local-flat-foldability">
                      <h3>
                        {text(APP_TEXT.localConditionsForSelectedVertex)}
                      </h3>
                      <dl>
                        <div>
                          <dt>{text(APP_TEXT.overall)}</dt>
                          <dd>
                            {localizedLocalFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.verdict,
                              locale,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>
                            {text(APP_TEXT.kawasakiCondition)}
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
                            {text(APP_TEXT.maekawaCondition)}
                          </dt>
                          <dd>
                            {localizedLocalFlatFoldabilityConditionLabel(
                              selectedLocalFlatFoldability.maekawa,
                              locale,
                            )}
                          </dd>
                        </div>
                        <div>
                          <dt>{text(APP_TEXT.foldDegree)}</dt>
                          <dd>{selectedLocalFlatFoldability.foldDegree}</dd>
                        </div>
                        <div>
                          <dt>
                            {text(APP_TEXT.mountainValley)}
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
                      {assignedLocalSufficiency && (
                        <p
                          className="local-flat-foldability-sufficiency"
                          aria-live="polite"
                        >
                          {assignedLocalSufficiency.result.status === 'proven'
                            ? text({
                              ja: `指定M/Vの局所十分性をBLB縮約 ${assignedLocalSufficiency.result.reduction_steps} 段で証明しました。`,
                              en: `Assigned M/V local sufficiency is proven by ${assignedLocalSufficiency.result.reduction_steps} BLB reduction step(s).`,
                            })
                            : text({
                              ja: assignedLocalSufficiency.result.reason === 'resource_limit'
                                ? '局所十分性は資源上限のため判定不能です。'
                                : assignedLocalSufficiency.result.reason === 'necessary_conditions_not_satisfied'
                                  ? '局所必要条件が成立しないため十分性を証明できません。'
                                  : '適用できる一意なstrict BLB縮約がないため局所十分性は判定不能です。',
                              en: assignedLocalSufficiency.result.reason === 'resource_limit'
                                ? 'Local sufficiency is indeterminate because the resource limit was reached.'
                                : assignedLocalSufficiency.result.reason === 'necessary_conditions_not_satisfied'
                                  ? 'Local sufficiency cannot be proven because the necessary conditions fail.'
                                  : 'Local sufficiency is indeterminate because no unique strict BLB reduction applies.',
                            })}
                        </p>
                      )}
                    </div>
                  )}
                  {localFlatFoldabilityPresentation.visibleItems.length > 0 && (
                    <>
                      <h3>
                        {text(APP_TEXT.verticesRequiringReview)}
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
                                aria-label={formattedText(APP_TEXT.vertexOrdinalLocalNecessaryConditionVerdictKawasakiConditionKawasakiMaek, {
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
                                  {formattedText(APP_TEXT.vertexOrdinal, { ordinal: item.ordinal })}
                                </span>
                                <span className="local-flat-foldability-item-detail">
                                  {reasonLabel || (
                                    formattedText(APP_TEXT.kawasakiKawasakiMaekawaMaekawa, {
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
                          {formattedText(APP_TEXT.countMoreVerticesSelectAVertexToReviewItsResult, {
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
                {text(APP_TEXT.satisfiedMeansOnlyThatTheLocalNecessaryConditionsWereVerified)}
              </p>
            </section>
          )}
          <GlobalFlatFoldabilityPanel
            job={globalFlatFoldabilityJob}
            localSummary={assignedLocalSummary}
            selectedVertexId={selectedVertexId}
            onSelectVertex={setSelectedVertexId}
            timeLimitSeconds={globalFlatFoldabilityTimeLimit}
            authority={nativeSnapshot ? {
              projectInstanceId: nativeSnapshot.project_instance_id,
              projectId: nativeSnapshot.project_id,
              revision: nativeSnapshot.revision,
            } : undefined}
            selectedFaceId={selectedFaceId}
            onSelectFace={setSelectedFaceId}
            onHoverFace={setHoveredLayerFaceId}
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
            <h2>{text(APP_TEXT.paper)}</h2>
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
                  {text(APP_TEXT.thickness2)}
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
                  <span>{text(APP_TEXT.frontColor)}</span>
                  <input
                    name="front_color"
                    type="color"
                    defaultValue={rgbaToHex(nativeSnapshot?.paper.front.color, '#ffffff')}
                    disabled={coreBusy || !nativeSnapshot}
                  />
                </label>
                <label className="paper-color-field">
                  <span>{text(APP_TEXT.backColor)}</span>
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
                  <span>{text(APP_TEXT.frontPattern)}</span>
                  <select
                    name="front_pattern"
                    defaultValue={builtinPaperPatternFromAsset(
                      nativeSnapshot?.paper.front.texture_asset,
                    ) ?? (nativeSnapshot?.paper.front.texture_asset ? 'custom' : 'none')}
                    disabled={coreBusy || !nativeSnapshot}
                  >
                    <option value="none">{text(APP_TEXT.noneSolid)}</option>
                    <option value="dots">{text(APP_TEXT.dots)}</option>
                    <option value="grid">{text(APP_TEXT.grid2)}</option>
                    <option value="stripes">{text(APP_TEXT.stripes)}</option>
                    {nativeSnapshot?.paper.front.texture_asset
                      && !builtinPaperPatternFromAsset(nativeSnapshot.paper.front.texture_asset)
                      ? <option value="custom">{text(APP_TEXT.importedImage)}</option>
                      : null}
                  </select>
                  <button
                    type="button"
                    disabled={coreBusy || !nativeSnapshot}
                    onClick={chooseFrontPaperTexture}
                  >
                    {text(APP_TEXT.importImage)}
                  </button>
                </label>
                <label className="paper-color-field">
                  <span>{text(APP_TEXT.backPattern)}</span>
                  <select
                    name="back_pattern"
                    defaultValue={builtinPaperPatternFromAsset(
                      nativeSnapshot?.paper.back.texture_asset,
                    ) ?? (nativeSnapshot?.paper.back.texture_asset ? 'custom' : 'none')}
                    disabled={coreBusy || !nativeSnapshot}
                  >
                    <option value="none">{text(APP_TEXT.noneSolid)}</option>
                    <option value="dots">{text(APP_TEXT.dots)}</option>
                    <option value="grid">{text(APP_TEXT.grid2)}</option>
                    <option value="stripes">{text(APP_TEXT.stripes)}</option>
                    {nativeSnapshot?.paper.back.texture_asset
                      && !builtinPaperPatternFromAsset(nativeSnapshot.paper.back.texture_asset)
                      ? <option value="custom">{text(APP_TEXT.importedImage)}</option>
                      : null}
                  </select>
                  <button
                    type="button"
                    disabled={coreBusy || !nativeSnapshot}
                    onClick={chooseBackPaperTexture}
                  >
                    {text(APP_TEXT.importImage)}
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
                {text(APP_TEXT.allowCutting)}
              </label>
              <div className="property-actions">
                <button type="submit" disabled={coreBusy || !nativeSnapshot}>
                  {text(APP_TEXT.updatePaperSettings)}
                </button>
              </div>
            </form>
            <div className="paper-size-editor">
              <h3>{text(APP_TEXT.paperSize)}</h3>
              <form
                key={paperResizeFormKey}
                className="paper-size-form"
                onSubmit={submitPaperResize}
                noValidate
              >
                <div className="paper-size-fields">
                  <label className="field">
                    <span>{text(APP_TEXT.width)}</span>
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
                      aria-label={formattedText(APP_TEXT.paperWidthUnit, { unit: lengthDisplayUnitLabelText })}
                    />
                    <span>{lengthDisplayUnitLabelText}</span>
                  </label>
                  <label className="field">
                    <span>{text(APP_TEXT.height)}</span>
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
                      aria-label={formattedText(APP_TEXT.paperHeightUnit, { unit: lengthDisplayUnitLabelText })}
                    />
                    <span>{lengthDisplayUnitLabelText}</span>
                  </label>
                </div>
                {!rectangularPaperSize && (
                  <p className="paper-size-note">
                    {text(APP_TEXT.paperThatIsNotRecognizedAsAnAxisAlignedRectangle)}
                  </p>
                )}
                <p className="paper-size-note">
                  {text(APP_TEXT.resizingProportionallyTransformsEveryVertexIncludingFoldLinesFromThe)}
                </p>
                <CreationDimensionExpressionSummary
                  key={nativeSnapshot?.project_id ?? 'no-project'}
                  binding={creationDimensionExpression}
                />
                {rectangularRatioReferenceAxis && (
                  <p className="paper-size-note">
                    {formattedText(APP_TEXT.forAPaperEdgeRatioAxisRemainsReadOnlyAt, {
                      axis: rectangularRatioReferenceAxis === 'width'
                        ? text(APP_TEXT.width2)
                        : text(APP_TEXT.height2),
                    })}
                  </p>
                )}
                <div className="property-actions">
                  <button
                    type="submit"
                    disabled={coreBusy || !nativeSnapshot || !rectangularPaperSize}
                  >
                    {text(APP_TEXT.resizePaper)}
                  </button>
                </div>
              </form>
            </div>
          </section>
          <section>
            <h2>{text(APP_TEXT.editHistory)}</h2>
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
                  {text(APP_TEXT.theUndoRedoHistoryLimitCouldNotBeChecked)}
                </p>
                <button
                  type="button"
                  disabled={coreBusy || recoveryBlocking}
                  onClick={() => setHistoryLimitRetrySequence(
                    (sequence) => sequence + 1,
                  )}
                >
                  {text(APP_TEXT.retry)}
                </button>
              </div>
            ) : historyLimitLoadState.kind === 'unavailable' ? (
              <p className="muted">
                {text(APP_TEXT.historyLimitSettingsAreAvailableInTheDesktopApp)}
              </p>
            ) : (
              <p className="muted" role="status" aria-live="polite">
                {text(APP_TEXT.checkingHistoryLimit)}
              </p>
            )}
          </section>
          <section className="fold-technique-workspace">
            <h2>
              {text(APP_TEXT.namedFoldTechniques)}
            </h2>
            <p className="muted">
              {text(APP_TEXT.createAndShareMultipleInstructionStepsAsDeclarativeDataThis)}
            </p>
            {foldTechniqueWorkspace && (
              <>
                <dl>
                  <div>
                    <dt>{text(APP_TEXT.packageID)}</dt>
                    <dd>{foldTechniqueWorkspace.document.package_id}</dd>
                  </div>
                  <div>
                    <dt>{text(APP_TEXT.techniques)}</dt>
                    <dd>
                      {foldTechniqueWorkspace.document.techniques.length
                        .toLocaleString(locale)}
                    </dd>
                  </div>
                  <div>
                    <dt>{text(APP_TEXT.shareState)}</dt>
                    <dd>
                      {foldTechniqueWorkspace.dirty
                        ? text(APP_TEXT.changedSaveAsRequired)
                        : text(APP_TEXT.saved)}
                    </dd>
                  </div>
                </dl>
                <label className="dialog-field">
                  <span>
                    {text(APP_TEXT.techniqueToAddToTimeline)}
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
                {text(APP_TEXT.create)}
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
                {text(APP_TEXT.importFile)}
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
                {text(APP_TEXT.edit)}
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
                {text(APP_TEXT.saveAs)}
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
                {text(APP_TEXT.buildTimelineProposal)}
              </button>
            </div>
            {foldTechniqueBusy && (
              <p role="status" aria-live="polite">
                {text(APP_TEXT.processingTheFoldTechniqueFile)}
              </p>
            )}
            {!isNativeFoldTechniqueFileAvailable() && (
              <p className="muted">
                {text(APP_TEXT.safeFileSelectionAndAtomicSavingAreAvailableInThe)}
              </p>
            )}
          </section>
          <section>
            <h2>{text(APP_TEXT.snap)}</h2>
            <div
              className="chip-row"
              aria-label={text(APP_TEXT.snapSettings)}
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
            <label className="angle-snap-field">
              <span>{text(APP_TEXT.dividePaperIntoN)}</span>
              <input
                type="number"
                min="2"
                max="63"
                step="1"
                value={gridDivisionsInput}
                placeholder={text(APP_TEXT.auto)}
                aria-invalid={!gridDivisionsValid}
                disabled={coreBusy}
                onChange={(event) => {
                  const next = updateGridPreferenceInput(
                    event.target.value,
                    gridDiagonals,
                  )
                  if (!next) return
                  setGridDivisionsInput(next.input)
                  setGridDiagonals(next.diagonals)
                }}
              />
              <small>{text(APP_TEXT.leaveBlankForAutomaticSpacingUse3ForThirdsOr)}</small>
            </label>
            <button
              type="button"
              className={`chip${gridDiagonals ? ' active' : ''}`}
              aria-pressed={gridDiagonals}
              disabled={coreBusy || !gridDivisionsValid || gridDivisions === null}
              onClick={() => setGridDiagonals((current) => !current)}
            >
              {text(APP_TEXT.paperDiagonals)}
            </button>
            <div className="angle-snap-settings">
              <h3>{text(APP_TEXT.angleSnap)}</h3>
              <label className="angle-snap-field">
                <span>{text(APP_TEXT.preset)}</span>
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
                    {text(APP_TEXT.customAngle)}
                  </option>
                </select>
              </label>
              <label className="angle-snap-field">
                <span>{text(APP_TEXT.angle)}</span>
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
                  {text(APP_TEXT.enterAnAngleGreaterThan0AndNoMoreThan)}
                </p>
              )}
              <div className="angle-reference-setting">
                <span>{text(APP_TEXT.reference)}</span>
                <div
                  className="chip-row"
                  role="group"
                  aria-label={text(APP_TEXT.angleSnapReference)}
                >
                  <button
                    type="button"
                    className={`chip${angleReferenceKind === 'global-horizontal' ? ' active' : ''}`}
                    aria-pressed={angleReferenceKind === 'global-horizontal'}
                    disabled={coreBusy}
                    onClick={() => setAngleReferenceKind('global-horizontal')}
                  >
                    {text(APP_TEXT.horizontal)}
                  </button>
                  <button
                    type="button"
                    className={`chip${angleReferenceKind === 'edge' ? ' active' : ''}`}
                    aria-pressed={angleReferenceKind === 'edge'}
                    disabled={coreBusy}
                    onClick={() => setAngleReferenceKind('edge')}
                  >
                    {text(APP_TEXT.directionReferenceEdge)}
                  </button>
                </div>
              </div>
              <p className="muted">
                {formattedText(APP_TEXT.currentAngleReference, {
                  angle: formatAngleDegrees(angleDegrees),
                  reference: angleReferenceKind === 'global-horizontal'
                    ? text(APP_TEXT.horizontalReference)
                    : text(APP_TEXT.directionEdgeReference),
                })}
              </p>
              {snapSettings.angle && angleReferenceKind === 'edge' && !parallelReferenceLine && (
                <p className="field-error" role="status">
                  {text(APP_TEXT.selectALineAndSetItAsTheDirectionReference)}
                </p>
              )}
            </div>
            {parallelReferenceLine ? (
              <div className="property-actions">
                <span className="muted" title={parallelReferenceLine.id}>
                  {formattedText(APP_TEXT.directionReferenceParallelAndAngleKind, {
                    kind: lineKindLabel(parallelReferenceLine.kind, locale),
                  })}
                </span>
                <button
                  type="button"
                  disabled={coreBusy}
                  onClick={() => setParallelReferenceEdgeId(null)}
                >
                  {text(APP_TEXT.clearReference)}
                </button>
              </div>
            ) : (
              <p className="muted">
                {text(APP_TEXT.selectALineAndChooseSetAsDirectionReferenceTo)}
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
        currentCamera={foldPreviewCamera?.poseModelKey === foldPreviewPoseModelKey
          ? foldPreviewCamera.camera
          : null}
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
        onOnionSkinChange={setInstructionOnionSkin}
        onionSkinStatus={instructionOnionSkinStatus}
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
                  {text(APP_TEXT.startFromOneSheet)}
                </span>
                <h2 id="new-project-title">
                  {text(APP_TEXT.newProject)}
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
                aria-label={text(APP_TEXT.close)}
              >
                ×
              </button>
            </header>
            <form onSubmit={submitNewProject} noValidate>
              <label className="dialog-field dialog-field-wide">
                <span>{text(APP_TEXT.projectName)}</span>
                <input
                  name="name"
                  defaultValue={text(APP_TEXT.untitledWork)}
                  maxLength={120}
                  required
                  autoFocus
                  disabled={coreBusy}
                />
              </label>

              <fieldset>
                <legend>{text(APP_TEXT.paperSize)}</legend>
                <div className="dialog-grid two-columns">
                  <label className="dialog-field">
                    <span>{text(APP_TEXT.width)}</span>
                    <NumericExpressionInput
                      id="new-project-width-expression"
                      name="width_expression"
                      defaultSource="400"
                      disabled={coreBusy}
                      ariaLabel={text(APP_TEXT.paperWidthExpressionMm)}
                    />
                  </label>
                  <label className="dialog-field">
                    <span>{text(APP_TEXT.height)}</span>
                    <NumericExpressionInput
                      id="new-project-height-expression"
                      name="height_expression"
                      defaultSource="400"
                      disabled={coreBusy}
                      ariaLabel={text(APP_TEXT.paperHeightExpressionMm)}
                    />
                  </label>
                </div>
              </fieldset>

              <fieldset>
                <legend>
                  {text(APP_TEXT.materialSettings)}
                </legend>
                <div className="dialog-grid three-columns">
                  <div className="dialog-field">
                    <label htmlFor="new-project-paper-thickness-mm">
                      {text(APP_TEXT.paperThickness)}
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
                    <span>{text(APP_TEXT.frontColor)}</span>
                    <input
                      name="front_color"
                      type="color"
                      defaultValue="#ffffff"
                      disabled={coreBusy}
                    />
                  </label>
                  <label className="dialog-field color-field">
                    <span>{text(APP_TEXT.backColor)}</span>
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
                  {text(APP_TEXT.allowCutLinesInThisProject)}
                </label>
              </fieldset>

              <p className="dialog-note">
                {text(APP_TEXT.createsRectangularPaperWithItsTopLeftAt00)}
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
                  {text(APP_TEXT.cancel2)}
                </button>
                <button type="submit" className="primary" disabled={coreBusy}>
                  {coreBusy
                    ? text(APP_TEXT.creating)
                    : text(APP_TEXT.create2)}
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
          onInvalidateValidation={invalidateSvgImportValidation}
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
        proofScopeDiagnosticsJson={createProofScopePresentation(
          globalFlatFoldabilityJob,
          assignedLocalSummary,
        ).diagnosticsJson}
      />

      <footer className="statusbar" inert={modalOpen}>
        <span>
          {formattedText(APP_TEXT.toolTool, {
            tool: benchmarkRun
              ? text(APP_TEXT.benchmarkSelection)
              : toolLabel(activeTool, locale),
          })}
        </span>
        <span>{coreStatus}</span>
        <span>
          {formattedText(APP_TEXT.snapStatus, { status: snapStatusLabel })}
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
            {text(APP_TEXT.diagnostics)}
          </button>
        )}
        <button
          type="button"
          className="benchmark-button"
          disabled={coreBusy || benchmarkLoading}
          onClick={() => void toggleBenchmark()}
        >
          {benchmarkLoading
            ? text(APP_TEXT.loading)
            : benchmarkRun
              ? text(APP_TEXT.returnToNormalPattern)
              : text(APP_TEXT.text10000EdgeTest)}
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
    duplicate_vertex_id: APP_TEXT.aVertexIDIsDuplicated,
    duplicate_edge_id: APP_TEXT.aLineIDIsDuplicated,
    invalid_paper: APP_TEXT.thePaperBoundaryOrPropertiesAreInvalid,
    invalid_crease_pattern: APP_TEXT.theCreasePatternGeometryIsInvalid,
    unsupported_active_edge: APP_TEXT.aLineKindCannotBeConvertedTo3D,
    too_many_active_fold_edges: APP_TEXT.theActiveFoldCountExceedsTheSupportedLimit,
    active_edge_outside_paper: APP_TEXT.aFoldLineLiesOutsideThePaper,
    disconnected_fold_graph: APP_TEXT.theFoldGraphIsDisconnected,
    non_separating_fold: APP_TEXT.aFoldLineDoesNotSeparateTwoFaces,
    unsupported_fold_graph: APP_TEXT.theFoldGraphIsUnsupportedByTheCurrent3DModel,
    invalid_edge_incidence: APP_TEXT.aLineHasInvalidFaceIncidence,
    fold_endpoint_not_on_boundary: APP_TEXT.aFoldEndpointIsNotOnThePaperBoundary,
    unsupported_adjacent_boundary_fold: APP_TEXT.aBoundaryAdjacentFoldIsUnsupported,
    unsupported_non_convex_fold_sheet: APP_TEXT.thisFoldOnANonConvexSheetIsUnsupported,
    degenerate_fold_face: APP_TEXT.aFoldLineProducesADegenerateFace,
    unrepresentable_face_area: APP_TEXT.aFaceAreaCannotBeRepresentedSafely,
    internal_boundary_resolution: APP_TEXT.facesCouldNotBeResolvedFromThePaperBoundary,
  }
  return selectLocalizedText(locale, labels[issue.kind])
}

function reportValidationUnexpected() {
  reportUnexpected('app.validation')
}

function editExpressionErrorMessage(error: unknown) {
  const category = numericExpressionNativeErrorCategory(error)
  if (category === 'native_unavailable') {
    return appMessage(APP_TEXT.expressionInputIsAvailableInTheDesktopApp)
  }
  if (category === 'resource_limit') {
    return appMessage(APP_TEXT.evaluationStoppedBecauseTheExpressionIsTooComplex)
  }
  return appMessage(APP_TEXT.enterAFiniteExpressionUsingDecimalsFractionsSquareRootsPi)
}

export default App
