import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import {
  DEFAULT_LOCALE,
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'

export type CurrentStaticCollisionDiagnosticReason =
  | 'proven_zero_thickness_penetration'
  | 'proven_positive_thickness_penetration'
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
  locale: Locale = DEFAULT_LOCALE,
): NativeStaticCollisionPresentation {
  if (state.kind === 'idle') {
    return {
      dataStatus: 'idle',
      badgeClass: 'is-idle',
      badgeText: selectLocalizedText(locale, NATIVE_COLLISION_TEXT.idleBadge),
      accessibleText: selectLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.idleAccessible,
      ),
      requiresSafetyReview: false,
    }
  }
  if (state.kind === 'waiting') {
    return {
      dataStatus: 'checking',
      badgeClass: 'is-checking',
      badgeText: selectLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.waitingBadge,
      ),
      accessibleText: localizedWithSafetyReview(
        locale,
        NATIVE_COLLISION_TEXT.waitingAccessible,
      ),
      requiresSafetyReview: true,
    }
  }
  if (state.kind === 'checking') {
    return {
      dataStatus: 'checking',
      badgeClass: 'is-checking',
      badgeText: selectLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.checkingBadge,
      ),
      accessibleText: localizedWithSafetyReview(
        locale,
        NATIVE_COLLISION_TEXT.checkingAccessible,
      ),
      requiresSafetyReview: true,
    }
  }
  if (state.kind === 'failed') {
    return unavailablePresentation(
      selectLocalizedText(locale, NATIVE_COLLISION_TEXT.failedBadge),
      localizedWithSafetyReview(
        locale,
        NATIVE_COLLISION_TEXT.failedAccessible,
      ),
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
      badgeText: selectLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.certifiedBadge,
      ),
      accessibleText: selectLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.certifiedAccessible,
      ),
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
      badgeText: formatLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.zeroThicknessPenetrationBadge,
        { countText },
      ),
      accessibleText: formatLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.zeroThicknessPenetrationAccessible,
        { countText },
      ),
      requiresSafetyReview: true,
    }
  }

  if (
    diagnostic.status === 'blocking'
    && diagnostic.reason === 'proven_positive_thickness_penetration'
    && validPositiveThicknessPenetration(diagnostic)
  ) {
    const count = diagnostic.provenPenetratingPairs
    return {
      dataStatus: 'penetrating',
      badgeClass: 'is-blocked',
      badgeText: formatLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.positiveThicknessPenetrationBadge,
        { count },
      ),
      accessibleText: formatLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.positiveThicknessPenetrationAccessible,
        { count },
      ),
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
      ? selectLocalizedText(locale, NATIVE_COLLISION_TEXT.evidenceLabel)
      : diagnostic.reason === 'resource_limit_exceeded'
        ? selectLocalizedText(locale, NATIVE_COLLISION_TEXT.resourceLabel)
        : selectLocalizedText(locale, NATIVE_COLLISION_TEXT.inconsistentLabel)
    const reason = diagnostic.reason === 'evidence_unavailable'
      ? NATIVE_COLLISION_TEXT.evidenceAccessible
      : diagnostic.reason === 'resource_limit_exceeded'
        ? NATIVE_COLLISION_TEXT.resourceAccessible
        : NATIVE_COLLISION_TEXT.inconsistentAccessible
    return {
      dataStatus: 'indeterminate',
      badgeClass: 'is-indeterminate',
      badgeText: formatLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.indeterminateBadge,
        { reasonLabel },
      ),
      accessibleText: localizedWithSafetyReview(locale, reason),
      requiresSafetyReview: true,
    }
  }

  return unavailablePresentation(
    selectLocalizedText(locale, NATIVE_COLLISION_TEXT.unavailableBadge),
    localizedWithSafetyReview(
      locale,
      NATIVE_COLLISION_TEXT.unavailableAccessible,
    ),
  )
}

function localizedWithSafetyReview(
  locale: Locale,
  prefix: LocalizedText,
): string {
  return formatLocalizedText(locale, NATIVE_COLLISION_TEXT.withSafetyReview, {
    prefix: selectLocalizedText(locale, prefix),
    safetyReview: selectLocalizedText(
      locale,
      NATIVE_COLLISION_TEXT.safetyReview,
    ),
  })
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

function validPositiveThicknessPenetration(
  diagnostic: CurrentStaticCollisionDiagnostic,
): diagnostic is CurrentStaticCollisionDiagnostic & Readonly<{
  expectedUnorderedFacePairs: number
  provenPenetratingPairs: number
  firstProvenPenetratingPair: CurrentStaticCollisionFacePair
}> {
  const expected = diagnostic.expectedUnorderedFacePairs
  const proven = diagnostic.provenPenetratingPairs
  const pair = diagnostic.firstProvenPenetratingPair
  return validCount(expected)
    && expected > 0
    && validCount(proven)
    && proven > 0
    && proven <= expected
    && pair !== null
    && isCanonicalNonNilUuid(pair.firstFaceId)
    && isCanonicalNonNilUuid(pair.secondFaceId)
    && pair.firstFaceId < pair.secondFaceId
}

const NATIVE_COLLISION_TEXT = Object.freeze({
  idleBadge: Object.freeze({
    ja: '厳密判定｜姿勢待機',
    en: 'Exact check | Waiting for pose',
  }),
  idleAccessible: Object.freeze({
    ja: '厳密衝突判定は、安定した表示姿勢を待っています。',
    en: 'The exact collision check is waiting for a stable displayed pose.',
  }),
  waitingBadge: Object.freeze({
    ja: '厳密判定｜姿勢確定待ち',
    en: 'Exact check | Waiting for stable pose',
  }),
  waitingAccessible: Object.freeze({
    ja: '表示姿勢の移動が終わってから厳密判定します。',
    en: 'The exact check will run after the displayed pose stops moving.',
  }),
  checkingBadge: Object.freeze({
    ja: '厳密判定｜確認中',
    en: 'Exact check | Checking',
  }),
  checkingAccessible: Object.freeze({
    ja: '現在の表示姿勢を厳密判定しています。',
    en: 'Running the exact check on the current displayed pose.',
  }),
  failedBadge: Object.freeze({
    ja: '厳密判定｜実行失敗・安全確認が必要',
    en: 'Exact check | Failed · safety review required',
  }),
  failedAccessible: Object.freeze({
    ja: '厳密衝突判定を完了できませんでした。',
    en: 'The exact collision check could not be completed.',
  }),
  certifiedBadge: Object.freeze({
    ja: '厳密判定｜ゼロ厚み面貫通・重なりなし',
    en: 'Exact check | No zero-thickness surface penetration or overlap',
  }),
  certifiedAccessible: Object.freeze({
    ja: '現在の表示姿勢では、対象となる全ての面ペアについて、ゼロ厚み面の貫通または正の面積を持つ重なりがないことを証明しました。',
    en: 'For the current displayed pose, every applicable face pair was proven to have no zero-thickness surface penetration or positive-area overlap.',
  }),
  zeroThicknessPenetrationBadge: Object.freeze({
    ja: '厳密判定｜ゼロ厚み面貫通・重なり{countText}・安全認定不可',
    en: 'Exact check | Zero-thickness surface penetration or overlap{countText} · safety certification denied',
  }),
  zeroThicknessPenetrationAccessible: Object.freeze({
    ja: '現在の表示姿勢でゼロ厚み面の貫通または正の面積を持つ重なり{countText}件を証明したため、安全認定を遮断しました。',
    en: 'Safety certification was blocked because zero-thickness surface penetration or positive-area overlap{countText} was proven in the current displayed pose.',
  }),
  positiveThicknessPenetrationBadge: Object.freeze({
    ja: '厳密判定｜紙厚を含む材料貫通 {count}・安全認定不可',
    en: 'Exact check | Material penetration including paper thickness {count} · safety certification denied',
  }),
  positiveThicknessPenetrationAccessible: Object.freeze({
    ja: '現在の表示姿勢で紙厚を含む材料の貫通{count}件を厳密証明したため、安全認定を遮断しました。',
    en: 'Safety certification was blocked because {count} material penetrations including paper thickness were exactly proven in the current displayed pose.',
  }),
  evidenceLabel: Object.freeze({
    ja: '証拠不足',
    en: 'Insufficient evidence',
  }),
  resourceLabel: Object.freeze({
    ja: '資源上限',
    en: 'Resource limit',
  }),
  inconsistentLabel: Object.freeze({
    ja: '状態不整合',
    en: 'Inconsistent state',
  }),
  evidenceAccessible: Object.freeze({
    ja: '必要な面ペア証拠を取得できませんでした。',
    en: 'The required face-pair evidence could not be obtained.',
  }),
  resourceAccessible: Object.freeze({
    ja: '厳密判定の資源上限に達しました。',
    en: 'The exact check reached its resource limit.',
  }),
  inconsistentAccessible: Object.freeze({
    ja: '姿勢または判定状態の整合性を確認できませんでした。',
    en: 'The pose or collision-check state could not be verified as consistent.',
  }),
  indeterminateBadge: Object.freeze({
    ja: '厳密判定｜{reasonLabel}・交差の可能性・判定保留',
    en: 'Exact check | {reasonLabel} · possible intersection / indeterminate',
  }),
  unavailableBadge: Object.freeze({
    ja: '厳密判定｜利用不可・安全確認が必要',
    en: 'Exact check | Unavailable · safety review required',
  }),
  unavailableAccessible: Object.freeze({
    ja: '現在の表示姿勢に対する厳密衝突判定を利用できません。',
    en: 'The exact collision check is unavailable for the current displayed pose.',
  }),
  safetyReview: Object.freeze({
    ja: 'この姿勢を安全確認済みとして扱わないでください。',
    en: 'Do not treat this pose as safety-verified.',
  }),
  withSafetyReview: Object.freeze({
    ja: '{prefix}{safetyReview}',
    en: '{prefix} {safetyReview}',
  }),
})
