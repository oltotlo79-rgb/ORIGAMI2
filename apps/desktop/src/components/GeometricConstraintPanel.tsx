import type {
  GeometricConstraintDocument,
  GeometricConstraintKind,
  GeometricConstraintPreflightResult,
} from '../lib/coreClient'
import { isCanonicalNonNilUuid } from '../lib/canonicalUuid.ts'
import { createGeometricConstraintPresentation } from '../lib/geometricConstraints'
import {
  formatLocalizedText,
  localeStore,
  useLocale,
  type Locale,
  type LocaleStore,
} from '../lib/i18n.ts'

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
  localeStore?: LocaleStore
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
  localeStore: localeStore_ = localeStore,
}: GeometricConstraintPanelProps) {
  const locale = useLocale(localeStore_)
  return (
    <section className="geometric-constraints" aria-labelledby="geometric-constraints-title">
      <div className="geometric-constraints-heading">
        <h2 id="geometric-constraints-title">
          {localized(locale, '幾何制約', 'Geometric constraints')}
        </h2>
        <span>
          {formatLocalizedText(locale, {
            ja: '{count}件',
            en: '{count} constraints',
          }, { count: document.constraints.length })}
        </span>
      </div>

      <div className="property-actions geometric-constraint-add-actions">
        <button
          type="button"
          disabled={disabled || selectedEdgeId === null}
          onClick={() => onAddOrientation('horizontal')}
        >
          {localized(
            locale,
            '選択線を水平に制約',
            'Constrain selected line horizontally',
          )}
        </button>
        <button
          type="button"
          disabled={disabled || selectedEdgeId === null}
          onClick={() => onAddOrientation('vertical')}
        >
          {localized(
            locale,
            '選択線を垂直に制約',
            'Constrain selected line vertically',
          )}
        </button>
      </div>
      {selectedEdgeId === null && (
        <p className="muted">
          {localized(
            locale,
            '水平・垂直制約を追加するには線を選択してください。',
            'Select a line before adding a horizontal or vertical constraint.',
          )}
        </p>
      )}

      <ConstraintPreflightStatus
        preflight={preflight}
        analyzing={analyzing}
        failed={analysisFailed}
        disabled={disabled}
        onRetry={onRetryAnalysis}
        locale={locale}
      />

      {document.constraints.length === 0 ? (
        <p className="muted">
          {localized(locale, '制約はまだありません。', 'No constraints yet.')}
        </p>
      ) : (
        <>
          <ol className="geometric-constraint-list">
            {document.constraints.slice(0, MAX_VISIBLE_CONSTRAINTS).map((record) => {
              const edge = primaryEdgeId(record.constraint)
              const presentation = createGeometricConstraintPresentation(
                record,
                locale,
              )
              const displayName = presentation?.displayName
                ?? localized(locale, '不明な制約', 'Unknown constraint')
              const targetSummary = presentation?.targetSummary
                ? shortenPresentationIds(presentation.targetSummary, locale)
                : localized(locale, '対象を確認できません', 'Target unavailable')
              return (
                <li key={record.id}>
                  <div>
                    <strong>{displayName}</strong>
                    <span>{targetSummary}</span>
                  </div>
                  <div className="geometric-constraint-row-actions">
                    {edge && (
                      <button
                        type="button"
                        disabled={disabled}
                        onClick={() => onSelectEdge(edge)}
                      >
                        {localized(locale, '対象を選択', 'Select target')}
                      </button>
                    )}
                    <button
                      type="button"
                      className="danger"
                      disabled={disabled}
                      aria-label={formatLocalizedText(locale, {
                        ja: '{name}制約を削除',
                        en: 'Delete {name} constraint',
                      }, { name: displayName })}
                      onClick={() => onRemove(record.id)}
                    >
                      {localized(locale, '削除', 'Delete')}
                    </button>
                  </div>
                </li>
              )
            })}
          </ol>
          {document.constraints.length > MAX_VISIBLE_CONSTRAINTS && (
            <p className="muted">
              {formatLocalizedText(locale, {
                ja: '先頭{visible}件を表示しています。残り{remaining}件は、表示中の制約を削除すると順に表示されます。',
                en: 'Showing the first {visible} constraints. The remaining {remaining} appear as displayed constraints are deleted.',
              }, {
                visible: MAX_VISIBLE_CONSTRAINTS,
                remaining:
                  document.constraints.length - MAX_VISIBLE_CONSTRAINTS,
              })}
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
  locale,
}: {
  preflight: GeometricConstraintPreflightResult | null
  analyzing: boolean
  failed: boolean
  disabled: boolean
  onRetry: () => void
  locale: Locale
}) {
  let className = 'is-pending'
  let role: 'status' | 'alert' = 'status'
  let message = localized(
    locale,
    '制約を診断しています…',
    'Analyzing constraints…',
  )

  if (!analyzing && failed) {
    className = 'is-blocking'
    role = 'alert'
    message = localized(
      locale,
      '制約診断を完了できませんでした。安全確認済みとして扱いません。',
      'Constraint analysis could not be completed. Do not treat the constraints as safety-verified.',
    )
  } else if (!analyzing && preflight?.status === 'direct_conflict') {
    className = 'is-blocking'
    role = 'alert'
    message = formatLocalizedText(locale, {
      ja: '直接矛盾があります（{count}件）。',
      en: '{count} direct conflicts found.',
    }, { count: preflight.conflicts.length })
  } else if (!analyzing && preflight?.status === 'unknown') {
    className = 'is-blocking'
    role = 'alert'
    message = formatLocalizedText(locale, {
      ja: '{reason}。安全確認済みとして扱いません。',
      en: '{reason} Do not treat the constraints as safety-verified.',
    }, { reason: unknownReasonLabel(preflight.reason, locale) })
  } else if (!analyzing && preflight?.status === 'no_direct_conflict') {
    className = 'is-clear'
    message = localized(
      locale,
      '直接矛盾は見つかりません（全制約の充足可能性は未証明）',
      'No direct conflicts found (satisfiability of all constraints is not proven)',
    )
  } else if (!analyzing) {
    message = localized(
      locale,
      '現在の制約は未診断です。',
      'The current constraints have not been analyzed.',
    )
  }

  return (
    <div
      className={`geometric-constraint-preflight ${className}`}
      role={role}
      aria-live={role === 'alert' ? 'assertive' : 'polite'}
      aria-atomic="true"
    >
      <span>{message}</span>
      {!analyzing && preflight?.status === 'direct_conflict' && (
        <ul
          className="geometric-constraint-conflicts"
          aria-label={localized(
            locale,
            '直接矛盾の原因',
            'Direct conflict causes',
          )}
        >
          {preflight.conflicts.slice(0, MAX_VISIBLE_DIRECT_CONFLICTS).map((conflict) => (
            <li key={[
              conflict.conflict.kind,
              ...conflict.constraint_ids,
            ].join(':')}>
              <strong>{directConflictLabel(conflict.conflict, locale)}</strong>
              <span>
                {formatLocalizedText(locale, {
                  ja: '原因となる制約: {ids}',
                  en: 'Causing constraints: {ids}',
                }, {
                  ids: conflict.constraint_ids
                    .map((id) => shortConstraintId(id, locale))
                    .join(locale === 'ja' ? '、' : ', '),
                })}
              </span>
            </li>
          ))}
          {preflight.conflicts.length > MAX_VISIBLE_DIRECT_CONFLICTS && (
            <li>
              {formatLocalizedText(locale, {
                ja: 'ほか{count}件の直接矛盾',
                en: '{count} more direct conflicts',
              }, {
                count:
                  preflight.conflicts.length - MAX_VISIBLE_DIRECT_CONFLICTS,
              })}
            </li>
          )}
        </ul>
      )}
      {!analyzing
        && preflight?.status === 'unknown'
        && preflight.unchecked_constraint_ids.length > 0 && (
          <span>
            {formatLocalizedText(locale, {
              ja: '未確認の制約: {ids}',
              en: 'Unchecked constraints: {ids}',
            }, {
              ids: formatConstraintIds(
                preflight.unchecked_constraint_ids,
                MAX_VISIBLE_UNCHECKED_CONSTRAINT_IDS,
                locale,
              ),
            })}
          </span>
      )}
      <button type="button" disabled={disabled || analyzing} onClick={onRetry}>
        {localized(locale, '再診断', 'Analyze again')}
      </button>
    </div>
  )
}

function directConflictLabel(
  conflict: Extract<
    GeometricConstraintPreflightResult,
    { status: 'direct_conflict' }
  >['conflicts'][number]['conflict'],
  locale: Locale,
) {
  switch (conflict.kind) {
    case 'different_fixed_lengths':
      return formatLocalizedText(locale, {
        ja: '同じ辺 {edge} に異なる長さが指定されています',
        en: 'Different lengths are assigned to the same edge {edge}',
      }, { edge: shortConstraintId(conflict.edge, locale) })
    case 'different_fixed_angles':
      return formatLocalizedText(locale, {
        ja: '同じ角に異なる角度が指定されています（頂点 {vertex}）',
        en: 'Different angles are assigned to the same angle (vertex {vertex})',
      }, { vertex: shortConstraintId(conflict.vertex, locale) })
    case 'different_length_ratios':
      return localized(
        locale,
        '同じ辺の組に異なる長さ比が指定されています',
        'Different length ratios are assigned to the same pair of edges',
      )
    case 'horizontal_and_vertical':
      return formatLocalizedText(locale, {
        ja: '辺 {edge} に水平と垂直が同時に指定されています',
        en: 'Edge {edge} is constrained as both horizontal and vertical',
      }, { edge: shortConstraintId(conflict.edge, locale) })
    case 'equal_length_with_different_fixed_lengths':
      return localized(
        locale,
        '等長にした辺へ異なる固定長が指定されています',
        'Edges constrained to equal length have different fixed lengths',
      )
    case 'parallel_with_fixed_non_parallel_angle':
      return localized(
        locale,
        '平行にした辺へ平行でない固定角が指定されています',
        'Parallel edges have a fixed angle that is not parallel',
      )
    case 'parallel_with_perpendicular_orientations':
      return localized(
        locale,
        '平行にした辺へ水平と垂直が別々に指定されています',
        'Parallel edges are separately constrained as horizontal and vertical',
      )
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
  locale: Locale,
) {
  switch (reason) {
    case 'work_limit_exceeded':
      return localized(
        locale,
        '診断の処理上限に達したため判定保留です',
        'Indeterminate because the analysis work limit was reached.',
      )
    case 'solver_required_constraint_kinds':
      return localized(
        locale,
        '完全な制約ソルバーが必要なため判定保留です',
        'Indeterminate because a complete constraint solver is required.',
      )
    case 'invalid_document_or_geometry':
      return localized(
        locale,
        '制約または展開図を検証できないため判定保留です',
        'Indeterminate because the constraints or crease pattern could not be validated.',
      )
  }
}

function shortenPresentationIds(summary: string, locale: Locale) {
  return summary.replace(
    /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gu,
    (id) => shortConstraintId(id, locale),
  )
}

function shortConstraintId(id: string, locale: Locale) {
  return isCanonicalNonNilUuid(id)
    ? `${id.slice(0, 8)}…${id.slice(-4)}`
    : localized(locale, '不正な識別子', 'invalid identifier')
}

function formatConstraintIds(
  ids: readonly string[],
  maximum: number,
  locale: Locale,
) {
  const visible = ids
    .slice(0, maximum)
    .map((id) => shortConstraintId(id, locale))
    .join(locale === 'ja' ? '、' : ', ')
  const remaining = ids.length - Math.min(ids.length, maximum)
  return remaining > 0
    ? formatLocalizedText(locale, {
      ja: '{visible}、ほか{remaining}件',
      en: '{visible}, {remaining} more',
    }, { visible, remaining })
    : visible
}

function localized(locale: Locale, ja: string, en: string): string {
  return locale === 'en' ? en : ja
}
