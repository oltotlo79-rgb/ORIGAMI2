export type CurrentStaticCollisionDiagnosticReason =
  | 'proven_zero_thickness_penetration'
  | 'evidence_unavailable'
  | 'resource_limit_exceeded'
  | 'inconsistent_state'
  | 'pose_authority_unavailable'

export type CurrentStaticCollisionFacePair = Readonly<{
  firstFaceId: string
  secondFaceId: string
}>

export type CurrentStaticCollisionDiagnostic = Readonly<{
  status: 'certified_nonblocking' | 'blocking' | 'unavailable'
  reason: CurrentStaticCollisionDiagnosticReason | null
  expectedUnorderedFacePairs: number | null
  provenPenetratingPairs: number | null
  firstProvenPenetratingPair: CurrentStaticCollisionFacePair | null
}>

export type NativeStaticCollisionViewState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'waiting' }>
  | Readonly<{ kind: 'checking' }>
  | Readonly<{
    kind: 'ready'
    diagnostic: CurrentStaticCollisionDiagnostic
  }>
  | Readonly<{ kind: 'failed' }>

export type BoundNativeStaticCollisionView = Readonly<{
  requestKey: string | null
  view: NativeStaticCollisionViewState
}>

export type NativeStaticCollisionPresentation = Readonly<{
  dataStatus:
    | 'idle'
    | 'checking'
    | 'certified_nonblocking'
    | 'penetrating'
    | 'indeterminate'
    | 'unavailable'
  badgeClass:
    | 'is-idle'
    | 'is-checking'
    | 'is-certified'
    | 'is-blocked'
    | 'is-indeterminate'
    | 'is-unavailable'
  badgeText: string
  accessibleText: string
  requiresSafetyReview: boolean
}>

const SAFETY_REVIEW = 'この姿勢を安全確認済みとして扱わないでください。'

/**
 * Selects the view synchronously during render. A result bound to any other
 * pose key is hidden before effects run, so an old green certificate cannot
 * be painted over a newly rendered pose.
 */
export function selectBoundNativeStaticCollisionView(
  moving: boolean,
  currentRequestKey: string | null,
  bound: BoundNativeStaticCollisionView,
): NativeStaticCollisionViewState {
  if (moving) return { kind: 'waiting' }
  if (currentRequestKey === null) return { kind: 'idle' }
  return bound.requestKey === currentRequestKey
    ? bound.view
    : { kind: 'checking' }
}

/**
 * Keeps native proof results visually separate from the browser-side
 * approximation. Every missing, malformed, or unresolved result is
 * fail-closed and therefore never receives the certified presentation.
 */
export function presentNativeStaticCollision(
  state: NativeStaticCollisionViewState,
): NativeStaticCollisionPresentation {
  if (state.kind === 'idle') {
    return {
      dataStatus: 'idle',
      badgeClass: 'is-idle',
      badgeText: '厳密判定｜姿勢待機',
      accessibleText: '厳密衝突判定は、安定した表示姿勢を待っています。',
      requiresSafetyReview: false,
    }
  }
  if (state.kind === 'waiting') {
    return {
      dataStatus: 'checking',
      badgeClass: 'is-checking',
      badgeText: '厳密判定｜姿勢確定待ち',
      accessibleText:
        `表示姿勢の移動が終わってから厳密判定します。${SAFETY_REVIEW}`,
      requiresSafetyReview: true,
    }
  }
  if (state.kind === 'checking') {
    return {
      dataStatus: 'checking',
      badgeClass: 'is-checking',
      badgeText: '厳密判定｜確認中',
      accessibleText: `現在の表示姿勢を厳密判定しています。${SAFETY_REVIEW}`,
      requiresSafetyReview: true,
    }
  }
  if (state.kind === 'failed') {
    return unavailablePresentation(
      '厳密判定｜実行失敗・安全確認が必要',
      `厳密衝突判定を完了できませんでした。${SAFETY_REVIEW}`,
    )
  }

  const diagnostic = state.diagnostic
  if (
    diagnostic.status === 'certified_nonblocking'
    && diagnostic.reason === null
    && validCount(diagnostic.expectedUnorderedFacePairs)
    && diagnostic.provenPenetratingPairs === 0
    && diagnostic.firstProvenPenetratingPair === null
  ) {
    return {
      dataStatus: 'certified_nonblocking',
      badgeClass: 'is-certified',
      badgeText: '厳密判定｜ゼロ厚み面貫通・重なりなし',
      accessibleText:
        '現在の表示姿勢では、対象となる全ての面ペアについて、ゼロ厚み面の貫通または正の面積を持つ重なりがないことを証明しました。',
      requiresSafetyReview: false,
    }
  }

  if (
    diagnostic.status === 'blocking'
    && diagnostic.reason === 'proven_zero_thickness_penetration'
  ) {
    const count = diagnostic.provenPenetratingPairs
    const countText = validCount(count) && count > 0 ? ` ${count}` : ''
    return {
      dataStatus: 'penetrating',
      badgeClass: 'is-blocked',
      badgeText:
        `厳密判定｜ゼロ厚み面貫通・重なり${countText}・安全認定不可`,
      accessibleText:
        `現在の表示姿勢でゼロ厚み面の貫通または正の面積を持つ重なり${countText}件を証明したため、安全認定を遮断しました。`,
      requiresSafetyReview: true,
    }
  }

  if (
    diagnostic.status === 'blocking'
    && (
      diagnostic.reason === 'evidence_unavailable'
      || diagnostic.reason === 'resource_limit_exceeded'
      || diagnostic.reason === 'inconsistent_state'
    )
  ) {
    const reasonLabel = diagnostic.reason === 'evidence_unavailable'
      ? '証拠不足'
      : diagnostic.reason === 'resource_limit_exceeded'
        ? '資源上限'
        : '状態不整合'
    const reason = diagnostic.reason === 'evidence_unavailable'
      ? '必要な面ペア証拠を取得できませんでした。'
      : diagnostic.reason === 'resource_limit_exceeded'
        ? '厳密判定の資源上限に達しました。'
        : '姿勢または判定状態の整合性を確認できませんでした。'
    return {
      dataStatus: 'indeterminate',
      badgeClass: 'is-indeterminate',
      badgeText: `厳密判定｜${reasonLabel}・交差の可能性・判定保留`,
      accessibleText: `${reason}${SAFETY_REVIEW}`,
      requiresSafetyReview: true,
    }
  }

  return unavailablePresentation(
    '厳密判定｜利用不可・安全確認が必要',
    `現在の表示姿勢に対する厳密衝突判定を利用できません。${SAFETY_REVIEW}`,
  )
}

function unavailablePresentation(
  badgeText: string,
  accessibleText: string,
): NativeStaticCollisionPresentation {
  return {
    dataStatus: 'unavailable',
    badgeClass: 'is-unavailable',
    badgeText,
    accessibleText,
    requiresSafetyReview: true,
  }
}

function validCount(value: number | null): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}
