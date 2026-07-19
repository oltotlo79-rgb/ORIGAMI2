import type {
  GeometricConstraintDocument,
  GeometricConstraintKind,
  GeometricConstraintPreflightResult,
} from '../lib/coreClient'
import { isCanonicalNonNilUuid } from '../lib/canonicalUuid.ts'
import { createGeometricConstraintPresentation } from '../lib/geometricConstraints'

const MAX_VISIBLE_CONSTRAINTS = 200
const MAX_VISIBLE_DIRECT_CONFLICTS = 50
const MAX_VISIBLE_UNCHECKED_CONSTRAINT_IDS = 20

type GeometricConstraintPanelProps = {
  document: GeometricConstraintDocument
  preflight: GeometricConstraintPreflightResult | null
  analyzing: boolean
  analysisFailed: boolean
  selectedEdgeId: string | null
  disabled: boolean
  onAddOrientation: (orientation: 'horizontal' | 'vertical') => void
  onRemove: (constraintId: string) => void
  onSelectEdge: (edgeId: string) => void
  onRetryAnalysis: () => void
}

export function GeometricConstraintPanel({
  document,
  preflight,
  analyzing,
  analysisFailed,
  selectedEdgeId,
  disabled,
  onAddOrientation,
  onRemove,
  onSelectEdge,
  onRetryAnalysis,
}: GeometricConstraintPanelProps) {
  return (
    <section className="geometric-constraints" aria-labelledby="geometric-constraints-title">
      <div className="geometric-constraints-heading">
        <h2 id="geometric-constraints-title">幾何制約</h2>
        <span>{document.constraints.length}件</span>
      </div>

      <div className="property-actions geometric-constraint-add-actions">
        <button
          type="button"
          disabled={disabled || selectedEdgeId === null}
          onClick={() => onAddOrientation('horizontal')}
        >
          選択線を水平に制約
        </button>
        <button
          type="button"
          disabled={disabled || selectedEdgeId === null}
          onClick={() => onAddOrientation('vertical')}
        >
          選択線を垂直に制約
        </button>
      </div>
      {selectedEdgeId === null && (
        <p className="muted">水平・垂直制約を追加するには線を選択してください。</p>
      )}

      <ConstraintPreflightStatus
        preflight={preflight}
        analyzing={analyzing}
        failed={analysisFailed}
        disabled={disabled}
        onRetry={onRetryAnalysis}
      />

      {document.constraints.length === 0 ? (
        <p className="muted">制約はまだありません。</p>
      ) : (
        <>
          <ol className="geometric-constraint-list">
            {document.constraints.slice(0, MAX_VISIBLE_CONSTRAINTS).map((record) => {
              const edge = primaryEdgeId(record.constraint)
              const presentation = createGeometricConstraintPresentation(record)
              const displayName = presentation?.displayName ?? '不明な制約'
              return (
                <li key={record.id}>
                  <div>
                    <strong>{displayName}</strong>
                    <span>{shortenPresentationIds(
                      presentation?.targetSummary ?? record.id,
                    )}</span>
                  </div>
                  <div className="geometric-constraint-row-actions">
                    {edge && (
                      <button
                        type="button"
                        disabled={disabled}
                        onClick={() => onSelectEdge(edge)}
                      >
                        対象を選択
                      </button>
                    )}
                    <button
                      type="button"
                      className="danger"
                      disabled={disabled}
                      aria-label={`${displayName}制約を削除`}
                      onClick={() => onRemove(record.id)}
                    >
                      削除
                    </button>
                  </div>
                </li>
              )
            })}
          </ol>
          {document.constraints.length > MAX_VISIBLE_CONSTRAINTS && (
            <p className="muted">
              先頭{MAX_VISIBLE_CONSTRAINTS}件を表示しています。残り
              {document.constraints.length - MAX_VISIBLE_CONSTRAINTS}件は、
              表示中の制約を削除すると順に表示されます。
            </p>
          )}
        </>
      )}
    </section>
  )
}

function ConstraintPreflightStatus({
  preflight,
  analyzing,
  failed,
  disabled,
  onRetry,
}: {
  preflight: GeometricConstraintPreflightResult | null
  analyzing: boolean
  failed: boolean
  disabled: boolean
  onRetry: () => void
}) {
  let className = 'is-pending'
  let role: 'status' | 'alert' = 'status'
  let message = '制約を診断しています…'

  if (!analyzing && failed) {
    className = 'is-blocking'
    role = 'alert'
    message = '制約診断を完了できませんでした。安全確認済みとして扱いません。'
  } else if (!analyzing && preflight?.status === 'direct_conflict') {
    className = 'is-blocking'
    role = 'alert'
    message = `直接矛盾があります（${preflight.conflicts.length}件）。`
  } else if (!analyzing && preflight?.status === 'unknown') {
    className = 'is-blocking'
    role = 'alert'
    message = `${unknownReasonLabel(preflight.reason)}。安全確認済みとして扱いません。`
  } else if (!analyzing && preflight?.status === 'no_direct_conflict') {
    className = 'is-clear'
    message = '直接矛盾は見つかりません（全制約の充足可能性は未証明）'
  } else if (!analyzing) {
    message = '現在の制約は未診断です。'
  }

  return (
    <div className={`geometric-constraint-preflight ${className}`} role={role} aria-live="polite">
      <span>{message}</span>
      {!analyzing && preflight?.status === 'direct_conflict' && (
        <ul className="geometric-constraint-conflicts" aria-label="直接矛盾の原因">
          {preflight.conflicts.slice(0, MAX_VISIBLE_DIRECT_CONFLICTS).map((conflict) => (
            <li key={[
              conflict.conflict.kind,
              ...conflict.constraint_ids,
            ].join(':')}>
              <strong>{directConflictLabel(conflict.conflict)}</strong>
              <span>
                原因となる制約: {conflict.constraint_ids.map(shortConstraintId).join('、')}
              </span>
            </li>
          ))}
          {preflight.conflicts.length > MAX_VISIBLE_DIRECT_CONFLICTS && (
            <li>
              ほか{preflight.conflicts.length - MAX_VISIBLE_DIRECT_CONFLICTS}件の直接矛盾
            </li>
          )}
        </ul>
      )}
      {!analyzing
        && preflight?.status === 'unknown'
        && preflight.unchecked_constraint_ids.length > 0 && (
          <span>
            未確認の制約: {formatConstraintIds(
              preflight.unchecked_constraint_ids,
              MAX_VISIBLE_UNCHECKED_CONSTRAINT_IDS,
            )}
          </span>
      )}
      <button type="button" disabled={disabled || analyzing} onClick={onRetry}>
        再診断
      </button>
    </div>
  )
}

function directConflictLabel(
  conflict: Extract<
    GeometricConstraintPreflightResult,
    { status: 'direct_conflict' }
  >['conflicts'][number]['conflict'],
) {
  switch (conflict.kind) {
    case 'different_fixed_lengths':
      return `同じ辺 ${shortConstraintId(conflict.edge)} に異なる長さが指定されています`
    case 'different_fixed_angles':
      return `同じ角に異なる角度が指定されています（頂点 ${shortConstraintId(conflict.vertex)}）`
    case 'different_length_ratios':
      return '同じ辺の組に異なる長さ比が指定されています'
    case 'horizontal_and_vertical':
      return `辺 ${shortConstraintId(conflict.edge)} に水平と垂直が同時に指定されています`
    case 'equal_length_with_different_fixed_lengths':
      return '等長にした辺へ異なる固定長が指定されています'
    case 'parallel_with_fixed_non_parallel_angle':
      return '平行にした辺へ平行でない固定角が指定されています'
    case 'parallel_with_perpendicular_orientations':
      return '平行にした辺へ水平と垂直が別々に指定されています'
  }
}

function primaryEdgeId(constraint: GeometricConstraintKind) {
  switch (constraint.kind) {
    case 'fixed_length':
    case 'horizontal':
    case 'vertical':
      return constraint.edge
    case 'fixed_angle':
    case 'equal_length':
    case 'parallel':
    case 'angle_bisector':
      return constraint.first_edge
    case 'point_on_line':
      return constraint.line_edge
    case 'mirror_symmetry':
      return constraint.axis_edge
    case 'length_ratio':
      return constraint.numerator_edge
    case 'rotational_symmetry':
      return null
  }
}

function unknownReasonLabel(
  reason: Extract<GeometricConstraintPreflightResult, { status: 'unknown' }>['reason'],
) {
  switch (reason) {
    case 'work_limit_exceeded':
      return '診断の処理上限に達したため判定保留です'
    case 'solver_required_constraint_kinds':
      return '完全な制約ソルバーが必要なため判定保留です'
    case 'invalid_document_or_geometry':
      return '制約または展開図を検証できないため判定保留です'
  }
}

function shortenPresentationIds(summary: string) {
  return summary.replace(
    /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gu,
    (id) => isCanonicalNonNilUuid(id) ? shortConstraintId(id) : id,
  )
}

function shortConstraintId(id: string) {
  return `${id.slice(0, 8)}…${id.slice(-4)}`
}

function formatConstraintIds(ids: readonly string[], maximum: number) {
  const visible = ids.slice(0, maximum).map(shortConstraintId).join('、')
  const remaining = ids.length - Math.min(ids.length, maximum)
  return remaining > 0 ? `${visible}、ほか${remaining}件` : visible
}
