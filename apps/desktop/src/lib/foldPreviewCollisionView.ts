import type { FoldPreviewHingeAngle } from './foldPreviewKinematics'
import type { FoldPreviewModel } from './foldPreviewModel'
import {
  DEFAULT_LOCALE,
  formatLocalizedText,
  selectLocalizedText,
  type Locale,
} from './i18n.ts'

export type CollisionSummary =
  | Readonly<{
      kind: 'ready'
      requestKey: string
      totalCandidates: number
      nonAdjacentCandidates: number
      hingeAdjacentCandidates: number
      narrowInteractions: number
      nonAdjacentPenetrations: number
      nonAdjacentContacts: number
      nonAdjacentAllowedSharedVertexContacts: number
      hingeInteractions: number
      hingeModelAllowedContacts: number
      hingeModelCorridorOverlaps: number
      hingeModelFlatSurfaceStacks: number
      hingeLayerOffsetUnmodeled: number
      hingeOutsidePenetrations: number
      hingeOutsideContacts: number
      hingeUnresolvedInteractions: number
      indeterminateInteractions: number
    }>
  | Readonly<{ kind: 'unavailable'; requestKey: string }>

export type CollisionPathDisclosure = 'unverified' | 'separately_reported'

export function collisionSummariesEqual(
  first: CollisionSummary | null,
  second: CollisionSummary,
) {
  if (
    !first
    || first.kind !== second.kind
    || first.requestKey !== second.requestKey
  ) return false
  return first.kind === 'unavailable'
    || (
      second.kind === 'ready'
      && first.totalCandidates === second.totalCandidates
      && first.nonAdjacentCandidates === second.nonAdjacentCandidates
      && first.hingeAdjacentCandidates === second.hingeAdjacentCandidates
      && first.narrowInteractions === second.narrowInteractions
      && first.nonAdjacentPenetrations === second.nonAdjacentPenetrations
      && first.nonAdjacentContacts === second.nonAdjacentContacts
      && first.nonAdjacentAllowedSharedVertexContacts
        === second.nonAdjacentAllowedSharedVertexContacts
      && first.hingeInteractions === second.hingeInteractions
      && first.hingeModelAllowedContacts === second.hingeModelAllowedContacts
      && first.hingeModelCorridorOverlaps === second.hingeModelCorridorOverlaps
      && first.hingeModelFlatSurfaceStacks === second.hingeModelFlatSurfaceStacks
      && first.hingeLayerOffsetUnmodeled === second.hingeLayerOffsetUnmodeled
      && first.hingeOutsidePenetrations === second.hingeOutsidePenetrations
      && first.hingeOutsideContacts === second.hingeOutsideContacts
      && first.hingeUnresolvedInteractions === second.hingeUnresolvedInteractions
      && first.indeterminateInteractions === second.indeterminateInteractions
    )
}

export function collisionPoseKey(
  model: Pick<FoldPreviewModel, 'projectId' | 'revision' | 'kind'> | null | undefined,
  fixedFaceId: string | null,
  thickness: number | null,
  angle: number,
  hingeAngles: readonly FoldPreviewHingeAngle[] | undefined,
) {
  if (!model) return ''
  const orderedHingeAngles = hingeAngles
    ? hingeAngles
      .map(({ edgeId, angleDegrees }) => [edgeId, angleDegrees] as const)
      .sort((first, second) => compareText(first[0], second[0]))
    : null
  return JSON.stringify([
    model.projectId,
    model.revision,
    model.kind,
    fixedFaceId,
    thickness,
    angle,
    orderedHingeAngles,
  ])
}

export function describeCollisionSummary(
  summary: CollisionSummary | null,
  accessible = false,
  pathDisclosure: CollisionPathDisclosure = 'unverified',
  locale: Locale = DEFAULT_LOCALE,
) {
  if (!summary) {
    return selectLocalizedText(
      locale,
      accessible
        ? COLLISION_VIEW_TEXT.pendingAccessible
        : COLLISION_VIEW_TEXT.pending,
    )
  }
  if (summary.kind === 'unavailable') {
    return selectLocalizedText(
      locale,
      accessible
        ? COLLISION_VIEW_TEXT.unavailableAccessible
        : COLLISION_VIEW_TEXT.unavailable,
    )
  }
  if (summary.totalCandidates === 0) {
    if (pathDisclosure === 'separately_reported') {
      return selectLocalizedText(
        locale,
        accessible
          ? COLLISION_VIEW_TEXT.clearSeparateAccessible
          : COLLISION_VIEW_TEXT.clearSeparate,
      )
    }
    return selectLocalizedText(
      locale,
      accessible
        ? COLLISION_VIEW_TEXT.clearUnverifiedAccessible
        : COLLISION_VIEW_TEXT.clearUnverified,
    )
  }
  const penetrationCount = summary.nonAdjacentPenetrations
    + summary.hingeOutsidePenetrations
  const contactCount = summary.nonAdjacentContacts + summary.hingeOutsideContacts
  const hingeModelCount = summary.hingeModelAllowedContacts
    + summary.hingeModelCorridorOverlaps
    + summary.hingeModelFlatSurfaceStacks
  const topologyModelCount = summary.nonAdjacentAllowedSharedVertexContacts
  const limitation = pathDisclosure === 'separately_reported'
    ? selectLocalizedText(locale, COLLISION_VIEW_TEXT.limitationSeparate)
    : selectLocalizedText(locale, COLLISION_VIEW_TEXT.limitationUnverified)
  const safetyReview = summary.hingeLayerOffsetUnmodeled
      + summary.hingeUnresolvedInteractions
      + summary.indeterminateInteractions
    > 0
    ? selectLocalizedText(locale, COLLISION_VIEW_TEXT.safetyReview)
    : ''
  return accessible
    ? formatLocalizedText(locale, COLLISION_VIEW_TEXT.detailedAccessible, {
      totalCandidates: summary.totalCandidates,
      narrowInteractions: summary.narrowInteractions,
      nonAdjacentPenetrations: summary.nonAdjacentPenetrations,
      hingeOutsidePenetrations: summary.hingeOutsidePenetrations,
      nonAdjacentContacts: summary.nonAdjacentContacts,
      topologyModelCount,
      hingeOutsideContacts: summary.hingeOutsideContacts,
      hingeModelAllowedContacts: summary.hingeModelAllowedContacts,
      hingeModelCorridorOverlaps: summary.hingeModelCorridorOverlaps,
      hingeModelFlatSurfaceStacks: summary.hingeModelFlatSurfaceStacks,
      hingeLayerOffsetUnmodeled: summary.hingeLayerOffsetUnmodeled,
      hingeUnresolvedInteractions: summary.hingeUnresolvedInteractions,
      indeterminateInteractions: summary.indeterminateInteractions,
      safetyReview,
      limitation,
    })
    : formatLocalizedText(locale, COLLISION_VIEW_TEXT.detailed, {
      penetrationCount,
      contactCount,
      topologyModelCount,
      hingeModelCount,
      hingeUnresolvedInteractions: summary.hingeUnresolvedInteractions,
      indeterminateInteractions: summary.indeterminateInteractions,
      totalCandidates: summary.totalCandidates,
      narrowInteractions: summary.narrowInteractions,
    })
}

export function collisionDataStatus(summary: CollisionSummary | null) {
  if (!summary) return 'pending'
  if (summary.kind === 'unavailable') return 'unavailable'
  if (summary.nonAdjacentPenetrations + summary.hingeOutsidePenetrations > 0) {
    return 'penetrating'
  }
  if (summary.hingeLayerOffsetUnmodeled > 0) return 'hinge-unresolved'
  if (summary.indeterminateInteractions > 0) return 'indeterminate'
  if (summary.hingeUnresolvedInteractions > 0) return 'hinge-unresolved'
  if (summary.nonAdjacentContacts + summary.hingeOutsideContacts > 0) return 'contact'
  if (summary.nonAdjacentAllowedSharedVertexContacts > 0) {
    return 'topology-model'
  }
  if (
    summary.hingeModelAllowedContacts
      + summary.hingeModelCorridorOverlaps
      + summary.hingeModelFlatSurfaceStacks
    > 0
  ) {
    return 'hinge-model'
  }
  return 'clear'
}

export function collisionBadgeClass(summary: CollisionSummary | null) {
  const status = collisionDataStatus(summary)
  if (status === 'pending') return 'is-pending'
  if (status === 'unavailable') return 'is-unavailable'
  if (status === 'penetrating') return 'has-penetrations'
  if (status === 'indeterminate' || status === 'hinge-unresolved') {
    return 'has-indeterminate'
  }
  if (status === 'contact') return 'has-contact'
  if (status === 'topology-model') return 'has-topology-allowance'
  if (status === 'hinge-model') return 'has-hinge-candidates'
  return 'is-clear'
}

export function collisionBadgeText(
  summary: CollisionSummary | null,
  locale: Locale = DEFAULT_LOCALE,
) {
  if (!summary) {
    return selectLocalizedText(locale, COLLISION_BADGE_TEXT.pending)
  }
  if (summary.kind === 'unavailable') {
    return selectLocalizedText(locale, COLLISION_BADGE_TEXT.unavailable)
  }
  const penetrationCount = summary.nonAdjacentPenetrations
    + summary.hingeOutsidePenetrations
  const contactCount = summary.nonAdjacentContacts + summary.hingeOutsideContacts
  const holdText = collisionHoldText(summary, locale)
  if (penetrationCount > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.penetrating, {
      penetrationCount,
      hingeOutsidePenetrations: summary.hingeOutsidePenetrations,
      contactCount,
      holdSuffix: holdText
        ? formatLocalizedText(locale, COLLISION_BADGE_TEXT.suffix, {
          detail: holdText,
        })
        : '',
    })
  }
  if (holdText) {
    return contactCount > 0
      ? formatLocalizedText(locale, COLLISION_BADGE_TEXT.holdWithContact, {
        holdText,
        contactCount,
      })
      : holdText
  }
  if (contactCount > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.contact, {
      contactCount,
      hingeOutsideContacts: summary.hingeOutsideContacts,
    })
  }
  if (summary.nonAdjacentAllowedSharedVertexContacts > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.sharedVertex, {
      count: summary.nonAdjacentAllowedSharedVertexContacts,
    })
  }
  if (summary.hingeModelFlatSurfaceStacks > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.flatStack, {
      count: summary.hingeModelFlatSurfaceStacks,
    })
  }
  if (summary.hingeModelCorridorOverlaps > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.corridor, {
      overlaps: summary.hingeModelCorridorOverlaps,
      contacts: summary.hingeModelAllowedContacts,
    })
  }
  if (summary.hingeModelAllowedContacts > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.hingeContact, {
      count: summary.hingeModelAllowedContacts,
    })
  }
  return summary.totalCandidates === 0
    ? selectLocalizedText(locale, COLLISION_BADGE_TEXT.clear)
    : formatLocalizedText(locale, COLLISION_BADGE_TEXT.noNarrowInteraction, {
      count: summary.totalCandidates,
    })
}

function collisionHoldText(
  summary: Extract<CollisionSummary, { kind: 'ready' }>,
  locale: Locale,
) {
  if (summary.hingeLayerOffsetUnmodeled > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.layerOffsetHold, {
      count: summary.hingeLayerOffsetUnmodeled,
    })
  }
  if (summary.indeterminateInteractions > 0) {
    const hingeDetail = summary.hingeUnresolvedInteractions > 0
      ? formatLocalizedText(locale, COLLISION_BADGE_TEXT.hingeDetail, {
        count: summary.hingeUnresolvedInteractions,
      })
      : ''
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.indeterminate, {
      count: summary.indeterminateInteractions,
      hingeDetail,
    })
  }
  if (summary.hingeUnresolvedInteractions > 0) {
    return formatLocalizedText(locale, COLLISION_BADGE_TEXT.hingeUnresolved, {
      count: summary.hingeUnresolvedInteractions,
    })
  }
  return ''
}

const COLLISION_VIEW_TEXT = Object.freeze({
  pending: Object.freeze({ ja: '衝突判定中', en: 'Collision check in progress' }),
  pendingAccessible: Object.freeze({
    ja: '現在姿勢の衝突候補を判定中',
    en: 'Checking collision candidates for the current pose',
  }),
  unavailable: Object.freeze({
    ja: '衝突判定不能・安全確認が必要',
    en: 'Collision check unavailable · safety review required',
  }),
  unavailableAccessible: Object.freeze({
    ja: '現在姿勢の衝突判定は利用できません。安全確認が必要です',
    en: 'Collision checking is unavailable for the current pose. Safety review is required.',
  }),
  clearSeparateAccessible: Object.freeze({
    ja: '現在姿勢の広域候補と狭域相互作用は0件。単一ヒンジの連続経路判定は別に表示しています',
    en: 'Current-pose broad-phase candidates and narrow-phase interactions: 0. Single-hinge continuous-path checking is shown separately.',
  }),
  clearSeparate: Object.freeze({
    ja: '現在姿勢: 衝突候補 0（経路判定は別表示）',
    en: 'Current pose: 0 collision candidates (path checking shown separately)',
  }),
  clearUnverifiedAccessible: Object.freeze({
    ja: '現在姿勢の広域候補と狭域相互作用は0件。連続運動中の衝突は未検証です',
    en: 'Current-pose broad-phase candidates and narrow-phase interactions: 0. Collisions during continuous motion have not been verified.',
  }),
  clearUnverified: Object.freeze({
    ja: '現在姿勢: 衝突候補 0（連続運動は未検証）',
    en: 'Current pose: 0 collision candidates (continuous motion unverified)',
  }),
  limitationSeparate: Object.freeze({
    ja: 'これは現在姿勢に対する中央面基準の近似判定で、実際の折り癖と層ずれは未検証です。単一ヒンジの連続経路判定は別に表示しています',
    en: 'This is an approximate mid-surface check of the current pose; actual creases and layer offsets have not been verified. Single-hinge continuous-path checking is shown separately.',
  }),
  limitationUnverified: Object.freeze({
    ja: 'これは現在姿勢に対する中央面基準の近似判定で、実際の折り癖、層ずれ、連続運動中の衝突は未検証です',
    en: 'This is an approximate mid-surface check of the current pose; actual creases, layer offsets, and collisions during continuous motion have not been verified.',
  }),
  safetyReview: Object.freeze({
    ja: '判定保留は安全確認が必要です。',
    en: 'Indeterminate results require safety review. ',
  }),
  detailedAccessible: Object.freeze({
    ja: '現在姿勢の広域候補は{totalCandidates}件、狭域相互作用は{narrowInteractions}件、非隣接貫通{nonAdjacentPenetrations}件、中央面基準の共有ヒンジモデル外貫通{hingeOutsidePenetrations}件、非隣接接触{nonAdjacentContacts}件、共有頂点のみと証明した許容接触{topologyModelCount}件、共有ヒンジモデル外接触{hingeOutsideContacts}件、モデルで許容した折り目境界接触{hingeModelAllowedContacts}件、折り目領域内重なり{hingeModelCorridorOverlaps}件、厚さ0の許容平坦積層{hingeModelFlatSurfaceStacks}件、層ずらし未再現{hingeLayerOffsetUnmodeled}件、ヒンジ未解決{hingeUnresolvedInteractions}件、交差の可能性・判定保留{indeterminateInteractions}件。{safetyReview}{limitation}',
    en: 'Current pose: {totalCandidates} broad-phase candidates, {narrowInteractions} narrow-phase interactions, {nonAdjacentPenetrations} non-adjacent penetrations, {hingeOutsidePenetrations} penetrations outside the mid-surface shared-hinge model, {nonAdjacentContacts} non-adjacent contacts, {topologyModelCount} allowed contacts proven to occur only at a shared vertex, {hingeOutsideContacts} contacts outside the shared-hinge model, {hingeModelAllowedContacts} crease-boundary contacts allowed by the model, {hingeModelCorridorOverlaps} overlaps within the crease region, {hingeModelFlatSurfaceStacks} allowed zero-thickness flat stacks, {hingeLayerOffsetUnmodeled} unmodeled layer offsets, {hingeUnresolvedInteractions} unresolved hinge interactions, and {indeterminateInteractions} possible intersections / indeterminate results. {safetyReview}{limitation}',
  }),
  detailed: Object.freeze({
    ja: '現在姿勢: 貫通 {penetrationCount}・接触 {contactCount}・共有頂点モデル許容 {topologyModelCount}・ヒンジモデル許容 {hingeModelCount}・未解決 {hingeUnresolvedInteractions}・交差の可能性・判定保留 {indeterminateInteractions}（広域 {totalCandidates}→狭域 {narrowInteractions}）',
    en: 'Current pose: penetration {penetrationCount} · contact {contactCount} · shared-vertex model allowed {topologyModelCount} · hinge model allowed {hingeModelCount} · unresolved {hingeUnresolvedInteractions} · possible intersection / indeterminate {indeterminateInteractions} (broad {totalCandidates} → narrow {narrowInteractions})',
  }),
})

const COLLISION_BADGE_TEXT = Object.freeze({
  pending: COLLISION_VIEW_TEXT.pending,
  unavailable: COLLISION_VIEW_TEXT.unavailable,
  suffix: Object.freeze({ ja: '・{detail}', en: ' · {detail}' }),
  penetrating: Object.freeze({
    ja: '貫通 {penetrationCount}（ヒンジ外 {hingeOutsidePenetrations}）・接触 {contactCount}{holdSuffix}',
    en: 'Penetration {penetrationCount} (outside hinge {hingeOutsidePenetrations}) · contact {contactCount}{holdSuffix}',
  }),
  holdWithContact: Object.freeze({
    ja: '{holdText}・接触 {contactCount}',
    en: '{holdText} · contact {contactCount}',
  }),
  contact: Object.freeze({
    ja: '接触 {contactCount}（ヒンジ外 {hingeOutsideContacts}）・貫通 0',
    en: 'Contact {contactCount} (outside hinge {hingeOutsideContacts}) · penetration 0',
  }),
  sharedVertex: Object.freeze({
    ja: '共有頂点の許容接触 {count}・貫通 0',
    en: 'Allowed shared-vertex contact {count} · penetration 0',
  }),
  flatStack: Object.freeze({
    ja: '厚さ0の許容平坦積層 {count}・通常貫通 0',
    en: 'Allowed zero-thickness flat stack {count} · ordinary penetration 0',
  }),
  corridor: Object.freeze({
    ja: '許容折り目領域内重なり {overlaps}・境界接触 {contacts}',
    en: 'Allowed crease-region overlap {overlaps} · boundary contact {contacts}',
  }),
  hingeContact: Object.freeze({
    ja: 'ヒンジ境界接触 {count}・他衝突 0',
    en: 'Hinge-boundary contact {count} · other collisions 0',
  }),
  clear: Object.freeze({
    ja: '現在姿勢: 衝突候補 0',
    en: 'Current pose: 0 collision candidates',
  }),
  noNarrowInteraction: Object.freeze({
    ja: '広域 {count} → 狭域相互作用 0',
    en: 'Broad phase {count} → narrow-phase interactions 0',
  }),
  layerOffsetHold: Object.freeze({
    ja: '層ずらし未再現のため判定不能 {count}・安全確認が必要・貫通許可なし',
    en: 'Indeterminate because layer offsets are not modeled {count} · safety review required · penetration not allowed',
  }),
  hingeDetail: Object.freeze({
    ja: '（ヒンジ未解決 {count}）',
    en: ' (unresolved hinge {count})',
  }),
  indeterminate: Object.freeze({
    ja: '交差の可能性・判定保留 {count}{hingeDetail}・安全確認が必要',
    en: 'Possible intersection / indeterminate {count}{hingeDetail} · safety review required',
  }),
  hingeUnresolved: Object.freeze({
    ja: '交差の可能性・判定保留（ヒンジ未解決 {count}）・安全確認が必要',
    en: 'Possible intersection / indeterminate (unresolved hinge {count}) · safety review required',
  }),
})

function compareText(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}
