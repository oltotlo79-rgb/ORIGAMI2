import {
  selectLocalizedText,
  type LocalizedText,
} from './i18n.ts'

export type FoldPreviewRenderErrorCode =
  | 'fixed_face_unavailable'
  | 'geometry_unavailable'
  | 'camera_unavailable'
  | 'render_unavailable'
  | 'tree_motion_unavailable'
  | 'tree_pose_application_failed'
  | 'tree_pose_render_failed'
  | 'scene_initialization_failed'
  | 'selection_render_failed'

export type FoldPreviewCorrectionAnalysisPhase =
  | 'preparing'
  | 'static_candidate_preparation'
  | 'static_candidate_analysis'
  | 'candidate_path_preparation'
  | 'candidate_path_analysis'

type FoldPreviewCorrectionDirection = 'increasing' | 'decreasing'

export type FoldPreviewPresentationText = Readonly<{
  trustedStatuses: readonly LocalizedText[]
  renderErrors: Readonly<
    Record<FoldPreviewRenderErrorCode, LocalizedText>
  >
  statusFaceCount: LocalizedText
  statusFaceSingular: LocalizedText
  statusFacePlural: LocalizedText
  statusHingeSingular: LocalizedText
  statusHingePlural: LocalizedText
  statusBlockedCount: LocalizedText
  statusIssueSingular: LocalizedText
  statusIssuePlural: LocalizedText
  statusAnalysisErrorPrefix: LocalizedText
  statusAnalysisFailed: LocalizedText
  statusWaiting: LocalizedText
  thicknessInvalid: LocalizedText
  thicknessEmphasised: LocalizedText
  thicknessLimited: LocalizedText
  thicknessNormal: LocalizedText
  correctionIdleBadge: LocalizedText
  correctionIdleAccessible: LocalizedText
  correctionWorkingBadge: LocalizedText
  correctionWorkingAccessible: LocalizedText
  correctionWorkingLive: LocalizedText
  correctionStaleBadge: LocalizedText
  correctionStaleAccessible: LocalizedText
  correctionNoCandidateBadge: LocalizedText
  correctionNoCandidateAccessible: LocalizedText
  correctionNoCandidateLive: LocalizedText
  correctionIndeterminateBadge: LocalizedText
  correctionIndeterminateAccessible: LocalizedText
  correctionIndeterminateLive: LocalizedText
  correctionPhases: Readonly<
    Record<FoldPreviewCorrectionAnalysisPhase, LocalizedText>
  >
  correctionInvalidCertifiedAccessible: LocalizedText
  correctionInvalidCertifiedLive: LocalizedText
  correctionDirections: Readonly<
    Record<FoldPreviewCorrectionDirection, LocalizedText>
  >
  correctionCertifiedLimitation: LocalizedText
  correctionCertifiedBadge: LocalizedText
  correctionCertifiedAccessible: LocalizedText
  treeAnglesUniform: LocalizedText
  treeAnglesPerHinge: LocalizedText
  treeAnglesAllHinges: LocalizedText
  treeAnglesRange: LocalizedText
  keyboardHingeSelected: LocalizedText
  keyboardFixedFaceSelected: LocalizedText
  keyboardHingeCleared: LocalizedText
  keyboardSelectionChanged: LocalizedText
  numberLocale: LocalizedText
  paperEdgeRatioLabel: LocalizedText
}>

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

const TRUSTED_STATUSES = Object.freeze([
  localized(
    '面・ヒンジ解析待ち',
    'Waiting for face and hinge analysis',
  ),
  localized(
    '面・ヒンジ解析中…',
    'Analyzing faces and hinges…',
  ),
  localized(
    '3D解析はデスクトップ版で利用できます',
    '3D analysis is available in the desktop app',
  ),
  localized(
    '3D入力の整合性検証で遮断',
    'Blocked by 3D input consistency validation',
  ),
])

const RENDER_ERRORS = Object.freeze({
  fixed_face_unavailable: localized(
    '固定面を安全に解決できませんでした',
    'The fixed face could not be resolved safely.',
  ),
  geometry_unavailable: localized(
    '3D面を安全に三角形化できませんでした',
    'The 3D faces could not be triangulated safely.',
  ),
  camera_unavailable: localized(
    '3Dカメラ操作を安全に継続できませんでした',
    'The 3D camera operation could not continue safely.',
  ),
  render_unavailable: localized(
    '3D描画を安全に継続できませんでした',
    '3D rendering could not continue safely.',
  ),
  tree_motion_unavailable: localized(
    '木構造の折り経路を安全に継続できませんでした',
    'The tree-fold motion path could not continue safely.',
  ),
  tree_pose_application_failed: localized(
    '木構造の折り姿勢を安全に適用できませんでした',
    'The tree-fold pose could not be applied safely.',
  ),
  tree_pose_render_failed: localized(
    '木構造の折り姿勢を安全に描画できませんでした',
    'The tree-fold pose could not be rendered safely.',
  ),
  scene_initialization_failed: localized(
    'このPCで3D描画を開始できませんでした',
    '3D rendering could not be started on this PC.',
  ),
  selection_render_failed: localized(
    '3D選択表示を安全に継続できませんでした',
    'The 3D selection display could not continue safely.',
  ),
}) satisfies Readonly<Record<FoldPreviewRenderErrorCode, LocalizedText>>

const CORRECTION_PHASES = Object.freeze({
  preparing: localized('準備中', 'Preparing'),
  static_candidate_preparation: localized(
    '静的候補の準備中',
    'Preparing static candidates',
  ),
  static_candidate_analysis: localized(
    '静的候補を確認中',
    'Checking static candidates',
  ),
  candidate_path_preparation: localized(
    '経路確認の準備中',
    'Preparing path checks',
  ),
  candidate_path_analysis: localized(
    '連続経路を確認中',
    'Checking continuous paths',
  ),
}) satisfies Readonly<
  Record<FoldPreviewCorrectionAnalysisPhase, LocalizedText>
>

const CORRECTION_DIRECTIONS = Object.freeze({
  increasing: localized('増加', 'increase'),
  decreasing: localized('減少', 'decrease'),
}) satisfies Readonly<Record<FoldPreviewCorrectionDirection, LocalizedText>>

export const FOLD_PREVIEW_PRESENTATION_TEXT = Object.freeze({
  trustedStatuses: TRUSTED_STATUSES,
  renderErrors: RENDER_ERRORS,
  statusFaceCount: localized(
    '{faces}面・{hinges}ヒンジ',
    '{faces} {faceNoun} · {hinges} {hingeNoun}',
  ),
  statusFaceSingular: localized('面', 'face'),
  statusFacePlural: localized('面', 'faces'),
  statusHingeSingular: localized('ヒンジ', 'hinge'),
  statusHingePlural: localized('ヒンジ', 'hinges'),
  statusBlockedCount: localized(
    '3D解析で遮断（{count}件）',
    '3D analysis blocked ({count} {issueNoun})',
  ),
  statusIssueSingular: localized('件', 'issue'),
  statusIssuePlural: localized('件', 'issues'),
  statusAnalysisErrorPrefix: localized(
    '3D解析エラー:',
    '3D analysis error:',
  ),
  statusAnalysisFailed: localized(
    '3D解析に失敗しました',
    '3D analysis failed.',
  ),
  statusWaiting: localized(
    '面・ヒンジ解析を待っています',
    'Waiting for face and hinge analysis.',
  ),
  thicknessInvalid: localized(
    '紙厚入力が無効なため3D表示のみ {length}（衝突判定には不使用）',
    'Invalid paper-thickness input; {length} is used only for the 3D display and not for collision checks.',
  ),
  thicknessEmphasised: localized(
    '紙厚 {length}（3D表示は視認用の最小厚、衝突判定は入力紙厚を使用）',
    'Paper thickness {length} (the 3D view uses a visible minimum; collision checks use the entered thickness)',
  ),
  thicknessLimited: localized(
    '紙厚 {length}（3D表示厚を上限調整、衝突判定は入力紙厚を使用）',
    'Paper thickness {length} (3D display thickness is capped; collision checks use the entered thickness)',
  ),
  thicknessNormal: localized(
    '紙厚 {length}',
    'Paper thickness {length}',
  ),
  correctionIdleBadge: localized('待機', 'Waiting'),
  correctionIdleAccessible: localized(
    '補正解析は待機中です。衝突で停止し、完全な解析根拠を得た場合だけ候補を調べます。',
    'Correction analysis is waiting. Candidates are examined only after motion stops at a collision with complete analysis evidence.',
  ),
  correctionWorkingBadge: localized(
    '作業中・{phase}',
    'Working · {phase}',
  ),
  correctionWorkingAccessible: localized(
    '補正解析は作業中です。{phase}。解析結果は3D表示や設計データへ自動適用されません。',
    'Correction analysis is in progress: {phase}. Results are not applied automatically to the 3D view or design data.',
  ),
  correctionWorkingLive: localized(
    '補正候補の解析を開始しました。結果は3D表示や設計データへ自動適用されません。',
    'Correction-candidate analysis started. Results are not applied automatically to the 3D view or design data.',
  ),
  correctionStaleBadge: localized(
    '古い結果を破棄済み',
    'Outdated result discarded',
  ),
  correctionStaleAccessible: localized(
    '姿勢または設計条件が変わったため、以前の補正解析を破棄しました。',
    'The previous correction analysis was discarded because the pose or design conditions changed.',
  ),
  correctionNoCandidateBadge: localized(
    '対応範囲内で候補なし',
    'No candidate in supported scope',
  ),
  correctionNoCandidateAccessible: localized(
    '現在の単一ヒンジ補正解析の対応範囲内では、認定できる候補が見つかりませんでした。折り不可能であることを意味しません。',
    'No certifiable candidate was found within the current single-hinge correction-analysis scope. This does not mean the fold is impossible.',
  ),
  correctionNoCandidateLive: localized(
    '現在の補正解析の対応範囲内では候補が見つかりませんでした。折り不可能であることを意味しません。',
    'No candidate was found within the current correction-analysis scope. This does not mean the fold is impossible.',
  ),
  correctionIndeterminateBadge: localized(
    '判定不能（安全側停止）',
    'Indeterminate (stopped safely)',
  ),
  correctionIndeterminateAccessible: localized(
    '補正解析は安全に判定を完了できなかったため停止しました。候補なしや折り不可能とは区別されます。',
    'Correction analysis stopped because it could not complete safely. This is distinct from finding no candidate or proving the fold impossible.',
  ),
  correctionIndeterminateLive: localized(
    '補正解析は判定不能として安全側に停止しました。候補なしや折り不可能とは区別されます。',
    'Correction analysis stopped safely with an indeterminate result. This is distinct from finding no candidate or proving the fold impossible.',
  ),
  correctionPhases: CORRECTION_PHASES,
  correctionInvalidCertifiedAccessible: localized(
    '補正解析結果を安全に表示できませんでした。結果は3D表示や設計データへ適用されていません。',
    'The correction-analysis result could not be displayed safely. It has not been applied to the 3D view or design data.',
  ),
  correctionInvalidCertifiedLive: localized(
    '補正解析結果を安全に表示できませんでした。',
    'The correction-analysis result could not be displayed safely.',
  ),
  correctionDirections: CORRECTION_DIRECTIONS,
  correctionCertifiedLimitation: localized(
    '解析時点の結果で、現在も有効であることは保証されません。現在姿勢から安全に移動できることを示さず、この表示から3D表示や設計データへ適用できません。層順と材料変形も未確認です。',
    'This result reflects the pose at analysis time and may no longer be current. It does not prove a safe motion from the current pose and cannot be applied from this display to the 3D view or design data. Layer order and material deformation are also unchecked.',
  ),
  correctionCertifiedBadge: localized(
    '解析上の補正候補{rank}・静的／連続経路確認済み（現在姿勢未照合）・{source}° → {target}°',
    'Analysis-only correction candidate {rank} · static and continuous path checked (current pose not matched) · {source}° → {target}°',
  ),
  correctionCertifiedAccessible: localized(
    '補正候補{rank}。選択した折り目を{source}度から{target}度へ{delta}度{direction}する単一ヒンジ経路は、静的衝突検査と連続経路検査を通過しました。{limitation}',
    'Correction candidate {rank}. The single-hinge path that would {direction} the selected crease by {delta} degrees, from {source} to {target} degrees, passed the static collision and continuous-path checks. {limitation}',
  ),
  treeAnglesUniform: localized(
    '一括 {angle}度',
    'Uniform {angle}°',
  ),
  treeAnglesPerHinge: localized('個別角度', 'Per-hinge angles'),
  treeAnglesAllHinges: localized(
    '全ヒンジ {angle}度',
    'All hinges {angle}°',
  ),
  treeAnglesRange: localized(
    '個別 {minimum}〜{maximum}度',
    'Per hinge {minimum}–{maximum}°',
  ),
  keyboardHingeSelected: localized(
    'ヒンジ {index}/{total} を選択しました',
    'Selected hinge {index} of {total}.',
  ),
  keyboardFixedFaceSelected: localized(
    '面 {index}/{total} を固定面に設定しました',
    'Set face {index} of {total} as the fixed face.',
  ),
  keyboardHingeCleared: localized(
    'ヒンジ選択を解除しました',
    'Cleared the hinge selection.',
  ),
  keyboardSelectionChanged: localized(
    '3D選択を変更しました',
    'The 3D selection changed.',
  ),
  numberLocale: localized('ja-JP', 'en-US'),
  paperEdgeRatioLabel: localized('紙辺比', 'paper-edge ratio'),
}) satisfies FoldPreviewPresentationText

export const FOLD_PREVIEW_PRESENTATION_INPUT = Object.freeze({
  statusFaceCountPatterns: Object.freeze({
    ja: Object.freeze(/^(\d+)面・(\d+)ヒンジ$/u),
    en: Object.freeze(/^(\d+) faces · (\d+) hinges$/u),
  }),
  statusBlockedCountPatterns: Object.freeze({
    ja: Object.freeze(/^3D解析で遮断（(\d+)件）$/u),
    en: Object.freeze(/^3D analysis blocked \((\d+) issues\)$/u),
  }),
  keyboardHingeSelectedPattern:
    Object.freeze(/^ヒンジ (\d+)\/(\d+) を選択しました$/u),
  keyboardFixedFaceSelectedPattern:
    Object.freeze(/^面 (\d+)\/(\d+) を固定面に設定しました$/u),
  keyboardHingeCleared: FOLD_PREVIEW_PRESENTATION_TEXT
    .keyboardHingeCleared.ja,
})

export function formatFoldPreviewPresentationAngle(
  value: number,
  locale: unknown,
): string {
  return value.toLocaleString(selectFoldPreviewNumberLocale(locale), {
    maximumFractionDigits: 1,
  })
}

export function selectFoldPreviewNumberLocale(locale: unknown): string {
  return selectLocalizedText(
    locale,
    FOLD_PREVIEW_PRESENTATION_TEXT.numberLocale,
  )
}

export function localizeFoldPreviewPaperEdgeRatioLength(
  formatted: string,
  unitLabel: string,
  locale: unknown,
): string {
  const sourceLabel = FOLD_PREVIEW_PRESENTATION_TEXT.paperEdgeRatioLabel.ja
  if (unitLabel !== sourceLabel || !formatted.endsWith(sourceLabel)) {
    return formatted
  }
  const targetLabel = selectLocalizedText(
    locale,
    FOLD_PREVIEW_PRESENTATION_TEXT.paperEdgeRatioLabel,
  )
  return `${formatted.slice(0, -sourceLabel.length)}${targetLabel}`
}
