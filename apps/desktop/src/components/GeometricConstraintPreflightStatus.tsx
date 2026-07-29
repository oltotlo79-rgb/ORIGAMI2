import type {
  GeometricConstraintDocument,
  GeometricConstraintPreflightResult,
  GeometricConstraintSemanticMus,
} from '../lib/coreClient'
import { isCanonicalNonNilUuid } from '../lib/canonicalUuid.ts'
import {
  GEOMETRIC_CONSTRAINT_PANEL_TEXT as TEXT,
} from '../lib/geometricConstraintPanelText.ts'
import {
  buildGeometricConstraintSemanticMusCertifiedViewModel,
} from '../lib/geometricConstraintSemanticMusViewModel.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'

const MAX_VISIBLE_DIRECT_CONFLICTS = 50
const MAX_VISIBLE_UNCHECKED_CONSTRAINT_IDS = 20

type DirectConflictPreflight = Extract<
  GeometricConstraintPreflightResult,
  { status: 'direct_conflict' }
>
type UnknownPreflight = Extract<
  GeometricConstraintPreflightResult,
  { status: 'unknown' }
>
type ConstraintPreflightStatusProps = Readonly<{
  document: GeometricConstraintDocument
  preflight: GeometricConstraintPreflightResult | null
  semanticMus: GeometricConstraintSemanticMus | null
  analyzing: boolean
  failed: boolean
  disabled: boolean
  onRetry: () => void
  locale: Locale
}>

export function ConstraintPreflightStatus({
  document,
  preflight,
  semanticMus,
  analyzing,
  failed,
  disabled,
  onRetry,
  locale,
}: ConstraintPreflightStatusProps) {
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
                  preflight.reason === 'solver_required_constraint_kinds'
                    ? document
                    : null,
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
  result: DirectConflictPreflight['bounded_direct_mus']
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
            mirrorResidual: view.anchoredMirrorResidualOnlyWitnessCount,
            unitParallelFixedAngleResidual:
              view.unitParallelFixedAngleResidualOnlyWitnessCount,
            unitTerminalTwoHopParallelAngleResidual:
              view.unitTerminalTwoHopParallelAngleResidualOnlyWitnessCount,
            unitTwoHopParallelResidual:
              view.unitTwoHopParallelResidualOnlyWitnessCount,
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
  conflict: DirectConflictPreflight['conflicts'][number]['conflict'],
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

function unknownReasonLabel(
  reason: UnknownPreflight['reason'],
  locale: Locale,
) {
  return selectLocalizedText(locale, TEXT.unknownReasonLabels[reason])
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
  document: GeometricConstraintDocument | null = null,
) {
  const constraintsById = document === null
    ? null
    : new Map(document.constraints.map(
      (record) => [record.id, record.constraint] as const,
    ))
  const visible = ids
    .slice(0, maximum)
    .map((id) => {
      const shortened = shortConstraintId(id, locale)
      if (constraintsById === null) return shortened
      const constraint = constraintsById.get(id)
      const kind = constraint
        ? selectLocalizedText(
            locale,
            TEXT.constraintKindNames[constraint.kind],
          )
        : selectLocalizedText(locale, TEXT.unknownConstraintKind)
      return `${shortened} (${kind})`
    })
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
