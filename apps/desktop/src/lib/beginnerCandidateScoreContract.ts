import type {
  BeginnerDesignProfileV1,
} from './coreClient.ts'
import type {
  BeginnerGeneratedPlanKindV1,
} from './beginnerGeneratedPlanContract.ts'
import { isCanonicalNonNilUuid } from './canonicalUuid.ts'
import { sha256BytesV1 } from './sha256Bytes.ts'

const CONSENSUS_PAIR_DOMAIN_V1 = Object.freeze(
  Array.from(
    'origami2-reference-consensus-pair-v1\0',
    (character) => character.charCodeAt(0),
  ),
)

function canonicalUuidBytes(value: string): ReadonlyArray<number> | null {
  if (!isCanonicalNonNilUuid(value)) return null
  const compact = value.replaceAll('-', '')
  return Object.freeze(Array.from(
    { length: 16 },
    (_, index) => Number.parseInt(compact.slice(index * 2, index * 2 + 2), 16),
  ))
}

export function beginnerReferenceConsensusPairDigestV1(
  leftAssetId: string,
  leftSha256: ReadonlyArray<number>,
  rightAssetId: string,
  rightSha256: ReadonlyArray<number>,
  metrics: Readonly<{
    componentError: number
    normalizedExtentError: number
    branchError: number
    agreementScore: number
    disagrees: boolean
  }>,
): ReadonlyArray<number> | null {
  const leftId = canonicalUuidBytes(leftAssetId)
  const rightId = canonicalUuidBytes(rightAssetId)
  const metricBytes = [
    metrics.componentError,
    metrics.normalizedExtentError,
    metrics.branchError,
    metrics.agreementScore,
  ]
  if (
    !leftId
    || !rightId
    || leftSha256.length !== 32
    || rightSha256.length !== 32
    || [...leftSha256, ...rightSha256, ...metricBytes].some((byte) =>
      !Number.isInteger(byte) || byte < 0 || byte > 255)
  ) return null
  return sha256BytesV1([
    ...CONSENSUS_PAIR_DOMAIN_V1,
    ...leftId,
    ...leftSha256,
    ...rightId,
    ...rightSha256,
    ...metricBytes,
    Number(metrics.disagrees),
  ])
}

function targetPartCount(
  profile: BeginnerDesignProfileV1,
  kind: BeginnerDesignProfileV1['generation_constraints']['target_parts'][number]['kind'],
): number {
  return profile.generation_constraints.target_parts.find(
    (part) => part.kind === kind,
  )?.count ?? 0
}

function targetApproximationUsesFourEndpointSpacing(
  profile: BeginnerDesignProfileV1,
  primaryKind: BeginnerGeneratedPlanKindV1,
): boolean {
  return primaryKind === 'symmetric_four_leg_base'
    || primaryKind === 'asymmetric_four_leg_landmark_base'
    || primaryKind === 'composite_horn_tail_ear_base'
    || primaryKind === 'composite_wing_antenna_base'
    || (
      primaryKind === 'composite_generic_target_base'
      && (profile.generation_constraints.protrusions ?? []).reduce(
        (sum, protrusion) => sum + protrusion.count,
        0,
      ) === 4
    )
    || (
      primaryKind === 'symmetric_wing_base'
      && targetPartCount(profile, 'wing') === 4
    )
}

export function beginnerExpectedTargetApproximationScoreV1(
  profile: BeginnerDesignProfileV1,
  primaryKind: BeginnerGeneratedPlanKindV1,
): number {
  const constraints = profile.generation_constraints
  if (
    !constraints.allowed_techniques.includes('valley_fold')
    && !constraints.allowed_techniques.includes('mountain_fold')
  ) return 0
  const protrusions = constraints.protrusions ?? []
  const contourBonus = Math.min(
    15,
    Math.max(
      0,
      (constraints.generic_body_outline_tenths_mm?.length ?? 4) - 4,
    ) + protrusions.reduce(
      (sum, protrusion) => sum + Math.max(
        0,
        (protrusion.local_outline_tenths_mm?.length ?? 3) - 3,
      ),
      0,
    ),
  )
  const surfaceBulgeBonus = Math.min(
    5,
    (constraints.bulge_targets ?? []).filter(
      (target) => target.reference_surface_binding !== undefined,
    ).length,
  )
  if (protrusions.length === 0) {
    const scale = constraints.detail_level === 'simple'
      ? 20
      : constraints.detail_level === 'standard'
        ? 25
        : 30
    const spacing = targetApproximationUsesFourEndpointSpacing(
      profile,
      primaryKind,
    ) ? 35 : 50
    return Math.min(
      100,
      40 + scale + Math.floor(spacing / 5)
        + contourBonus + surfaceBulgeBonus,
    )
  }
  const highestPriority = () => protrusions.reduce((selected, target) =>
    target.priority > selected.priority
      || (target.priority === selected.priority && target.id < selected.id)
      ? target
      : selected)
  const horizontalPair = (requiredPriority?: number) =>
    protrusions.find((target) =>
      target.count === 2
      && target.symmetry === 'bilateral'
      && target.direction_milli[0] !== 0
      && target.direction_milli[1] === 0
      && (
        requiredPriority === undefined
        || target.priority === requiredPriority
      ))
  const verticalSingle = () => protrusions.find((target) =>
    target.count === 1
    && target.symmetry === 'none'
    && target.direction_milli[0] === 0
    && target.direction_milli[1] !== 0)
  let target = null as (typeof protrusions)[number] | null
  if (
    primaryKind === 'composite_generic_target_base'
    || primaryKind.startsWith('asymmetric_')
  ) {
    target = highestPriority()
  } else if (
    primaryKind === 'composite_horn_tail_base'
    || primaryKind === 'composite_horn_ear_base'
    || primaryKind === 'composite_horn_tail_ear_base'
    || primaryKind === 'composite_complete_animal_base'
    || primaryKind === 'composite_complete_winged_animal_base'
  ) {
    target = verticalSingle() ?? null
  } else if (primaryKind === 'composite_tail_ear_base') {
    target = protrusions.find((item) =>
      item.count === 1 && item.symmetry === 'none') ?? null
  } else if (primaryKind === 'composite_wing_antenna_base') {
    target = horizontalPair() ?? null
  } else if (primaryKind === 'composite_complete_insect_base') {
    target = horizontalPair(60) ?? null
  } else if (primaryKind === 'symmetric_six_leg_base') {
    target = protrusions.filter((item) =>
      item.count === 2
      && item.symmetry === 'bilateral'
      && item.direction_milli[0] !== 0)
      .sort((left, right) =>
        left.position_tenths_mm[1] - right.position_tenths_mm[1]
        || left.id - right.id)[0] ?? null
  } else if (protrusions.length === 1) {
    target = protrusions[0]!
  }
  if (!target) return 0
  const base = 60 + Math.floor(Math.min(target.priority, 100) * 2 / 5)
  return Math.min(100, base + contourBonus + surfaceBulgeBonus)
}
