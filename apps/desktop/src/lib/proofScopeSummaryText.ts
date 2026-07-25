import type { LocalizedText } from './i18n.ts'

export type ProofScopeSummaryText = Readonly<Record<
  | 'proofCoverage'
  | 'separateProofs'
  | 'global'
  | 'globalCertificate'
  | 'targetScope'
  | 'targetScopeValue'
  | 'localSummary'
  | 'localCertificate'
  | 'globalNotChecked'
  | 'globalInProgress'
  | 'globalPossible'
  | 'globalImpossible'
  | 'globalUnknown'
  | 'globalUnavailable'
  | 'localNecessaryFailed'
  | 'localSufficientProven'
  | 'localIndeterminate'
  | 'localUnavailable'
  | 'localCounts'
  | 'relatedVertices'
  | 'vertexLabel'
  | 'hiddenVertices'
  | 'diagnosticsSummary'
  | 'diagnosticsJson',
  LocalizedText
>>

const text = (ja: string, en: string): LocalizedText =>
  Object.freeze({ ja, en })

export const PROOF_SCOPE_SUMMARY_TEXT =
  Object.freeze({
    proofCoverage: text('証明範囲', 'Proof coverage'),
    separateProofs: text(
      '全体判定・局所必要条件・局所十分性は、互いに別の証明です。',
      'The global result, local necessary conditions, and local sufficiency are separate proofs.',
    ),
    global: text('全体', 'Global'),
    globalCertificate: text('全体certificate', 'Global certificate'),
    targetScope: text('対象範囲', 'Target scope'),
    targetScopeValue: text(
      '対応対象クラス内の折り図全体',
      'Entire pattern within the supported target class',
    ),
    localSummary: text('局所summary', 'Local summary'),
    localCertificate: text('局所certificate', 'Local certificate'),
    globalNotChecked: text('未判定', 'Not checked'),
    globalInProgress: text('判定中', 'In progress'),
    globalPossible: text('可能', 'Possible'),
    globalImpossible: text('不可能', 'Impossible'),
    globalUnknown: text('不明', 'Unknown'),
    globalUnavailable: text('利用不可', 'Unavailable'),
    localNecessaryFailed: text('必要条件不成立', 'Necessary failed'),
    localSufficientProven: text('十分性証明', 'Sufficiency proven'),
    localIndeterminate: text('判定不能', 'Indeterminate'),
    localUnavailable: text('未取得', 'Unavailable'),
    localCounts: text(
      '必要条件不成立 {necessaryFailed}・十分性証明 {sufficientProven}・判定不能 {indeterminate}',
      'Necessary failed {necessaryFailed}; sufficiency proven {sufficientProven}; indeterminate {indeterminate}',
    ),
    relatedVertices: text('関連頂点', 'Related vertices'),
    vertexLabel: text('頂点 {index}', 'Vertex {index}'),
    hiddenVertices: text('ほか {count} 頂点', '{count} more vertices'),
    diagnosticsSummary: text(
      '決定的diagnostics summary',
      'Deterministic diagnostics summary',
    ),
    diagnosticsJson: text(
      '証明範囲diagnostics JSON',
      'Proof coverage diagnostics JSON',
    ),
  }) satisfies ProofScopeSummaryText
