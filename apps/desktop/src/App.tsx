import { type FormEvent, useEffect, useMemo, useState } from 'react'
import { CreaseCanvas, type CreaseLine } from './components/CreaseCanvas'
import { FoldPreview } from './components/FoldPreview'
import {
  addEdge,
  addVertex,
  generateBenchmarkPattern,
  getProjectSnapshot,
  isNativeCoreAvailable,
  moveVertex,
  redo,
  removeEdge,
  removeVertex,
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
        setNativeSnapshot(snapshot)
        setCoreStatus(`Rustコア revision ${snapshot.revision}`)
      })
      .catch((error: unknown) => setCoreStatus(`コアエラー: ${String(error)}`))
  }, [])

  async function runNativeEdit(action: (revision: number) => Promise<ProjectSnapshot>) {
    if (!nativeSnapshot) return false
    try {
      const snapshot = await action(nativeSnapshot.revision)
      setNativeSnapshot(snapshot)
      setValidation(null)
      setCoreStatus(`Rustコア revision ${snapshot.revision}`)
      return true
    } catch (error) {
      setCoreStatus(`コアエラー: ${String(error)}`)
      return false
    }
  }

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
    void runNativeEdit((revision) => addEdge(revision, start, vertexId, activeTool))
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
    void runNativeEdit((revision) => moveVertex(revision, selectedVertex.id, x, y))
  }

  async function runValidation() {
    if (!nativeSnapshot) return
    try {
      const result = await validateProject()
      if (result.revision !== nativeSnapshot.revision) {
        setCoreStatus('検証中に内容が変更されたため、再度検証してください')
        return
      }
      setValidation(result)
      setCoreStatus(result.is_valid
        ? `revision ${result.revision}: 幾何検証に合格`
        : `revision ${result.revision}: ${result.issues.length}件の問題`)
    } catch (error) {
      setCoreStatus(`検証エラー: ${String(error)}`)
    }
  }

  return (
    <main className="app-shell">
      <header className="titlebar">
        <div className="brand-mark" aria-hidden="true">◇</div>
        <strong>ORIGAMI2</strong>
        <span className="document-name">無題のプロジェクト</span>
        <nav className="top-actions" aria-label="プロジェクト操作">
          <button
            type="button"
            disabled={!nativeSnapshot?.can_undo}
            onClick={() => runNativeEdit(undo)}
          >
            元に戻す
          </button>
          <button
            type="button"
            disabled={!nativeSnapshot?.can_redo}
            onClick={() => runNativeEdit(redo)}
          >
            やり直す
          </button>
          <button
            type="button"
            disabled={!nativeSnapshot}
            onClick={() => runNativeEdit((revision) => addVertex(revision, 200, 200))}
          >
            中央に頂点
          </button>
          <button type="button">開く</button>
          <button type="button">保存</button>
          <button
            type="button"
            className="primary"
            disabled={!nativeSnapshot}
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
              onSelectLine={(lineId) => {
                setSelectedLineId(lineId)
                if (lineId) setSelectedVertexId(null)
              }}
              onAddVertex={(x, y) =>
                runNativeEdit((revision) => addVertex(revision, x, y))
              }
              onSelectVertex={selectCanvasVertex}
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
                    onClick={() => {
                      void runNativeEdit((revision) => removeEdge(revision, selectedLine.id))
                        .then((removed) => {
                          if (removed) setSelectedLineId(null)
                        })
                    }}
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
                      defaultValue={selectedVertex.position.x}
                    />
                  </label>
                  <label className="field">
                    Y
                    <input
                      name="y"
                      type="number"
                      step="any"
                      defaultValue={selectedVertex.position.y}
                    />
                  </label>
                  <div className="property-actions">
                    <button type="submit">座標を更新</button>
                    <button
                      type="button"
                      className="danger"
                      onClick={() => {
                        void runNativeEdit((revision) => removeVertex(revision, selectedVertex.id))
                          .then((removed) => {
                            if (removed) setSelectedVertexId(null)
                          })
                      }}
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
                disabled={!nativeSnapshot}
                onChange={(event) =>
                  runNativeEdit((revision) =>
                    setCuttingAllowed(revision, event.target.checked),
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

export default App
