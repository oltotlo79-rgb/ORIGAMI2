import { useState } from 'react'
import type {
  GeometricConstraintSolvePreview,
  GeometricConstraintDocument,
  GeometricConstraintKind,
  GeometricConstraintPreflightResult,
  GeometricConstraintSemanticMus,
} from '../lib/coreClient'
import { isCanonicalNonNilUuid } from '../lib/canonicalUuid.ts'
import {
  createGeometricConstraintPresentation,
  normalizeGeometricConstraintKind,
} from '../lib/geometricConstraints'
import {
  GEOMETRIC_CONSTRAINT_PANEL_TEXT as TEXT,
  type GeometricConstraintCreationFieldLabel,
} from '../lib/geometricConstraintPanelText.ts'
import {
  buildGeometricConstraintSemanticMusCertifiedViewModel,
} from '../lib/geometricConstraintSemanticMusViewModel.ts'
import {
  formatLocalizedText,
  localeStore,
  selectLocalizedText,
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
  label: GeometricConstraintCreationFieldLabel
}>
const edgeField = (
  name: string,
  label: GeometricConstraintCreationFieldLabel,
): CreationField => ({ name, resource: 'edge', label })
const vertexField = (
  name: string,
  label: GeometricConstraintCreationFieldLabel,
): CreationField => ({ name, resource: 'vertex', label })
const CONSTRAINT_CREATION_FIELDS: Readonly<
  Record<GeometricConstraintKind['kind'], readonly CreationField[]>
> = {
  fixed_length: [edgeField('edge', 'targetLine')],
  fixed_angle: [
    vertexField('vertex', 'angleVertex'),
    edgeField('first_edge', 'firstLine'),
    edgeField('second_edge', 'secondLine'),
  ],
  horizontal: [edgeField('edge', 'targetLine')],
  vertical: [edgeField('edge', 'targetLine')],
  equal_length: [
    edgeField('first_edge', 'firstLine'),
    edgeField('second_edge', 'secondLine'),
  ],
  parallel: [
    edgeField('first_edge', 'firstLine'),
    edgeField('second_edge', 'secondLine'),
  ],
  point_on_line: [
    vertexField('vertex', 'targetPoint'),
    edgeField('line_edge', 'referenceLine'),
  ],
  mirror_symmetry: [
    vertexField('first_vertex', 'firstPoint'),
    vertexField('second_vertex', 'secondPoint'),
    edgeField('axis_edge', 'symmetryAxis'),
  ],
  rotational_symmetry: [
    vertexField('center_vertex', 'rotationCenter'),
    vertexField('source_vertex', 'sourcePoint'),
    vertexField('target_vertex', 'correspondingPoint'),
  ],
  angle_bisector: [
    vertexField('vertex', 'angleVertex'),
    edgeField('first_edge', 'firstLine'),
    edgeField('second_edge', 'secondLine'),
    edgeField('bisector_edge', 'bisectorLine'),
  ],
  length_ratio: [
    edgeField('numerator_edge', 'numeratorLine'),
    edgeField('denominator_edge', 'denominatorLine'),
  ],
}

type GeometricConstraintPanelProps = {
  document: GeometricConstraintDocument
  preflight: GeometricConstraintPreflightResult | null
  semanticMus?: GeometricConstraintSemanticMus | null
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
  onPreviewExpressionSolve?: () => Promise<GeometricConstraintSolvePreview>
  localeStore?: LocaleStore
}

export function GeometricConstraintPanel({
  document,
  preflight,
  semanticMus = null,
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
  onPreviewExpressionSolve,
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
          {selectLocalizedText(locale, TEXT.title)}
        </h2>
        <span>
          {formatLocalizedText(
            locale,
            TEXT.constraintCount,
            { count: document.constraints.length },
          )}
        </span>
      </div>

      <div className="property-actions geometric-constraint-add-actions">
        <button
          type="button"
          disabled={disabled || selectedEdgeId === null}
          onClick={() => onAddOrientation('horizontal')}
        >
          {selectLocalizedText(locale, TEXT.addHorizontal)}
        </button>
        <button
          type="button"
          disabled={disabled || selectedEdgeId === null}
          onClick={() => onAddOrientation('vertical')}
        >
          {selectLocalizedText(locale, TEXT.addVertical)}
        </button>
      </div>
      <fieldset disabled={disabled || solveBusy}>
        <legend>{selectLocalizedText(locale, TEXT.moveLegend)}</legend>
        <label className="field">
          {selectLocalizedText(locale, TEXT.xAxis)}
          <input
            aria-label={selectLocalizedText(locale, TEXT.solveXAria)}
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
          {selectLocalizedText(locale, TEXT.yAxis)}
          <input
            aria-label={selectLocalizedText(locale, TEXT.solveYAria)}
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
          {selectLocalizedText(locale, TEXT.preview)}
        </button>
        {solvePreview && (
          <div className="geometric-constraint-solve-preview" role="status">
            <p>
              {selectLocalizedText(locale, TEXT.changedVertices)}:{' '}
              {solvePreview.changedVertices.length}
              {selectLocalizedText(locale, TEXT.detailSeparator)}
              {selectLocalizedText(locale, TEXT.iterations)}:{' '}
              {solvePreview.iterations}
              {selectLocalizedText(locale, TEXT.detailSeparator)}
              {selectLocalizedText(locale, TEXT.residual)}:{' '}
              {solvePreview.maximumResidual.toExponential(2)}
            </p>
            <p>
              {selectLocalizedText(locale, TEXT.rank)}{' '}
              {solvePreview.rank}/{solvePreview.equationCount}
              {selectLocalizedText(locale, TEXT.detailSeparator)}
              {selectLocalizedText(locale, TEXT.degreesOfFreedom)}{' '}
              {solvePreview.degreesOfFreedom}
              {selectLocalizedText(locale, TEXT.detailSeparator)}
              {selectLocalizedText(locale, TEXT.condition)}{' '}
              {solvePreview.conditionEstimate.toExponential(2)}
              {selectLocalizedText(locale, TEXT.detailSeparator)}
              {selectLocalizedText(
                locale,
                TEXT.systemClassifications[
                  solvePreview.systemClassification === 'under_constrained'
                    ? 'under_constrained'
                    : solvePreview.systemClassification === 'over_constrained'
                      ? 'over_constrained'
                      : 'well_constrained'
                ],
              )}
            </p>
            {solvePreview.exactSatisfaction && (
              <p className="geometric-constraint-exact-satisfaction">
                {formatLocalizedText(
                  locale,
                  TEXT.exactSatisfaction,
                  {
                    constraintCount:
                      solvePreview.exactSatisfaction.constraintCount,
                    equationCount:
                      solvePreview.exactSatisfaction.equationCount,
                    scope: selectLocalizedText(
                      locale,
                      solvePreview.exactSatisfaction
                        .replayableAcrossRuntimes
                        ? TEXT.deterministicReplayableScope
                        : TEXT.currentRuntimeFallbackScope,
                    ),
                  },
                )}
              </p>
            )}
            <svg
              viewBox="-2 -2 4 4"
              aria-label={selectLocalizedText(locale, TEXT.movePreview)}
            >
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
              {selectLocalizedText(locale, TEXT.apply)}
            </button>
            <button type="button" onClick={() => setSolvePreview(null)}>
              {selectLocalizedText(locale, TEXT.cancel)}
            </button>
          </div>
        )}
        {solveError && (
          <p role="alert">
            {selectLocalizedText(locale, TEXT.solveError)}
          </p>
        )}
      </fieldset>
      <fieldset disabled={disabled || solveBusy || selectedEdgeGeometry === null}>
        <legend>
          {selectLocalizedText(locale, TEXT.edgeTransformLegend)}
        </legend>
        {([
          [TEXT.edgeDeltaX, edgeDeltaX, setEdgeDeltaX],
          [TEXT.edgeDeltaY, edgeDeltaY, setEdgeDeltaY],
          [TEXT.edgeRotation, edgeRotation, setEdgeRotation],
          [TEXT.edgeLengthScale, edgeScale, setEdgeScale],
        ] as const).map(([label, value, setter]) => (
          <label className="field" key={label.en}>
            {selectLocalizedText(locale, label)}
            <input
              aria-label={selectLocalizedText(locale, label)}
              value={value}
              onChange={(event) => setter(event.currentTarget.value)}
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
          {selectLocalizedText(locale, TEXT.previewEdgeTransform)}
        </button>
      </fieldset>
      <button
        type="button"
        disabled={disabled || solveBusy || !onPreviewExpressionSolve}
        onClick={() => {
          if (!onPreviewExpressionSolve) return
          setSolveBusy(true)
          setSolveError(false)
          void onPreviewExpressionSolve()
            .then(setSolvePreview)
            .catch(() => setSolveError(true))
            .finally(() => setSolveBusy(false))
        }}
      >
        {selectLocalizedText(locale, TEXT.reevaluateSavedExpressions)}
      </button>
      <p className="muted">
        {selectLocalizedText(locale, TEXT.references)}
      </p>
      {selectedEdgeId === null && (
        <p className="muted">
          {selectLocalizedText(locale, TEXT.selectEdgeHint)}
        </p>
      )}
      <fieldset disabled={disabled}>
        <legend>{selectLocalizedText(locale, TEXT.creationLegend)}</legend>
        <label className="field">
          {selectLocalizedText(locale, TEXT.constraintKind)}
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
                {formatLocalizedText(
                  locale,
                  TEXT.createKind,
                  { name: constraintKindName(kind, locale) },
                )}
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
              {selectLocalizedText(
                locale,
                TEXT.creationFieldLabels[field.label],
              )}
              <select
                aria-label={selectLocalizedText(
                  locale,
                  TEXT.creationFieldLabels[field.label],
                )}
                value={value}
                onChange={(event) => {
                  const selected = event.currentTarget.value
                  setCreationTargets((current) => ({
                    ...current,
                    [field.name]: selected,
                  }))
                  setCreationInvalid(false)
                }}
              >
                <option value="">
                  {selectLocalizedText(locale, TEXT.selectPrompt)}
                </option>
                {options.map((id) => <option key={id} value={id}>{shortId(id)}</option>)}
              </select>
            </label>
          )
        })}
        {constraintScalar(creationKind) && (
          <label className="field">
            {selectLocalizedText(locale, constraintScalar(creationKind)!)}
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
          {selectLocalizedText(locale, TEXT.addFormConstraint)}
        </button>
        <p className={creationInvalid ? 'status-invalid' : 'muted'}>
          {creationInvalid
            ? selectLocalizedText(locale, TEXT.creationInvalid)
            : selectLocalizedText(locale, TEXT.creationHint)}
        </p>
      </fieldset>
      <fieldset disabled={disabled}>
        <legend>
          {selectLocalizedText(locale, TEXT.allKindsLegend)}
        </legend>
        <label className="field">
          {selectLocalizedText(locale, TEXT.constraintJson)}
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
              : selectLocalizedText(
                  locale,
                  TEXT.constraintJsonPlaceholder,
                )}
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
            {selectLocalizedText(locale, TEXT.addConstraint)}
          </button>
        </div>
        <p className={constraintJsonInvalid ? 'status-invalid' : 'muted'}>
          {constraintJsonInvalid
            ? selectLocalizedText(locale, TEXT.jsonInvalid)
            : selectLocalizedText(locale, TEXT.jsonHint)}
        </p>
      </fieldset>

      <ConstraintPreflightStatus
        preflight={preflight}
        semanticMus={semanticMus}
        analyzing={analyzing}
        failed={analysisFailed}
        disabled={disabled}
        onRetry={onRetryAnalysis}
        locale={locale}
      />

      {document.constraints.length === 0 ? (
        <p className="muted">
          {selectLocalizedText(locale, TEXT.noConstraints)}
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
                ?? selectLocalizedText(locale, TEXT.unknownConstraint)
              const targetSummary = presentation?.targetSummary
                ? shortenPresentationIds(presentation.targetSummary, locale)
                : selectLocalizedText(locale, TEXT.targetUnavailable)
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
                        {selectLocalizedText(locale, TEXT.selectTarget)}
                      </button>
                    )}
                    <button
                      type="button"
                      className="danger"
                      disabled={disabled}
                      aria-label={formatLocalizedText(
                        locale,
                        TEXT.deleteConstraint,
                        { name: displayName },
                      )}
                      onClick={() => onRemove(record.id)}
                    >
                      {selectLocalizedText(locale, TEXT.delete)}
                    </button>
                  </div>
                </li>
              )
            })}
          </ol>
          {document.constraints.length > MAX_VISIBLE_CONSTRAINTS && (
            <p className="muted">
              {formatLocalizedText(
                locale,
                TEXT.constraintListTruncated,
                {
                  visible: MAX_VISIBLE_CONSTRAINTS,
                  remaining:
                    document.constraints.length - MAX_VISIBLE_CONSTRAINTS,
                },
              )}
            </p>
          )}
        </>
      )}
    </section>
  )
}

function ConstraintPreflightStatus({
  preflight,
  semanticMus,
  analyzing,
  failed,
  disabled,
  onRetry,
  locale,
}: {
  preflight: GeometricConstraintPreflightResult | null
  semanticMus: GeometricConstraintSemanticMus | null
  analyzing: boolean
  failed: boolean
  disabled: boolean
  onRetry: () => void
  locale: Locale
}) {
  let className = 'is-pending'
  let role: 'status' | 'alert' = 'status'
  let message = selectLocalizedText(locale, TEXT.analyzing)

  if (!analyzing && failed) {
    className = 'is-blocking'
    role = 'alert'
    message = selectLocalizedText(locale, TEXT.analysisFailed)
  } else if (!analyzing && preflight?.status === 'direct_conflict') {
    className = 'is-blocking'
    role = 'alert'
    message = formatLocalizedText(
      locale,
      TEXT.directConflictCount,
      { count: preflight.conflicts.length },
    )
  } else if (!analyzing && preflight?.status === 'unknown') {
    className = 'is-blocking'
    role = 'alert'
    message = formatLocalizedText(
      locale,
      TEXT.unknownStatus,
      { reason: unknownReasonLabel(preflight.reason, locale) },
    )
  } else if (!analyzing && preflight?.status === 'proven_satisfiable') {
    className = 'is-clear'
    message = formatLocalizedText(
      locale,
      TEXT.provenSatisfiable,
      {
        constraintCount: preflight.constraint_count,
        equationCount: preflight.equation_count,
        scope: selectLocalizedText(
          locale,
          preflight.replayable_across_runtimes
            ? TEXT.deterministicReplayableScope
            : TEXT.currentRuntimeFallbackScope,
        ),
      },
    )
  } else if (!analyzing && preflight?.status === 'no_direct_conflict') {
    className = 'is-clear'
    message = selectLocalizedText(locale, TEXT.noDirectConflict)
  } else if (!analyzing) {
    message = selectLocalizedText(locale, TEXT.unanalyzed)
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
        <>
          <ul
            className="geometric-constraint-conflicts"
            aria-label={selectLocalizedText(
              locale,
              TEXT.directConflictCauses,
            )}
          >
            {preflight.conflicts.slice(0, MAX_VISIBLE_DIRECT_CONFLICTS).map((conflict) => (
              <li key={[
                conflict.conflict.kind,
                ...conflict.constraint_ids,
              ].join(':')}>
                <strong>{directConflictLabel(conflict.conflict, locale)}</strong>
                <span>
                  {formatLocalizedText(
                    locale,
                    TEXT.causingConstraints,
                    {
                      ids: conflict.constraint_ids
                        .map((id) => shortConstraintId(id, locale))
                        .join(selectLocalizedText(
                          locale,
                          TEXT.idListSeparator,
                        )),
                    },
                  )}
                </span>
              </li>
            ))}
            {preflight.conflicts.length > MAX_VISIBLE_DIRECT_CONFLICTS && (
              <li>
                {formatLocalizedText(
                  locale,
                  TEXT.additionalDirectConflicts,
                  {
                    count:
                      preflight.conflicts.length - MAX_VISIBLE_DIRECT_CONFLICTS,
                  },
                )}
              </li>
            )}
          </ul>
          <BoundedDirectMusStatus
            result={preflight.bounded_direct_mus}
            locale={locale}
          />
          <SemanticMusStatus result={semanticMus} locale={locale} />
        </>
      )}
      {!analyzing
        && preflight?.status === 'unknown'
        && preflight.unchecked_constraint_ids.length > 0 && (
          <span>
            {formatLocalizedText(
              locale,
              TEXT.uncheckedConstraints,
              {
                ids: formatConstraintIds(
                  preflight.unchecked_constraint_ids,
                  MAX_VISIBLE_UNCHECKED_CONSTRAINT_IDS,
                  locale,
                ),
              },
            )}
          </span>
      )}
      <button type="button" disabled={disabled || analyzing} onClick={onRetry}>
        {selectLocalizedText(locale, TEXT.analyzeAgain)}
      </button>
    </div>
  )
}

function BoundedDirectMusStatus({
  result,
  locale,
}: {
  result: Extract<
    GeometricConstraintPreflightResult,
    { status: 'direct_conflict' }
  >['bounded_direct_mus']
  locale: Locale
}) {
  if (result.status === 'proven_unsatisfiable') {
    return (
      <p className="geometric-constraint-bounded-mus">
        {formatLocalizedText(
          locale,
          TEXT.boundedMusProven,
          {
            count: result.constraint_ids.length,
            calls: result.oracle_calls,
            ids: result.constraint_ids
              .map((id) => shortConstraintId(id, locale))
              .join(selectLocalizedText(locale, TEXT.idListSeparator)),
          },
        )}
      </p>
    )
  }
  const label = result.reason === 'constraint_limit_exceeded'
    ? TEXT.boundedMusConstraintLimit
    : result.reason === 'cancelled'
      ? TEXT.boundedMusCancelled
      : result.reason === 'deadline_reached'
        ? TEXT.boundedMusDeadlineReached
        : TEXT.boundedMusIncomplete
  return (
    <p className="geometric-constraint-bounded-mus">
      {selectLocalizedText(locale, label)}
    </p>
  )
}

function SemanticMusStatus({
  result,
  locale,
}: {
  result: GeometricConstraintSemanticMus | null
  locale: Locale
}) {
  let detail: string
  if (result === null) {
    detail = selectLocalizedText(locale, TEXT.semanticMusLegacyUnavailable)
  } else if (result.status === 'certified') {
    const view =
      buildGeometricConstraintSemanticMusCertifiedViewModel(result)
    detail = view === null
      ? selectLocalizedText(locale, TEXT.semanticMusLegacyUnavailable)
      : formatLocalizedText(
          locale,
          TEXT.semanticMusCertified,
          {
            count: view.constraintCount,
            calls: view.directOracleCalls,
            checks: view.deletionWitnessChecks,
            work: view.deletionWitnessWork,
            current: view.currentAssignmentWitnessCount,
            axis: view.axisExactificationWitnessCount,
            constructive: view.singleConstraintConstructiveWitnessCount,
            pairConstructive: view.pairConstraintConstructiveWitnessCount,
            pairAlgebraic: view.pairConstraintAlgebraicWitnessCount,
            lengthConstructive: view.lengthConstraintConstructiveWitnessCount,
            zeroClosure: view.zeroLengthClosureConstructiveWitnessCount,
            scope: selectLocalizedText(
              locale,
              view.replayableAcrossRuntimes
                ? TEXT.deterministicReplayableScope
                : TEXT.currentRuntimeFallbackScope,
            ),
            ids: view.constraintIds
              .map((id) => shortConstraintId(id, locale))
              .join(selectLocalizedText(locale, TEXT.idListSeparator)),
          },
        )
  } else {
    const reason = selectLocalizedText(
      locale,
      TEXT.semanticMusUnknownReasonLabels[result.reason],
    )
    detail = result.direct_core_constraint_ids.length > 0
      ? formatLocalizedText(
          locale,
          TEXT.semanticMusUnknownWithCore,
          {
            count: result.direct_core_constraint_ids.length,
            reason,
            certified: result.certified_deletion_witnesses,
            checks: result.deletion_witness_checks,
            work: result.deletion_witness_work,
            ids: result.direct_core_constraint_ids
              .map((id) => shortConstraintId(id, locale))
              .join(selectLocalizedText(locale, TEXT.idListSeparator)),
          },
        )
      : formatLocalizedText(
          locale,
          TEXT.semanticMusUnknownWithoutCore,
          {
            reason,
            calls: result.direct_oracle_calls,
          },
        )
  }
  return (
    <section
      className="geometric-constraint-semantic-mus"
      aria-label={selectLocalizedText(locale, TEXT.semanticMusHeading)}
    >
      <strong>{selectLocalizedText(locale, TEXT.semanticMusHeading)}</strong>
      <p>{detail}</p>
      <p className="muted">
        {selectLocalizedText(locale, TEXT.semanticMusNoAuthority)}
      </p>
    </section>
  )
}

function directConflictLabel(
  conflict: Extract<
    GeometricConstraintPreflightResult,
    { status: 'direct_conflict' }
  >['conflicts'][number]['conflict'],
  locale: Locale,
) {
  const label = TEXT.directConflictLabels[conflict.kind]
  switch (conflict.kind) {
    case 'different_fixed_lengths':
    case 'horizontal_and_vertical':
      return formatLocalizedText(
        locale,
        label,
        { edge: shortConstraintId(conflict.edge, locale) },
      )
    case 'different_fixed_angles':
      return formatLocalizedText(
        locale,
        label,
        { vertex: shortConstraintId(conflict.vertex, locale) },
      )
    default:
      return selectLocalizedText(locale, label)
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
  return selectLocalizedText(locale, TEXT.constraintKindNames[kind])
}

function constraintScalar(kind: GeometricConstraintKind['kind']) {
  return kind === 'fixed_length'
    || kind === 'fixed_angle'
    || kind === 'rotational_symmetry'
    || kind === 'length_ratio'
    ? TEXT.scalarLabels[kind]
    : null
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
  return selectLocalizedText(locale, TEXT.unknownReasonLabels[reason])
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
    : selectLocalizedText(locale, TEXT.invalidIdentifier)
}

function formatConstraintIds(
  ids: readonly string[],
  maximum: number,
  locale: Locale,
) {
  const visible = ids
    .slice(0, maximum)
    .map((id) => shortConstraintId(id, locale))
    .join(selectLocalizedText(locale, TEXT.idListSeparator))
  const remaining = ids.length - Math.min(ids.length, maximum)
  return remaining > 0
    ? formatLocalizedText(
      locale,
      TEXT.remainingIds,
      { visible, remaining },
    )
    : visible
}
