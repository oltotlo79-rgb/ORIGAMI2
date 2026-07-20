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
  | Readonly<{ kind: 'ready'; response: StackedFoldReadResponse }>
  | Readonly<{ kind: 'failed'; reason: 'analysis' | 'invalid' | 'apply' | 'stale' }>

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
      setView({ kind: 'ready', response: result.response })
    } else if (result.status === 'failed') {
      setView({ kind: 'failed', reason: result.reason === 'invalid_response' ? 'invalid' : 'analysis' })
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
    try {
      await applyStackedFoldTransaction(token)
      tokenRef.current = null
      const next = await refreshSnapshot()
      onApplied(next)
      setView({ kind: 'idle' })
      setConfirmed(false)
    } catch {
      setView({ kind: 'failed', reason: 'apply' })
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
            : view.reason === 'apply'
              ? t('適用できませんでした。プレビューは失効しました。', 'Apply failed; the preview is no longer trusted.')
              : t('この入力ではnative証明を完成できませんでした。', 'A native proof could not be completed for this input.')}
        </p>
      )}
      {view.kind === 'ready' && (
        <div className="stacked-fold-proof" data-ready={ready}>
          <dl>
            <div><dt>{t('対象面', 'Target faces')}</dt><dd>{view.response.targetFaces.length}</dd></div>
            <div><dt>{t('折り線', 'Creases')}</dt><dd>{view.response.materialSegments.length}</dd></div>
            <div><dt>{t('終端衝突', 'Endpoint collision')}</dt><dd>{view.response.endpointCollision.hasBlockingHold ? t('停止', 'Blocked') : t('なし', 'Clear')}</dd></div>
            <div><dt>{t('連続経路', 'Continuous path')}</dt><dd>{view.response.continuousPath.continuousClearanceCertified ? t('証明済み', 'Certified') : t('未証明', 'Uncertified')}</dd></div>
            <div><dt>{t('層順序', 'Layer order')}</dt><dd>{view.response.flatEndpointLayerOrder.certified ? t('証明済み', 'Certified') : t('未証明', 'Uncertified')}</dd></div>
            <div><dt>{t('追加頂点 / 辺', 'Added vertices / edges')}</dt><dd>{view.response.transactionProposal.addedVertexCount} / {view.response.transactionProposal.addedEdgeCount}</dd></div>
          </dl>
          {failureText.map((failure) => <p role="status" key={failure}>{failure}</p>)}
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
