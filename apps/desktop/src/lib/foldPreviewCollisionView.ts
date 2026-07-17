import type { FoldPreviewHingeAngle } from './foldPreviewKinematics'
import type { FoldPreviewModel } from './foldPreviewModel'

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
      hingeInteractions: number
      hingeModelAllowedContacts: number
      hingeModelCorridorOverlaps: number
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
      && first.hingeInteractions === second.hingeInteractions
      && first.hingeModelAllowedContacts === second.hingeModelAllowedContacts
      && first.hingeModelCorridorOverlaps === second.hingeModelCorridorOverlaps
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
) {
  if (!summary) return accessible ? '現在姿勢の衝突候補を判定中' : '衝突判定中'
  if (summary.kind === 'unavailable') {
    return accessible ? '現在姿勢の衝突判定は利用できません' : '衝突判定不能'
  }
  if (summary.totalCandidates === 0) {
    if (pathDisclosure === 'separately_reported') {
      return accessible
        ? '現在姿勢の広域候補と狭域相互作用は0件。単一ヒンジの連続経路判定は別に表示しています'
        : '現在姿勢: 衝突候補 0（経路判定は別表示）'
    }
    return accessible
      ? '現在姿勢の広域候補と狭域相互作用は0件。連続運動中の衝突は未検証です'
      : '現在姿勢: 衝突候補 0（連続運動は未検証）'
  }
  const penetrationCount = summary.nonAdjacentPenetrations
    + summary.hingeOutsidePenetrations
  const contactCount = summary.nonAdjacentContacts + summary.hingeOutsideContacts
  const hingeModelCount = summary.hingeModelAllowedContacts
    + summary.hingeModelCorridorOverlaps
  const limitation = pathDisclosure === 'separately_reported'
    ? 'これは現在姿勢に対する中央面基準の近似判定で、実際の折り癖と層ずれは未検証です。単一ヒンジの連続経路判定は別に表示しています'
    : 'これは現在姿勢に対する中央面基準の近似判定で、実際の折り癖、層ずれ、連続運動中の衝突は未検証です'
  return accessible
    ? `現在姿勢の広域候補は${summary.totalCandidates}件、狭域相互作用は${summary.narrowInteractions}件、非隣接貫通${summary.nonAdjacentPenetrations}件、中央面基準の共有ヒンジモデル外貫通${summary.hingeOutsidePenetrations}件、非隣接接触${summary.nonAdjacentContacts}件、共有ヒンジモデル外接触${summary.hingeOutsideContacts}件、モデルで許容した折り目境界接触${summary.hingeModelAllowedContacts}件、折り目領域内重なり${summary.hingeModelCorridorOverlaps}件、ヒンジ未解決${summary.hingeUnresolvedInteractions}件、数値または方針不確定${summary.indeterminateInteractions}件。${limitation}`
    : `現在姿勢: 貫通 ${penetrationCount}・接触 ${contactCount}・ヒンジモデル許容 ${hingeModelCount}・未解決 ${summary.hingeUnresolvedInteractions}・不確定 ${summary.indeterminateInteractions}（広域 ${summary.totalCandidates}→狭域 ${summary.narrowInteractions}）`
}

export function collisionDataStatus(summary: CollisionSummary | null) {
  if (!summary) return 'pending'
  if (summary.kind === 'unavailable') return 'unavailable'
  if (summary.nonAdjacentPenetrations + summary.hingeOutsidePenetrations > 0) {
    return 'penetrating'
  }
  if (summary.indeterminateInteractions > 0) return 'indeterminate'
  if (summary.nonAdjacentContacts + summary.hingeOutsideContacts > 0) return 'contact'
  if (summary.hingeUnresolvedInteractions > 0) return 'hinge-unresolved'
  if (summary.hingeModelAllowedContacts + summary.hingeModelCorridorOverlaps > 0) {
    return 'hinge-model'
  }
  return 'clear'
}

export function collisionBadgeClass(summary: CollisionSummary | null) {
  if (!summary || summary.kind === 'unavailable') return 'is-unavailable'
  if (summary.nonAdjacentPenetrations + summary.hingeOutsidePenetrations > 0) {
    return 'has-penetrations'
  }
  if (summary.indeterminateInteractions > 0) return 'has-indeterminate'
  if (summary.nonAdjacentContacts + summary.hingeOutsideContacts > 0) return 'has-contact'
  if (
    summary.hingeUnresolvedInteractions > 0
    || summary.hingeModelAllowedContacts + summary.hingeModelCorridorOverlaps > 0
  ) return 'has-hinge-candidates'
  return 'is-clear'
}

export function collisionBadgeText(summary: CollisionSummary | null) {
  if (!summary) return '衝突判定中'
  if (summary.kind === 'unavailable') return '衝突判定不能'
  const penetrationCount = summary.nonAdjacentPenetrations
    + summary.hingeOutsidePenetrations
  const contactCount = summary.nonAdjacentContacts + summary.hingeOutsideContacts
  if (penetrationCount > 0) {
    return `貫通 ${penetrationCount}（ヒンジ外 ${summary.hingeOutsidePenetrations}）・接触 ${contactCount}`
  }
  if (summary.indeterminateInteractions > 0) {
    return `不確定 ${summary.indeterminateInteractions}・ヒンジ未解決 ${summary.hingeUnresolvedInteractions}`
  }
  if (contactCount > 0) {
    return `接触 ${contactCount}（ヒンジ外 ${summary.hingeOutsideContacts}）・貫通 0`
  }
  if (summary.hingeUnresolvedInteractions > 0) {
    return `ヒンジ未解決 ${summary.hingeUnresolvedInteractions}・貫通 0`
  }
  if (summary.hingeModelCorridorOverlaps > 0) {
    return `許容折り目領域内重なり ${summary.hingeModelCorridorOverlaps}・境界接触 ${summary.hingeModelAllowedContacts}`
  }
  if (summary.hingeModelAllowedContacts > 0) {
    return `ヒンジ境界接触 ${summary.hingeModelAllowedContacts}・他衝突 0`
  }
  return summary.totalCandidates === 0
    ? '現在姿勢: 衝突候補 0'
    : `広域 ${summary.totalCandidates} → 狭域相互作用 0`
}

function compareText(first: string, second: string) {
  return first < second ? -1 : first > second ? 1 : 0
}
