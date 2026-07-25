import type { AssignedLocalSufficiencySummaryResponseV1 } from '../lib/coreClient.ts'
import {
  formatLocalizedText,
  selectLocalizedText,
  useLocale,
  type LocaleStore,
  type LocalizedText,
} from '../lib/i18n.ts'
import {
  PROOF_SCOPE_SUMMARY_TEXT as TEXT,
} from '../lib/proofScopeSummaryText.ts'
import { createProofScopePresentation } from '../lib/proofScopePresentation.ts'

export function ProofScopeSummary({
  globalJob,
  localSummary,
  localeStore,
  selectedVertexId,
  onSelectVertex,
}: Readonly<{
  globalJob: unknown
  localSummary: AssignedLocalSufficiencySummaryResponseV1 | null
  localeStore: LocaleStore
  selectedVertexId?: string | null
  onSelectVertex?(vertexId: string): void
}>) {
  const locale = useLocale(localeStore)
  const presentation = createProofScopePresentation(globalJob, localSummary)
  const { global, local } = presentation.diagnostics
  const label = (value: LocalizedText) => selectLocalizedText(locale, value)
  return (
    <section className="proof-scope-summary" aria-label={label(TEXT.proofCoverage)}>
      <h4>{label(TEXT.proofCoverage)}</h4>
      <p>{label(TEXT.separateProofs)}</p>
      <dl>
        <div>
          <dt>{label(TEXT.global)}</dt>
          <dd data-proof-global={global.status}>{label(globalStatus(global.status))}</dd>
        </div>
        <div>
          <dt>{label(TEXT.globalCertificate)}</dt>
          <dd>{global.certificateModel} / v{global.certificateVersion}</dd>
        </div>
        <div>
          <dt>{label(TEXT.targetScope)}</dt>
          <dd>{label(TEXT.targetScopeValue)}</dd>
        </div>
        <div>
          <dt>{label(TEXT.localSummary)}</dt>
          <dd>
            {local.status === 'unavailable'
              ? label(TEXT.localUnavailable)
              : formatLocalizedText(locale, TEXT.localCounts, {
                necessaryFailed: local.necessaryFailed,
                sufficientProven: local.sufficientProven,
                indeterminate: local.indeterminate,
              })}
          </dd>
        </div>
        <div>
          <dt>{label(TEXT.localCertificate)}</dt>
          <dd>{local.certificateModel} / v{local.certificateVersion}</dd>
        </div>
      </dl>
      {presentation.selectableVertices.length > 0 && (
        <ul aria-label={label(TEXT.relatedVertices)}>
          {presentation.selectableVertices.map((vertex, index) => (
            <li key={vertex.id}>
              <button
                type="button"
                aria-pressed={selectedVertexId === vertex.id}
                onClick={() => onSelectVertex?.(vertex.id)}
              >
                {formatLocalizedText(locale, TEXT.vertexLabel, { index: index + 1 })}
                {' · '}
                {label(localStatus(vertex.status))}
              </button>
            </li>
          ))}
        </ul>
      )}
      {presentation.hiddenVertexCount > 0 && (
        <p>{formatLocalizedText(locale, TEXT.hiddenVertices, {
          count: presentation.hiddenVertexCount,
        })}</p>
      )}
      <details>
        <summary>{label(TEXT.diagnosticsSummary)}</summary>
        <textarea
          aria-label={label(TEXT.diagnosticsJson)}
          readOnly
          value={presentation.diagnosticsJson}
          rows={12}
        />
      </details>
    </section>
  )
}

function globalStatus(status: string) {
  const labels = {
    not_checked: TEXT.globalNotChecked,
    in_progress: TEXT.globalInProgress,
    possible: TEXT.globalPossible,
    impossible: TEXT.globalImpossible,
    unknown: TEXT.globalUnknown,
    unavailable: TEXT.globalUnavailable,
  } as const
  return labels[status as keyof typeof labels] ?? labels.unavailable
}

function localStatus(status: string) {
  if (status === 'necessary_failed') return TEXT.localNecessaryFailed
  if (status === 'sufficient_proven') return TEXT.localSufficientProven
  return TEXT.localIndeterminate
}
