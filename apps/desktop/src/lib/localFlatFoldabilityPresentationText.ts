import type {
  LocalFlatFoldabilityCondition,
  LocalFlatFoldabilityReason,
  LocalFlatFoldabilityReport,
} from './coreClient.ts'
import {
  DEFAULT_LOCALE,
  formatMessage,
  isLocale,
  type Locale,
} from './i18n.ts'

type ReadyStatus = Exclude<LocalFlatFoldabilityReport['status'], 'blocked'>
type NonNullReason = Exclude<LocalFlatFoldabilityReason, null>

export type LocalFlatFoldabilitySummaryCounts = Readonly<{
  satisfied: number
  violated: number
  notApplicable: number
  indeterminate: number
}>

export type LocalFlatFoldabilityPresentationText = Readonly<{
  conditionLabels: Readonly<Record<LocalFlatFoldabilityCondition, string>>
  reasonLabels: Readonly<Record<NonNullReason, string>>
  invalidSummary: string
  blockedSummary: string
  countDetail: string
  summaries: Readonly<Record<ReadyStatus, string>>
}>

const JA_CONDITION_LABELS = Object.freeze({
  satisfied: '成立',
  violated: '不成立',
  not_applicable: '対象外',
  indeterminate: '判定不能',
}) satisfies Readonly<Record<LocalFlatFoldabilityCondition, string>>

const EN_CONDITION_LABELS = Object.freeze({
  satisfied: 'Satisfied',
  violated: 'Violated',
  not_applicable: 'Not applicable',
  indeterminate: 'Indeterminate',
}) satisfies Readonly<Record<LocalFlatFoldabilityCondition, string>>

const JA_REASON_LABELS = Object.freeze({
  paper_boundary: '紙の輪郭頂点は現在の局所条件の対象外です',
  cut_incident: '切断線に接している頂点は現在の局所条件の対象外です',
  fold_degree_limit:
    '折り線次数が厳密計算上限（{limit}）を超えたため判定不能です',
  no_incident_fold_edges:
    '判定対象の山折り・谷折り線がないため対象外です',
}) satisfies Readonly<Record<NonNullReason, string>>

const EN_REASON_LABELS = Object.freeze({
  paper_boundary:
    'Paper boundary vertices are outside the current local model.',
  cut_incident:
    'Vertices incident to a cut line are outside the current local model.',
  fold_degree_limit:
    'Indeterminate because the fold degree exceeds the exact limit ({limit}).',
  no_incident_fold_edges:
    'Not applicable because there are no incident mountain or valley folds.',
}) satisfies Readonly<Record<NonNullReason, string>>

const JA_SUMMARIES = Object.freeze({
  necessary_conditions_satisfied:
    '対応範囲内の局所必要条件が成立しました（{detail}）。',
  not_applicable:
    '現在の局所条件を適用できる頂点がありません（{detail}）。',
  violated: '局所必要条件に不成立の頂点があります（{detail}）。',
  indeterminate:
    '局所必要条件を判定できない頂点があります（{detail}）。',
}) satisfies Readonly<Record<ReadyStatus, string>>

const EN_SUMMARIES = Object.freeze({
  necessary_conditions_satisfied:
    'The supported local necessary conditions are satisfied ({detail}).',
  not_applicable:
    'No vertices are eligible for the current local conditions ({detail}).',
  violated:
    'At least one vertex violates the local necessary conditions ({detail}).',
  indeterminate:
    'At least one vertex has indeterminate local necessary conditions ({detail}).',
}) satisfies Readonly<Record<ReadyStatus, string>>

const JA_TEXT = Object.freeze({
  conditionLabels: JA_CONDITION_LABELS,
  reasonLabels: JA_REASON_LABELS,
  invalidSummary:
    '局所平坦折り条件の結果を確認できませんでした。成立とは扱いません。',
  blockedSummary:
    '前段の幾何構造に問題があるため、局所平坦折り条件は判定していません。',
  countDetail:
    '成立{satisfied}、不成立{violated}、対象外{notApplicable}、判定不能{indeterminate}',
  summaries: JA_SUMMARIES,
}) satisfies LocalFlatFoldabilityPresentationText

const EN_TEXT = Object.freeze({
  conditionLabels: EN_CONDITION_LABELS,
  reasonLabels: EN_REASON_LABELS,
  invalidSummary:
    'The local flat-foldability result could not be verified and is not treated as satisfied.',
  blockedSummary:
    'Local flat-foldability conditions were not checked because the preceding geometry is invalid.',
  countDetail:
    'satisfied {satisfied}, violated {violated}, not applicable {notApplicable}, indeterminate {indeterminate}',
  summaries: EN_SUMMARIES,
}) satisfies LocalFlatFoldabilityPresentationText

export const LOCAL_FLAT_FOLDABILITY_PRESENTATION_TEXT = Object.freeze({
  ja: JA_TEXT,
  en: EN_TEXT,
}) satisfies Readonly<Record<Locale, LocalFlatFoldabilityPresentationText>>

function normalizeLocale(locale: unknown): Locale {
  return isLocale(locale) ? locale : DEFAULT_LOCALE
}

export function selectLocalFlatFoldabilityPresentationText(
  locale: unknown,
): LocalFlatFoldabilityPresentationText {
  return LOCAL_FLAT_FOLDABILITY_PRESENTATION_TEXT[normalizeLocale(locale)]
}

export function formatLocalFlatFoldabilityReason(
  reason: LocalFlatFoldabilityReason,
  maxExactFoldDegree: number,
  locale: unknown,
): string {
  if (reason === null) return ''
  return formatMessage(
    selectLocalFlatFoldabilityPresentationText(locale).reasonLabels[reason],
    { limit: maxExactFoldDegree },
  )
}

export function formatLocalFlatFoldabilitySummary(
  status: ReadyStatus,
  counts: LocalFlatFoldabilitySummaryCounts,
  locale: unknown,
): string {
  const copy = selectLocalFlatFoldabilityPresentationText(locale)
  const detail = formatMessage(copy.countDetail, {
    satisfied: counts.satisfied,
    violated: counts.violated,
    notApplicable: counts.notApplicable,
    indeterminate: counts.indeterminate,
  })
  return formatMessage(copy.summaries[status], { detail })
}
