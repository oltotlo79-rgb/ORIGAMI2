import type {
  FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
} from './foldPreviewTreeSingleHingeCorrectionAnalysisCoordinator.ts'
import type { FoldPreviewHingeAngle } from './foldPreviewKinematics.ts'
import type { Locale } from './i18n.ts'
import type { ResolvedLengthDisplayUnit } from './lengthUnit.ts'

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

export type FoldPreviewKeyboardAnnouncement =
  | Readonly<{
    kind: 'hinge_selected'
    index: number
    total: number
  }>
  | Readonly<{
    kind: 'fixed_face_selected'
    index: number
    total: number
  }>
  | Readonly<{
    kind: 'hinge_cleared'
  }>
  | Readonly<{
    kind: 'selection_changed'
  }>

export type FoldPreviewCorrectionAnalysisView = Readonly<{
  badgeText: string
  badgeClass: string
  accessibleText: string
  liveText: string
}>

const STATUS_MESSAGES = Object.freeze([
  Object.freeze({
    ja: '面・ヒンジ解析待ち',
    en: 'Waiting for face and hinge analysis',
  }),
  Object.freeze({
    ja: '面・ヒンジ解析中…',
    en: 'Analyzing faces and hinges…',
  }),
  Object.freeze({
    ja: '3D解析はデスクトップ版で利用できます',
    en: '3D analysis is available in the desktop app',
  }),
  Object.freeze({
    ja: '3D入力の整合性検証で遮断',
    en: 'Blocked by 3D input consistency validation',
  }),
])

const JAPANESE_FACE_COUNT = /^(\d+)面・(\d+)ヒンジ$/u
const ENGLISH_FACE_COUNT = /^(\d+) faces · (\d+) hinges$/u
const JAPANESE_BLOCKED_COUNT = /^3D解析で遮断（(\d+)件）$/u
const ENGLISH_BLOCKED_COUNT = /^3D analysis blocked \((\d+) issues\)$/u
const JAPANESE_HINGE_ANNOUNCEMENT =
  /^ヒンジ (\d+)\/(\d+) を選択しました$/u
const JAPANESE_FACE_ANNOUNCEMENT =
  /^面 (\d+)\/(\d+) を固定面に設定しました$/u

export function foldPreviewText(
  locale: Locale,
  ja: string,
  en: string,
): string {
  return locale === 'en' ? en : ja
}

export function describeFoldPreviewRenderError(
  code: FoldPreviewRenderErrorCode,
  locale: Locale,
): string {
  const messages: Readonly<Record<
    FoldPreviewRenderErrorCode,
    Readonly<{ ja: string; en: string }>
  >> = {
    fixed_face_unavailable: {
      ja: '固定面を安全に解決できませんでした',
      en: 'The fixed face could not be resolved safely.',
    },
    geometry_unavailable: {
      ja: '3D面を安全に三角形化できませんでした',
      en: 'The 3D faces could not be triangulated safely.',
    },
    camera_unavailable: {
      ja: '3Dカメラ操作を安全に継続できませんでした',
      en: 'The 3D camera operation could not continue safely.',
    },
    render_unavailable: {
      ja: '3D描画を安全に継続できませんでした',
      en: '3D rendering could not continue safely.',
    },
    tree_motion_unavailable: {
      ja: '木構造の折り経路を安全に継続できませんでした',
      en: 'The tree-fold motion path could not continue safely.',
    },
    tree_pose_application_failed: {
      ja: '木構造の折り姿勢を安全に適用できませんでした',
      en: 'The tree-fold pose could not be applied safely.',
    },
    tree_pose_render_failed: {
      ja: '木構造の折り姿勢を安全に描画できませんでした',
      en: 'The tree-fold pose could not be rendered safely.',
    },
    scene_initialization_failed: {
      ja: 'このPCで3D描画を開始できませんでした',
      en: '3D rendering could not be started on this PC.',
    },
    selection_render_failed: {
      ja: '3D選択表示を安全に継続できませんでした',
      en: 'The 3D selection display could not continue safely.',
    },
  }
  return foldPreviewText(locale, messages[code].ja, messages[code].en)
}

/**
 * Converts the App-owned topology status into a small trusted presentation
 * vocabulary. Unknown or error-suffixed input is never copied into the UI.
 */
export function describeFoldPreviewStatus(
  value: unknown,
  locale: Locale,
): string {
  if (typeof value === 'string') {
    for (const message of STATUS_MESSAGES) {
      if (value === message.ja || value === message.en) {
        return foldPreviewText(locale, message.ja, message.en)
      }
    }

    const faceCount = matchSafeCounts(value, [
      JAPANESE_FACE_COUNT,
      ENGLISH_FACE_COUNT,
    ])
    if (faceCount) {
      const [faces, hinges] = faceCount
      return foldPreviewText(
        locale,
        `${faces}面・${hinges}ヒンジ`,
        `${faces} ${faces === 1 ? 'face' : 'faces'} · ${hinges} ${hinges === 1 ? 'hinge' : 'hinges'}`,
      )
    }

    const blockedCount = matchSafeCounts(value, [
      JAPANESE_BLOCKED_COUNT,
      ENGLISH_BLOCKED_COUNT,
    ])
    if (blockedCount) {
      const count = blockedCount[0]
      return foldPreviewText(
        locale,
        `3D解析で遮断（${count}件）`,
        `3D analysis blocked (${count} ${count === 1 ? 'issue' : 'issues'})`,
      )
    }

    if (
      value.startsWith('3D解析エラー:')
      || value.startsWith('3D analysis error:')
    ) {
      return foldPreviewText(
        locale,
        '3D解析に失敗しました',
        '3D analysis failed.',
      )
    }
  }
  return foldPreviewText(
    locale,
    '面・ヒンジ解析を待っています',
    'Waiting for face and hinge analysis.',
  )
}

export function describeFoldPreviewThickness(
  input: Readonly<{
    hasAuthoritativeThickness: boolean
    thicknessIsEmphasised: boolean
    thicknessIsLimited: boolean
    formattedLength: string
    lengthDisplayUnit: ResolvedLengthDisplayUnit
  }>,
  locale: Locale,
): string {
  const length = formatFoldPreviewLength(
    input.formattedLength,
    input.lengthDisplayUnit,
    locale,
  )
  if (!input.hasAuthoritativeThickness) {
    return foldPreviewText(
      locale,
      `紙厚入力が無効なため3D表示のみ ${length}（衝突判定には不使用）`,
      `Invalid paper-thickness input; ${length} is used only for the 3D display and not for collision checks.`,
    )
  }
  if (input.thicknessIsEmphasised) {
    return foldPreviewText(
      locale,
      `紙厚 ${length}（3D表示は視認用の最小厚、衝突判定は入力紙厚を使用）`,
      `Paper thickness ${length} (the 3D view uses a visible minimum; collision checks use the entered thickness)`,
    )
  }
  if (input.thicknessIsLimited) {
    return foldPreviewText(
      locale,
      `紙厚 ${length}（3D表示厚を上限調整、衝突判定は入力紙厚を使用）`,
      `Paper thickness ${length} (3D display thickness is capped; collision checks use the entered thickness)`,
    )
  }
  return foldPreviewText(
    locale,
    `紙厚 ${length}`,
    `Paper thickness ${length}`,
  )
}

export function describeFoldPreviewCorrectionAnalysis(
  state: FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
  locale: Locale,
): FoldPreviewCorrectionAnalysisView {
  switch (state.status) {
    case 'idle':
      return {
        badgeText: foldPreviewText(locale, '待機', 'Waiting'),
        badgeClass: 'is-idle',
        accessibleText: foldPreviewText(
          locale,
          '補正解析は待機中です。衝突で停止し、完全な解析根拠を得た場合だけ候補を調べます。',
          'Correction analysis is waiting. Candidates are examined only after motion stops at a collision with complete analysis evidence.',
        ),
        liveText: '',
      }
    case 'working': {
      const phaseText = correctionAnalysisPhaseText(state.phase, locale)
      return {
        badgeText: foldPreviewText(
          locale,
          `作業中・${phaseText}`,
          `Working · ${phaseText}`,
        ),
        badgeClass: 'is-working',
        accessibleText: foldPreviewText(
          locale,
          `補正解析は作業中です。${phaseText}。解析結果は3D表示や設計データへ自動適用されません。`,
          `Correction analysis is in progress: ${phaseText}. Results are not applied automatically to the 3D view or design data.`,
        ),
        liveText: foldPreviewText(
          locale,
          '補正候補の解析を開始しました。結果は3D表示や設計データへ自動適用されません。',
          'Correction-candidate analysis started. Results are not applied automatically to the 3D view or design data.',
        ),
      }
    }
    case 'stale':
      return {
        badgeText: foldPreviewText(
          locale,
          '古い結果を破棄済み',
          'Outdated result discarded',
        ),
        badgeClass: 'is-stale',
        accessibleText: foldPreviewText(
          locale,
          '姿勢または設計条件が変わったため、以前の補正解析を破棄しました。',
          'The previous correction analysis was discarded because the pose or design conditions changed.',
        ),
        liveText: foldPreviewText(
          locale,
          '姿勢または設計条件が変わったため、以前の補正解析を破棄しました。',
          'The previous correction analysis was discarded because the pose or design conditions changed.',
        ),
      }
    case 'no_candidate':
      return {
        badgeText: foldPreviewText(
          locale,
          '対応範囲内で候補なし',
          'No candidate in supported scope',
        ),
        badgeClass: 'is-no-candidate',
        accessibleText: foldPreviewText(
          locale,
          '現在の単一ヒンジ補正解析の対応範囲内では、認定できる候補が見つかりませんでした。折り不可能であることを意味しません。',
          'No certifiable candidate was found within the current single-hinge correction-analysis scope. This does not mean the fold is impossible.',
        ),
        liveText: foldPreviewText(
          locale,
          '現在の補正解析の対応範囲内では候補が見つかりませんでした。折り不可能であることを意味しません。',
          'No candidate was found within the current correction-analysis scope. This does not mean the fold is impossible.',
        ),
      }
    case 'indeterminate':
      return {
        badgeText: foldPreviewText(
          locale,
          '判定不能（安全側停止）',
          'Indeterminate (stopped safely)',
        ),
        badgeClass: 'is-indeterminate',
        accessibleText: foldPreviewText(
          locale,
          '補正解析は安全に判定を完了できなかったため停止しました。候補なしや折り不可能とは区別されます。',
          'Correction analysis stopped because it could not complete safely. This is distinct from finding no candidate or proving the fold impossible.',
        ),
        liveText: foldPreviewText(
          locale,
          '補正解析は判定不能として安全側に停止しました。候補なしや折り不可能とは区別されます。',
          'Correction analysis stopped safely with an indeterminate result. This is distinct from finding no candidate or proving the fold impossible.',
        ),
      }
    case 'certified':
      return certifiedCorrectionAnalysisView(state.presentation, locale)
  }
}

export function describeFoldPreviewTreeAngles(
  hingeAngles: readonly FoldPreviewHingeAngle[] | undefined,
  uniformAngle: number,
  locale: Locale,
): string {
  if (!hingeAngles || hingeAngles.length === 0) {
    const angle = formatFoldPreviewAngle(uniformAngle, locale)
    return foldPreviewText(
      locale,
      `一括 ${angle}度`,
      `Uniform ${angle}°`,
    )
  }
  const values = hingeAngles.map(({ angleDegrees }) => angleDegrees)
  if (
    !values.every(
      (value) => Number.isFinite(value) && value >= 0 && value <= 180,
    )
  ) {
    return foldPreviewText(locale, '個別角度', 'Per-hinge angles')
  }
  const minimum = Math.min(...values)
  const maximum = Math.max(...values)
  const minimumText = formatFoldPreviewAngle(minimum, locale)
  const maximumText = formatFoldPreviewAngle(maximum, locale)
  return minimum === maximum
    ? foldPreviewText(
        locale,
        `全ヒンジ ${minimumText}度`,
        `All hinges ${minimumText}°`,
      )
    : foldPreviewText(
        locale,
        `個別 ${minimumText}〜${maximumText}度`,
        `Per hinge ${minimumText}–${maximumText}°`,
      )
}

export function formatFoldPreviewAngle(
  value: number,
  locale: Locale,
): string {
  return value.toLocaleString(locale === 'en' ? 'en-US' : 'ja-JP', {
    maximumFractionDigits: 1,
  })
}

export function normalizeFoldPreviewKeyboardAnnouncement(
  value: unknown,
): FoldPreviewKeyboardAnnouncement {
  if (value === 'ヒンジ選択を解除しました') {
    return Object.freeze({ kind: 'hinge_cleared' })
  }
  if (typeof value !== 'string') {
    return Object.freeze({ kind: 'selection_changed' })
  }
  const hingeMatch = JAPANESE_HINGE_ANNOUNCEMENT.exec(value)
  if (hingeMatch) {
    const counts = safeAnnouncementCounts(hingeMatch[1], hingeMatch[2])
    if (counts) {
      return Object.freeze({
        kind: 'hinge_selected',
        index: counts[0],
        total: counts[1],
      })
    }
  }
  const faceMatch = JAPANESE_FACE_ANNOUNCEMENT.exec(value)
  if (faceMatch) {
    const counts = safeAnnouncementCounts(faceMatch[1], faceMatch[2])
    if (counts) {
      return Object.freeze({
        kind: 'fixed_face_selected',
        index: counts[0],
        total: counts[1],
      })
    }
  }
  return Object.freeze({ kind: 'selection_changed' })
}

export function describeFoldPreviewKeyboardAnnouncement(
  announcement: FoldPreviewKeyboardAnnouncement,
  locale: Locale,
): string {
  switch (announcement.kind) {
    case 'hinge_selected':
      return foldPreviewText(
        locale,
        `ヒンジ ${announcement.index}/${announcement.total} を選択しました`,
        `Selected hinge ${announcement.index} of ${announcement.total}.`,
      )
    case 'fixed_face_selected':
      return foldPreviewText(
        locale,
        `面 ${announcement.index}/${announcement.total} を固定面に設定しました`,
        `Set face ${announcement.index} of ${announcement.total} as the fixed face.`,
      )
    case 'hinge_cleared':
      return foldPreviewText(
        locale,
        'ヒンジ選択を解除しました',
        'Cleared the hinge selection.',
      )
    case 'selection_changed':
      return foldPreviewText(
        locale,
        '3D選択を変更しました',
        'The 3D selection changed.',
      )
  }
}

function correctionAnalysisPhaseText(
  phase: Extract<
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
    { status: 'working' }
  >['phase'],
  locale: Locale,
) {
  switch (phase) {
    case 'preparing':
      return foldPreviewText(locale, '準備中', 'Preparing')
    case 'static_candidate_preparation':
      return foldPreviewText(
        locale,
        '静的候補の準備中',
        'Preparing static candidates',
      )
    case 'static_candidate_analysis':
      return foldPreviewText(
        locale,
        '静的候補を確認中',
        'Checking static candidates',
      )
    case 'candidate_path_preparation':
      return foldPreviewText(
        locale,
        '経路確認の準備中',
        'Preparing path checks',
      )
    case 'candidate_path_analysis':
      return foldPreviewText(
        locale,
        '連続経路を確認中',
        'Checking continuous paths',
      )
  }
}

function certifiedCorrectionAnalysisView(
  presentation: Extract<
    FoldPreviewTreeSingleHingeCorrectionAnalysisCoordinatorState,
    { status: 'certified' }
  >['presentation'],
  locale: Locale,
): FoldPreviewCorrectionAnalysisView {
  const rank = presentation.candidate.rank
  const source = presentation.angles.sourceDegrees
  const target = presentation.angles.targetDegrees
  const delta = presentation.angles.absoluteDeltaDegrees
  const expectedDelta = Math.abs(target - source)
  const expectedDirection = target > source ? 'increasing' : 'decreasing'
  if (
    !Number.isSafeInteger(rank)
    || rank < 1
    || !validAngle(source)
    || !validAngle(target)
    || source === target
    || !Number.isFinite(delta)
    || delta <= 0
    || delta > 180
    || delta !== expectedDelta
    || presentation.angles.deltaDegrees !== target - source
    || presentation.angles.direction !== expectedDirection
  ) {
    return {
      badgeText: foldPreviewText(
        locale,
        '判定不能（安全側停止）',
        'Indeterminate (stopped safely)',
      ),
      badgeClass: 'is-indeterminate',
      accessibleText: foldPreviewText(
        locale,
        '補正解析結果を安全に表示できませんでした。結果は3D表示や設計データへ適用されていません。',
        'The correction-analysis result could not be displayed safely. It has not been applied to the 3D view or design data.',
      ),
      liveText: foldPreviewText(
        locale,
        '補正解析結果を安全に表示できませんでした。',
        'The correction-analysis result could not be displayed safely.',
      ),
    }
  }

  const sourceText = preciseAngle(source)
  const targetText = preciseAngle(target)
  const deltaText = preciseAngle(delta)
  const direction = presentation.angles.direction === 'increasing'
    ? foldPreviewText(locale, '増加', 'increase')
    : foldPreviewText(locale, '減少', 'decrease')
  const limitation = foldPreviewText(
    locale,
    '解析時点の結果で、現在も有効であることは保証されません。現在姿勢から安全に移動できることを示さず、この表示から3D表示や設計データへ適用できません。層順と材料変形も未確認です。',
    'This result reflects the pose at analysis time and may no longer be current. It does not prove a safe motion from the current pose and cannot be applied from this display to the 3D view or design data. Layer order and material deformation are also unchecked.',
  )
  const badgeText = foldPreviewText(
    locale,
    `解析上の補正候補${rank}・静的／連続経路確認済み（現在姿勢未照合）・${sourceText}° → ${targetText}°`,
    `Analysis-only correction candidate ${rank} · static and continuous path checked (current pose not matched) · ${sourceText}° → ${targetText}°`,
  )
  const accessibleText = foldPreviewText(
    locale,
    `補正候補${rank}。選択した折り目を${sourceText}度から${targetText}度へ${deltaText}度${direction}する単一ヒンジ経路は、静的衝突検査と連続経路検査を通過しました。${limitation}`,
    `Correction candidate ${rank}. The single-hinge path that would ${direction} the selected crease by ${deltaText} degrees, from ${sourceText} to ${targetText} degrees, passed the static collision and continuous-path checks. ${limitation}`,
  )
  return {
    badgeText,
    badgeClass: 'is-certified',
    accessibleText,
    liveText: accessibleText,
  }
}

function formatFoldPreviewLength(
  formatted: string,
  unit: ResolvedLengthDisplayUnit,
  locale: Locale,
) {
  return locale === 'en' && unit.label === '紙辺比'
    ? formatted.replace(/紙辺比$/u, 'paper-edge ratio')
    : formatted
}

function matchSafeCounts(
  value: string,
  expressions: readonly RegExp[],
): readonly number[] | null {
  for (const expression of expressions) {
    const match = expression.exec(value)
    if (!match) continue
    const counts = match.slice(1).map(Number)
    if (
      counts.every(
        (count) => Number.isSafeInteger(count) && count >= 0,
      )
    ) return counts
  }
  return null
}

function safeAnnouncementCounts(
  indexValue: string | undefined,
  totalValue: string | undefined,
): readonly [number, number] | null {
  const index = Number(indexValue)
  const total = Number(totalValue)
  return Number.isSafeInteger(index)
    && Number.isSafeInteger(total)
    && index > 0
    && total > 0
    && index <= total
    ? Object.freeze([index, total])
    : null
}

function validAngle(value: number) {
  return Number.isFinite(value) && value >= 0 && value <= 180
}

function preciseAngle(value: number) {
  const rounded = Math.round(value * 1_000) / 1_000
  return Object.is(rounded, -0) ? '0' : String(rounded)
}
