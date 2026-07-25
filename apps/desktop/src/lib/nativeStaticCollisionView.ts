import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import {
  DEFAULT_LOCALE,
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
  type LocalizedText,
} from './i18n.ts'
import {
  NATIVE_STATIC_COLLISION_PAIR_DISPOSITION_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_POLICY_TEXT,
  NATIVE_STATIC_COLLISION_PAIR_TOPOLOGY_TEXT,
  NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT,
  NATIVE_STATIC_COLLISION_VIEW_TEXT as NATIVE_COLLISION_TEXT,
} from './nativeStaticCollisionViewText.ts'

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
  const localizedCounts = formatLocalizedText(
    locale,
    NATIVE_COLLISION_TEXT.pairCounts,
    {
      total: pairs.length,
      separated: counts.separated,
      touching: counts.touching,
      allowed: counts.allowed,
      penetrating: counts.penetrating,
      indeterminate: counts.indeterminate,
    },
  )
  const omittedText = omittedPairCount === 0
    ? null
    : formatLocalizedText(locale, NATIVE_COLLISION_TEXT.omittedPairs, {
      total: pairs.length,
      displayed: displayedPairs.length,
      omitted: omittedPairCount,
    })
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
        ? selectLocalizedText(
          locale,
          NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT.strictTransversalDualGate,
        )
        : null,
      pair.wholeFaceOverlapProven
        ? selectLocalizedText(
          locale,
          NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT.wholeFaceOverlap,
        )
        : null,
      pair.sharedHingeBoundaryContactProven
        ? selectLocalizedText(
          locale,
          NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT.sharedHingeBoundaryContact,
        )
        : null,
      pair.sharedHingeSolidClassified
        ? selectLocalizedText(
          locale,
          NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT.sharedHingeSolidClassification,
        )
        : null,
    ].filter((marker): marker is string => marker !== null)
    const markerText = proofMarkers.length === 0
      ? ''
      : formatLocalizedText(locale, NATIVE_COLLISION_TEXT.pairBasis, {
        markers: proofMarkers.join(selectLocalizedText(
          locale,
          NATIVE_COLLISION_TEXT.proofMarkerSeparator,
        )),
      })
    const pairText = [
      pair.firstFaceId,
      pair.secondFaceId,
    ].join(selectLocalizedText(locale, NATIVE_COLLISION_TEXT.pairConnector))
    return Object.freeze({
      key: `${pair.firstFaceId}:${pair.secondFaceId}`,
      firstFaceId: pair.firstFaceId,
      secondFaceId: pair.secondFaceId,
      disposition: pair.disposition,
      risk,
      rowClass: `is-${pair.disposition.replace('_', '-')}`,
      text: formatLocalizedText(locale, NATIVE_COLLISION_TEXT.pairText, {
        index: index + 1,
        disposition,
        pair: pairText,
        topology,
        evidence,
        policy,
        markerText,
      }),
      accessibleText: formatLocalizedText(
        locale,
        NATIVE_COLLISION_TEXT.pairAccessibleText,
        {
          index: index + 1,
          firstFaceId: pair.firstFaceId,
          secondFaceId: pair.secondFaceId,
          disposition,
          topology,
          evidence,
          policy,
          markerText,
        },
      ),
    })
  })
  return Object.freeze({
    countsText: localizedCounts,
    accessibleCountsText: formatLocalizedText(
      locale,
      NATIVE_COLLISION_TEXT.pairAccessibleCounts,
      {
        counts: localizedCounts,
        display: omittedText ?? selectLocalizedText(
          locale,
          NATIVE_COLLISION_TEXT.allPairsDisplayed,
        ),
      },
    ),
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
  return selectLocalizedText(
    locale,
    NATIVE_STATIC_COLLISION_PAIR_DISPOSITION_TEXT[disposition],
  )
}

function pairTopologyLabel(
  topology: CurrentStaticCollisionTopology,
  locale: Locale,
): string {
  return selectLocalizedText(
    locale,
    NATIVE_STATIC_COLLISION_PAIR_TOPOLOGY_TEXT[topology],
  )
}

function pairEvidenceLabel(
  evidence: CurrentStaticCollisionEvidence,
  locale: Locale,
): string {
  return selectLocalizedText(
    locale,
    NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT[evidence],
  )
}

function pairPolicyLabel(
  policy: CurrentStaticCollisionPolicyDecision,
  locale: Locale,
): string {
  return selectLocalizedText(
    locale,
    NATIVE_STATIC_COLLISION_PAIR_POLICY_TEXT[policy],
  )
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
