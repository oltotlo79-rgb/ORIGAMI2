import { useEffect, useMemo, useState } from 'react'
import { CreaseCanvas, type CreaseLine } from './components/CreaseCanvas'
import { FoldPreview } from './components/FoldPreview'
import {
  addVertex,
  generateBenchmarkPattern,
  getProjectSnapshot,
  isNativeCoreAvailable,
  redo,
  undo,
  type ProjectSnapshot,
} from './lib/coreClient'
import './App.css'

const SAMPLE_LINES: CreaseLine[] = [
  { id: 'v-1', x1: 20, y1: 20, x2: 20, y2: 380, kind: 'boundary' },
  { id: 'v-2', x1: 20, y1: 20, x2: 380, y2: 20, kind: 'boundary' },
  { id: 'v-3', x1: 380, y1: 20, x2: 380, y2: 380, kind: 'boundary' },
  { id: 'v-4', x1: 20, y1: 380, x2: 380, y2: 380, kind: 'boundary' },
  { id: 'm-1', x1: 20, y1: 200, x2: 380, y2: 200, kind: 'mountain' },
  { id: 'm-2', x1: 200, y1: 20, x2: 200, y2: 380, kind: 'mountain' },
  { id: 'v-5', x1: 20, y1: 20, x2: 380, y2: 380, kind: 'valley' },
  { id: 'v-6', x1: 380, y1: 20, x2: 20, y2: 380, kind: 'valley' },
]

function App() {
  const [selectedLineId, setSelectedLineId] = useState<string | null>('m-1')
  const [foldAngle, setFoldAngle] = useState(52)
  const [activeTool, setActiveTool] = useState('select')
  const [benchmarkStatus, setBenchmarkStatus] = useState('未実行')
  const [nativeSnapshot, setNativeSnapshot] = useState<ProjectSnapshot | null>(null)
  const [coreStatus, setCoreStatus] = useState(
    isNativeCoreAvailable() ? 'コア接続中…' : 'ブラウザ試作モード',
  )
  const selectedLine = useMemo(
    () => SAMPLE_LINES.find((line) => line.id === selectedLineId),
    [selectedLineId],
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
    if (!nativeSnapshot) return
    try {
      const snapshot = await action(nativeSnapshot.revision)
      setNativeSnapshot(snapshot)
      setCoreStatus(`Rustコア revision ${snapshot.revision}`)
    } catch (error) {
      setCoreStatus(`コアエラー: ${String(error)}`)
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
          <button type="button" className="primary">検証</button>
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
              onClick={() => setActiveTool(id)}
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
              <span className="panel-meta">400 × 400 mm · 8本</span>
            </div>
            <CreaseCanvas
              lines={SAMPLE_LINES}
              vertices={nativeSnapshot?.crease_pattern.vertices.map((vertex) => ({
                id: vertex.id,
                x: vertex.position.x,
                y: vertex.position.y,
              }))}
              tool={activeTool}
              selectedLineId={selectedLineId}
              onSelectLine={setSelectedLineId}
              onAddVertex={(x, y) =>
                runNativeEdit((revision) => addVertex(revision, x, y))
              }
            />
          </article>

          <article className="panel preview-panel">
            <div className="panel-heading">
              <span>3D プレビュー</span>
              <span className="status-ready">検証前</span>
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
              <dl>
                <div><dt>ID</dt><dd>{selectedLine.id}</dd></div>
                <div><dt>種類</dt><dd>{lineKindLabel(selectedLine.kind)}</dd></div>
                <div><dt>始点</dt><dd>{selectedLine.x1}, {selectedLine.y1}</dd></div>
                <div><dt>終点</dt><dd>{selectedLine.x2}, {selectedLine.y2}</dd></div>
              </dl>
            ) : <p className="muted">線を選択してください</p>}
          </section>
          <section>
            <h2>紙</h2>
            <label className="field">厚さ <input defaultValue="0.10" /> mm</label>
            <label className="check"><input type="checkbox" /> 切断を許可</label>
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

export default App
