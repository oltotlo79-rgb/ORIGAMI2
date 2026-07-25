import {
  GLOBAL_FLAT_FOLDABILITY_TARGET_CLASS,
  type GlobalFlatFoldabilityErrorCategory,
  type GlobalFlatFoldabilityPhase,
  type GlobalFlatFoldabilityProofCategory,
  type GlobalFlatFoldabilityUnknownReason,
} from './globalFlatFoldability.ts'
import {
  DEFAULT_LOCALE,
  formatMessage,
  isLocale,
  type Locale,
} from './i18n.ts'

export type GlobalFlatFoldabilityPresentationText = Readonly<{
  targetClass: string
  cancelledLabel: string
  cancelledDetail: string
  cancelledLive: string
  calculationErrorLabel: string
  calculationErrorLive: string
  staleLabel: string
  staleDetail: string
  staleLive: string
  idleLabel: string
  idleDetail: string
  invalidDetail: string
  invalidLive: string
  cancellingLabel: string
  cancellingDetail: string
  queuedLabel: string
  queuedDetail: string
  runningLabel: string
  runningDetail: string
  activeLive: string
  possibleLabel: string
  possibleDetail: string
  possibleLive: string
  layerOrderModelLabel: string
  layerCountLabel: string
  layerCountValue: string
  maximumOverlapLabel: string
  maximumOverlapValue: string
  referenceFaceLabel: string
  faceValue: string
  layerOrderViewLabel: string
  available: string
  unavailable: string
  impossibleLabel: string
  impossibleDetail: string
  impossibleLive: string
  proofTypeLabel: string
  targetFacesLabel: string
  exhaustiveFacesWithRemaining: string
  exhaustiveFaces: string
  unknownLabel: string
  unknownLive: string
  unknownReasonLabel: string
  checkModelLabel: string
  targetClassLabel: string
  elapsedTimeLabel: string
  facesLabel: string
  overlapCellsLabel: string
  constraintsLabel: string
  searchNodesLabel: string
  progressWithoutTotal: string
  progressWithTotal: string
  itemCount: string
  milliseconds: string
  seconds: string
  minutes: string
  minutesAndSeconds: string
  listSeparator: string
  phases: Readonly<Record<GlobalFlatFoldabilityPhase, string>>
  unknownReasons: Readonly<
    Record<GlobalFlatFoldabilityUnknownReason, string>
  >
  proofLabels: Readonly<Record<GlobalFlatFoldabilityProofCategory, string>>
  errorMessages: Readonly<Record<GlobalFlatFoldabilityErrorCategory, string>>
}>

const JA_PHASES = Object.freeze({
  capturing: '編集内容を取得しています',
  validating_local_conditions: '局所平坦折り条件を確認しています',
  building_flat_embedding: '平面配置を構築しています',
  building_overlap_arrangement: '重なり領域を構築しています',
  building_constraints: '層順序の制約を構築しています',
  propagating: '確定した層順序を伝播しています',
  searching: '層順序を探索しています',
  verifying_certificate: '判定根拠を再検証しています',
  completed: '判定結果を確定しています',
}) satisfies Readonly<Record<GlobalFlatFoldabilityPhase, string>>

const EN_PHASES = Object.freeze({
  capturing: 'Capturing edits',
  validating_local_conditions: 'Checking local flat-foldability conditions',
  building_flat_embedding: 'Building the flat embedding',
  building_overlap_arrangement: 'Building overlap regions',
  building_constraints: 'Building layer-order constraints',
  propagating: 'Propagating determined layer order',
  searching: 'Searching layer order',
  verifying_certificate: 'Verifying the result certificate',
  completed: 'Finalizing the result',
}) satisfies Readonly<Record<GlobalFlatFoldabilityPhase, string>>

const JA_UNKNOWN_REASONS = Object.freeze({
  unsupported_topology:
    '切断、穴、未接続材料など、初版の対象外となる面構造が含まれています。',
  non_convex_face:
    '凸多角形でない面があるため、初版の対象として判定できませんでした。',
  time_limit_reached:
    '選択した時間制限内に証明を完了できませんでした。時間を延ばして再判定できます。',
  work_limit_reached:
    '処理件数が初版の作業上限に達したため、証明を完了できませんでした。',
  exact_number_limit_reached:
    '正確な数値計算が初版の安全上限に達したため、証明を完了できませんでした。',
  overlap_arrangement_limit_reached:
    '重なり領域の構築が初版の安全上限に達したため、証明を完了できませんでした。',
  constraint_limit_reached:
    '層順序の制約数が初版の安全上限に達したため、証明を完了できませんでした。',
  proof_not_completed:
    '可または不可を確定できる証明を完成できませんでした。',
  local_conditions_indeterminate:
    '局所平坦折り条件に未確定の頂点があるため、全体判定を確定できませんでした。',
}) satisfies Readonly<Record<GlobalFlatFoldabilityUnknownReason, string>>

const EN_UNKNOWN_REASONS = Object.freeze({
  unsupported_topology:
    'The face structure includes cuts, holes, disconnected material, or another topology outside the initial release scope.',
  non_convex_face:
    'The check is indeterminate because at least one face is not a convex polygon supported by the initial release.',
  time_limit_reached:
    'The proof could not be completed within the selected time limit. Choose a longer limit and run the check again.',
  work_limit_reached:
    'The proof could not be completed because the initial-release work limit was reached.',
  exact_number_limit_reached:
    'The proof could not be completed because exact arithmetic reached the initial-release safety limit.',
  overlap_arrangement_limit_reached:
    'The proof could not be completed because overlap-region construction reached the initial-release safety limit.',
  constraint_limit_reached:
    'The proof could not be completed because the number of layer-order constraints reached the initial-release safety limit.',
  proof_not_completed:
    'A proof establishing Possible or Impossible could not be completed.',
  local_conditions_indeterminate:
    'The global result is indeterminate because at least one vertex has an indeterminate local flat-foldability condition.',
}) satisfies Readonly<Record<GlobalFlatFoldabilityUnknownReason, string>>

const JA_PROOF_LABELS = Object.freeze({
  local_conditions_violated: '局所必要条件の明示的な違反',
  inconsistent_flat_embedding: '平面配置の経路間矛盾',
  layer_constraints_contradictory: '層順序制約の矛盾',
  exhaustive_search_no_solution: '全候補の探索完了（解なし）',
}) satisfies Readonly<Record<GlobalFlatFoldabilityProofCategory, string>>

const EN_PROOF_LABELS = Object.freeze({
  local_conditions_violated: 'Explicit violation of local necessary conditions',
  inconsistent_flat_embedding: 'Path inconsistency in the flat embedding',
  layer_constraints_contradictory: 'Contradictory layer-order constraints',
  exhaustive_search_no_solution: 'Exhaustive search completed (no solution)',
}) satisfies Readonly<Record<GlobalFlatFoldabilityProofCategory, string>>

const JA_ERROR_MESSAGES = Object.freeze({
  invalid_request:
    '判定を開始するための条件を確認できませんでした。現在の編集内容で再試行してください。',
  snapshot_unavailable:
    '判定用の編集内容を安全に取得できませんでした。現在の編集内容で再試行してください。',
  worker_unavailable:
    '判定処理を開始できませんでした。少し待ってから再試行してください。',
  result_unavailable:
    '完了した判定結果を安全に取得できませんでした。再判定してください。',
  internal_failure:
    '判定処理を安全に完了できませんでした。作品は変更されていません。',
}) satisfies Readonly<Record<GlobalFlatFoldabilityErrorCategory, string>>

const EN_ERROR_MESSAGES = Object.freeze({
  invalid_request:
    'The conditions required to start the check could not be verified. Retry with the current edits.',
  snapshot_unavailable:
    'The edits required for the check could not be captured safely. Retry with the current edits.',
  worker_unavailable:
    'The check could not be started. Wait briefly and retry.',
  result_unavailable:
    'The completed result could not be retrieved safely. Run the check again.',
  internal_failure:
    'The check could not be completed safely. The work was not changed.',
}) satisfies Readonly<Record<GlobalFlatFoldabilityErrorCategory, string>>

const JA_TEXT = Object.freeze({
  targetClass: GLOBAL_FLAT_FOLDABILITY_TARGET_CLASS,
  cancelledLabel: '中止',
  cancelledDetail:
    '全体平坦折り判定を中止しました。判定途中の候補は採用していません。',
  cancelledLive: '全体平坦折り判定を中止しました。',
  calculationErrorLabel: '計算エラー',
  calculationErrorLive: '全体平坦折り判定は計算エラーで終了しました。',
  staleLabel: '古い結果',
  staleDetail:
    '判定開始後に編集内容が変わったため、この結果は現在の作品へ適用できません。現在の内容で再判定してください。',
  staleLive:
    '全体平坦折り判定の結果は古いため、現在の作品へ適用できません。',
  idleLabel: '未判定',
  idleDetail:
    '時間制限を選び、現在の編集内容について判定を開始できます。',
  invalidDetail:
    '判定結果の形式を安全に確認できませんでした。内容は表示せず、現在の編集内容で再判定できます。',
  invalidLive: '全体平坦折り判定の結果を安全に確認できませんでした。',
  cancellingLabel: '中止しています',
  cancellingDetail:
    '中止しています。処理が安全に終了するまでお待ちください。',
  queuedLabel: '開始待ち',
  queuedDetail: '判定開始を待っています。',
  runningLabel: '判定中',
  runningDetail: '判定中も展開図の編集を続けられます。',
  activeLive: '{label}。{phase}。',
  possibleLabel: '可',
  possibleDetail:
    '理想的な厚さ0のモデルで、条件を満たす層順序を構成し、判定根拠を再検証できました。',
  possibleLive: '全体平坦折り判定の結果は、可です。',
  layerOrderModelLabel: '層順序モデル',
  layerCountLabel: '層数',
  layerCountValue: '{count}層',
  maximumOverlapLabel: '最大重なり',
  maximumOverlapValue: '{count} ply',
  referenceFaceLabel: '基準面',
  faceValue: '面 {number}',
  layerOrderViewLabel: '層順3D表示',
  available: '利用できます',
  unavailable: '利用できません',
  impossibleLabel: '不可',
  impossibleDetail:
    '初版の対象クラス内で、平坦折り可能な層順序が存在しないことを有限の根拠で確認しました。',
  impossibleLive: '全体平坦折り判定の結果は、不可です。',
  proofTypeLabel: '証明種別',
  targetFacesLabel: '対象面（FaceKey順・最大20件）',
  exhaustiveFacesWithRemaining: '全体：{faces}（ほか{remaining}面）',
  exhaustiveFaces: '全体：{faces}',
  unknownLabel: '不明',
  unknownLive: '全体平坦折り判定の結果は、不明です。{reason}',
  unknownReasonLabel: '確定できない理由',
  checkModelLabel: '判定モデル',
  targetClassLabel: '対象クラス',
  elapsedTimeLabel: '経過時間',
  facesLabel: '面',
  overlapCellsLabel: '重なりcell',
  constraintsLabel: '制約',
  searchNodesLabel: '探索node',
  progressWithoutTotal: '{completed}件完了（総数は計算中）',
  progressWithTotal: '{completed} / {total}件完了',
  itemCount: '{count}件',
  milliseconds: '{milliseconds}ミリ秒',
  seconds: '{seconds}秒',
  minutes: '{minutes}分',
  minutesAndSeconds: '{minutes}分{seconds}秒',
  listSeparator: '、',
  phases: JA_PHASES,
  unknownReasons: JA_UNKNOWN_REASONS,
  proofLabels: JA_PROOF_LABELS,
  errorMessages: JA_ERROR_MESSAGES,
}) satisfies GlobalFlatFoldabilityPresentationText

const EN_TEXT = Object.freeze({
  targetClass:
    'Convex polygonal faces (no cuts, holes, or disconnected material)',
  cancelledLabel: 'Cancelled',
  cancelledDetail:
    'The global flat-foldability check was cancelled. Intermediate candidates were not accepted.',
  cancelledLive: 'The global flat-foldability check was cancelled.',
  calculationErrorLabel: 'Calculation error',
  calculationErrorLive:
    'The global flat-foldability check ended with a calculation error.',
  staleLabel: 'Outdated result',
  staleDetail:
    'The project changed after the check started, so this result does not apply to the current work. Run the check again.',
  staleLive:
    'The global flat-foldability result is outdated and does not apply to the current work.',
  idleLabel: 'Not checked',
  idleDetail: 'Select a time limit to check the current edits.',
  invalidDetail:
    'The result format could not be verified safely. Its contents are hidden; run the check again with the current edits.',
  invalidLive:
    'The global flat-foldability result could not be verified safely.',
  cancellingLabel: 'Cancelling',
  cancellingDetail: 'Cancelling. Wait for the process to end safely.',
  queuedLabel: 'Queued',
  queuedDetail: 'Waiting for the check to start.',
  runningLabel: 'Checking',
  runningDetail:
    'You can continue editing the crease pattern while the check runs.',
  activeLive: '{label}. {phase}.',
  possibleLabel: 'Possible',
  possibleDetail:
    'A layer order satisfying the conditions was constructed and its certificate was verified for the ideal zero-thickness model.',
  possibleLive: 'The global flat-foldability result is Possible.',
  layerOrderModelLabel: 'Layer-order model',
  layerCountLabel: 'Layer count',
  layerCountValue: '{count} layers',
  maximumOverlapLabel: 'Maximum overlap',
  maximumOverlapValue: '{count} ply',
  referenceFaceLabel: 'Reference face',
  faceValue: 'Face {number}',
  layerOrderViewLabel: '3D layer-order view',
  available: 'Available',
  unavailable: 'Unavailable',
  impossibleLabel: 'Impossible',
  impossibleDetail:
    'Finite evidence established that no flat-foldable layer order exists within the initial-release target class.',
  impossibleLive: 'The global flat-foldability result is Impossible.',
  proofTypeLabel: 'Proof type',
  targetFacesLabel: 'Target faces (FaceKey order, up to 20)',
  exhaustiveFacesWithRemaining:
    'All: {faces} ({remaining} more faces)',
  exhaustiveFaces: 'All: {faces}',
  unknownLabel: 'Unknown',
  unknownLive:
    'The global flat-foldability result is Unknown. {reason}',
  unknownReasonLabel: 'Reason for indeterminate result',
  checkModelLabel: 'Check model',
  targetClassLabel: 'Target class',
  elapsedTimeLabel: 'Elapsed time',
  facesLabel: 'Faces',
  overlapCellsLabel: 'Overlap cells',
  constraintsLabel: 'Constraints',
  searchNodesLabel: 'Search nodes',
  progressWithoutTotal:
    '{completed} completed (total still being calculated)',
  progressWithTotal: '{completed} / {total} completed',
  itemCount: '{count}',
  milliseconds: '{milliseconds} ms',
  seconds: '{seconds} s',
  minutes: '{minutes} min',
  minutesAndSeconds: '{minutes} min {seconds} s',
  listSeparator: ', ',
  phases: EN_PHASES,
  unknownReasons: EN_UNKNOWN_REASONS,
  proofLabels: EN_PROOF_LABELS,
  errorMessages: EN_ERROR_MESSAGES,
}) satisfies GlobalFlatFoldabilityPresentationText

export const GLOBAL_FLAT_FOLDABILITY_PRESENTATION_TEXT = Object.freeze({
  ja: JA_TEXT,
  en: EN_TEXT,
}) satisfies Readonly<
  Record<Locale, GlobalFlatFoldabilityPresentationText>
>

const NUMBER_LOCALES = Object.freeze({
  ja: 'ja-JP',
  en: 'en-US',
}) satisfies Readonly<Record<Locale, string>>

function normalizeLocale(locale: unknown): Locale {
  return isLocale(locale) ? locale : DEFAULT_LOCALE
}

export function selectGlobalFlatFoldabilityPresentationText(
  locale: unknown,
): GlobalFlatFoldabilityPresentationText {
  return GLOBAL_FLAT_FOLDABILITY_PRESENTATION_TEXT[normalizeLocale(locale)]
}

export function formatGlobalFlatFoldabilityCount(
  value: number,
  locale: unknown,
) {
  return value.toLocaleString(NUMBER_LOCALES[normalizeLocale(locale)])
}

export function formatGlobalFlatFoldabilityActiveLive(
  label: string,
  phase: string,
  locale: unknown,
) {
  return formatMessage(
    selectGlobalFlatFoldabilityPresentationText(locale).activeLive,
    { label, phase },
  )
}

export function formatGlobalFlatFoldabilityLayerCount(
  count: number,
  locale: unknown,
) {
  return formatMessage(
    selectGlobalFlatFoldabilityPresentationText(locale).layerCountValue,
    { count: formatGlobalFlatFoldabilityCount(count, locale) },
  )
}

export function formatGlobalFlatFoldabilityMaximumOverlap(
  count: number,
  locale: unknown,
) {
  return formatMessage(
    selectGlobalFlatFoldabilityPresentationText(locale).maximumOverlapValue,
    { count: formatGlobalFlatFoldabilityCount(count, locale) },
  )
}

export function formatGlobalFlatFoldabilityFaceNumber(
  faceNumber: number,
  locale: unknown,
) {
  return formatMessage(
    selectGlobalFlatFoldabilityPresentationText(locale).faceValue,
    { number: formatGlobalFlatFoldabilityCount(faceNumber, locale) },
  )
}

export function formatGlobalFlatFoldabilityFaceList(
  faceNumbers: readonly number[],
  locale: unknown,
) {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  return faceNumbers
    .map((faceNumber) =>
      formatGlobalFlatFoldabilityFaceNumber(faceNumber, locale))
    .join(copy.listSeparator)
}

export function formatGlobalFlatFoldabilityExhaustiveFaces(
  faceNumbers: readonly number[],
  totalFaceCount: number,
  locale: unknown,
) {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  const faces = formatGlobalFlatFoldabilityFaceList(faceNumbers, locale)
  const remaining = totalFaceCount - faceNumbers.length
  return remaining > 0
    ? formatMessage(copy.exhaustiveFacesWithRemaining, {
      faces,
      remaining: formatGlobalFlatFoldabilityCount(remaining, locale),
    })
    : formatMessage(copy.exhaustiveFaces, { faces })
}

export function formatGlobalFlatFoldabilityUnknownLive(
  reason: string,
  locale: unknown,
) {
  return formatMessage(
    selectGlobalFlatFoldabilityPresentationText(locale).unknownLive,
    { reason },
  )
}

export function formatGlobalFlatFoldabilityProgressWork(
  completed: number,
  total: number | null,
  locale: unknown,
) {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  return total === null
    ? formatMessage(copy.progressWithoutTotal, {
      completed: formatGlobalFlatFoldabilityCount(completed, locale),
    })
    : formatMessage(copy.progressWithTotal, {
      completed: formatGlobalFlatFoldabilityCount(completed, locale),
      total: formatGlobalFlatFoldabilityCount(total, locale),
    })
}

export function formatGlobalFlatFoldabilityItemCount(
  value: number,
  locale: unknown,
) {
  return formatMessage(
    selectGlobalFlatFoldabilityPresentationText(locale).itemCount,
    { count: formatGlobalFlatFoldabilityCount(value, locale) },
  )
}

export function formatGlobalFlatFoldabilityElapsedMilliseconds(
  milliseconds: number,
  locale: unknown,
) {
  const copy = selectGlobalFlatFoldabilityPresentationText(locale)
  if (milliseconds < 1_000) {
    return formatMessage(copy.milliseconds, {
      milliseconds: formatGlobalFlatFoldabilityCount(milliseconds, locale),
    })
  }
  if (milliseconds < 60_000) {
    const seconds = Math.round(milliseconds / 100) / 10
    return formatMessage(copy.seconds, {
      seconds: seconds.toLocaleString(NUMBER_LOCALES[normalizeLocale(locale)], {
        maximumFractionDigits: 1,
      }),
    })
  }
  const minutes = Math.floor(milliseconds / 60_000)
  const seconds = Math.floor((milliseconds % 60_000) / 1_000)
  return seconds === 0
    ? formatMessage(copy.minutes, {
      minutes: formatGlobalFlatFoldabilityCount(minutes, locale),
    })
    : formatMessage(copy.minutesAndSeconds, {
      minutes: formatGlobalFlatFoldabilityCount(minutes, locale),
      seconds: formatGlobalFlatFoldabilityCount(seconds, locale),
    })
}
