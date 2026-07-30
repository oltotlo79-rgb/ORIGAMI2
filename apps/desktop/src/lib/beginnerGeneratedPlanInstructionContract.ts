import { analyzeGenericSkeletonTree } from './genericSkeletonTree.ts'
import {
  snapshotCanonicalSkeletonSegmentsV1,
  snapshotDensePlainArray,
  type BeginnerSkeletonSegmentV1,
} from './beginnerGeneratedPlanSnapshot.ts'
import type {
  BeginnerGeneratedPlanInstructionContextV1,
  BeginnerGeneratedPlanKindV1,
} from './beginnerGeneratedPlanTypes.ts'

const NON_GENERIC_BEGINNER_INSTRUCTION_CODE_V1 = Object.freeze({
  symmetric_four_leg_base: 'symmetric_four_leg_base',
  symmetric_wing_base: 'symmetric_wing_base',
  symmetric_bird_base: 'symmetric_bird_base',
  asymmetric_bird_landmark_base: 'asymmetric_bird_landmark_base',
  asymmetric_four_leg_landmark_base: 'asymmetric_four_leg_landmark_base',
  asymmetric_insect_landmark_base: 'asymmetric_insect_landmark_base',
  asymmetric_fish_landmark_base: 'asymmetric_fish_landmark_base',
  symmetric_fish_base: 'symmetric_fish_base',
  symmetric_ear_base: 'symmetric_ear_base',
  symmetric_horn_base: 'symmetric_horn_base',
  symmetric_antenna_base: 'symmetric_antenna_base',
  symmetric_insect_leg_pair_base: 'symmetric_insect_leg_pair_base',
  symmetric_six_leg_base: 'symmetric_six_leg_base',
  center_axis_tail_base: 'center_axis_tail_base',
  center_axis_horn_base: 'center_axis_horn_base',
  center_axis_antenna_base: 'center_axis_antenna_base',
  composite_tail_ear_base: 'composite_tail_ear_base',
  composite_horn_ear_base: 'composite_horn_ear_base',
  composite_horn_tail_base: 'composite_horn_tail_base',
  composite_horn_tail_ear_base: 'composite_horn_tail_ear_base',
  composite_wing_antenna_base: 'composite_wing_antenna_base',
  composite_complete_insect_base: 'composite_complete_insect_base',
  composite_complete_animal_base: 'composite_complete_animal_base',
  composite_complete_winged_animal_base:
    'composite_complete_winged_animal_base',
  vertical_book_fold: 'book_fold_vertical',
  horizontal_book_fold: 'book_fold_horizontal',
  diagonal_fold: 'diagonal_fold',
} satisfies Readonly<Record<
  Exclude<BeginnerGeneratedPlanKindV1, 'composite_generic_target_base'>,
  string
>>)

const MAX_BEGINNER_GENERATED_INSTRUCTION_LENGTH_V1 = 256
const GENERIC_TREE_RATIO_SCALE_V1 = 1_000_000n
const MAX_U32_BIGINT = 4_294_967_295n
const RADIAL_CORNER_SUPPORT_ELIGIBLE_KINDS_V1 =
  new Set<BeginnerGeneratedPlanKindV1>([
    'symmetric_four_leg_base',
    'symmetric_six_leg_base',
    'composite_horn_tail_ear_base',
    'composite_wing_antenna_base',
    'composite_complete_insect_base',
    'composite_complete_animal_base',
    'composite_complete_winged_animal_base',
    'composite_generic_target_base',
  ])

function isCanonicalRadialCornerSupportInstructionV1(
  value: unknown,
  maximumAdded: 4 | 5,
): value is string {
  if (typeof value !== 'string') return false
  const match =
    /^bounded_radial_corner_support_v1:added=([0-5]):covered=4$/u
      .exec(value)
  return match !== null && Number(match[1]) <= maximumAdded
}

/**
 * Admits only the instruction sequence emitted by the corresponding native
 * plan context. Generic-tree ratios are recomputed with integer arithmetic so
 * values above JavaScript's exact multiplication range cannot be rounded.
 */
export function beginnerGeneratedPlanInstructionsAreCanonicalV1(
  kind: BeginnerGeneratedPlanKindV1,
  instructionCodes: readonly string[],
  skeletonSegments: readonly BeginnerSkeletonSegmentV1[],
  context: BeginnerGeneratedPlanInstructionContextV1,
): boolean {
  if (
    typeof kind !== 'string'
    || (context !== 'candidate' && context !== 'grid')
  ) return false
  const codes = snapshotDensePlainArray(instructionCodes, 4)
  if (
    !codes
    || codes.some((code) =>
      typeof code !== 'string'
      || code.length > MAX_BEGINNER_GENERATED_INSTRUCTION_LENGTH_V1)
  ) return false
  if (kind !== 'composite_generic_target_base') {
    if (!Object.hasOwn(NON_GENERIC_BEGINNER_INSTRUCTION_CODE_V1, kind)) {
      return false
    }
    const expected =
      NON_GENERIC_BEGINNER_INSTRUCTION_CODE_V1[
        kind as keyof typeof NON_GENERIC_BEGINNER_INSTRUCTION_CODE_V1
      ]
    if (codes.length === 1) return codes[0] === expected
    return RADIAL_CORNER_SUPPORT_ELIGIBLE_KINDS_V1.has(kind)
      && codes.length === 2
      && codes[0] === expected
      && isCanonicalRadialCornerSupportInstructionV1(codes[1], 4)
  }
  const segments = snapshotCanonicalSkeletonSegmentsV1(skeletonSegments)
  if (!segments) return false
  if (segments.some((segment, index) =>
    segment.start.x_tenths_mm > segment.end.x_tenths_mm
    || (
      segment.start.x_tenths_mm === segment.end.x_tenths_mm
      && segment.start.y_tenths_mm > segment.end.y_tenths_mm
    )
    || (index > 0 && segments[index - 1]!.id >= segment.id))) {
    return false
  }
  const analysis = analyzeGenericSkeletonTree(segments)
  if (analysis.status !== 'tree') return false
  const squaredLengths = segments.map((segment) => {
    const dx = BigInt(segment.end.x_tenths_mm)
      - BigInt(segment.start.x_tenths_mm)
    const dy = BigInt(segment.end.y_tenths_mm)
      - BigInt(segment.start.y_tenths_mm)
    return dx * dx + dy * dy
  })
  const minimum = squaredLengths.reduce<bigint | null>(
    (current, value) => current === null || value < current ? value : current,
    null,
  )
  if (minimum === null || minimum === 0n) return false
  const ratios = squaredLengths.map((value) =>
    value * GENERIC_TREE_RATIO_SCALE_V1 / minimum)
  if (ratios.some((ratio) => ratio < 1n || ratio > MAX_U32_BIGINT)) {
    return false
  }
  const degree = new Map<string, number>()
  for (const segment of segments) {
    for (const point of [segment.start, segment.end]) {
      const key = `${point.x_tenths_mm}:${point.y_tenths_mm}`
      degree.set(key, (degree.get(key) ?? 0) + 1)
    }
  }
  const leaves = [...degree.values()].filter((value) => value === 1).length
  const axial = `bounded_tree_river_axial_v1:${ratios.join(',')}`
  const topology =
    `bounded_tree_branch_topology_v1:nodes=${analysis.pointCount}`
    + `:leaves=${leaves}:bars=${analysis.edgeCount}`
  const radialSupportOffset =
    isCanonicalRadialCornerSupportInstructionV1(codes[1], 5) ? 1 : 0
  if (
    codes[0] !== axial
    || codes[1 + radialSupportOffset] !== topology
  ) return false
  if (context === 'candidate') {
    return codes.length === 2 + radialSupportOffset
  }
  return codes.length === 3 + radialSupportOffset && (
    codes[2 + radialSupportOffset]
      === 'bounded_tree_paper_orientation_v1:horizontal'
    || codes[2 + radialSupportOffset]
      === 'bounded_tree_paper_orientation_v1:vertical'
  )
}
