import { type FormEvent, useEffect, useMemo, useRef, useState } from 'react'
import {
  applyStackedFoldTransaction,
  cancelStackedFoldTransactionPreview,
  proposeCurrentStackedFoldRead,
  type ProjectSnapshot,
} from '../lib/coreClient'
import { selectLocalizedText, type Locale } from '../lib/i18n'
import {
  createStackedFoldReadCoordinator,
  type StackedFoldReadCoordinator,
} from '../lib/stackedFoldReadCoordinator'
import type {
  StackedFoldFixedSide,
  StackedFoldReadResponse,
  StackedFoldRotationDirection,
} from '../lib/stackedFoldRead'
import type { LayerOrderViewerCell } from '../lib/currentLayerOrderView'

type SelectedLine = Readonly<{
  id: string
  start: Readonly<{ x: number; y: number }>
  end: Readonly<{ x: number; y: number }>
}>

type Props = Readonly<{
  locale: Locale
  snapshot: ProjectSnapshot
  selectedLine: SelectedLine | null
  disabled: boolean
  onApplied(snapshot: ProjectSnapshot): void
  refreshSnapshot(): Promise<ProjectSnapshot>
}>

type View =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'reading' }>
  | Readonly<{ kind: 'ready'; response: StackedFoldReadResponse; applyFailed: boolean }>
  | Readonly<{ kind: 'failed'; reason: 'analysis' | 'invalid' | 'apply' | 'stale' | 'cycle_nonclosing' | 'cycle_path_uncertified' }>
  | Readonly<{ kind: 'refresh_failed' }>

export function StackedFoldPanel({
  locale,
  snapshot,
  selectedLine,
  disabled,
  onApplied,
  refreshSnapshot,
}: Props) {
  const t = (ja: string, en: string) => selectLocalizedText(locale, { ja, en })
  const authorityRef = useRef(snapshot)
  authorityRef.current = snapshot
  const [fixedSide, setFixedSide] = useState<StackedFoldFixedSide>('left')
  const [rotationDirection, setRotationDirection] =
    useState<StackedFoldRotationDirection>('positive')
  const [angle, setAngle] = useState('180')
  const [confirmed, setConfirmed] = useState(false)
  const [applying, setApplying] = useState(false)
  const [view, setView] = useState<View>({ kind: 'idle' })
  const [selectedCell, setSelectedCell] = useState<string | null>(null)
  const [selectedFace, setSelectedFace] = useState<string | null>(null)
  const [hoveredFace, setHoveredFace] = useState<string | null>(null)
  const tokenRef = useRef<string | null>(null)
  const coordinator = useMemo<StackedFoldReadCoordinator>(() =>
    createStackedFoldReadCoordinator({
      transport: proposeCurrentStackedFoldRead,
      getAuthority: () => {
        const current = authorityRef.current
        return {
          projectInstanceId: current.project_instance_id,
          projectId: current.project_id,
          revision: current.revision,
        }
      },
    }), [])

  const cancelToken = (token: string | null) => {
    if (!token) return
    void cancelStackedFoldTransactionPreview(token).catch(() => undefined)
  }

  useEffect(() => {
    coordinator.invalidate()
    cancelToken(tokenRef.current)
    tokenRef.current = null
    setConfirmed(false)
    setSelectedCell(null)
    setSelectedFace(null)
    setHoveredFace(null)
    setView({ kind: 'idle' })
  }, [
    coordinator,
    snapshot.project_instance_id,
    snapshot.project_id,
    snapshot.revision,
    selectedLine?.id,
    fixedSide,
    rotationDirection,
    angle,
  ])

  useEffect(() => () => {
    coordinator.dispose()
    cancelToken(tokenRef.current)
  }, [coordinator])

  async function preview(event: FormEvent) {
    event.preventDefault()
    if (!selectedLine || disabled || applying) return
    const requestedAngleDegrees = Number(angle)
    setConfirmed(false)
    setView({ kind: 'reading' })
    const result = await coordinator.read({
      expectedProjectInstanceId: snapshot.project_instance_id,
      expectedProjectId: snapshot.project_id,
      expectedRevision: snapshot.revision,
      first: [selectedLine.start.x, 0, -selectedLine.start.y],
      second: [selectedLine.end.x, 0, -selectedLine.end.y],
      fixedSide,
      rotationDirection,
      requestedAngleDegrees,
    })
    if (result.status === 'ready') {
      tokenRef.current = result.response.transactionProposal.transactionToken
      setView({ kind: 'ready', response: result.response, applyFailed: false })
    } else if (result.status === 'failed') {
      setView({
        kind: 'failed',
        reason: result.reason === 'invalid_response'
          ? 'invalid'
          : result.reason === 'cycle_nonclosing' || result.reason === 'cycle_path_uncertified'
            ? result.reason
            : 'analysis',
      })
    } else if (result.reason === 'stale_authority') {
      setView({ kind: 'failed', reason: 'stale' })
    } else {
      setView({ kind: 'idle' })
    }
  }

  async function apply() {
    if (
      view.kind !== 'ready' ||
      !view.response.transactionProposal.readyForAtomicApply ||
      !confirmed ||
      applying
    ) return
    const token = view.response.transactionProposal.transactionToken
    if (!token || token !== tokenRef.current) return
    setApplying(true)
    let committed = false
    try {
      await applyStackedFoldTransaction(token)
      committed = true
      tokenRef.current = null
      const next = await refreshSnapshot()
      onApplied(next)
      setView({ kind: 'idle' })
      setConfirmed(false)
    } catch {
      setView(committed
        ? { kind: 'refresh_failed' }
        : { kind: 'ready', response: view.response, applyFailed: true })
    } finally {
      setApplying(false)
    }
  }

  async function retryRefresh() {
    setApplying(true)
    try {
      onApplied(await refreshSnapshot())
      setView({ kind: 'idle' })
    } catch {
      setView({ kind: 'refresh_failed' })
    } finally {
      setApplying(false)
    }
  }

  const ready = view.kind === 'ready' && view.response.transactionProposal.readyForAtomicApply
  const failureText = view.kind === 'ready'
    ? view.response.transactionProposal.failureClasses.map((failure) =>
        failure === 'continuous_path_uncertified'
          ? t('連続経路の無衝突証明がありません。', 'The continuous path is not collision-certified.')
          : t('折り後の層順序を証明できません。', 'The target layer order is not certified.'))
    : []

  return (
    <section className="property-section stacked-fold-panel" aria-busy={view.kind === 'reading' || applying}>
      <h2>{t('一直線の折り重ね', 'Straight-line stacked fold')}</h2>
      <p className="muted">
        {selectedLine
          ? t('選択中の線を折り軸としてnative証明を作成します。', 'The selected line is used as the axis for a native proof.')
          : t('2Dキャンバスで折り軸にする線を選択してください。', 'Select a fold-axis line on the 2D canvas.')}
      </p>
      <form onSubmit={(event) => void preview(event)}>
        <label>
          <span>{t('固定側', 'Fixed side')}</span>
          <select value={fixedSide} onChange={(event) => setFixedSide(event.target.value as StackedFoldFixedSide)} disabled={disabled || applying}>
            <option value="left">{t('線の左側', 'Left of line')}</option>
            <option value="right">{t('線の右側', 'Right of line')}</option>
          </select>
        </label>
        <label>
          <span>{t('回転方向', 'Rotation direction')}</span>
          <select value={rotationDirection} onChange={(event) => setRotationDirection(event.target.value as StackedFoldRotationDirection)} disabled={disabled || applying}>
            <option value="positive">{t('正方向', 'Positive')}</option>
            <option value="negative">{t('負方向', 'Negative')}</option>
          </select>
        </label>
        <label>
          <span>{t('角度（度）', 'Angle (degrees)')}</span>
          <input value={angle} onChange={(event) => setAngle(event.target.value)} type="number" min="0.000001" max="180" step="any" required disabled={disabled || applying} />
        </label>
        <button type="submit" disabled={!selectedLine || disabled || applying || view.kind === 'reading'}>
          {view.kind === 'reading' ? t('証明中…', 'Proving…') : t('安全性を確認', 'Verify safety')}
        </button>
      </form>
      {view.kind === 'failed' && (
        <p role="alert">
          {view.reason === 'stale'
            ? t('編集内容が変わりました。もう一度確認してください。', 'The project changed. Verify again.')
            : view.reason === 'cycle_nonclosing'
              ? t('循環hingeの終端が閉じないため適用できません。', 'The cyclic hinge endpoint does not close, so apply is disabled.')
              : view.reason === 'cycle_path_uncertified'
                ? t('循環hingeの終端は閉じますが、連続経路を証明できないため適用できません。', 'The cyclic endpoint closes, but its continuous path is uncertified, so apply is disabled.')
            : view.reason === 'apply'
              ? t('適用できませんでした。プレビューは失効しました。', 'Apply failed; the preview is no longer trusted.')
              : t('この入力ではnative証明を完成できませんでした。', 'A native proof could not be completed for this input.')}
        </p>
      )}
      {view.kind === 'refresh_failed' && (
        <div role="alert">
          <p>{t('折り重ねは適用済みですが、最新表示を取得できませんでした。', 'The stacked fold was applied, but the refreshed project could not be loaded.')}</p>
          <button type="button" disabled={applying} onClick={() => void retryRefresh()}>
            {t('最新表示を再取得', 'Retry refresh')}
          </button>
        </div>
      )}
      {view.kind === 'ready' && (
        <div className="stacked-fold-proof" data-ready={ready}>
          <dl>
            <div><dt>{t('対象面', 'Target faces')}</dt><dd>{view.response.targetFaces.length}</dd></div>
            <div><dt>{t('折り線', 'Creases')}</dt><dd>{view.response.materialSegments.length}</dd></div>
            <div><dt>{t('対象hinge', 'Target hinges')}</dt><dd>{view.response.topologyProof.targetHingeCount}</dd></div>
            <div><dt>{t('終端衝突', 'Endpoint collision')}</dt><dd>{view.response.endpointCollision.hasBlockingHold ? t('停止', 'Blocked') : t('なし', 'Clear')}</dd></div>
            <div><dt>{t('連続経路', 'Continuous path')}</dt><dd>{view.response.continuousPath.continuousClearanceCertified ? t('証明済み', 'Certified') : t('未証明', 'Uncertified')}</dd></div>
            <div><dt>{t('最初の停止確認角', 'First proven blocking sample')}</dt><dd>{view.response.continuousPath.firstSampledBlockingAngleDegrees === null ? t('なし', 'None') : `${view.response.continuousPath.firstSampledBlockingAngleDegrees}°`}</dd></div>
            <div><dt>{t('経路証明model', 'Path certificate model')}</dt><dd>{view.response.continuousPath.continuousCertificateModelId ?? t('なし', 'None')}</dd></div>
            <div><dt>{t('区間leaf数', 'Interval leaves')}</dt><dd>{view.response.continuousPath.intervalLeafCount}</dd></div>
            <div><dt>{t('区間pair work', 'Interval pair work')}</dt><dd>{view.response.continuousPath.intervalPairWork}</dd></div>
            <div><dt>{t('証明済み紙厚', 'Certified thickness')}</dt><dd>{view.response.continuousPath.paperThicknessMm} mm</dd></div>
            <div><dt>{t('層順序', 'Layer order')}</dt><dd>{view.response.flatEndpointLayerOrder.certified ? t('証明済み', 'Certified') : t('未証明', 'Uncertified')}</dd></div>
            <div><dt>{t('追加頂点 / 辺', 'Added vertices / edges')}</dt><dd>{view.response.transactionProposal.addedVertexCount} / {view.response.transactionProposal.addedEdgeCount}</dd></div>
          </dl>
          <p>{t(
            'この証明は表示された紙厚・2三角形・1ヒンジ・90度以下だけを対象とします。一般の多面、別の紙厚、工作性は保証しません。',
            'This certificate covers only the displayed thickness, two triangular faces, one hinge, and a path up to 90°. It does not guarantee general multi-face folds, another thickness, or physical manufacturability.',
          )}</p>
          <LayerOrderViewer
            locale={locale}
            cells={view.response.crossedCells}
            selectedCell={selectedCell}
            selectedFace={selectedFace}
            hoveredFace={hoveredFace}
            onSelectCell={setSelectedCell}
            onSelectFace={setSelectedFace}
            onHoverFace={setHoveredFace}
          />
          {failureText.map((failure) => <p role="status" key={failure}>{failure}</p>)}
          {view.applyFailed && (
            <p role="alert">{t('適用できませんでした。同じ証明済みpreviewで再試行できます。', 'Apply failed. You can retry with the same certified preview.')}</p>
          )}
          <label>
            <input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} disabled={!ready || applying} />
            {t('証明済みの変更内容を確認しました。', 'I reviewed the certified changes.')}
          </label>
          <button type="button" onClick={() => void apply()} disabled={!ready || !confirmed || applying}>
            {applying ? t('適用中…', 'Applying…') : t('折り重ねを適用', 'Apply stacked fold')}
          </button>
          {!ready && <p className="muted">{t('未証明のため適用は無効です。', 'Apply is disabled because the case is not fully certified.')}</p>}
        </div>
      )}
    </section>
  )
}

export function LayerOrderViewer({
  locale,
  cells,
  selectedCell,
  selectedFace,
  hoveredFace,
  onSelectCell,
  onSelectFace,
  onHoverFace,
}: Readonly<{
  locale: Locale
  cells: readonly LayerOrderViewerCell[]
  selectedCell: string | null
  selectedFace: string | null
  hoveredFace: string | null
  onSelectCell(value: string): void
  onSelectFace(value: string): void
  onHoverFace(value: string | null): void
}>) {
  const t = (ja: string, en: string) => selectLocalizedText(locale, { ja, en })
  const active = cells.find((cell) => cell.cellKeySha256 === selectedCell) ?? cells[0]
  if (!active) return null
  const xs = active.boundaryWorld.map((point) => point[0])
  const zs = active.boundaryWorld.map((point) => point[2])
  const minX = Math.min(...xs); const maxX = Math.max(...xs)
  const minZ = Math.min(...zs); const maxZ = Math.max(...zs)
  const spanX = Math.max(maxX - minX, 1)
  const spanZ = Math.max(maxZ - minZ, 1)
  const polygon = active.boundaryWorld.map((point) =>
    `${20 + ((point[0] - minX) / spanX) * 180},${20 + ((point[2] - minZ) / spanZ) * 110}`,
  ).join(' ')
  return <section className="stacked-fold-layer-viewer" aria-label={t('3D層順ビューア', '3D layer-order viewer')}>
    <h3>{t('重なりセルと層順', 'Overlap cells and layer order')}</h3>
    <p className="muted">{t(
      '認証済みの現在poseと層順を読み取り専用で表示します。',
      'Read-only view of the authenticated current pose and layer order.',
    )}</p>
    <div className="stacked-fold-cell-tabs" role="list">
      {cells.map((cell, index) => <button type="button" role="listitem"
        aria-pressed={cell.cellKeySha256 === active.cellKeySha256}
        key={cell.cellKeySha256} onClick={() => onSelectCell(cell.cellKeySha256)}>
        {t('cell', 'Cell')} {index + 1}
      </button>)}
    </div>
    <svg viewBox="0 0 240 180" role="img"
      aria-label={t('front/back層の分解表示', 'Exploded front/back layer stack')}>
      {active.bottomToTopFaces.map((face, index) => {
        const offset = (active.bottomToTopFaces.length - 1 - index) * 9
        const highlighted = face === selectedFace || face === hoveredFace
        return <polygon key={face} points={polygon} transform={`translate(${offset} ${-offset})`}
          fill={highlighted ? '#f6b73c' : `hsl(${205 + index * 22} 55% 62%)`}
          fillOpacity="0.72" stroke={highlighted ? '#6b3e00' : '#29465b'}
          tabIndex={0} onClick={() => onSelectFace(face)}
          onMouseEnter={() => onHoverFace(face)} onMouseLeave={() => onHoverFace(null)}
          onFocus={() => onHoverFace(face)} onBlur={() => onHoverFace(null)}>
          <title>{`${index === 0 ? 'back / bottom' : index === active.bottomToTopFaces.length - 1 ? 'front / top' : 'middle'}: ${face}`}</title>
        </polygon>
      })}
    </svg>
    <ol className="stacked-fold-layer-list">
      {active.bottomToTopFaces.map((face, index) => <li key={face}>
        <button type="button" aria-pressed={face === selectedFace}
          onMouseEnter={() => onHoverFace(face)} onMouseLeave={() => onHoverFace(null)}
          onClick={() => onSelectFace(face)}>
          {index === 0 ? t('裏面 / 最下層', 'Back / bottom')
            : index === active.bottomToTopFaces.length - 1
              ? t('表面 / 最上層', 'Front / top')
              : t('中間層', 'Middle')} · {face.slice(0, 8)}
        </button>
      </li>)}
    </ol>
  </section>
}
