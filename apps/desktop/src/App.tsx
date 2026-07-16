import { getCurrentWindow } from '@tauri-apps/api/window'
import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { CreaseCanvas, type CreaseLine } from './components/CreaseCanvas'
import { FoldPreview } from './components/FoldPreview'
import {
  addEdge,
  addVertex,
  generateBenchmarkPattern,
  getProjectSnapshot,
  isNativeCoreAvailable,
  moveVertex,
  openProject,
  redo,
  removeEdge,
  removeVertex,
  saveProject,
  saveProjectAs,
  setCuttingAllowed,
  undo,
  type ProjectSnapshot,
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
        (edge.kind !== 'mountain' && edge.kind !== 'valley' && edge.kind !== 'cut')
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
  const selectedVertex = useMemo(
    () => nativeSnapshot?.crease_pattern.vertices.find(
      (vertex) => vertex.id === selectedVertexId,
    ),
    [nativeSnapshot, selectedVertexId],
  )

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
      const removed = await runNativeEdit((projectId, revision) =>
        removeEdge(projectId, revision, selectedLine.id))
      if (removed) setSelectedLineId(null)
      return
    }
    if (selectedVertex) {
      const removed = await runNativeEdit((projectId, revision) =>
        removeVertex(projectId, revision, selectedVertex.id))
      if (removed) setSelectedVertexId(null)
    }
  }, [runNativeEdit, selectedLine, selectedVertex])

  useEffect(() => {
    function handleKeyboardShortcut(event: KeyboardEvent) {
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
  }, [deleteSelection, nativeSnapshot, runNativeEdit, selectedLine, selectedVertex])

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
      <header className="titlebar">
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
            disabled={coreBusy || !nativeSnapshot}
            onClick={() => runNativeEdit((projectId, revision) =>
              addVertex(projectId, revision, 200, 200))}
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

      <section className="workspace">
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
              disabled={coreBusy}
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
                400 × 400 mm · {nativeLines.length.toLocaleString()}本
              </span>
            </div>
            <CreaseCanvas
              lines={nativeLines}
              vertices={nativeSnapshot?.crease_pattern.vertices.map((vertex) => ({
                id: vertex.id,
                x: vertex.position.x,
                y: vertex.position.y,
              }))}
              tool={activeTool}
              selectedVertexId={selectedVertexId}
              pendingVertexId={pendingEdgeStart}
              selectedLineId={selectedLineId}
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
            <FoldPreview angle={foldAngle} />
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
                </dl>
                <div className="property-actions">
                  <button
                    type="button"
                    className="danger"
                    disabled={coreBusy}
                    onClick={() => void deleteSelection()}
                  >
                    線を削除
                  </button>
                </div>
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
                      disabled={coreBusy}
                      onClick={() => void deleteSelection()}
                    >
                      頂点を削除
                    </button>
                  </div>
                  <p className="muted">接続線がある頂点は、線を削除してから削除します。</p>
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
                    {validation.issues.slice(0, 20).map((issue, index) => (
                      <li key={`${issue.code}:${index}`}>
                        <button
                          type="button"
                          onClick={() => {
                            const edgeId = issue.edges[0]
                            const vertexId = issue.vertices[0]
                            if (edgeId) {
                              setSelectedLineId(edgeId)
                              setSelectedVertexId(null)
                            } else if (vertexId) {
                              setSelectedVertexId(vertexId)
                              setSelectedLineId(null)
                            }
                          }}
                        >
                          {validationIssueLabel(issue.code)}
                        </button>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </section>
          )}
          <section>
            <h2>紙</h2>
            <label className="field">厚さ <input defaultValue="0.10" /> mm</label>
            <label className="check">
              <input
                type="checkbox"
                checked={nativeSnapshot?.cutting_allowed ?? false}
                disabled={coreBusy || !nativeSnapshot}
                onChange={(event) =>
                  runNativeEdit((projectId, revision) =>
                    setCuttingAllowed(projectId, revision, event.target.checked),
                  )
                }
              />{' '}
              切断を許可
            </label>
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

      <section className="timeline panel">
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

      <footer className="statusbar">
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
  }[code] ?? code
}

function isEditingText(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.matches('input, textarea')) return true
  return target.isContentEditable || Boolean(target.closest('[contenteditable="true"]'))
}

export default App
