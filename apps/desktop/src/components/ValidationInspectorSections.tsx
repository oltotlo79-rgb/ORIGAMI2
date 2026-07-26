import { APP_TEXT } from '../lib/appText.ts'
import {
  localizedLocalFlatFoldabilityConditionLabel,
  localizedLocalFlatFoldabilityReasonLabel,
  localizedLocalFlatFoldabilitySummary,
  validationIssueLabel,
} from '../lib/appPresentation.ts'
import type {
  AssignedLocalSufficiencyResponseV1,
  AssignedLocalSufficiencySummaryResponseV1,
  ValidationSnapshot,
} from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from '../lib/i18n.ts'
import type {
  createLocalFlatFoldabilityPresentation,
} from '../lib/localFlatFoldabilityPresentation.ts'
import { BulkIntersectionRepairControl } from './BulkIntersectionRepairControl.tsx'
import type { CreaseLine } from './CreaseCanvas.tsx'

type LocalPresentation = ReturnType<
  typeof createLocalFlatFoldabilityPresentation
>

export function ValidationInspectorSections({
  locale,
  validation,
  lines,
  vertices,
  unsplitIntersectionCount,
  bulkIntersectionRepairPending,
  controlsDisabled,
  onRepairAllIntersections,
  localPresentation,
  benchmarkActive,
  selectedVertexId,
  assignedLocalSummaryStatus,
  assignedLocalSummary,
  assignedLocalSufficiency,
  onSelectLine,
  onSelectSummaryVertex,
  onSelectVertex,
}: Readonly<{
  locale: Locale
  validation: ValidationSnapshot | null
  lines: readonly CreaseLine[]
  vertices: readonly Readonly<{ id: string }>[]
  unsplitIntersectionCount: number
  bulkIntersectionRepairPending: boolean
  controlsDisabled: boolean
  onRepairAllIntersections: () => void | Promise<void>
  localPresentation: LocalPresentation | null
  benchmarkActive: boolean
  selectedVertexId: string | null
  assignedLocalSummaryStatus:
    'idle' | 'loading' | 'retrying' | 'ready' | 'failed'
  assignedLocalSummary: AssignedLocalSufficiencySummaryResponseV1 | null
  assignedLocalSufficiency: AssignedLocalSufficiencyResponseV1 | null
  onSelectLine: (lineId: string) => void
  onSelectSummaryVertex: (vertexId: string) => void
  onSelectVertex: (vertexId: string) => void
}>) {
  const text = (localized: Parameters<typeof selectLocalizedText>[1]) => (
    selectLocalizedText(locale, localized)
  )
  const formattedText = (
    localized: Parameters<typeof formatLocalizedText>[1],
    variables: Parameters<typeof formatLocalizedText>[2],
  ) => formatLocalizedText(locale, localized, variables)
  const selectedLocal = selectedVertexId
    ? localPresentation?.verticesById.get(selectedVertexId)
    : undefined

  return (
    <>
      {validation && (
        <section
          className={validation.is_valid
            ? 'validation-report valid'
            : 'validation-report invalid'}
        >
          <h2>{text(APP_TEXT.geometryValidation)}</h2>
          {validation.is_valid ? (
            <p>{text(APP_TEXT.noIssuesWereFound)}</p>
          ) : (
            <>
              <p>{formattedText(APP_TEXT.countIssuesWereFound, {
                count: validation.issues.length,
              })}</p>
              <BulkIntersectionRepairControl
                count={unsplitIntersectionCount}
                pending={bulkIntersectionRepairPending}
                disabled={controlsDisabled}
                locale={locale}
                onConfirm={() => void onRepairAllIntersections()}
              />
              <ul>
                {validation.issues.slice(0, 20).map((issue, index) => {
                  const edgeId = issue.edges.find((id) =>
                    lines.some((line) => line.id === id))
                  const vertexId = issue.vertices.find((id) =>
                    vertices.some((vertex) => vertex.id === id))
                  const label = validationIssueLabel(issue.code, locale)
                  return (
                    <li key={`${issue.code}:${index}`}>
                      {edgeId || vertexId ? (
                        <button
                          type="button"
                          onClick={() => {
                            if (edgeId) onSelectLine(edgeId)
                            else if (vertexId) onSelectVertex(vertexId)
                          }}
                        >
                          {label}
                        </button>
                      ) : (
                        <span>{label}</span>
                      )}
                    </li>
                  )
                })}
              </ul>
            </>
          )}
        </section>
      )}
      {localPresentation && !benchmarkActive && (
        <section
          className={`local-flat-foldability-report is-${
            localPresentation.kind === 'ready'
              ? localPresentation.reportStatus
              : localPresentation.kind
          }`}
        >
          <h2>{text(APP_TEXT.localFlatFoldabilityConditions)}</h2>
          <p
            id="local-flat-foldability-summary"
            className="local-flat-foldability-summary"
            role="status"
            aria-live="polite"
            aria-atomic="true"
          >
            {localizedLocalFlatFoldabilitySummary(
              localPresentation,
              locale,
            )}
          </p>
          {localPresentation.maxExactFoldDegree !== null && (
            <p className="local-flat-foldability-coverage">
              {formattedText(
                APP_TEXT.coverageASingleInteriorVertexZeroThicknessModelFoldDegree,
                { degree: localPresentation.maxExactFoldDegree },
              )}
            </p>
          )}
          {localPresentation.kind === 'ready' && (
            <>
              <ul
                className="local-flat-foldability-counts"
                aria-label={text(APP_TEXT.vertexCountsByLocalFlatFoldabilityResult)}
              >
                {([
                  ['satisfied', APP_TEXT.satisfied,
                    localPresentation.counts.satisfied],
                  ['violated', APP_TEXT.violated,
                    localPresentation.counts.violated],
                  ['not-applicable', APP_TEXT.notApplicable,
                    localPresentation.counts.notApplicable],
                  ['indeterminate', APP_TEXT.indeterminate2,
                    localPresentation.counts.indeterminate],
                ] as const).map(([kind, label, count]) => (
                  <li key={kind} className={`is-${kind}`}>
                    <span>{text(label)}</span>
                    <strong>{count.toLocaleString(locale)}</strong>
                  </li>
                ))}
              </ul>
              {(assignedLocalSummaryStatus === 'loading'
                || assignedLocalSummaryStatus === 'retrying') && (
                <p role="status">{text({
                  ja: assignedLocalSummaryStatus === 'retrying'
                    ? '旧解析の終了を待って局所十分性summaryを再試行しています…'
                    : '全頂点の指定M/V局所十分性を有界解析しています…',
                  en: assignedLocalSummaryStatus === 'retrying'
                    ? 'Waiting for the previous worker to exit, then retrying the summary…'
                    : 'Running the bounded assigned M/V local-sufficiency summary…',
                })}</p>
              )}
              {assignedLocalSummaryStatus === 'failed' && (
                <p role="alert">
                  {text(APP_TEXT.theAllVertexLocalSufficiencySummaryIsUnavailable)}
                </p>
              )}
              {assignedLocalSummary && (
                <section
                  aria-label={text(APP_TEXT.allVertexLocalSufficiencySummary)}
                >
                  <p>
                    {text(APP_TEXT.necessaryConditionFailureProvenSufficiencyAndIndeterminateAreSeparatePas)}
                  </p>
                  <ul>
                    {assignedLocalSummary.vertices.map((item) => (
                      <li key={item.vertex}>
                        <button
                          type="button"
                          onClick={() => onSelectSummaryVertex(item.vertex)}
                        >
                          {item.vertex.slice(0, 8)} · {
                            item.status === 'necessary_failed'
                              ? text(APP_TEXT.necessaryFailed)
                              : item.status === 'sufficient_proven'
                                ? text(APP_TEXT.sufficiencyProven)
                                : text(APP_TEXT.indeterminate2)
                          }
                        </button>
                      </li>
                    ))}
                  </ul>
                </section>
              )}
              {selectedLocal && (
                <div className="selected-local-flat-foldability">
                  <h3>{text(APP_TEXT.localConditionsForSelectedVertex)}</h3>
                  <dl>
                    <div>
                      <dt>{text(APP_TEXT.overall)}</dt>
                      <dd>{localizedLocalFlatFoldabilityConditionLabel(
                        selectedLocal.verdict,
                        locale,
                      )}</dd>
                    </div>
                    <div>
                      <dt>{text(APP_TEXT.kawasakiCondition)}</dt>
                      <dd>{localizedLocalFlatFoldabilityConditionLabel(
                        selectedLocal.kawasaki,
                        locale,
                      )}</dd>
                    </div>
                    <div>
                      <dt>{text(APP_TEXT.maekawaCondition)}</dt>
                      <dd>{localizedLocalFlatFoldabilityConditionLabel(
                        selectedLocal.maekawa,
                        locale,
                      )}</dd>
                    </div>
                    <div>
                      <dt>{text(APP_TEXT.foldDegree)}</dt>
                      <dd>{selectedLocal.foldDegree}</dd>
                    </div>
                    <div>
                      <dt>{text(APP_TEXT.mountainValley)}</dt>
                      <dd>
                        {selectedLocal.mountainCount}
                        {' / '}
                        {selectedLocal.valleyCount}
                      </dd>
                    </div>
                  </dl>
                  {selectedLocal.reason && (
                    <p className="local-flat-foldability-reason">
                      {localizedLocalFlatFoldabilityReasonLabel(
                        selectedLocal.reason,
                        localPresentation.maxExactFoldDegree,
                        locale,
                      )}
                    </p>
                  )}
                  {assignedLocalSufficiency && (
                    <p
                      className="local-flat-foldability-sufficiency"
                      aria-live="polite"
                    >
                      {assignedLocalSufficiency.result.status === 'proven'
                        ? text({
                            ja: `指定M/Vの局所十分性をBLB縮約 ${assignedLocalSufficiency.result.reduction_steps} 段で証明しました。`,
                            en: `Assigned M/V local sufficiency is proven by ${assignedLocalSufficiency.result.reduction_steps} BLB reduction step(s).`,
                          })
                        : text({
                            ja: assignedLocalSufficiency.result.reason
                                === 'resource_limit'
                              ? '局所十分性は資源上限のため判定不能です。'
                              : assignedLocalSufficiency.result.reason
                                  === 'necessary_conditions_not_satisfied'
                                ? '局所必要条件が成立しないため十分性を証明できません。'
                                : '適用できる一意なstrict BLB縮約がないため局所十分性は判定不能です。',
                            en: assignedLocalSufficiency.result.reason
                                === 'resource_limit'
                              ? 'Local sufficiency is indeterminate because the resource limit was reached.'
                              : assignedLocalSufficiency.result.reason
                                  === 'necessary_conditions_not_satisfied'
                                ? 'Local sufficiency cannot be proven because the necessary conditions fail.'
                                : 'Local sufficiency is indeterminate because no unique strict BLB reduction applies.',
                          })}
                    </p>
                  )}
                </div>
              )}
              {localPresentation.visibleItems.length > 0 && (
                <>
                  <h3>{text(APP_TEXT.verticesRequiringReview)}</h3>
                  <ul className="local-flat-foldability-items">
                    {localPresentation.visibleItems.map((item) => {
                      const verdictLabel =
                        localizedLocalFlatFoldabilityConditionLabel(
                          item.verdict,
                          locale,
                        )
                      const reasonLabel =
                        localizedLocalFlatFoldabilityReasonLabel(
                          item.reason,
                          localPresentation.maxExactFoldDegree,
                          locale,
                        )
                      return (
                        <li key={item.vertexId}>
                          <button
                            type="button"
                            aria-pressed={selectedVertexId === item.vertexId}
                            aria-label={formattedText(
                              APP_TEXT.vertexOrdinalLocalNecessaryConditionVerdictKawasakiConditionKawasakiMaek,
                              {
                                ordinal: item.ordinal,
                                verdict: verdictLabel,
                                kawasaki:
                                  localizedLocalFlatFoldabilityConditionLabel(
                                    item.kawasaki,
                                    locale,
                                  ),
                                maekawa:
                                  localizedLocalFlatFoldabilityConditionLabel(
                                    item.maekawa,
                                    locale,
                                  ),
                                reason: reasonLabel,
                              },
                            )}
                            onClick={() => {
                              if (vertices.some(
                                ({ id }) => id === item.vertexId,
                              )) onSelectVertex(item.vertexId)
                            }}
                          >
                            <span
                              className={`local-verdict is-${item.verdict}`}
                            >
                              {verdictLabel}
                            </span>
                            <span>{formattedText(APP_TEXT.vertexOrdinal, {
                              ordinal: item.ordinal,
                            })}</span>
                            <span className="local-flat-foldability-item-detail">
                              {reasonLabel || formattedText(
                                APP_TEXT.kawasakiKawasakiMaekawaMaekawa,
                                {
                                  kawasaki:
                                    localizedLocalFlatFoldabilityConditionLabel(
                                      item.kawasaki,
                                      locale,
                                    ),
                                  maekawa:
                                    localizedLocalFlatFoldabilityConditionLabel(
                                      item.maekawa,
                                      locale,
                                    ),
                                },
                              )}
                            </span>
                          </button>
                        </li>
                      )
                    })}
                  </ul>
                  {localPresentation.hiddenItemCount > 0 && (
                    <p className="muted">
                      {formattedText(
                        APP_TEXT.countMoreVerticesSelectAVertexToReviewItsResult,
                        {
                          count: localPresentation.hiddenItemCount
                            .toLocaleString(locale),
                        },
                      )}
                    </p>
                  )}
                </>
              )}
            </>
          )}
          <p className="local-flat-foldability-disclaimer">
            {text(APP_TEXT.satisfiedMeansOnlyThatTheLocalNecessaryConditionsWereVerified)}
          </p>
        </section>
      )}
    </>
  )
}
