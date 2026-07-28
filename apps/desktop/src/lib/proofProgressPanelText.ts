import type { LocalizedText } from './i18n.ts'
import type {
  ProofFailureLocation,
  ProofFailureReason,
  ProofProgressState,
  UnprovenHistoryStatus,
} from './proofProgressModel.ts'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

const status = Object.freeze({
  proving: localized('証明中', 'Proving'),
  certified: localized('証明済み', 'Certified'),
  blocked: localized('証明失敗', 'Proof failed'),
  evidence_insufficient: localized(
    '証明不能（証拠不足）',
    'Unproven (insufficient evidence)',
  ),
  resource_limit: localized('資源上限', 'Resource limit'),
  cancelled: localized('取消', 'Cancelled'),
  deadline: localized('期限到達', 'Deadline reached'),
  stale: localized('古い結果', 'Stale result'),
}) satisfies Readonly<Record<ProofProgressState, LocalizedText>>

const locations = Object.freeze({
  applied_trimmed_base: localized(
    '履歴上限より前の適用済み操作',
    'Applied operation before the retained undo history',
  ),
  applied_retained_undo: localized(
    'Undo可能な適用済み操作',
    'Applied operation retained in undo history',
  ),
  unapplied_redo: localized(
    '現在は未適用のRedo操作',
    'Currently unapplied operation in redo history',
  ),
}) satisfies Readonly<Record<ProofFailureLocation, LocalizedText>>

const reasons = Object.freeze({
  blocked: localized('衝突が証明されました', 'A blocking collision was proven'),
  evidence_insufficient: localized(
    '証拠が不足しています',
    'The available evidence is insufficient',
  ),
  resource_limit: localized('証明が資源上限に達しました', 'Proof reached its resource limit'),
  cancelled: localized('証明が取り消されました', 'Proof was cancelled'),
  deadline: localized('証明が期限に達しました', 'Proof reached its deadline'),
}) satisfies Readonly<Record<ProofFailureReason, LocalizedText>>

const unprovenStatuses = Object.freeze({
  awaitingProof: localized('証明待ち', 'Awaiting proof'),
  proofBlocked: localized('証明失敗', 'Proof blocked'),
  unknownEvidenceInsufficient: localized(
    '証拠不足',
    'Evidence insufficient',
  ),
  unknownResourceLimit: localized('資源上限', 'Resource limit'),
  unknownCancelled: localized('取消', 'Cancelled'),
  unknownDeadlineReached: localized('期限到達', 'Deadline reached'),
}) satisfies Readonly<Record<UnprovenHistoryStatus, LocalizedText>>

export const PROOF_PROGRESS_PANEL_TEXT = Object.freeze({
  ariaLabel: localized('証明の進捗', 'Proof progress'),
  title: localized('証明の進捗', 'Proof progress'),
  postApplyStarting: localized(
    '事後証明ジョブを開始しています。',
    'Starting the post-Apply proof job.',
  ),
  postApplyUnavailable: localized(
    '事後証明の進捗を取得できません。折り操作は未証明のままです。',
    'Post-Apply proof progress is unavailable. The fold remains unproven.',
  ),
  statusLabel: localized('状態', 'Status'),
  status,
  certifiedBadge: localized('証明済み', 'Proven'),
  unprovenBadge: localized('未証明', 'Unproven'),
  pairProgress: localized(
    '証明済みペア {proven} / 全ペア {total}',
    'Proven pairs {proven} / total pairs {total}',
  ),
  pairProgressUnknownTotal: localized(
    '証明済みペア {proven} / 全ペア 不明',
    'Proven pairs {proven} / total pairs unknown',
  ),
  appliedUnprovenWarning: localized(
    '未証明の折り操作 {count} 件が現在の文書に適用されています。',
    '{count} unproven fold operation(s) are applied to the current document.',
  ),
  unappliedRedoNotice: localized(
    '未証明の折り操作 {count} 件はRedo履歴にのみあり、現在は未適用です。',
    '{count} unproven fold operation(s) exist only in redo history and are currently unapplied.',
  ),
  unprovenCounts: localized(
    '適用中 {applied} 件 / 現在は未適用のRedo {redo} 件',
    'Applied {applied} / currently unapplied redo {redo}',
  ),
  appliedBreakdown: localized('適用中の未証明内訳', 'Applied unproven breakdown'),
  redoBreakdown: localized(
    '現在は未適用のRedo内訳',
    'Currently unapplied redo breakdown',
  ),
  unprovenStatuses,
  unprovenSummaryUnavailable: localized(
    '未証明状態の件数を安全に確認できません。証明済みとして扱いません。',
    'Unproven-state counts could not be verified safely. They are not treated as proven.',
  ),
  speculativeApplyWarning: localized(
    'この適用は未証明です。安全性の証明ではなく、未証明として履歴に記録され、事後証明の対象になります。',
    'This Apply is unproven. It is not a safety certificate; it is recorded in history as unproven and is a candidate for post-Apply proof.',
  ),
  speculativeApplyGroup: localized(
    '未証明の投機的適用',
    'Unproven speculative Apply',
  ),
  speculativeConfirmation: localized(
    '未証明であり、安全性の証明ではないことを理解して適用します。',
    'I understand this is unproven and is not a safety certificate.',
  ),
  applySpeculative: localized(
    '未証明の折り重ねを適用',
    'Apply unproven stacked fold',
  ),
  applyingSpeculative: localized(
    '未証明の折り重ねを適用中…',
    'Applying unproven stacked fold…',
  ),
  proofFailureTitle: localized('事後証明の結果', 'Post-Apply proof result'),
  failureLocationLabel: localized('対象', 'Affected operation'),
  failureReasonLabel: localized('理由', 'Reason'),
  locations,
  reasons,
  subsequentEdits: localized(
    'この操作の後に {count} 件の編集があります。',
    '{count} edit(s) were made after this operation.',
  ),
  revertUnavailable: localized(
    'この履歴位置は直接Undoできません。',
    'This history location cannot be reverted directly.',
  ),
  destructiveConfirmation: localized(
    '後続編集が失われる可能性を理解し、戻す操作を確認します。',
    'I understand that subsequent edits may be lost and confirm the revert.',
  ),
  revertSteps: localized(
    '{steps} 手分を戻すよう要求',
    'Request revert by {steps} undo step(s)',
  ),
  revertRequested: localized('戻す要求を送信しました。', 'The revert request was submitted.'),
})
