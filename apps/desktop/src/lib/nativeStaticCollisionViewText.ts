import type {
  CurrentStaticCollisionEvidence,
  CurrentStaticCollisionPairDisposition,
  CurrentStaticCollisionPolicyDecision,
  CurrentStaticCollisionTopology,
} from './nativeStaticCollisionView.ts'
import type { LocalizedText } from './i18n.ts'

type NativeStaticCollisionViewTextKey =
  | 'idleBadge'
  | 'idleAccessible'
  | 'waitingBadge'
  | 'waitingAccessible'
  | 'checkingBadge'
  | 'checkingAccessible'
  | 'failedBadge'
  | 'failedAccessible'
  | 'certifiedBadge'
  | 'certifiedAccessible'
  | 'zeroThicknessPenetrationBadge'
  | 'zeroThicknessPenetrationAccessible'
  | 'positiveThicknessPenetrationBadge'
  | 'positiveThicknessPenetrationAccessible'
  | 'evidenceLabel'
  | 'resourceLabel'
  | 'inconsistentLabel'
  | 'evidenceAccessible'
  | 'resourceAccessible'
  | 'inconsistentAccessible'
  | 'indeterminateBadge'
  | 'unavailableBadge'
  | 'unavailableAccessible'
  | 'safetyReview'
  | 'withSafetyReview'
  | 'pairCounts'
  | 'omittedPairs'
  | 'pairBasis'
  | 'pairText'
  | 'pairAccessibleText'
  | 'pairAccessibleCounts'
  | 'allPairsDisplayed'
  | 'pairConnector'
  | 'proofMarkerSeparator'

export type NativeStaticCollisionProofMarker =
  | 'strictTransversalDualGate'
  | 'wholeFaceOverlap'
  | 'sharedHingeBoundaryContact'
  | 'sharedHingeSolidClassification'

function localized(ja: string, en: string): LocalizedText {
  return Object.freeze({ ja, en })
}

export const NATIVE_STATIC_COLLISION_VIEW_TEXT: Readonly<
  Record<NativeStaticCollisionViewTextKey, LocalizedText>
> = Object.freeze({
  idleBadge: localized(
    '厳密判定｜姿勢待機',
    'Exact check | Waiting for pose',
  ),
  idleAccessible: localized(
    '厳密衝突判定は、安定した表示姿勢を待っています。',
    'The exact collision check is waiting for a stable displayed pose.',
  ),
  waitingBadge: localized(
    '厳密判定｜姿勢確定待ち',
    'Exact check | Waiting for stable pose',
  ),
  waitingAccessible: localized(
    '表示姿勢の移動が終わってから厳密判定します。',
    'The exact check will run after the displayed pose stops moving.',
  ),
  checkingBadge: localized(
    '厳密判定｜確認中',
    'Exact check | Checking',
  ),
  checkingAccessible: localized(
    '現在の表示姿勢を厳密判定しています。',
    'Running the exact check on the current displayed pose.',
  ),
  failedBadge: localized(
    '厳密判定｜実行失敗・安全確認が必要',
    'Exact check | Failed · safety review required',
  ),
  failedAccessible: localized(
    '厳密衝突判定を完了できませんでした。',
    'The exact collision check could not be completed.',
  ),
  certifiedBadge: localized(
    '厳密判定｜ゼロ厚み面貫通・重なりなし',
    'Exact check | No zero-thickness surface penetration or overlap',
  ),
  certifiedAccessible: localized(
    '現在の表示姿勢では、対象となる全ての面ペアについて、ゼロ厚み面の貫通または正の面積を持つ重なりがないことを証明しました。',
    'For the current displayed pose, every applicable face pair was proven to have no zero-thickness surface penetration or positive-area overlap.',
  ),
  zeroThicknessPenetrationBadge: localized(
    '厳密判定｜ゼロ厚み面貫通・重なり{countText}・安全認定不可',
    'Exact check | Zero-thickness surface penetration or overlap{countText} · safety certification denied',
  ),
  zeroThicknessPenetrationAccessible: localized(
    '現在の表示姿勢でゼロ厚み面の貫通または正の面積を持つ重なり{countText}件を証明したため、安全認定を遮断しました。',
    'Safety certification was blocked because zero-thickness surface penetration or positive-area overlap{countText} was proven in the current displayed pose.',
  ),
  positiveThicknessPenetrationBadge: localized(
    '厳密判定｜紙厚を含む材料貫通 {count}・安全認定不可',
    'Exact check | Material penetration including paper thickness {count} · safety certification denied',
  ),
  positiveThicknessPenetrationAccessible: localized(
    '現在の表示姿勢で紙厚を含む材料の貫通{count}件を厳密証明したため、安全認定を遮断しました。',
    'Safety certification was blocked because {count} material penetrations including paper thickness were exactly proven in the current displayed pose.',
  ),
  evidenceLabel: localized('証拠不足', 'Insufficient evidence'),
  resourceLabel: localized('資源上限', 'Resource limit'),
  inconsistentLabel: localized('状態不整合', 'Inconsistent state'),
  evidenceAccessible: localized(
    '必要な面ペア証拠を取得できませんでした。',
    'The required face-pair evidence could not be obtained.',
  ),
  resourceAccessible: localized(
    '厳密判定の資源上限に達しました。',
    'The exact check reached its resource limit.',
  ),
  inconsistentAccessible: localized(
    '姿勢または判定状態の整合性を確認できませんでした。',
    'The pose or collision-check state could not be verified as consistent.',
  ),
  indeterminateBadge: localized(
    '厳密判定｜{reasonLabel}・交差の可能性・判定保留',
    'Exact check | {reasonLabel} · possible intersection / indeterminate',
  ),
  unavailableBadge: localized(
    '厳密判定｜利用不可・安全確認が必要',
    'Exact check | Unavailable · safety review required',
  ),
  unavailableAccessible: localized(
    '現在の表示姿勢に対する厳密衝突判定を利用できません。',
    'The exact collision check is unavailable for the current displayed pose.',
  ),
  safetyReview: localized(
    'この姿勢を安全確認済みとして扱わないでください。',
    'Do not treat this pose as safety-verified.',
  ),
  withSafetyReview: localized(
    '{prefix}{safetyReview}',
    '{prefix} {safetyReview}',
  ),
  pairCounts: localized(
    '面ペア {total}件: 分離 {separated} / 接触 {touching} / 許容 {allowed} / 貫通 {penetrating} / 判定保留 {indeterminate}',
    'Face pairs {total}: separated {separated} / touching {touching} / allowed {allowed} / penetrating {penetrating} / indeterminate {indeterminate}',
  ),
  omittedPairs: localized(
    '全{total}件中{displayed}件を表示し、{omitted}件を省略しています。貫通・判定保留を優先表示しています。',
    'Showing {displayed} of {total} pairs; {omitted} omitted. Penetrating and indeterminate pairs are prioritized.',
  ),
  pairBasis: localized(
    ' / 根拠: {markers}',
    ' / basis: {markers}',
  ),
  pairText: localized(
    '{index}. {disposition} — {pair} — {topology} / {evidence} / 方針 {policy}{markerText}',
    '{index}. {disposition} — {pair} — {topology} / {evidence} / policy {policy}{markerText}',
  ),
  pairAccessibleText: localized(
    '面ペア {index}、{firstFaceId} と {secondFaceId}。分類 {disposition}。位相 {topology}。幾何根拠 {evidence}。方針判定 {policy}{markerText}。',
    'Face pair {index}, {firstFaceId} and {secondFaceId}. Classification {disposition}. Topology {topology}. Geometric evidence {evidence}. Policy decision {policy}{markerText}.',
  ),
  pairAccessibleCounts: localized(
    '{counts}。判定保留は貫通と同じく安全確認を遮断します。{display}',
    '{counts}. Indeterminate pairs block safety confirmation with the same prominence as penetration. {display}',
  ),
  allPairsDisplayed: localized(
    '全ペアを表示しています。',
    'All pairs are displayed.',
  ),
  pairConnector: localized(' ↔ ', ' ↔ '),
  proofMarkerSeparator: localized('・', ', '),
})

export const NATIVE_STATIC_COLLISION_PAIR_DISPOSITION_TEXT: Readonly<
  Record<CurrentStaticCollisionPairDisposition, LocalizedText>
> = Object.freeze({
  separated: localized('分離', 'separated'),
  touching: localized('接触', 'touching'),
  allowed: localized('許容', 'allowed'),
  penetrating: localized('貫通', 'penetrating'),
  indeterminate: localized('判定保留', 'indeterminate'),
})

export const NATIVE_STATIC_COLLISION_PAIR_TOPOLOGY_TEXT: Readonly<
  Record<CurrentStaticCollisionTopology, LocalizedText>
> = Object.freeze({
  no_shared_feature: localized('共有要素なし', 'no shared feature'),
  shared_vertex: localized('頂点共有', 'shared vertex'),
  shared_hinge_edge: localized('ヒンジ辺共有', 'shared hinge edge'),
})

export const NATIVE_STATIC_COLLISION_PAIR_EVIDENCE_TEXT: Readonly<
  Record<CurrentStaticCollisionEvidence, LocalizedText>
> = Object.freeze({
  separated: localized('離間', 'separated'),
  point_contact: localized('点接触', 'point contact'),
  boundary_line_contact: localized('線接触', 'boundary line contact'),
  boundary_area_contact: localized('境界面接触', 'boundary area contact'),
  shared_feature_contact: localized(
    '共有要素上の接触',
    'shared-feature contact',
  ),
  shared_feature_thickness_overlap: localized(
    '共有要素の厚み重なり',
    'shared-feature thickness overlap',
  ),
  shared_feature_flat_stack: localized(
    '共有要素の平坦積層（層順認証時のみ許容）',
    'shared-feature flat stack (allowed only with certified layer order)',
  ),
  coplanar_area_overlap: localized(
    '同一平面の面積重なり',
    'coplanar area overlap',
  ),
  transversal_crossing: localized('横断交差', 'transversal crossing'),
  positive_volume_overlap: localized(
    '正体積重なり',
    'positive-volume overlap',
  ),
  indeterminate: localized(
    '幾何判定保留',
    'geometric evidence indeterminate',
  ),
})

export const NATIVE_STATIC_COLLISION_PAIR_POLICY_TEXT: Readonly<
  Record<CurrentStaticCollisionPolicyDecision, LocalizedText>
> = Object.freeze({
  separated: localized('分離', 'separated'),
  touching: localized('接触', 'touching'),
  allowed_shared_vertex_contact: localized(
    '共有頂点接触を許容',
    'allowed shared-vertex contact',
  ),
  requires_hinge_model: localized(
    'ヒンジモデル必須',
    'hinge model required',
  ),
  penetrating: localized('貫通', 'penetrating'),
  indeterminate: localized('判定保留', 'indeterminate'),
})

export const NATIVE_STATIC_COLLISION_PROOF_MARKER_TEXT: Readonly<
  Record<NativeStaticCollisionProofMarker, LocalizedText>
> = Object.freeze({
  strictTransversalDualGate: localized(
    '横断交差の二重証明',
    'dual-gate transversal proof',
  ),
  wholeFaceOverlap: localized(
    '面全体の重なり証明',
    'whole-face overlap proof',
  ),
  sharedHingeBoundaryContact: localized(
    '共有ヒンジ境界限定接触の証明',
    'shared-hinge boundary-only contact proof',
  ),
  sharedHingeSolidClassification: localized(
    '共有ヒンジ実体分類',
    'shared-hinge solid classification',
  ),
})
