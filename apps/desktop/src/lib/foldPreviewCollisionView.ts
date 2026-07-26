import type { FoldPreviewHingeAngle } from './foldPreviewKinematics'
import type { FoldPreviewModel } from './foldPreviewModel'
import {
  COLLISION_BADGE_TEXT,
  COLLISION_VIEW_TEXT,
} from './foldPreviewCollisionViewText.ts'
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

function compareText(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}
