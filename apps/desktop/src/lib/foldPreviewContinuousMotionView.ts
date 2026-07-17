import type {
  FoldPreviewContinuousMotionRunnerState,
} from './foldPreviewContinuousMotionRunner'

export type FoldPreviewContinuousMotionView = Readonly<{
  status:
    | 'preparing'
    | 'idle'
    | 'running'
    | 'clear'
    | 'blocked'
    | 'indeterminate'
    | 'unavailable'
  badgeClass: string
  badgeText: string
  accessibleText: string
  terminalAnnouncement: string | null
}>

const LIMITATION =
  '中央面基準の単一ヒンジ線形経路に限る判定で、実際の折り癖と層ずれは含みません'

/**
 * Converts the continuous runner's safety state into deliberately qualified
 * Japanese UI copy. A missing or malformed snapshot never reads as safe.
 */
export function describeFoldPreviewContinuousMotion(
  state: FoldPreviewContinuousMotionRunnerState | null,
): FoldPreviewContinuousMotionView {
  if (!state) {
    return view(
      'preparing',
      'is-pending',
      '経路判定を準備中',
      `単一ヒンジの連続経路判定を準備しています。${LIMITATION}`,
    )
  }
  if (
    !validAngle(state.applied)
    || !validAngle(state.start)
    || (state.requested !== null && !validAngle(state.requested))
  ) {
    return view(
      'unavailable',
      'is-unavailable',
      '経路判定不能',
      `連続経路の角度状態が不正なため判定を利用できません。${LIMITATION}`,
    )
  }

  const applied = formatMotionAngle(state.applied)
  const requested = state.requested === null
    ? null
    : formatMotionAngle(state.requested)
  if (state.status === 'idle') {
    return view(
      'idle',
      'is-pending',
      `経路判定待機・表示 ${applied}°`,
      `単一ヒンジの連続経路判定は待機中です。現在の表示角は${applied}度です。${LIMITATION}`,
    )
  }
  if (state.status === 'running' && requested !== null) {
    return view(
      'running',
      'is-running',
      `経路検証中・表示 ${applied}° / 指定 ${requested}°`,
      `指定角${requested}度への連続経路を検証中です。現在の表示角は${applied}度です。判定完了までは経路確認済みとして扱いません。${LIMITATION}`,
    )
  }
  if (
    state.status === 'clear'
    && requested !== null
    && validClearResult(state.result)
    && state.applied === state.requested
  ) {
    const text =
      `指定角${requested}度までの連続経路を確認しました。表示角は${applied}度です。${LIMITATION}`
    return view(
      'clear',
      'is-clear',
      `中央面・単一経路確認済み・表示 ${applied}°`,
      text,
      text,
    )
  }
  if (
    state.status === 'blocked'
    && requested !== null
    && validBlockedResult(state.result)
    && appliedMatchesCertifiedTime(state, state.result.certifiedSafeThrough)
  ) {
    if (
      state.result.certifiedSafeThrough === 0
      && state.result.unsafeBracket[1] === 0
    ) {
      const text =
        `開始姿勢で衝突を検出しました。表示角は${applied}度ですが、安全確認済みの姿勢として扱いません。${LIMITATION}`
      return view(
        'blocked',
        'is-blocked',
        `開始姿勢で衝突・安全確認なし / 指定 ${requested}°`,
        text,
        text,
      )
    }
    if (state.result.certifiedSafeThrough === 0) {
      const text =
        `開始姿勢の点判定は通過しましたが、直後の区間で衝突を検出したため、連続経路として安全な移動量を確認できません。表示角${applied}度から進めません。${LIMITATION}`
      return view(
        'blocked',
        'is-blocked',
        `開始直後の区間で衝突・移動なし / 指定 ${requested}°`,
        text,
        text,
      )
    }
    const text =
      `指定角${requested}度への経路で衝突を検出したため、最後に経路を確認できた${applied}度で停止しました。${LIMITATION}`
    return view(
      'blocked',
      'is-blocked',
      `衝突手前で停止・表示 ${applied}° / 指定 ${requested}°`,
      text,
      text,
    )
  }
  if (
    state.status === 'indeterminate'
    && requested !== null
    && state.result === null
  ) {
    const text =
      `指定角${requested}度への経路判定を開始または継続できないため、現在の表示角${applied}度から進めません。表示角は安全確認済みとして扱いません。${LIMITATION}`
    return view(
      'indeterminate',
      'is-indeterminate',
      `経路判定不能・表示 ${applied}° / 指定 ${requested}°`,
      text,
      text,
    )
  }
  if (
    state.status === 'indeterminate'
    && requested !== null
    && validIndeterminateResult(state.result)
    && appliedMatchesCertifiedTime(state, state.result.certifiedSafeThrough)
  ) {
    if (
      state.result.certifiedSafeThrough === 0
      && state.result.unresolvedBracket[1] === 0
    ) {
      const text =
        `開始姿勢を判定できないため、現在の表示角${applied}度から進めません。表示角は安全確認済みとして扱いません。${LIMITATION}`
      return view(
        'indeterminate',
        'is-indeterminate',
        `開始姿勢を判定不能・安全確認なし / 指定 ${requested}°`,
        text,
        text,
      )
    }
    if (state.result.certifiedSafeThrough === 0) {
      const text =
        `開始姿勢の点判定は通過しましたが、直後の区間を確認できないため、連続経路として安全な移動量を確認できません。表示角${applied}度から進めません。${LIMITATION}`
      return view(
        'indeterminate',
        'is-indeterminate',
        `開始直後の区間を判定不能・移動なし / 指定 ${requested}°`,
        text,
        text,
      )
    }
    const text =
      `指定角${requested}度までの安全を確認できないため、最後に経路を確認できた${applied}度で停止しました。${LIMITATION}`
    return view(
      'indeterminate',
      'is-indeterminate',
      `経路を確認できず停止・表示 ${applied}° / 指定 ${requested}°`,
      text,
      text,
    )
  }
  return view(
    'unavailable',
    'is-unavailable',
    `経路判定停止・表示 ${applied}°`,
    `単一ヒンジの連続経路判定は利用できません。現在の表示角は${applied}度です。${LIMITATION}`,
  )
}

function view(
  status: FoldPreviewContinuousMotionView['status'],
  badgeClass: string,
  badgeText: string,
  accessibleText: string,
  terminalAnnouncement: string | null = null,
): FoldPreviewContinuousMotionView {
  return {
    status,
    badgeClass,
    badgeText,
    accessibleText,
    terminalAnnouncement,
  }
}

function validAngle(value: number) {
  return Number.isFinite(value) && value >= 0 && value <= 180
}

function validClearResult(
  result: FoldPreviewContinuousMotionRunnerState['result'],
) {
  return result?.kind === 'clear'
    && result.certifiedSafeThrough === 1
    && result.stopTime === 1
}

function validBlockedResult(
  result: FoldPreviewContinuousMotionRunnerState['result'],
): result is Extract<
  NonNullable<FoldPreviewContinuousMotionRunnerState['result']>,
  { kind: 'blocked' }
> {
  return result?.kind === 'blocked'
    && validNonTerminalTime(result.certifiedSafeThrough)
    && result.stopTime === result.certifiedSafeThrough
    && validBracket(result.unsafeBracket)
    && result.unsafeBracket[0] === result.certifiedSafeThrough
}

function validIndeterminateResult(
  result: FoldPreviewContinuousMotionRunnerState['result'],
): result is Extract<
  NonNullable<FoldPreviewContinuousMotionRunnerState['result']>,
  { kind: 'indeterminate' }
> {
  return result?.kind === 'indeterminate'
    && validNonTerminalTime(result.certifiedSafeThrough)
    && result.stopTime === result.certifiedSafeThrough
    && validBracket(result.unresolvedBracket)
    && result.unresolvedBracket[0] === result.certifiedSafeThrough
    && typeof result.reason === 'string'
    && result.reason.length > 0
}

function validBracket(value: readonly [number, number]) {
  return Array.isArray(value)
    && value.length === 2
    && validUnitTime(value[0])
    && validUnitTime(value[1])
    && value[0] <= value[1]
}

function validUnitTime(value: number) {
  return Number.isFinite(value) && value >= 0 && value <= 1
}

function validNonTerminalTime(value: number) {
  return validUnitTime(value) && value < 1
}

function appliedMatchesCertifiedTime(
  state: FoldPreviewContinuousMotionRunnerState,
  certifiedSafeThrough: number,
) {
  if (state.requested === null) return false
  return state.applied === state.start
    + (state.requested - state.start) * certifiedSafeThrough
}

function formatMotionAngle(value: number) {
  const rounded = Math.round(value * 1_000) / 1_000
  return Number.isInteger(rounded)
    ? String(rounded)
    : rounded.toFixed(3).replace(/0+$/u, '').replace(/\.$/u, '')
}
