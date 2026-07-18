import { getCurrentWindow } from '@tauri-apps/api/window'
import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  CreaseCanvas,
  type CreaseCanvasRenderMetrics,
  type CreaseLine,
  type PaperBounds,
  type PaperPolygonPoint,
} from './components/CreaseCanvas'
import { DiagnosticsDialog } from './components/DiagnosticsDialog'
import { FoldPreview } from './components/FoldPreview'
import {
  addEdge,
  addVertex,
  analyzeProjectTopology,
  connectEdgeIntersection,
  connectIntersectionCluster,
  connectTJunction,
  generateBenchmarkPattern,
  getProjectSnapshot,
  isNativeCoreAvailable,
  moveVertex,
  newProject,
  openProject,
  redo,
  removeBoundaryVertex,
  removeEdge,
  removeVertex,
  resizeRectangularPaper,
  saveProject,
  saveProjectAs,
  splitBoundaryEdge,
  splitEdge,
  undo,
  updatePaperProperties,
  type ProjectSnapshot,
  type ProjectTopologyResponse,
  type RgbaColor,
  type ValidationSnapshot,
  validateProject,
} from './lib/coreClient'
import { buildFoldPreviewModel } from './lib/foldPreviewModel'
import type { FoldPreviewHingeAngle } from './lib/foldPreviewKinematics'
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
import { reportUnexpected } from './lib/diagnosticsRuntime'
import { isDiagnosticsShareAvailable } from './lib/diagnosticsShare'
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
  const [coreStatus, setCoreStatus] = useState(
    isNativeCoreAvailable() ? 'コア接続中…' : 'ブラウザ試作モード',
  )
  const [pendingEdgeStart, setPendingEdgeStart] = useState<string | null>(null)
  const [cancelInteractionToken, setCancelInteractionToken] = useState(0)
  const [fileOperation, setFileOperation] = useState<'open' | 'save' | 'save_as' | null>(null)
  const [coreBusy, setCoreBusy] = useState(false)
  const [newProjectOpen, setNewProjectOpen] = useState(false)
  const [newProjectError, setNewProjectError] = useState<string | null>(null)
  const [diagnosticsDialogOpen, setDiagnosticsDialogOpen] = useState(false)
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
  const angleInputRef = useRef<HTMLInputElement>(null)
  const benchmarkRequestIdRef = useRef(0)
  const topologyRequestIdRef = useRef(0)
  const diagnosticsButtonRef = useRef<HTMLButtonElement>(null)
  const modalOpen = newProjectOpen || diagnosticsDialogOpen
  const closeDiagnosticsDialog = useCallback(() => {
    setDiagnosticsDialogOpen(false)
    requestAnimationFrame(() => diagnosticsButtonRef.current?.focus())
  }, [])
  const applySnapshot = useCallback((snapshot: ProjectSnapshot) => {
    topologyRequestIdRef.current += 1
    latestSnapshotRef.current = snapshot
    setNativeSnapshot(snapshot)
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
  const rectangularPaperSize = useMemo(
    () => resolveRectangularPaperSize(nativeSnapshot),
    [nativeSnapshot],
  )
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

  const updateUniformFoldAngle = (value: number) => {
    const nextAngle = normalizeFoldAngle(value)
    if (nextAngle === null) return
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
    ? `${formatMillimetres(paperBounds.maxX - paperBounds.minX)} × ${formatMillimetres(paperBounds.maxY - paperBounds.minY)} mm`
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
      ].join(':')
    : 'paper-unavailable'
  const paperResizeFormKey = nativeSnapshot && rectangularPaperSize
    ? `${nativeSnapshot.project_id}:${rectangularPaperSize.width}:${rectangularPaperSize.height}`
    : `${nativeSnapshot?.project_id ?? 'paper-unavailable'}:not-rectangular`
  const snapStatusLabel = SNAP_OPTIONS
    .filter(({ kind }) => snapSettings[kind])
    .map(({ label }) => label)
    .join('・') || 'なし'

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
    if (!selectedVertex) return
    const form = new FormData(event.currentTarget)
    const x = Number(form.get('x'))
    const y = Number(form.get('y'))
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
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
    const thicknessInput = String(form.get('thickness_mm') ?? '').trim()
    const thicknessMm = Number(thicknessInput)
    const frontColor = parseHexColor(String(form.get('front_color') ?? ''))
    const backColor = parseHexColor(String(form.get('back_color') ?? ''))
    if (!thicknessInput || !Number.isFinite(thicknessMm) || thicknessMm < 0) {
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
    if (!resolveRectangularPaperSize(current)) {
      setCoreStatus('現在の紙は軸平行な長方形ではないため、サイズを変更できません')
      return
    }

    const form = new FormData(event.currentTarget)
    const widthInput = String(form.get('width_mm') ?? '').trim()
    const heightInput = String(form.get('height_mm') ?? '').trim()
    const widthMm = Number(widthInput)
    const heightMm = Number(heightInput)
    if (!widthInput || !Number.isFinite(widthMm) || widthMm <= 0) {
      setCoreStatus('用紙の幅には0より大きい有限の数値を入力してください')
      return
    }
    if (!heightInput || !Number.isFinite(heightMm) || heightMm <= 0) {
      setCoreStatus('用紙の高さには0より大きい有限の数値を入力してください')
      return
    }

    void runNativeEdit((projectId, revision) =>
      resizeRectangularPaper(projectId, revision, widthMm, heightMm))
  }

  async function runValidation() {
    const current = latestSnapshotRef.current
    if (!current || coreOperationRef.current) return
    coreOperationRef.current = true
    setCoreBusy(true)
    setCancelInteractionToken((token) => token + 1)
    try {
      const result = await validateProject()
      if (
        result.project_id !== current.project_id ||
        result.revision !== current.revision
      ) {
        setCoreStatus('検証中に内容が変更されたため、再度検証してください')
        return
      }
      setValidation(result)
      setCoreStatus(result.is_valid
        ? `revision ${result.revision}: 幾何検証に合格`
        : `revision ${result.revision}: ${result.issues.length}件の問題`)
    } catch (error) {
      reportUnexpected('app.validation')
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
    const thicknessMm = Number(form.get('thickness_mm'))
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
    if (!Number.isFinite(thicknessMm) || thicknessMm < 0) {
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
      applySnapshot(snapshot)
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
      applySnapshot(response.project)
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
            onClick={() => void runFileOperation('open')}
          >
            {fileOperation === 'open' ? '開いています…' : '開く'}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            onClick={() => void runFileOperation('save')}
          >
            {fileOperation === 'save' ? '保存中…' : '保存'}
          </button>
          <button
            type="button"
            disabled={coreBusy || !nativeSnapshot}
            onClick={() => void runFileOperation('save_as')}
          >
            {fileOperation === 'save_as' ? '保存中…' : '別名保存'}
          </button>
          <button
            type="button"
            className="primary"
            disabled={coreBusy || !nativeSnapshot}
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
              measurementLabel={formatLineMeasurementLabel(selectedLineMeasurement)}
              snapSettings={snapSettings}
              parallelReference={benchmarkRun ? null : parallelReferenceLine}
              angleConfig={angleSnapConfig}
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
              model={foldPreviewModel}
              statusMessage={foldPreviewStatus}
              frontColor={nativeSnapshot?.paper.front.color}
              backColor={nativeSnapshot?.paper.back.color}
              thicknessMm={nativeSnapshot?.paper.thickness_mm}
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
                  <div><dt>始点</dt><dd>{selectedLine.x1}, {selectedLine.y1}</dd></div>
                  <div><dt>終点</dt><dd>{selectedLine.x2}, {selectedLine.y2}</dd></div>
                  <div><dt>ΔX</dt><dd>{formatMeasurementValue(selectedLineMeasurement?.deltaX, ' mm')}</dd></div>
                  <div><dt>ΔY</dt><dd>{formatMeasurementValue(selectedLineMeasurement?.deltaY, ' mm')}</dd></div>
                  <div><dt>長さ</dt><dd>{formatMeasurementValue(selectedLineMeasurement?.length, ' mm')}</dd></div>
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
                  key={`${selectedVertex.id}:${selectedVertex.position.x}:${selectedVertex.position.y}`}
                  className="coordinate-form"
                  onSubmit={submitVertexPosition}
                >
                  <label className="field">
                    X
                    <input
                      name="x"
                      type="number"
                      step="any"
                      disabled={coreBusy}
                      defaultValue={selectedVertex.position.x}
                    />
                  </label>
                  <label className="field">
                    Y
                    <input
                      name="y"
                      type="number"
                      step="any"
                      disabled={coreBusy}
                      defaultValue={selectedVertex.position.y}
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
          <section>
            <h2>紙</h2>
            <form
              key={paperFormKey}
              className="paper-properties-form"
              onSubmit={submitPaperProperties}
              noValidate
            >
              <label className="field">
                <span>厚さ</span>
                <input
                  name="thickness_mm"
                  type="number"
                  min="0"
                  step="any"
                  defaultValue={nativeSnapshot?.paper.thickness_mm ?? ''}
                  required
                  disabled={coreBusy || !nativeSnapshot}
                  aria-label="紙厚"
                />
                <span>mm</span>
              </label>
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
                    <input
                      name="width_mm"
                      type="number"
                      min="0"
                      step="any"
                      defaultValue={rectangularPaperSize?.width ?? ''}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      aria-label="用紙の幅"
                    />
                    <span>mm</span>
                  </label>
                  <label className="field">
                    <span>高さ</span>
                    <input
                      name="height_mm"
                      type="number"
                      min="0"
                      step="any"
                      defaultValue={rectangularPaperSize?.height ?? ''}
                      required
                      disabled={coreBusy || !rectangularPaperSize}
                      aria-label="用紙の高さ"
                    />
                    <span>mm</span>
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

      <section className="timeline panel" inert={modalOpen}>
        <div className="timeline-controls">
          <button type="button" aria-label="先頭へ">|◀</button>
          <button type="button" aria-label="再生">▶</button>
          <strong>折り手順</strong>
          <span>00:02.4 / 00:08.0</span>
        </div>
        <div className="timeline-track">
          <span className="step selected">1　中央を山折り</span>
          <span className="step">2　対角線を谷折り</span>
          <span className="step add">＋ 手順を追加</span>
        </div>
      </section>

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
            <form onSubmit={submitNewProject}>
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
                  <label className="dialog-field">
                    <span>紙厚</span>
                    <span className="number-with-unit">
                      <input
                        name="thickness_mm"
                        type="number"
                        defaultValue="0.10"
                        min="0"
                        step="any"
                        required
                        disabled={coreBusy}
                      />
                      mm
                    </span>
                  </label>
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

      <DiagnosticsDialog
        open={diagnosticsDialogOpen}
        onClose={closeDiagnosticsDialog}
      />

      <footer className="statusbar" inert={modalOpen}>
        <span>ツール: {benchmarkRun ? '性能テスト選択' : toolLabel(activeTool)}</span>
        <span>{coreStatus}</span>
        <span>スナップ: {snapStatusLabel}</span>
        <span className="status-spacer" />
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

function formatMillimetres(value: number) {
  return value.toLocaleString('ja-JP', { maximumFractionDigits: 3 })
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

function formatLineMeasurementLabel(measurement: LineMeasurement | null) {
  if (!measurement) return '計測不可'
  return `${formatMeasurementValue(measurement.length, ' mm')} / ${formatMeasurementValue(measurement.angleDegrees, '°', 2)}`
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
