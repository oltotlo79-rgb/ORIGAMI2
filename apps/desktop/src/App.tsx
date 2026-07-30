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
import { DiagnosticsDialog } from './components/DiagnosticsDialog'
import { FoldImportDialog } from './components/FoldImportDialog'
import { Fold3dFramesLauncher } from './components/Fold3dFramesLauncher'
import { FoldPreview } from './components/FoldPreview'
import { EffectiveCutDiagnosticPanel } from './components/EffectiveCutDiagnosticPanel'
import { FoldTechniqueEditorDialog } from './components/FoldTechniqueEditorDialog'
import { FoldTechniqueTimelinePreviewDialog } from './components/FoldTechniqueTimelinePreviewDialog'
import { GeometricConstraintPanel } from './components/GeometricConstraintPanel'
import { GlobalFlatFoldabilityPanel } from './components/GlobalFlatFoldabilityPanel'
import { InstructionExportDialog } from './components/InstructionExportDialog'
import { InstructionTimelinePanel } from './components/InstructionTimelinePanel'
import { KeyboardShortcutControl } from './components/KeyboardShortcutControl'
import { LanguageControl } from './components/LanguageControl'
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
  resolveCreaseAuthoringLayerId,
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
import { PaperThicknessInput } from './components/PaperThicknessInput'
import {
  formatLength,
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
  type AngleSnapConfig,
  type AngleSnapReferenceKind,
  type SnapSettings,
} from './lib/snap'
import {
  classifyVertexPlacementAuthorityV1,
  isSupportedIntersectionPlacement,
  type VertexPlacement,
} from './lib/vertexPlacement'
import {
  moveConstructedVertexV1,
  placeConstructedVertexV1,
} from './lib/constructedVertexClient.ts'
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
import { PaperInspectorSection } from './components/PaperInspectorSection'
import { HistoryLimitInspectorSection } from './components/HistoryLimitInspectorSection'
import { FoldTechniqueInspectorSection } from './components/FoldTechniqueInspectorSection'
import { SnapInspectorSection } from './components/SnapInspectorSection'
import { SNAP_INSPECTOR_OPTIONS } from './lib/snapInspectorOptions'
import { MirrorSelectionPanel } from './components/MirrorSelectionPanel'
import { ElementMetadataForm } from './components/ElementMetadataForm'
import { SelectedLineInspector } from './components/SelectedLineInspector'
import { SelectedFaceInspector } from './components/SelectedFaceInspector'
import {
  BenchmarkVertexInspector,
  DirectVertexInspector,
  SelectedVertexInspector,
} from './components/SelectedVertexInspector'
import { BeginnerDesignEditorSection } from './components/BeginnerDesignEditorSection'
import { ValidationInspectorSections } from './components/ValidationInspectorSections'
import {
  ProjectMemoAndCandidateSection,
} from './components/ProjectMemoAndCandidateSection'
import {
  formatBytes,
  localFlatFoldabilityCoreStatus,
  normalizeFoldAngle,
  toolLabel,
} from './lib/appPresentation'
import {
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
    centerVertexId: string
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
  const beginnerEditorState = useBeginnerEditorState({
    snapshot: nativeSnapshot,
    getCurrentSnapshot: () => latestSnapshotRef.current,
    getSelectedFaceId: () => selectedFaceId,
  })
  const {
    beginnerDesignFormRef,
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
    beginnerBodyOutlineMode,
    setBeginnerBodyOutlineMode,
    beginnerProtrusionKinds,
    setBeginnerProtrusionKinds,
    beginnerBulgeTargets,
  } = beginnerEditorState
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
  const beginnerReferenceWorkflow = useBeginnerReferenceWorkflow({
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
      setBeginnerProtrusionKinds,
      setBeginnerSkeletonSegments,
      setBeginnerComponentBridgeOverride,
    },
  })
  const {
    beginnerCandidateBusy,
    cancelBeginnerCandidates,
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
      setBeginnerProtrusionKinds,
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
    beginnerSilhouetteThresholds,
    beginnerSilhouetteCropRoi,
    beginnerSilhouetteOrientation,
    beginnerSilhouetteMirror,
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
    else {
      setCompassCircles((current) => current.flatMap((circle) => {
        const center = admittedSnapshot.crease_pattern.vertices.find(
          ({ id }) => id === circle.centerVertexId,
        )
        return center
          ? [{
            ...circle,
            centerX: center.position.x,
            centerY: center.position.y,
          }]
          : []
      }))
    }
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
  const creaseAuthoringLayerId = useMemo(
    () => resolveCreaseAuthoringLayerId(nativeSnapshot?.project_layers),
    [nativeSnapshot?.project_layers],
  )
  const nativeLines = nativeLayerView.lines
  const nativeVertices = nativeLayerView.vertices
  const vertexToolAvailable = !nativeLayerView.defaultLayerLocked
    || nativeLines.some((line) => !line.locked)
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
  const radialArrayCenterVertexIds = selectedLine && nativeSnapshot
    ? [selectedLine.startVertexId, selectedLine.endVertexId].filter(
        (vertexId) => !nativeSnapshot.paper.boundary_vertices.includes(vertexId),
      )
    : []
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
    setPendingEdgeStart(null)
  }, [creaseAuthoringLayerId])
  useEffect(() => {
    const lineToolActive = activeTool === 'mountain'
      || activeTool === 'valley'
      || activeTool === 'auxiliary'
      || activeTool === 'cut'
    if (
      (!lineToolActive || creaseAuthoringLayerId !== null)
      && (activeTool !== 'vertex' || vertexToolAvailable)
    ) return
    setActiveTool('select')
    setPendingEdgeStart(null)
    setCancelInteractionToken((token) => token + 1)
  }, [activeTool, creaseAuthoringLayerId, vertexToolAvailable])
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
  const snapStatusLabel = SNAP_INSPECTOR_OPTIONS
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

  const runCreaseAuthoringEdit = useCallback((
    action: (
      projectId: string,
      revision: number,
      projectInstanceId: string,
      targetLayer: string,
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
    ) return Promise.reject(new Error())
    const targetLayer = resolveCreaseAuthoringLayerId(
      baseSnapshot.project_layers,
    )
    if (!targetLayer) {
      return Promise.reject(new Error())
    }
    return action(
      projectId,
      revision,
      projectInstanceId,
      targetLayer,
      baseSnapshot,
    )
  }), [runNativeEdit])

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
    const center = String(form.get('radial_array_center') ?? '')
    if (!Number.isInteger(copies) || copies < 1 || copies > 3
      || ![90, 180, 270].includes(angle) || (angle === 180 && copies !== 1)
      || ![selectedLine.startVertexId, selectedLine.endVertexId].includes(center)
      || current.paper.boundary_vertices.includes(center)) {
      setRadialArrayPreview(null)
      return
    }
    const request: RadialArrayRequest = {
      center,
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
      const authorityRoute = classifyVertexPlacementAuthorityV1(placement)
      if (authorityRoute.kind === 'invalid-native') {
        throw new Error('invalid_constructed_vertex_authority')
      }
      if (authorityRoute.kind === 'native') {
        snapshot = await placeConstructedVertexV1(
          projectInstanceId,
          projectId,
          revision,
          authorityRoute.placement,
        )
      } else if (placement.operation === 'add') {
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
    if (!creaseAuthoringLayerId) {
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
    void runCreaseAuthoringEdit((
      projectId,
      revision,
      projectInstanceId,
      targetLayer,
    ) => addEdge(
      projectId,
      revision,
      projectInstanceId,
      start,
      vertexId,
      activeTool,
      targetLayer,
    ))
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
    if (!current || !selectedVertex) return
    const currentVertices = current.crease_pattern.vertices.filter(
      (vertex) => vertex.id === selectedVertex.id,
    )
    if (currentVertices.length !== 1) return
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
      const succeeded = await runCreaseAuthoringEdit((
        projectId,
        revision,
        projectInstanceId,
        targetLayer,
      ) => addRayToFirstTarget(
        projectId,
        revision,
        projectInstanceId,
        selectedVertex.id,
        angleMicrodegrees,
        edgeKind,
        targetLayer,
      ))
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
      const previousVertexIds = new Set(
        current.crease_pattern.vertices.map(({ id }) => id),
      )
      const result: { snapshot: ProjectSnapshot | null } = { snapshot: null }
      const succeeded = await runCreaseAuthoringEdit(async (
        projectId,
        revision,
        projectInstanceId,
        targetLayer,
      ) => {
        const snapshot = await addConnectedVertex(
          projectId,
          revision,
          projectInstanceId,
          selectedVertex.id,
          millimetreExpressionSource(
            lengthDisplayExpression,
            currentUnit.millimetresPerUnit,
          ),
          angleDegreesExpression,
          edgeKind,
          targetLayer,
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
    if (selectedVertexLocked) return
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
    if (!current || !selectedFace || !creaseAuthoringLayerId) return
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
    await runCreaseAuthoringEdit((
      projectId,
      revision,
      projectInstanceId,
      targetLayer,
    ) => addEdge(
      projectId,
      revision,
      projectInstanceId,
      start,
      end,
      kind,
      targetLayer,
    ))
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
            runNativeEdit={runNativeEdit}
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
                || (id === 'vertex' && !vertexToolAvailable)
                || (
                  id !== 'select'
                  && id !== 'measure'
                  && id !== 'vertex'
                  && creaseAuthoringLayerId === null
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
                : (vertexId, x, y, nativeConstruction) => {
                    if (nativeLayerView.lockedVertexIds.has(vertexId)) return
                    void runNativeEdit((projectId, revision, projectInstanceId) =>
                      nativeConstruction
                        ? moveConstructedVertexV1(
                            projectInstanceId,
                            projectId,
                            revision,
                            vertexId,
                            nativeConstruction,
                          )
                        : moveVertex(
                            projectId,
                            revision,
                            projectInstanceId,
                            vertexId,
                            x,
                            y,
                          ))
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
            <MirrorSelectionPanel
              locale={locale}
              coreBusy={coreBusy}
              mirrorBusy={mirrorBusy}
              candidateBusy={beginnerCandidateBusy}
              currentSelectionAvailable={Boolean(selectedVertex || selectedLine)}
              selectedVertexCount={mirrorVertexIds.length}
              selectedEdgeCount={mirrorEdgeIds.length}
              mode={mirrorMode}
              axis={mirrorAxis}
              preview={mirrorPreview}
              onAddCurrentSelection={addCurrentToMirrorSelection}
              onCancelCandidateGeneration={cancelBeginnerCandidates}
              onCancelMirrorSelection={cancelMirrorSelection}
              onModeChange={(mode) => {
                setMirrorMode(mode)
                setMirrorPreview(null)
              }}
              onAxisChange={(key, value) => {
                setMirrorAxis((current) => ({ ...current, [key]: value }))
                setMirrorPreview(null)
              }}
              onPreview={previewCurrentMirrorSelection}
              onApply={applyCurrentMirrorSelection}
            />
            {selectedElementTarget && (
              <ElementMetadataForm
                locale={locale}
                target={selectedElementTarget}
                metadata={selectedElementMetadata}
                revision={nativeSnapshot?.revision ?? 0}
                disabled={coreBusy}
                onSubmit={submitElementMetadata}
              />
            )}
            {selectedLine ? (
              <SelectedLineInspector
                locale={locale}
                line={selectedLine}
                displayUnit={displayedLengthUnit}
                displayUnitLabel={lengthDisplayUnitLabelText}
                coreBusy={coreBusy}
                benchmarkActive={Boolean(benchmarkRun)}
                parallelReferenceEdgeId={parallelReferenceEdgeId}
                linearArrayPreview={linearArrayPreview}
                radialArrayPreview={radialArrayPreview}
                radialArrayCenterVertexIds={radialArrayCenterVertexIds}
                onDeleteBenchmarkLine={deleteBenchmarkLine}
                onSubmitMove={(event) => void submitMoveSelectedEdge(event)}
                onSubmitMirror={(event) => void submitMirrorSelectedEdge(event)}
                onSubmitRotate={(event) => void submitRotateSelectedEdge(event)}
                onSubmitLinearArray={(event) => void submitLinearArrayPreview(event)}
                onInvalidateLinearArray={() => {
                  linearArrayRequestSequenceRef.current += 1
                  setLinearArrayPreview(null)
                }}
                onConfirmLinearArray={confirmCurrentLinearArray}
                onSubmitRadialArray={(event) => void submitRadialArrayPreview(event)}
                onInvalidateRadialArray={() => {
                  radialArrayRequestSequenceRef.current += 1
                  setRadialArrayPreview(null)
                }}
                onConfirmRadialArray={confirmCurrentRadialArray}
                onToggleParallelReference={(lineId) =>
                  setParallelReferenceEdgeId((current) => (
                    current === lineId ? null : lineId
                  ))}
                onSplitBoundaryEdge={splitSelectedBoundaryEdge}
                onDeleteSelection={deleteSelection}
              />
            ) : selectedFace ? (
              <SelectedFaceInspector
                locale={locale}
                face={selectedFace}
                removableEdges={selectedFaceRemovableEdges}
                locked={selectedFaceLocked}
                creaseAuthoringAvailable={creaseAuthoringLayerId !== null}
                coreBusy={coreBusy}
                cuttingAllowed={nativeSnapshot?.cutting_allowed ?? false}
                displayUnitLabel={lengthDisplayUnitLabelText}
                onSubmitMove={(event) => void submitMoveSelectedFace(event)}
                onSubmitSplit={(event) => void submitSplitSelectedFace(event)}
                onSubmitMerge={(event) => void submitMergeSelectedFace(event)}
              />
            ) : selectedBenchmarkVertex ? (
              <BenchmarkVertexInspector
                locale={locale}
                vertex={selectedBenchmarkVertex}
              />
            ) : selectedVertex ? (
              <SelectedVertexInspector
                locale={locale}
                vertex={selectedVertex}
                expression={selectedVertexExpression}
                displayUnit={lengthDisplayUnit}
                displayUnitLabel={lengthDisplayUnitLabelText}
                coreBusy={coreBusy}
                locked={selectedVertexLocked}
                creaseAuthoringAvailable={creaseAuthoringLayerId !== null}
                boundary={selectedVertexIsBoundary}
                boundaryVertexCount={paperBoundaryVertexCount}
                cuttingAllowed={nativeSnapshot?.cutting_allowed ?? false}
                compassCircleCount={compassCircles.length}
                onSubmit={submitVertexPosition}
                onDeleteSelection={deleteSelection}
                onAddCompassCircle={(circle) => setCompassCircles(
                  (current) => [...current, circle].slice(-64),
                )}
                onClearCompassCircles={() => setCompassCircles([])}
              />
            ) : nativeSnapshot && !benchmarkRun ? (
              <DirectVertexInspector
                locale={locale}
                projectInstanceId={nativeSnapshot.project_instance_id}
                displayUnit={lengthDisplayUnit}
                displayUnitLabel={lengthDisplayUnitLabelText}
                coreBusy={coreBusy}
                defaultLayerLocked={nativeLayerView.defaultLayerLocked}
                onSubmit={(event) => void submitDirectVertex(event)}
              />
            ) : (
              <p className="muted">{text(APP_TEXT.selectALineOrVertex)}</p>
            )}
          </section>
          {nativeSnapshot && !benchmarkRun && (
            <ProjectMemoAndCandidateSection
              locale={locale}
              snapshot={nativeSnapshot}
              coreBusy={coreBusy}
              recoveryBlocking={recoveryBlocking}
              skeletonTreeStatus={beginnerSkeletonTree.status}
              candidateWorkflow={beginnerCandidateWorkflow}
              gridWorkflow={beginnerGridWorkflow}
              onSubmitMemo={(event) => void submitProjectMemo(event)}
            />
          )}
          {nativeSnapshot && !benchmarkRun && (
            <BeginnerDesignEditorSection
              locale={locale}
              snapshot={nativeSnapshot}
              coreBusy={coreBusy}
              recoveryBlocking={recoveryBlocking}
              selectedFaceId={selectedFaceId}
              candidateWorkflow={beginnerCandidateWorkflow}
              editor={beginnerEditorState}
              recognitionWorkflow={beginnerRecognitionWorkflow}
              referenceWorkflow={beginnerReferenceWorkflow}
              onSubmit={submitBeginnerDesignProfile}
            />
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
              semanticMus={geometricConstraintPreflight?.semantic_mus ?? null}
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
          <ValidationInspectorSections
            locale={locale}
            validation={validation}
            lines={nativeLines}
            vertices={nativeVertices}
            unsplitIntersectionCount={unsplitIntersectionCount}
            bulkIntersectionRepairPending={bulkIntersectionRepairPending}
            controlsDisabled={coreBusy || fileOperation !== null}
            onRepairAllIntersections={repairAllIntersections}
            localPresentation={localFlatFoldabilityPresentation}
            benchmarkActive={Boolean(benchmarkRun)}
            selectedVertexId={selectedVertexId}
            assignedLocalSummaryStatus={assignedLocalSummaryStatus}
            assignedLocalSummary={assignedLocalSummary}
            assignedLocalSufficiency={assignedLocalSufficiency}
            onSelectLine={(lineId) => {
              setSelectedLineId(lineId)
              setSelectedVertexId(null)
            }}
            onSelectVertex={(vertexId) => {
              setSelectedVertexId(vertexId)
              setSelectedLineId(null)
            }}
            onSelectSummaryVertex={setSelectedVertexId}
          />
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
          <PaperInspectorSection
            locale={locale}
            snapshot={nativeSnapshot}
            coreBusy={coreBusy}
            lengthDisplayUnit={lengthDisplayUnit}
            lengthDisplayUnitLabelText={lengthDisplayUnitLabelText}
            boundaryLengthReferences={boundaryLengthReferences}
            paperFormKey={paperFormKey}
            paperResizeFormKey={paperResizeFormKey}
            rectangularPaperSize={rectangularPaperSize}
            rectangularRatioReferenceAxis={rectangularRatioReferenceAxis}
            creationDimensionExpression={creationDimensionExpression}
            onLengthUnitChange={changeLengthDisplayUnit}
            onSubmitPaperProperties={submitPaperProperties}
            onChooseFrontPaperTexture={chooseFrontPaperTexture}
            onChooseBackPaperTexture={chooseBackPaperTexture}
            onSubmitPaperResize={submitPaperResize}
          />
          <HistoryLimitInspectorSection
            locale={locale}
            snapshot={nativeSnapshot}
            settings={boundHistoryLimitSettings}
            loadState={historyLimitLoadState}
            disabled={coreBusy || recoveryBlocking}
            onApplied={acceptAppliedHistoryLimit}
            onRetry={() => setHistoryLimitRetrySequence(
              (sequence) => sequence + 1,
            )}
          />
          <FoldTechniqueInspectorSection
            locale={locale}
            workspace={foldTechniqueWorkspace}
            selectedIndex={foldTechniqueSelectedIndex}
            coreBusy={coreBusy}
            fileBusy={foldTechniqueBusy}
            timelineBusy={foldTechniqueTimelineBusy}
            projectAvailable={nativeSnapshot !== null}
            nativeFileAvailable={isNativeFoldTechniqueFileAvailable()}
            nativeCoreAvailable={isNativeCoreAvailable()}
            onSelectTechnique={setFoldTechniqueSelectedIndex}
            onCreate={openNewFoldTechniqueEditor}
            onImport={importFoldTechniqueFile}
            onEdit={openCurrentFoldTechniqueEditor}
            onSaveAs={saveCurrentFoldTechniqueAs}
            onPreviewTimeline={previewSelectedFoldTechniqueTimeline}
          />
          <SnapInspectorSection
            locale={locale}
            coreBusy={coreBusy}
            snapSettings={snapSettings}
            gridDivisionsInput={gridDivisionsInput}
            gridDivisionsValid={gridDivisionsValid}
            gridDivisions={gridDivisions}
            gridDiagonals={gridDiagonals}
            selectedAnglePreset={selectedAnglePreset}
            angleDegrees={angleDegrees}
            angleDegreesInput={angleDegreesInput}
            angleInputIsValid={angleInputIsValid}
            angleInputRef={angleInputRef}
            angleReferenceKind={angleReferenceKind}
            parallelReferenceLine={parallelReferenceLine}
            onSnapSettingsChange={setSnapSettings}
            onGridPreferenceChange={(input, diagonals) => {
              setGridDivisionsInput(input)
              setGridDiagonals(diagonals)
            }}
            onGridDiagonalsChange={setGridDiagonals}
            onAngleDegreesChange={setAngleDegrees}
            onAngleDegreesInputChange={setAngleDegreesInput}
            onAngleReferenceKindChange={setAngleReferenceKind}
            onClearParallelReference={() => setParallelReferenceEdgeId(null)}
          />
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
