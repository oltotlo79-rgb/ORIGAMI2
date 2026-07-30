import {
  snapshotDensePlainArray,
  snapshotPlainDataRecord,
} from './beginnerGeneratedPlanSnapshot.ts'
import type {
  BeginnerGeneratedPlanKindV1,
} from './beginnerGeneratedPlanTypes.ts'

export {
  beginnerGeneratedPlanInstructionsAreCanonicalV1,
} from './beginnerGeneratedPlanInstructionContract.ts'
export {
  beginnerGenericFeatureBindingIdentityIsCanonicalV1,
  MAX_BEGINNER_GENERIC_FEATURE_BINDINGS_V1,
} from './beginnerGenericFeatureBindingContract.ts'
export type {
  BeginnerGeneratedPlanInstructionContextV1,
  BeginnerGeneratedPlanKindV1,
} from './beginnerGeneratedPlanTypes.ts'

type BeginnerTargetPartKindV1 =
  | 'head'
  | 'torso'
  | 'leg'
  | 'horn'
  | 'ear'
  | 'wing'
  | 'fin'
  | 'antenna'
  | 'tail'

type BeginnerTargetPartRecordV1 = Readonly<{
  kind: BeginnerTargetPartKindV1
  count: number
}>

const BEGINNER_TARGET_PART_KINDS_V1 = new Set<BeginnerTargetPartKindV1>([
  'head',
  'torso',
  'leg',
  'horn',
  'ear',
  'wing',
  'fin',
  'antenna',
  'tail',
])

// All bounded append stages may coexist in one native generic plan:
// center + 32 semantic endpoints, body outline, 14 eight-point local outlines,
// five uncovered paper-corner supports, and the 17-node / 16-bar tree witness.
export const MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1 = 183
export const MAX_BEGINNER_GENERIC_PLAN_EDGES_V1 = 181
export const MAX_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1 = 127
export const MAX_BEGINNER_SPECIALIZED_PLAN_EDGES_V1 = 126
export const MIN_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1 = 2
export const MAX_BEGINNER_GENERAL_FEATURE_ENDPOINTS_V1 = 14

const MINIMUM_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1 = Object.freeze({
  symmetric_four_leg_base: 5,
  symmetric_wing_base: 3,
  symmetric_bird_base: 3,
  asymmetric_bird_landmark_base: 5,
  asymmetric_four_leg_landmark_base: 5,
  asymmetric_insect_landmark_base: 5,
  asymmetric_fish_landmark_base: 5,
  symmetric_fish_base: 3,
  symmetric_ear_base: 3,
  symmetric_horn_base: 3,
  symmetric_antenna_base: 3,
  symmetric_insect_leg_pair_base: 3,
  symmetric_six_leg_base: 7,
  center_axis_tail_base: 2,
  center_axis_horn_base: 2,
  center_axis_antenna_base: 2,
  composite_tail_ear_base: 4,
  composite_horn_ear_base: 4,
  composite_horn_tail_base: 3,
  composite_horn_tail_ear_base: 5,
  composite_wing_antenna_base: 5,
  composite_complete_insect_base: 11,
  composite_complete_animal_base: 9,
  composite_complete_winged_animal_base: 11,
} satisfies Readonly<Record<
  Exclude<
    BeginnerGeneratedPlanKindV1,
    | 'composite_generic_target_base'
    | 'vertical_book_fold'
    | 'horizontal_book_fold'
    | 'diagonal_fold'
  >,
  number
>>)

export function beginnerGeneratedPlanSizeIsAdmissibleV1(
  kind: BeginnerGeneratedPlanKindV1,
  vertexCount: number,
  edgeCount: number,
): boolean {
  if (
    typeof kind !== 'string'
    || !Number.isInteger(vertexCount)
    || !Number.isInteger(edgeCount)
  ) {
    return false
  }
  if (
    kind === 'vertical_book_fold'
    || kind === 'horizontal_book_fold'
    || kind === 'diagonal_fold'
  ) return vertexCount === 2 && edgeCount === 1
  if (kind === 'composite_generic_target_base') {
    return vertexCount >= 2
      && edgeCount >= 1
      && vertexCount <= MAX_BEGINNER_GENERIC_PLAN_VERTICES_V1
      && edgeCount <= MAX_BEGINNER_GENERIC_PLAN_EDGES_V1
  }
  if (!Object.hasOwn(MINIMUM_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1, kind)) {
    return false
  }
  const minimumVertices =
    MINIMUM_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1[
      kind as keyof typeof MINIMUM_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1
    ]
  return vertexCount >= minimumVertices
    && edgeCount >= minimumVertices - 1
    // Every specialized native base is a star/tree. Body and local outlines
    // append equal vertex/edge counts, preserving this exact difference.
    && vertexCount === edgeCount + 1
    // Native specialized families use at most ten base endpoints. The shared
    // contour budget is one 16-point body plus twelve 8-point local outlines.
    && vertexCount <= MAX_BEGINNER_SPECIALIZED_PLAN_VERTICES_V1
    && edgeCount <= MAX_BEGINNER_SPECIALIZED_PLAN_EDGES_V1
}

export const MAX_BEGINNER_TARGET_PART_RECORDS_V1 = 10

export function beginnerTargetPartRecordCountIsAdmissibleV1(
  records: readonly unknown[],
): boolean {
  return snapshotDensePlainArray(
    records,
    MAX_BEGINNER_TARGET_PART_RECORDS_V1,
  ) !== null
}

function snapshotBeginnerTargetPartsV1(
  value: unknown,
): readonly BeginnerTargetPartRecordV1[] | null {
  const records = snapshotDensePlainArray(
    value,
    MAX_BEGINNER_TARGET_PART_RECORDS_V1,
  )
  if (!records) return null
  const parts: BeginnerTargetPartRecordV1[] = []
  const kinds = new Set<BeginnerTargetPartKindV1>()
  let total = 0
  for (const raw of records) {
    const part = snapshotPlainDataRecord(raw, ['kind', 'count'])
    if (
      !part
      || typeof part.kind !== 'string'
      || !BEGINNER_TARGET_PART_KINDS_V1.has(
        part.kind as BeginnerTargetPartKindV1,
      )
      || kinds.has(part.kind as BeginnerTargetPartKindV1)
      || !Number.isInteger(part.count)
      || Number(part.count) < 1
      || Number(part.count) > 8
    ) return null
    total += Number(part.count)
    if (total > 32) return null
    const kind = part.kind as BeginnerTargetPartKindV1
    kinds.add(kind)
    parts.push({ kind, count: Number(part.count) })
  }
  return parts
}

function targetPartSignatureMatchesV1(
  parts: readonly BeginnerTargetPartRecordV1[],
  features: readonly Readonly<{
    kind: BeginnerTargetPartKindV1
    count: number
  }>[],
): boolean {
  if (parts.length !== features.length + 2) return false
  const expected = new Map<BeginnerTargetPartKindV1, number>([
    ['head', 1],
    ['torso', 1],
    ...features.map((feature) => [feature.kind, feature.count] as const),
  ])
  return parts.every((part) => expected.get(part.kind) === part.count)
}

/**
 * Rechecks the semantic target signature that selects each native plan family.
 * Generic custom targets may intentionally omit semantic part records because
 * their physical endpoints remain bound by the native contour witness.
 */
export function beginnerGeneratedPlanTargetPartsAreCompatibleV1(
  kind: BeginnerGeneratedPlanKindV1,
  value: unknown,
): boolean {
  if (typeof kind !== 'string') return false
  const parts = snapshotBeginnerTargetPartsV1(value)
  if (!parts) return false
  if (kind === 'composite_generic_target_base') return true
  if (
    kind === 'vertical_book_fold'
    || kind === 'horizontal_book_fold'
    || kind === 'diagonal_fold'
  ) {
    return parts.some((part) => part.kind === 'head' && part.count === 1)
      && parts.some((part) => part.kind === 'torso' && part.count === 1)
      && parts.some((part) =>
        part.kind !== 'head' && part.kind !== 'torso')
  }
  type SpecializedKind = Exclude<
    BeginnerGeneratedPlanKindV1,
    | 'composite_generic_target_base'
    | 'vertical_book_fold'
    | 'horizontal_book_fold'
    | 'diagonal_fold'
  >
  type Feature = Readonly<{
    kind: BeginnerTargetPartKindV1
    count: number
  }>
  const signatures: Partial<Record<
    SpecializedKind,
    readonly (readonly Feature[])[]
  >> = {
    symmetric_four_leg_base: [[{ kind: 'leg', count: 4 }]],
    asymmetric_four_leg_landmark_base: [[{ kind: 'leg', count: 4 }]],
    symmetric_wing_base: [
      [{ kind: 'wing', count: 2 }],
      [{ kind: 'wing', count: 4 }],
    ],
    symmetric_bird_base: [[{ kind: 'wing', count: 2 }]],
    asymmetric_bird_landmark_base: [[{ kind: 'wing', count: 2 }]],
    asymmetric_insect_landmark_base: [[
      { kind: 'tail', count: 1 },
      { kind: 'wing', count: 2 },
      { kind: 'leg', count: 6 },
    ]],
    asymmetric_fish_landmark_base: [[
      { kind: 'tail', count: 1 },
      { kind: 'fin', count: 2 },
    ]],
    symmetric_fish_base: [[{ kind: 'fin', count: 2 }]],
    symmetric_ear_base: [[{ kind: 'ear', count: 2 }]],
    symmetric_horn_base: [[{ kind: 'horn', count: 2 }]],
    symmetric_antenna_base: [[{ kind: 'antenna', count: 2 }]],
    symmetric_insect_leg_pair_base: [[{ kind: 'leg', count: 2 }]],
    symmetric_six_leg_base: [[{ kind: 'leg', count: 6 }]],
    center_axis_tail_base: [[{ kind: 'tail', count: 1 }]],
    center_axis_horn_base: [[{ kind: 'horn', count: 1 }]],
    center_axis_antenna_base: [[{ kind: 'antenna', count: 1 }]],
    composite_tail_ear_base: [[
      { kind: 'tail', count: 1 },
      { kind: 'ear', count: 2 },
    ]],
    composite_horn_ear_base: [[
      { kind: 'horn', count: 1 },
      { kind: 'ear', count: 2 },
    ]],
    composite_horn_tail_base: [[
      { kind: 'horn', count: 1 },
      { kind: 'tail', count: 1 },
    ]],
    composite_horn_tail_ear_base: [[
      { kind: 'horn', count: 1 },
      { kind: 'tail', count: 1 },
      { kind: 'ear', count: 2 },
    ]],
    composite_wing_antenna_base: [[
      { kind: 'wing', count: 2 },
      { kind: 'antenna', count: 2 },
    ]],
    composite_complete_insect_base: [[
      { kind: 'wing', count: 2 },
      { kind: 'antenna', count: 2 },
      { kind: 'leg', count: 6 },
    ]],
    composite_complete_animal_base: [[
      { kind: 'horn', count: 1 },
      { kind: 'tail', count: 1 },
      { kind: 'ear', count: 2 },
      { kind: 'leg', count: 4 },
    ]],
    composite_complete_winged_animal_base: [[
      { kind: 'horn', count: 1 },
      { kind: 'tail', count: 1 },
      { kind: 'ear', count: 2 },
      { kind: 'leg', count: 4 },
      { kind: 'wing', count: 2 },
    ]],
  }
  const allowed = signatures[
    kind as keyof typeof signatures
  ]
  return allowed?.some((features) =>
    targetPartSignatureMatchesV1(parts, features)
  ) ?? false
}
