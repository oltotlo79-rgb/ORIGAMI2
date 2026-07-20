import { useState } from 'react'
import type {
  GeometricConstraintSolvePreview,
  GeometricConstraintDocument,
  GeometricConstraintKind,
  GeometricConstraintPreflightResult,
} from '../lib/coreClient'
import { isCanonicalNonNilUuid } from '../lib/canonicalUuid.ts'
import {
  createGeometricConstraintPresentation,
  normalizeGeometricConstraintKind,
} from '../lib/geometricConstraints'
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
const CONSTRAINT_KINDS: readonly GeometricConstraintKind['kind'][] = [
  'fixed_length', 'fixed_angle', 'horizontal', 'vertical', 'equal_length',
  'parallel', 'point_on_line', 'mirror_symmetry', 'rotational_symmetry',
  'angle_bisector', 'length_ratio',
]
type CreationField = Readonly<{
  name: string
  resource: 'edge' | 'vertex'
  ja: string
  en: string
}>
const edgeField = (name: string, ja: string, en: string): CreationField =>
  ({ name, resource: 'edge', ja, en })
const vertexField = (name: string, ja: string, en: string): CreationField =>
  ({ name, resource: 'vertex', ja, en })
const CONSTRAINT_CREATION_FIELDS: Readonly<
  Record<GeometricConstraintKind['kind'], readonly CreationField[]>
> = {
  fixed_length: [edgeField('edge', '対象線', 'Target line')],
  fixed_angle: [
    vertexField('vertex', '角の頂点', 'Angle vertex'),
    edgeField('first_edge', '1本目の線', 'First line'),
    edgeField('second_edge', '2本目の線', 'Second line'),
  ],
  horizontal: [edgeField('edge', '対象線', 'Target line')],
  vertical: [edgeField('edge', '対象線', 'Target line')],
  equal_length: [
    edgeField('first_edge', '1本目の線', 'First line'),
    edgeField('second_edge', '2本目の線', 'Second line'),
  ],
  parallel: [
    edgeField('first_edge', '1本目の線', 'First line'),
    edgeField('second_edge', '2本目の線', 'Second line'),
  ],
  point_on_line: [
    vertexField('vertex', '対象点', 'Target point'),
    edgeField('line_edge', '基準線', 'Reference line'),
  ],
  mirror_symmetry: [
    vertexField('first_vertex', '1点目', 'First point'),
    vertexField('second_vertex', '2点目', 'Second point'),
    edgeField('axis_edge', '対称軸', 'Symmetry axis'),
  ],
  rotational_symmetry: [
    vertexField('center_vertex', '回転中心', 'Rotation center'),
    vertexField('source_vertex', '元の点', 'Source point'),
    vertexField('target_vertex', '対応点', 'Target point'),
  ],
  angle_bisector: [
    vertexField('vertex', '角の頂点', 'Angle vertex'),
    edgeField('first_edge', '1本目の線', 'First line'),
    edgeField('second_edge', '2本目の線', 'Second line'),
    edgeField('bisector_edge', '二等分線', 'Bisector line'),
  ],
  length_ratio: [
    edgeField('numerator_edge', '分子側の線', 'Numerator line'),
    edgeField('denominator_edge', '分母側の線', 'Denominator line'),
  ],
}

type GeometricConstraintPanelProps = {
  document: GeometricConstraintDocument
  preflight: GeometricConstraintPreflightResult | null
  analyzing: boolean
  analysisFailed: boolean
  selectedEdgeId: string | null
  selectedVertexId?: string | null
  selectedVertexPosition?: Readonly<{ x: number; y: number }> | null
  selectedEdgeGeometry?: Readonly<{ id: string; x1: number; y1: number; x2: number; y2: number }> | null
  edges?: readonly Readonly<{ id: string }>[]
  vertices?: readonly Readonly<{ id: string }>[]
  disabled: boolean
  onAddOrientation: (orientation: 'horizontal' | 'vertical') => void
  onAddConstraint: (constraint: GeometricConstraintKind) => void
  onRemove: (constraintId: string) => void
  onSelectEdge: (edgeId: string) => void
  onRetryAnalysis: () => void
  onPreviewSolve?: (vertexId: string, x: number, y: number) => Promise<GeometricConstraintSolvePreview>
  onApplySolve?: (token: string) => Promise<boolean>
  onPreviewEdgeSolve?: (
    edgeId: string, startX: number, startY: number, endX: number, endY: number,
  ) => Promise<GeometricConstraintSolvePreview>
  localeStore?: LocaleStore
}

export function GeometricConstraintPanel({
  document,
  preflight,
  analyzing,
  analysisFailed,
  selectedEdgeId,
  selectedVertexId = null,
  selectedVertexPosition = null,
  selectedEdgeGeometry = null,
  edges = [],
  vertices = [],
  disabled,
  onAddOrientation,
  onAddConstraint,
  onRemove,
  onSelectEdge,
  onRetryAnalysis,
  onPreviewSolve,
  onApplySolve,
  onPreviewEdgeSolve,
  localeStore: localeStore_ = localeStore,
}: GeometricConstraintPanelProps) {
  const locale = useLocale(localeStore_)
  const [constraintJson, setConstraintJson] = useState('')
  const [constraintJsonInvalid, setConstraintJsonInvalid] = useState(false)
  const [creationKind, setCreationKind] =
    useState<GeometricConstraintKind['kind']>('fixed_length')
  const [creationTargets, setCreationTargets] = useState<Record<string, string>>({})
  const [creationScalar, setCreationScalar] = useState('10')
  const [creationInvalid, setCreationInvalid] = useState(false)
  const [solveX, setSolveX] = useState('')
  const [solveY, setSolveY] = useState('')
  const [solvePreview, setSolvePreview] = useState<GeometricConstraintSolvePreview | null>(null)
  const [solveError, setSolveError] = useState(false)
  const [solveBusy, setSolveBusy] = useState(false)
  const [edgeDeltaX, setEdgeDeltaX] = useState('0')
  const [edgeDeltaY, setEdgeDeltaY] = useState('0')
  const [edgeRotation, setEdgeRotation] = useState('0')
  const [edgeScale, setEdgeScale] = useState('1')
  const edgeIds = uniqueIds(edges.map(({ id }) => id), selectedEdgeId)
  const vertexIds = uniqueIds(vertices.map(({ id }) => id), selectedVertexId)
  const creationFields = CONSTRAINT_CREATION_FIELDS[creationKind]
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
      <fieldset disabled={disabled || solveBusy}>
        <legend>{localized(locale, '拘束を保った移動', 'Constraint-preserving move')}</legend>
        <label className="field">
          X (mm)
          <input
            aria-label="Solver X"
            inputMode="decimal"
            value={solveX}
            placeholder={selectedVertexPosition?.x.toString() ?? ''}
            onChange={(event) => {
              setSolveX(event.currentTarget.value)
              setSolvePreview(null)
            }}
          />
        </label>
        <label className="field">
          Y (mm)
          <input
            aria-label="Solver Y"
            inputMode="decimal"
            value={solveY}
            placeholder={selectedVertexPosition?.y.toString() ?? ''}
            onChange={(event) => {
              setSolveY(event.currentTarget.value)
              setSolvePreview(null)
            }}
          />
        </label>
        <button
          type="button"
          disabled={selectedVertexId === null || !onPreviewSolve}
          onClick={() => {
            if (!selectedVertexId || !onPreviewSolve) return
            const x = solveX === '' ? selectedVertexPosition?.x : Number(solveX)
            const y = solveY === '' ? selectedVertexPosition?.y : Number(solveY)
            if (x === undefined || y === undefined || !Number.isFinite(x) || !Number.isFinite(y)) {
              setSolveError(true)
              return
            }
            setSolveBusy(true)
            setSolveError(false)
            void onPreviewSolve(selectedVertexId, x, y)
              .then(setSolvePreview)
              .catch(() => setSolveError(true))
              .finally(() => setSolveBusy(false))
          }}
        >
          {localized(locale, 'プレビュー', 'Preview')}
        </button>
        {solvePreview && (
          <div className="geometric-constraint-solve-preview" role="status">
            <p>
              {localized(locale, '変更頂点', 'Changed vertices')}: {solvePreview.changedVertices.length}
              {' · '}{localized(locale, '反復', 'Iterations')}: {solvePreview.iterations}
              {' · '}residual: {solvePreview.maximumResidual.toExponential(2)}
            </p>
            <p>
              rank {solvePreview.rank}/{solvePreview.equationCount}
              {' · '}DOF {solvePreview.degreesOfFreedom}
              {' · '}condition {solvePreview.conditionEstimate.toExponential(2)}
              {' · '}{localized(
                locale,
                solvePreview.systemClassification === 'under_constrained'
                  ? '拘束不足'
                  : solvePreview.systemClassification === 'over_constrained'
                    ? '過剰拘束'
                    : '完全拘束',
                solvePreview.systemClassification === 'under_constrained'
                  ? 'Under-constrained'
                  : solvePreview.systemClassification === 'over_constrained'
                    ? 'Over-constrained'
                    : 'Well-constrained',
              )}
            </p>
            <svg viewBox="-2 -2 4 4" aria-label={localized(locale, '移動プレビュー', 'Move preview')}>
              {solvePreview.changedVertices.slice(0, 256).map((vertex) => (
                <circle
                  key={vertex.vertexId}
                  cx={vertex.x}
                  cy={vertex.y}
                  r="0.06"
                  className="constraint-solver-ghost"
                />
              ))}
            </svg>
            <button
              type="button"
              onClick={() => {
                if (!onApplySolve) return
                setSolveBusy(true)
                void onApplySolve(solvePreview.token)
                  .then((applied) => {
                    if (applied) setSolvePreview(null)
                    else setSolveError(true)
                  })
                  .catch(() => setSolveError(true))
                  .finally(() => setSolveBusy(false))
              }}
            >
              {localized(locale, '適用', 'Apply')}
            </button>
            <button type="button" onClick={() => setSolvePreview(null)}>
              {localized(locale, 'キャンセル', 'Cancel')}
            </button>
          </div>
        )}
        {solveError && (
          <p role="alert">
            {localized(
              locale,
              '拘束を満たす解を安全に作成できませんでした。',
              'A safe constraint solution could not be created.',
            )}
          </p>
        )}
      </fieldset>
      <fieldset disabled={disabled || solveBusy || selectedEdgeGeometry === null}>
        <legend>{localized(locale, '拘束を保った辺操作', 'Constraint-preserving edge transform')}</legend>
        {[
          ['Edge delta X', edgeDeltaX, setEdgeDeltaX],
          ['Edge delta Y', edgeDeltaY, setEdgeDeltaY],
          ['Edge rotation (degrees)', edgeRotation, setEdgeRotation],
          ['Edge length scale', edgeScale, setEdgeScale],
        ].map(([label, value, setter]) => (
          <label className="field" key={label as string}>
            {label as string}
            <input
              aria-label={label as string}
              value={value as string}
              onChange={(event) => (setter as (value: string) => void)(event.currentTarget.value)}
            />
          </label>
        ))}
        <button
          type="button"
          disabled={!onPreviewEdgeSolve || !selectedEdgeGeometry}
          onClick={() => {
            if (!onPreviewEdgeSolve || !selectedEdgeGeometry) return
            const values = [edgeDeltaX, edgeDeltaY, edgeRotation, edgeScale].map(Number)
            if (values.some((value) => !Number.isFinite(value)) || values[3]! <= 0) {
              setSolveError(true)
              return
            }
            const [dx, dy, degrees, scale] = values as [number, number, number, number]
            const centerX = (selectedEdgeGeometry.x1 + selectedEdgeGeometry.x2) / 2 + dx
            const centerY = (selectedEdgeGeometry.y1 + selectedEdgeGeometry.y2) / 2 + dy
            const radians = degrees * Math.PI / 180
            const halfX = (selectedEdgeGeometry.x2 - selectedEdgeGeometry.x1) * scale / 2
            const halfY = (selectedEdgeGeometry.y2 - selectedEdgeGeometry.y1) * scale / 2
            const rotatedX = halfX * Math.cos(radians) - halfY * Math.sin(radians)
            const rotatedY = halfX * Math.sin(radians) + halfY * Math.cos(radians)
            setSolveBusy(true)
            void onPreviewEdgeSolve(
              selectedEdgeGeometry.id,
              centerX - rotatedX, centerY - rotatedY,
              centerX + rotatedX, centerY + rotatedY,
            ).then(setSolvePreview).catch(() => setSolveError(true)).finally(() => setSolveBusy(false))
          }}
        >
          {localized(locale, '辺をプレビュー', 'Preview edge transform')}
        </button>
      </fieldset>
      {selectedEdgeId === null && (
        <p className="muted">
          {localized(
            locale,
            '水平・垂直制約を追加するには線を選択してください。',
            'Select a line before adding a horizontal or vertical constraint.',
          )}
        </p>
      )}
      <fieldset disabled={disabled}>
        <legend>{localized(locale, '制約をフォームから追加', 'Add constraint from form')}</legend>
        <label className="field">
          {localized(locale, '制約種別', 'Constraint kind')}
          <select
            value={creationKind}
            onChange={(event) => {
              setCreationKind(event.currentTarget.value as GeometricConstraintKind['kind'])
              setCreationTargets({})
              setCreationInvalid(false)
            }}
          >
            {CONSTRAINT_KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {formatLocalizedText(locale, {
                  ja: '{name}を作成',
                  en: 'Create {name}',
                }, { name: constraintKindName(kind, locale) })}
              </option>
            ))}
          </select>
        </label>
        {creationFields.map((field, index) => {
          const options = field.resource === 'edge' ? edgeIds : vertexIds
          const resourceIndex = creationFields.slice(0, index)
            .filter(({ resource }) => resource === field.resource).length
          const preferred = field.resource === 'edge' && resourceIndex === 0
            ? selectedEdgeId
            : field.resource === 'vertex' && resourceIndex === 0
              ? selectedVertexId
              : null
          const value = creationTargets[field.name]
            ?? preferred ?? options[resourceIndex] ?? options[0] ?? ''
          return (
            <label className="field" key={field.name}>
              {localized(locale, field.ja, field.en)}
              <select
                aria-label={localized(locale, field.ja, field.en)}
                value={value}
                onChange={(event) => {
                  setCreationTargets((current) => ({
                    ...current,
                    [field.name]: event.currentTarget.value,
                  }))
                  setCreationInvalid(false)
                }}
              >
                <option value="">{localized(locale, '選択してください', 'Select…')}</option>
                {options.map((id) => <option key={id} value={id}>{shortId(id)}</option>)}
              </select>
            </label>
          )
        })}
        {constraintScalar(creationKind) && (
          <label className="field">
            {localized(locale, constraintScalar(creationKind)!.ja, constraintScalar(creationKind)!.en)}
            <input
              type="number"
              step="any"
              value={creationScalar}
              aria-invalid={creationInvalid}
              onChange={(event) => {
                setCreationScalar(event.currentTarget.value)
                setCreationInvalid(false)
              }}
            />
          </label>
        )}
        <button
          type="button"
          disabled={disabled}
          onClick={() => {
            const resolved = Object.fromEntries(creationFields.map((field, index) => {
              const options = field.resource === 'edge' ? edgeIds : vertexIds
              const resourceIndex = creationFields.slice(0, index)
                .filter(({ resource }) => resource === field.resource).length
              const preferred = field.resource === 'edge' && resourceIndex === 0
                ? selectedEdgeId
                : field.resource === 'vertex' && resourceIndex === 0
                  ? selectedVertexId
                  : null
              return [field.name, creationTargets[field.name]
                ?? preferred ?? options[resourceIndex] ?? options[0] ?? '']
            }))
            const constraint = createConstraint(
              creationKind,
              resolved,
              Number(creationScalar),
            )
            if (!constraint) {
              setCreationInvalid(true)
              return
            }
            onAddConstraint(constraint)
            setCreationInvalid(false)
          }}
        >
          {localized(locale, 'フォームの制約を追加', 'Add form constraint')}
        </button>
        <p className={creationInvalid ? 'status-invalid' : 'muted'}>
          {creationInvalid
            ? localized(locale, '必要な対象と有効な数値を指定してください。', 'Choose every required target and enter a valid value.')
            : localized(locale, '対象は現在のproject要素から選択します。追加は一回のUndoで戻せます。', 'Targets come from the current project. One Undo removes the addition.')}
        </p>
      </fieldset>
      <fieldset disabled={disabled}>
        <legend>
          {localized(locale, '全11種の制約を追加', 'Add any of the 11 constraint kinds')}
        </legend>
        <label className="field">
          {localized(locale, '制約JSON', 'Constraint JSON')}
          <textarea
            value={constraintJson}
            rows={6}
            maxLength={2_048}
            aria-invalid={constraintJsonInvalid}
            placeholder={selectedEdgeId
              ? JSON.stringify({
                  kind: 'fixed_length',
                  edge: selectedEdgeId,
                  length_mm: 100,
                })
              : '{"kind":"equal_length","first_edge":"UUID","second_edge":"UUID"}'}
            onChange={(event) => {
              setConstraintJson(event.currentTarget.value)
              setConstraintJsonInvalid(false)
            }}
          />
        </label>
        <div className="property-actions">
          <button
            type="button"
            disabled={disabled}
            onClick={() => {
              let parsed: unknown
              try {
                parsed = JSON.parse(constraintJson)
              } catch {
                setConstraintJsonInvalid(true)
                return
              }
              const constraint = normalizeGeometricConstraintKind(parsed)
              if (!constraint) {
                setConstraintJsonInvalid(true)
                return
              }
              onAddConstraint(constraint)
              setConstraintJson('')
              setConstraintJsonInvalid(false)
            }}
          >
            {localized(locale, '制約を追加', 'Add constraint')}
          </button>
        </div>
        <p className={constraintJsonInvalid ? 'status-invalid' : 'muted'}>
          {constraintJsonInvalid
            ? localized(
                locale,
                '制約JSONの種別、ID、値、またはfieldが不正です。',
                'The constraint kind, IDs, values, or fields are invalid.',
              )
            : localized(
                locale,
                'fixed_length / fixed_angle / horizontal / vertical / equal_length / parallel / point_on_line / mirror_symmetry / rotational_symmetry / angle_bisector / length_ratio を厳格JSONで指定します。',
                'Use strict JSON for fixed_length, fixed_angle, horizontal, vertical, equal_length, parallel, point_on_line, mirror_symmetry, rotational_symmetry, angle_bisector, or length_ratio.',
              )}
        </p>
      </fieldset>

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

function uniqueIds(values: readonly string[], preferred: string | null) {
  return [...new Set(preferred ? [preferred, ...values] : values)]
    .filter(isCanonicalNonNilUuid)
}

function shortId(id: string) {
  return `${id.slice(0, 8)}…${id.slice(-4)}`
}

function constraintKindName(kind: GeometricConstraintKind['kind'], locale: Locale) {
  const names: Record<GeometricConstraintKind['kind'], readonly [string, string]> = {
    fixed_length: ['長さ固定', 'Fixed length'],
    fixed_angle: ['角度固定', 'Fixed angle'],
    horizontal: ['水平', 'Horizontal'],
    vertical: ['垂直', 'Vertical'],
    equal_length: ['等長', 'Equal length'],
    parallel: ['平行', 'Parallel'],
    point_on_line: ['点を線上に配置', 'Point on line'],
    mirror_symmetry: ['線対称', 'Mirror symmetry'],
    rotational_symmetry: ['回転対称', 'Rotational symmetry'],
    angle_bisector: ['角の二等分', 'Angle bisector'],
    length_ratio: ['長さの比', 'Length ratio'],
  }
  return localized(locale, ...names[kind])
}

function constraintScalar(kind: GeometricConstraintKind['kind']) {
  if (kind === 'fixed_length') return { ja: '長さ (mm)', en: 'Length (mm)' }
  if (kind === 'fixed_angle' || kind === 'rotational_symmetry') {
    return { ja: '角度 (度)', en: 'Angle (degrees)' }
  }
  if (kind === 'length_ratio') return { ja: '長さの比', en: 'Length ratio' }
  return null
}

function createConstraint(
  kind: GeometricConstraintKind['kind'],
  target: Readonly<Record<string, string>>,
  scalar: number,
): GeometricConstraintKind | null {
  const raw: unknown = (() => {
    switch (kind) {
      case 'fixed_length':
        return { kind, edge: target.edge, length_mm: scalar }
      case 'fixed_angle':
        return { kind, vertex: target.vertex, first_edge: target.first_edge,
          second_edge: target.second_edge, angle_degrees: scalar }
      case 'horizontal':
      case 'vertical':
        return { kind, edge: target.edge }
      case 'equal_length':
      case 'parallel':
        return { kind, first_edge: target.first_edge, second_edge: target.second_edge }
      case 'point_on_line':
        return { kind, vertex: target.vertex, line_edge: target.line_edge }
      case 'mirror_symmetry':
        return { kind, first_vertex: target.first_vertex,
          second_vertex: target.second_vertex, axis_edge: target.axis_edge }
      case 'rotational_symmetry':
        return { kind, center_vertex: target.center_vertex,
          source_vertex: target.source_vertex, target_vertex: target.target_vertex,
          angle_degrees: scalar }
      case 'angle_bisector':
        return { kind, vertex: target.vertex, first_edge: target.first_edge,
          second_edge: target.second_edge, bisector_edge: target.bisector_edge }
      case 'length_ratio':
        return { kind, numerator_edge: target.numerator_edge,
          denominator_edge: target.denominator_edge, ratio: scalar }
    }
  })()
  return normalizeGeometricConstraintKind(raw)
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
