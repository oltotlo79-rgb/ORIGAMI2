import type { AssignedLocalSufficiencySummaryResponseV1 } from '../lib/coreClient.ts'
import { useLocale, type LocaleStore } from '../lib/i18n.ts'
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
  return (
    <section className="proof-scope-summary" aria-label={locale === 'ja' ? '証明範囲' : 'Proof coverage'}>
      <h4>{locale === 'ja' ? '証明範囲' : 'Proof coverage'}</h4>
      <p>
        {locale === 'ja'
          ? '全体判定・局所必要条件・局所十分性は、互いに別の証明です。'
          : 'The global result, local necessary conditions, and local sufficiency are separate proofs.'}
      </p>
      <dl>
        <div>
          <dt>{locale === 'ja' ? '全体' : 'Global'}</dt>
          <dd data-proof-global={global.status}>{globalStatus(global.status, locale)}</dd>
        </div>
        <div>
          <dt>{locale === 'ja' ? '全体certificate' : 'Global certificate'}</dt>
          <dd>{global.certificateModel} / v{global.certificateVersion}</dd>
        </div>
        <div>
          <dt>{locale === 'ja' ? '対象範囲' : 'Target scope'}</dt>
          <dd>{locale === 'ja' ? '対応対象クラス内の折り図全体' : 'Entire pattern within the supported target class'}</dd>
        </div>
        <div>
          <dt>{locale === 'ja' ? '局所summary' : 'Local summary'}</dt>
          <dd>
            {local.status === 'unavailable'
              ? locale === 'ja' ? '未取得' : 'Unavailable'
              : locale === 'ja'
                ? `必要条件不成立 ${local.necessaryFailed}・十分性証明 ${local.sufficientProven}・判定不能 ${local.indeterminate}`
                : `Necessary failed ${local.necessaryFailed}; sufficiency proven ${local.sufficientProven}; indeterminate ${local.indeterminate}`}
          </dd>
        </div>
        <div>
          <dt>{locale === 'ja' ? '局所certificate' : 'Local certificate'}</dt>
          <dd>{local.certificateModel} / v{local.certificateVersion}</dd>
        </div>
      </dl>
      {presentation.selectableVertices.length > 0 && (
        <ul aria-label={locale === 'ja' ? '関連頂点' : 'Related vertices'}>
          {presentation.selectableVertices.map((vertex, index) => (
            <li key={vertex.id}>
              <button
                type="button"
                aria-pressed={selectedVertexId === vertex.id}
                onClick={() => onSelectVertex?.(vertex.id)}
              >
                {locale === 'ja' ? `頂点 ${index + 1}` : `Vertex ${index + 1}`}
                {' · '}
                {localStatus(vertex.status, locale)}
              </button>
            </li>
          ))}
        </ul>
      )}
      {presentation.hiddenVertexCount > 0 && (
        <p>{locale === 'ja'
          ? `ほか ${presentation.hiddenVertexCount} 頂点`
          : `${presentation.hiddenVertexCount} more vertices`}</p>
      )}
      <details>
        <summary>{locale === 'ja' ? '決定的diagnostics summary' : 'Deterministic diagnostics summary'}</summary>
        <textarea
          aria-label={locale === 'ja' ? '証明範囲diagnostics JSON' : 'Proof coverage diagnostics JSON'}
          readOnly
          value={presentation.diagnosticsJson}
          rows={12}
        />
      </details>
    </section>
  )
}

function globalStatus(status: string, locale: 'ja' | 'en') {
  const labels = {
    not_checked: ['未判定', 'Not checked'],
    in_progress: ['判定中', 'In progress'],
    possible: ['可能', 'Possible'],
    impossible: ['不可能', 'Impossible'],
    unknown: ['不明', 'Unknown'],
    unavailable: ['利用不可', 'Unavailable'],
  } as const
  const label = labels[status as keyof typeof labels] ?? labels.unavailable
  return label[locale === 'ja' ? 0 : 1]
}

function localStatus(status: string, locale: 'ja' | 'en') {
  if (status === 'necessary_failed') return locale === 'ja' ? '必要条件不成立' : 'Necessary failed'
  if (status === 'sufficient_proven') return locale === 'ja' ? '十分性証明' : 'Sufficiency proven'
  return locale === 'ja' ? '判定不能' : 'Indeterminate'
}
