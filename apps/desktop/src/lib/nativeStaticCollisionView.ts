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

export type CurrentStaticCollisionTopology =
  | 'no_shared_feature'
  | 'shared_vertex'
  | 'shared_hinge_edge'

export type CurrentStaticCollisionEvidence =
  | 'separated'
  | 'point_contact'
  | 'boundary_line_contact'
  | 'boundary_area_contact'
  | 'shared_feature_contact'
  | 'shared_feature_thickness_overlap'
  | 'shared_feature_flat_stack'
  | 'coplanar_area_overlap'
  | 'transversal_crossing'
  | 'positive_volume_overlap'
  | 'indeterminate'

export type CurrentStaticCollisionPolicyDecision =
  | 'separated'
  | 'touching'
  | 'allowed_shared_vertex_contact'
  | 'requires_hinge_model'
  | 'penetrating'
  | 'indeterminate'

export type CurrentStaticCollisionPairDisposition =
  | 'separated'
  | 'touching'
  | 'allowed'
  | 'penetrating'
  | 'indeterminate'

export type CurrentStaticCollisionPairClassificationCounts = Readonly<{
  separated: number
  touching: number
  allowed: number
  penetrating: number
  indeterminate: number
  candidateExcluded: number
}>

export type CurrentStaticCollisionPairDiagnostic =
  CurrentStaticCollisionFacePair & Readonly<{
    topology: CurrentStaticCollisionTopology
    evidence: CurrentStaticCollisionEvidence
    policyDecision: CurrentStaticCollisionPolicyDecision
    disposition: CurrentStaticCollisionPairDisposition
    strictTransversalDualGateProven: boolean
    wholeFaceOverlapProven: boolean
    sharedHingeBoundaryContactProven: boolean
    sharedHingeSolidClassified: boolean
  }>

export type CurrentStaticCollisionDiagnostic = Readonly<{
  status: 'certified_nonblocking' | 'blocking' | 'unavailable'
  reason: CurrentStaticCollisionDiagnosticReason | null
  expectedUnorderedFacePairs: number | null
  provenPenetratingPairs: number | null
  firstProvenPenetratingPair: CurrentStaticCollisionFacePair | null
  pairClassificationCounts:
    | CurrentStaticCollisionPairClassificationCounts
    | null
  pairDiagnostics: readonly CurrentStaticCollisionPairDiagnostic[] | null
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

export type NativeStaticCollisionPairPresentation = Readonly<{
  key: string
  firstFaceId: string
  secondFaceId: string
  disposition: CurrentStaticCollisionPairDisposition
  risk: 'informational' | 'warning' | 'blocking'
  rowClass: string
  text: string
  accessibleText: string
}>

export type NativeStaticCollisionPairDetailsPresentation = Readonly<{
  countsText: string
  accessibleCountsText: string
  pairs: readonly NativeStaticCollisionPairPresentation[]
  hasBlockingPair: boolean
  totalPairCount: number
  displayedPairCount: number
  omittedPairCount: number
  omittedText: string | null
}>

const MAX_RENDERED_STATIC_COLLISION_PAIRS = 200

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

/**
 * Formats the complete native pair snapshot without dropping safe, touching,
 * or unresolved rows. `indeterminate` deliberately shares the blocking risk
 * level used for proven penetration so an undecided pair cannot disappear
 * behind an aggregate badge.
 */
export function presentNativeStaticCollisionPairDiagnostics(
  diagnostic: CurrentStaticCollisionDiagnostic,
  locale: Locale = DEFAULT_LOCALE,
): NativeStaticCollisionPairDetailsPresentation | null {
  // Treat direct callers that bypass the strict native parser fail-closed.
  const counts = diagnostic.pairClassificationCounts ?? null
  const pairs = diagnostic.pairDiagnostics ?? null
  if (
    counts === null
    || pairs === null
    || !validPairClassificationCounts(counts, pairs)
  ) return null

  const blockingPairs = pairs.filter((pair) => (
    pair.disposition === 'penetrating'
    || pair.disposition === 'indeterminate'
  ))
  const nonblockingPairs = pairs.filter((pair) => (
    pair.disposition !== 'penetrating'
    && pair.disposition !== 'indeterminate'
  ))
  const displayedPairs = blockingPairs
    .slice(0, MAX_RENDERED_STATIC_COLLISION_PAIRS)
  const remainingCapacity =
    MAX_RENDERED_STATIC_COLLISION_PAIRS - displayedPairs.length
  if (remainingCapacity > 0) {
    displayedPairs.push(...nonblockingPairs.slice(0, remainingCapacity))
  }
  const omittedPairCount = pairs.length - displayedPairs.length
  const localizedCounts = locale === 'ja'
    ? `面ペア ${pairs.length}件: 分離 ${counts.separated} / 接触 ${counts.touching} / 許容 ${counts.allowed} / 貫通 ${counts.penetrating} / 判定保留 ${counts.indeterminate}`
    : `Face pairs ${pairs.length}: separated ${counts.separated} / touching ${counts.touching} / allowed ${counts.allowed} / penetrating ${counts.penetrating} / indeterminate ${counts.indeterminate}`
  const omittedText = omittedPairCount === 0
    ? null
    : locale === 'ja'
      ? `全${pairs.length}件中${displayedPairs.length}件を表示し、${omittedPairCount}件を省略しています。貫通・判定保留を優先表示しています。`
      : `Showing ${displayedPairs.length} of ${pairs.length} pairs; ${omittedPairCount} omitted. Penetrating and indeterminate pairs are prioritized.`
  const rows = displayedPairs.map((pair, index) => {
    const risk = pair.disposition === 'penetrating'
      || pair.disposition === 'indeterminate'
      ? 'blocking'
      : pair.disposition === 'touching'
        ? 'warning'
        : 'informational'
    const disposition = pairDispositionLabel(pair.disposition, locale)
    const topology = pairTopologyLabel(pair.topology, locale)
    const evidence = pairEvidenceLabel(pair.evidence, locale)
    const policy = pairPolicyLabel(pair.policyDecision, locale)
    const proofMarkers = [
      pair.strictTransversalDualGateProven
        ? locale === 'ja' ? '横断交差の二重証明' : 'dual-gate transversal proof'
        : null,
      pair.wholeFaceOverlapProven
        ? locale === 'ja' ? '面全体の重なり証明' : 'whole-face overlap proof'
        : null,
      pair.sharedHingeBoundaryContactProven
        ? locale === 'ja'
          ? '共有ヒンジ境界限定接触の証明'
          : 'shared-hinge boundary-only contact proof'
        : null,
      pair.sharedHingeSolidClassified
        ? locale === 'ja' ? '共有ヒンジ実体分類' : 'shared-hinge solid classification'
        : null,
    ].filter((marker): marker is string => marker !== null)
    const markerText = proofMarkers.length === 0
      ? ''
      : locale === 'ja'
        ? ` / 根拠: ${proofMarkers.join('・')}`
        : ` / basis: ${proofMarkers.join(', ')}`
    const pairText = `${pair.firstFaceId} ↔ ${pair.secondFaceId}`
    return Object.freeze({
      key: `${pair.firstFaceId}:${pair.secondFaceId}`,
      firstFaceId: pair.firstFaceId,
      secondFaceId: pair.secondFaceId,
      disposition: pair.disposition,
      risk,
      rowClass: `is-${pair.disposition.replace('_', '-')}`,
      text: locale === 'ja'
        ? `${index + 1}. ${disposition} — ${pairText} — ${topology} / ${evidence} / 方針 ${policy}${markerText}`
        : `${index + 1}. ${disposition} — ${pairText} — ${topology} / ${evidence} / policy ${policy}${markerText}`,
      accessibleText: locale === 'ja'
        ? `面ペア ${index + 1}、${pair.firstFaceId} と ${pair.secondFaceId}。分類 ${disposition}。位相 ${topology}。幾何根拠 ${evidence}。方針判定 ${policy}${markerText}。`
        : `Face pair ${index + 1}, ${pair.firstFaceId} and ${pair.secondFaceId}. Classification ${disposition}. Topology ${topology}. Geometric evidence ${evidence}. Policy decision ${policy}${markerText}.`,
    })
  })
  return Object.freeze({
    countsText: localizedCounts,
    accessibleCountsText: locale === 'ja'
      ? `${localizedCounts}。判定保留は貫通と同じく安全確認を遮断します。${omittedText ?? '全ペアを表示しています。'}`
      : `${localizedCounts}. Indeterminate pairs block safety confirmation with the same prominence as penetration. ${omittedText ?? 'All pairs are displayed.'}`,
    pairs: Object.freeze(rows),
    hasBlockingPair:
      counts.penetrating > 0 || counts.indeterminate > 0,
    totalPairCount: pairs.length,
    displayedPairCount: displayedPairs.length,
    omittedPairCount,
    omittedText,
  })
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

function validPairClassificationCounts(
  counts: CurrentStaticCollisionPairClassificationCounts,
  pairs: readonly CurrentStaticCollisionPairDiagnostic[],
): boolean {
  const values = [
    counts.separated,
    counts.touching,
    counts.allowed,
    counts.penetrating,
    counts.indeterminate,
    counts.candidateExcluded,
  ]
  if (
    counts.candidateExcluded !== 0
    || values.some((value) => !validCount(value))
  ) return false
  const sum = values.reduce((total, value) => total + value, 0)
  if (!Number.isSafeInteger(sum) || sum !== pairs.length) return false
  const actual = {
    separated: 0,
    touching: 0,
    allowed: 0,
    penetrating: 0,
    indeterminate: 0,
  }
  for (const pair of pairs) actual[pair.disposition] += 1
  return actual.separated === counts.separated
    && actual.touching === counts.touching
    && actual.allowed === counts.allowed
    && actual.penetrating === counts.penetrating
    && actual.indeterminate === counts.indeterminate
}

function pairDispositionLabel(
  disposition: CurrentStaticCollisionPairDisposition,
  locale: Locale,
): string {
  const labels = locale === 'ja'
    ? {
      separated: '分離',
      touching: '接触',
      allowed: '許容',
      penetrating: '貫通',
      indeterminate: '判定保留',
    }
    : {
      separated: 'separated',
      touching: 'touching',
      allowed: 'allowed',
      penetrating: 'penetrating',
      indeterminate: 'indeterminate',
    }
  return labels[disposition]
}

function pairTopologyLabel(
  topology: CurrentStaticCollisionTopology,
  locale: Locale,
): string {
  const labels = locale === 'ja'
    ? {
      no_shared_feature: '共有要素なし',
      shared_vertex: '頂点共有',
      shared_hinge_edge: 'ヒンジ辺共有',
    }
    : {
      no_shared_feature: 'no shared feature',
      shared_vertex: 'shared vertex',
      shared_hinge_edge: 'shared hinge edge',
    }
  return labels[topology]
}

function pairEvidenceLabel(
  evidence: CurrentStaticCollisionEvidence,
  locale: Locale,
): string {
  const ja = {
    separated: '離間',
    point_contact: '点接触',
    boundary_line_contact: '線接触',
    boundary_area_contact: '境界面接触',
    shared_feature_contact: '共有要素上の接触',
    shared_feature_thickness_overlap: '共有要素の厚み重なり',
    shared_feature_flat_stack: '共有要素の平坦積層',
    coplanar_area_overlap: '同一平面の面積重なり',
    transversal_crossing: '横断交差',
    positive_volume_overlap: '正体積重なり',
    indeterminate: '幾何判定保留',
  } satisfies Record<CurrentStaticCollisionEvidence, string>
  const en = {
    separated: 'separated',
    point_contact: 'point contact',
    boundary_line_contact: 'boundary line contact',
    boundary_area_contact: 'boundary area contact',
    shared_feature_contact: 'shared-feature contact',
    shared_feature_thickness_overlap: 'shared-feature thickness overlap',
    shared_feature_flat_stack: 'shared-feature flat stack',
    coplanar_area_overlap: 'coplanar area overlap',
    transversal_crossing: 'transversal crossing',
    positive_volume_overlap: 'positive-volume overlap',
    indeterminate: 'geometric evidence indeterminate',
  } satisfies Record<CurrentStaticCollisionEvidence, string>
  return (locale === 'ja' ? ja : en)[evidence]
}

function pairPolicyLabel(
  policy: CurrentStaticCollisionPolicyDecision,
  locale: Locale,
): string {
  const ja = {
    separated: '分離',
    touching: '接触',
    allowed_shared_vertex_contact: '共有頂点接触を許容',
    requires_hinge_model: 'ヒンジモデル必須',
    penetrating: '貫通',
    indeterminate: '判定保留',
  } satisfies Record<CurrentStaticCollisionPolicyDecision, string>
  const en = {
    separated: 'separated',
    touching: 'touching',
    allowed_shared_vertex_contact: 'allowed shared-vertex contact',
    requires_hinge_model: 'hinge model required',
    penetrating: 'penetrating',
    indeterminate: 'indeterminate',
  } satisfies Record<CurrentStaticCollisionPolicyDecision, string>
  return (locale === 'ja' ? ja : en)[policy]
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
