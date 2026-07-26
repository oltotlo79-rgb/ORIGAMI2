import type { LocalizedText } from './i18n.ts'

export type FoldPreviewCollisionViewTextKey =
  | 'pending'
  | 'pendingAccessible'
  | 'unavailable'
  | 'unavailableAccessible'
  | 'clearSeparateAccessible'
  | 'clearSeparate'
  | 'clearUnverifiedAccessible'
  | 'clearUnverified'
  | 'limitationSeparate'
  | 'limitationUnverified'
  | 'safetyReview'
  | 'detailedAccessible'
  | 'detailed'

export type FoldPreviewCollisionBadgeTextKey =
  | 'pending'
  | 'unavailable'
  | 'suffix'
  | 'penetrating'
  | 'holdWithContact'
  | 'contact'
  | 'sharedVertex'
  | 'flatStack'
  | 'corridor'
  | 'hingeContact'
  | 'clear'
  | 'noNarrowInteraction'
  | 'layerOffsetHold'
  | 'hingeDetail'
  | 'indeterminate'
  | 'hingeUnresolved'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const COLLISION_VIEW_TEXT = Object.freeze({
  pending: localized('衝突判定中', 'Collision check in progress'),
  pendingAccessible: localized(
    '現在姿勢の衝突候補を判定中',
    'Checking collision candidates for the current pose',
  ),
  unavailable: localized(
    '衝突判定不能・安全確認が必要',
    'Collision check unavailable · safety review required',
  ),
  unavailableAccessible: localized(
    '現在姿勢の衝突判定は利用できません。安全確認が必要です',
    'Collision checking is unavailable for the current pose. Safety review is required.',
  ),
  clearSeparateAccessible: localized(
    '現在姿勢の広域候補と狭域相互作用は0件。単一ヒンジの連続経路判定は別に表示しています',
    'Current-pose broad-phase candidates and narrow-phase interactions: 0. Single-hinge continuous-path checking is shown separately.',
  ),
  clearSeparate: localized(
    '現在姿勢: 衝突候補 0（経路判定は別表示）',
    'Current pose: 0 collision candidates (path checking shown separately)',
  ),
  clearUnverifiedAccessible: localized(
    '現在姿勢の広域候補と狭域相互作用は0件。連続運動中の衝突は未検証です',
    'Current-pose broad-phase candidates and narrow-phase interactions: 0. Collisions during continuous motion have not been verified.',
  ),
  clearUnverified: localized(
    '現在姿勢: 衝突候補 0（連続運動は未検証）',
    'Current pose: 0 collision candidates (continuous motion unverified)',
  ),
  limitationSeparate: localized(
    'これは現在姿勢に対する中央面基準の近似判定で、実際の折り癖と層ずれは未検証です。単一ヒンジの連続経路判定は別に表示しています',
    'This is an approximate mid-surface check of the current pose; actual creases and layer offsets have not been verified. Single-hinge continuous-path checking is shown separately.',
  ),
  limitationUnverified: localized(
    'これは現在姿勢に対する中央面基準の近似判定で、実際の折り癖、層ずれ、連続運動中の衝突は未検証です',
    'This is an approximate mid-surface check of the current pose; actual creases, layer offsets, and collisions during continuous motion have not been verified.',
  ),
  safetyReview: localized(
    '判定保留は安全確認が必要です。',
    'Indeterminate results require safety review. ',
  ),
  detailedAccessible: localized(
    '現在姿勢の広域候補は{totalCandidates}件、狭域相互作用は{narrowInteractions}件、非隣接貫通{nonAdjacentPenetrations}件、中央面基準の共有ヒンジモデル外貫通{hingeOutsidePenetrations}件、非隣接接触{nonAdjacentContacts}件、共有頂点のみと証明した許容接触{topologyModelCount}件、共有ヒンジモデル外接触{hingeOutsideContacts}件、モデルで許容した折り目境界接触{hingeModelAllowedContacts}件、折り目領域内重なり{hingeModelCorridorOverlaps}件、厚さ0の許容平坦積層{hingeModelFlatSurfaceStacks}件、層ずらし未再現{hingeLayerOffsetUnmodeled}件、ヒンジ未解決{hingeUnresolvedInteractions}件、交差の可能性・判定保留{indeterminateInteractions}件。{safetyReview}{limitation}',
    'Current pose: {totalCandidates} broad-phase candidates, {narrowInteractions} narrow-phase interactions, {nonAdjacentPenetrations} non-adjacent penetrations, {hingeOutsidePenetrations} penetrations outside the mid-surface shared-hinge model, {nonAdjacentContacts} non-adjacent contacts, {topologyModelCount} allowed contacts proven to occur only at a shared vertex, {hingeOutsideContacts} contacts outside the shared-hinge model, {hingeModelAllowedContacts} crease-boundary contacts allowed by the model, {hingeModelCorridorOverlaps} overlaps within the crease region, {hingeModelFlatSurfaceStacks} allowed zero-thickness flat stacks, {hingeLayerOffsetUnmodeled} unmodeled layer offsets, {hingeUnresolvedInteractions} unresolved hinge interactions, and {indeterminateInteractions} possible intersections / indeterminate results. {safetyReview}{limitation}',
  ),
  detailed: localized(
    '現在姿勢: 貫通 {penetrationCount}・接触 {contactCount}・共有頂点モデル許容 {topologyModelCount}・ヒンジモデル許容 {hingeModelCount}・未解決 {hingeUnresolvedInteractions}・交差の可能性・判定保留 {indeterminateInteractions}（広域 {totalCandidates}→狭域 {narrowInteractions}）',
    'Current pose: penetration {penetrationCount} · contact {contactCount} · shared-vertex model allowed {topologyModelCount} · hinge model allowed {hingeModelCount} · unresolved {hingeUnresolvedInteractions} · possible intersection / indeterminate {indeterminateInteractions} (broad {totalCandidates} → narrow {narrowInteractions})',
  ),
}) satisfies Readonly<
  Record<FoldPreviewCollisionViewTextKey, LocalizedText>
>

export const COLLISION_BADGE_TEXT = Object.freeze({
  pending: COLLISION_VIEW_TEXT.pending,
  unavailable: COLLISION_VIEW_TEXT.unavailable,
  suffix: localized('・{detail}', ' · {detail}'),
  penetrating: localized(
    '貫通 {penetrationCount}（ヒンジ外 {hingeOutsidePenetrations}）・接触 {contactCount}{holdSuffix}',
    'Penetration {penetrationCount} (outside hinge {hingeOutsidePenetrations}) · contact {contactCount}{holdSuffix}',
  ),
  holdWithContact: localized(
    '{holdText}・接触 {contactCount}',
    '{holdText} · contact {contactCount}',
  ),
  contact: localized(
    '接触 {contactCount}（ヒンジ外 {hingeOutsideContacts}）・貫通 0',
    'Contact {contactCount} (outside hinge {hingeOutsideContacts}) · penetration 0',
  ),
  sharedVertex: localized(
    '共有頂点の許容接触 {count}・貫通 0',
    'Allowed shared-vertex contact {count} · penetration 0',
  ),
  flatStack: localized(
    '厚さ0の許容平坦積層 {count}・通常貫通 0',
    'Allowed zero-thickness flat stack {count} · ordinary penetration 0',
  ),
  corridor: localized(
    '許容折り目領域内重なり {overlaps}・境界接触 {contacts}',
    'Allowed crease-region overlap {overlaps} · boundary contact {contacts}',
  ),
  hingeContact: localized(
    'ヒンジ境界接触 {count}・他衝突 0',
    'Hinge-boundary contact {count} · other collisions 0',
  ),
  clear: localized(
    '現在姿勢: 衝突候補 0',
    'Current pose: 0 collision candidates',
  ),
  noNarrowInteraction: localized(
    '広域 {count} → 狭域相互作用 0',
    'Broad phase {count} → narrow-phase interactions 0',
  ),
  layerOffsetHold: localized(
    '層ずらし未再現のため判定不能 {count}・安全確認が必要・貫通許可なし',
    'Indeterminate because layer offsets are not modeled {count} · safety review required · penetration not allowed',
  ),
  hingeDetail: localized(
    '（ヒンジ未解決 {count}）',
    ' (unresolved hinge {count})',
  ),
  indeterminate: localized(
    '交差の可能性・判定保留 {count}{hingeDetail}・安全確認が必要',
    'Possible intersection / indeterminate {count}{hingeDetail} · safety review required',
  ),
  hingeUnresolved: localized(
    '交差の可能性・判定保留（ヒンジ未解決 {count}）・安全確認が必要',
    'Possible intersection / indeterminate (unresolved hinge {count}) · safety review required',
  ),
}) satisfies Readonly<
  Record<FoldPreviewCollisionBadgeTextKey, LocalizedText>
>
