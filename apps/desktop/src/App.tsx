import { getCurrentWindow } from '@tauri-apps/api/window'
import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  CreaseCanvas,
  type CreaseLine,
  type PaperBounds,
  type PaperPolygonPoint,
} from './components/CreaseCanvas'
import { FoldPreview } from './components/FoldPreview'
import {
  addEdge,
  addVertex,
  generateBenchmarkPattern,
  getProjectSnapshot,
  isNativeCoreAvailable,
  moveVertex,
  newProject,
  openProject,
  redo,
  removeEdge,
  removeVertex,
  resizeRectangularPaper,
  saveProject,
  saveProjectAs,
  splitBoundaryEdge,
  undo,
  updatePaperProperties,
  type ProjectSnapshot,
  type RgbaColor,
  type ValidationSnapshot,
  validateProject,
} from './lib/coreClient'
import './App.css'

function App() {
  const [selectedLineId, setSelectedLineId] = useState<string | null>(null)
  const [selectedVertexId, setSelectedVertexId] = useState<string | null>(null)
  const [foldAngle, setFoldAngle] = useState(52)
  const [activeTool, setActiveTool] = useState('select')
  const [benchmarkStatus, setBenchmarkStatus] = useState('未実行')
  const [nativeSnapshot, setNativeSnapshot] = useState<ProjectSnapshot | null>(null)
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
  const coreOperationRef = useRef(false)
  const latestSnapshotRef = useRef<ProjectSnapshot | null>(null)
  const applySnapshot = useCallback((snapshot: ProjectSnapshot) => {
    latestSnapshotRef.current = snapshot
    setNativeSnapshot(snapshot)
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
  const selectedLine = useMemo(
    () => nativeLines.find((line) => line.id === selectedLineId),
    [nativeLines, selectedLineId],
  )
  const selectedLineMeasurement = selectedLine ? measureCreaseLine(selectedLine) : null
  const selectedVertex = useMemo(
    () => nativeSnapshot?.crease_pattern.vertices.find(
      (vertex) => vertex.id === selectedVertexId,
    ),
    [nativeSnapshot, selectedVertexId],
  )
  const boundaryVertexIds = useMemo(() => new Set(
    nativeSnapshot?.crease_pattern.edges
      .filter((edge) => edge.kind === 'boundary')
      .flatMap((edge) => [edge.start, edge.end]) ?? [],
  ), [nativeSnapshot])
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

  useEffect(() => {
    if (!isNativeCoreAvailable()) return
    getProjectSnapshot()
      .then((snapshot) => {
        applySnapshot(snapshot)
        setCoreStatus(`Rustコア revision ${snapshot.revision}`)
      })
      .catch((error: unknown) => setCoreStatus(`コアエラー: ${String(error)}`))
  }, [applySnapshot])

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
      if (!disposed) setCoreStatus(`終了確認の初期化エラー: ${String(error)}`)
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
      if (selectedVertexIsBoundary) {
        setCoreStatus('輪郭頂点の削除は紙形状編集から行います')
        return
      }
      const removed = await runNativeEdit((projectId, revision) =>
        removeVertex(projectId, revision, selectedVertex.id))
      if (removed) setSelectedVertexId(null)
    }
  }, [runNativeEdit, selectedLine, selectedVertex, selectedVertexIsBoundary])

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

  useEffect(() => {
    function handleKeyboardShortcut(event: KeyboardEvent) {
      if (event.key.toLowerCase() === 'escape' && newProjectOpen) {
        event.preventDefault()
        if (coreBusy) return
        setNewProjectOpen(false)
        setNewProjectError(null)
        return
      }
      if (newProjectOpen) return
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
  }, [coreBusy, deleteSelection, nativeSnapshot, newProjectOpen, runNativeEdit, selectedLine, selectedVertex])

  function selectVertexForEdge(vertexId: string) {
    if (activeTool !== 'mountain' && activeTool !== 'valley' && activeTool !== 'cut') return
    if (!pendingEdgeStart) {
      setPendingEdgeStart(vertexId)
      setCoreStatus('折り線の終点を選択してください')
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
    if (activeTool === 'select') {
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

  return (
    <main className="app-shell">
      <header className="titlebar" inert={newProjectOpen}>
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

      <section className="workspace" inert={newProjectOpen}>
        <aside className="tool-rail" aria-label="作図ツール">
          {[
            ['select', '↖', '選択'],
            ['vertex', '＋', '頂点'],
            ['mountain', '━', '山折り'],
            ['valley', '┅', '谷折り'],
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
                {paperSizeLabel} · {nativeLines.length.toLocaleString()}本
              </span>
            </div>
            <CreaseCanvas
              lines={nativeLines}
              paperBounds={paperBounds}
              paperPolygon={paperPolygon}
              paperColor={paperFrontColor}
              vertices={nativeSnapshot?.crease_pattern.vertices.map((vertex) => ({
                id: vertex.id,
                x: vertex.position.x,
                y: vertex.position.y,
              }))}
              tool={activeTool}
              selectedVertexId={selectedVertexId}
              pendingVertexId={pendingEdgeStart}
              selectedLineId={selectedLineId}
              measurementLabel={formatLineMeasurementLabel(selectedLineMeasurement)}
              cancelInteractionToken={cancelInteractionToken}
              disabled={coreBusy}
              onSelectLine={(lineId) => {
                setSelectedLineId(lineId)
                if (lineId) setSelectedVertexId(null)
              }}
              onAddVertex={(x, y) =>
                runNativeEdit((projectId, revision) =>
                  addVertex(projectId, revision, x, y))
              }
              onSelectVertex={selectCanvasVertex}
              onMoveVertex={(vertexId, x, y) => {
                void runNativeEdit((projectId, revision) =>
                  moveVertex(projectId, revision, vertexId, x, y))
              }}
            />
          </article>

          <article className="panel preview-panel">
            <div className="panel-heading">
              <span>3D プレビュー</span>
              <span className={validation?.is_valid ? 'status-valid' : validation ? 'status-invalid' : 'status-ready'}>
                {validation
                  ? validation.is_valid
                    ? '幾何検証 OK'
                    : `${validation.issues.length}件の問題`
                  : '検証前'}
              </span>
            </div>
            <FoldPreview
              angle={foldAngle}
              paperBounds={paperBounds}
              frontColor={nativeSnapshot?.paper.front.color}
              backColor={nativeSnapshot?.paper.back.color}
              thicknessMm={nativeSnapshot?.paper.thickness_mm}
            />
            <div className="fold-control">
              <label htmlFor="fold-angle">折り角</label>
              <input
                id="fold-angle"
                type="range"
                min="-180"
                max="180"
                value={foldAngle}
                onChange={(event) => setFoldAngle(Number(event.target.value))}
              />
              <output>{foldAngle}°</output>
            </div>
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
                <div className="property-actions">
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
                {selectedLine.kind === 'boundary' && (
                  <p className="muted">分割後に選択される新しい頂点を移動して、紙の輪郭を編集できます。</p>
                )}
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
                      disabled={coreBusy || selectedVertexIsBoundary}
                      onClick={() => void deleteSelection()}
                    >
                      頂点を削除
                    </button>
                  </div>
                  <p className="muted">
                    {selectedVertexIsBoundary
                      ? '輪郭頂点の構成変更は、今後追加する紙形状編集から行います。'
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
            <div className="chip-row">
              <button type="button" className="chip active">頂点</button>
              <button type="button" className="chip active">交点</button>
              <button type="button" className="chip">中点</button>
            </div>
          </section>
        </aside>
      </section>

      <section className="timeline panel" inert={newProjectOpen}>
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

      <footer className="statusbar" inert={newProjectOpen}>
        <span>ツール: {toolLabel(activeTool)}</span>
        <span>{coreStatus}</span>
        <span>スナップ: 頂点・交点</span>
        <span className="status-spacer" />
        <button
          type="button"
          className="benchmark-button"
          onClick={async () => {
            setBenchmarkStatus('生成中…')
            const startedAt = performance.now()
            const result = await generateBenchmarkPattern(10_000)
            setBenchmarkStatus(`${result.edge_count.toLocaleString()}本 / ${(performance.now() - startedAt).toFixed(1)}ms`)
          }}
        >
          10,000本テスト
        </button>
        <span>{benchmarkStatus}</span>
      </footer>
    </main>
  )
}

function lineKindLabel(kind: CreaseLine['kind']) {
  return { mountain: '山折り', valley: '谷折り', boundary: '輪郭線', cut: '切断線' }[kind]
}

function toolLabel(tool: string) {
  return { select: '選択', vertex: '頂点', mountain: '山折り', valley: '谷折り', cut: '切断', measure: '計測' }[tool]
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
