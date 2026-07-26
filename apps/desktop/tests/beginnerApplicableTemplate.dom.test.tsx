import { describe, expect, it } from 'vitest'

import {
  isBeginnerApplicableTemplate,
} from '../src/lib/beginnerApplicableTemplate.ts'
import type { BeginnerCandidateResponseV1 } from '../src/lib/coreClient.ts'

type GeneratedPlanKind =
  BeginnerCandidateResponseV1['generated_plans'][number]['kind']

const ALL_PLAN_KINDS = [
  'symmetric_four_leg_base',
  'symmetric_wing_base',
  'symmetric_bird_base',
  'asymmetric_bird_landmark_base',
  'asymmetric_four_leg_landmark_base',
  'asymmetric_insect_landmark_base',
  'asymmetric_fish_landmark_base',
  'symmetric_fish_base',
  'symmetric_ear_base',
  'symmetric_horn_base',
  'symmetric_antenna_base',
  'symmetric_insect_leg_pair_base',
  'symmetric_six_leg_base',
  'center_axis_tail_base',
  'center_axis_horn_base',
  'center_axis_antenna_base',
  'composite_tail_ear_base',
  'composite_horn_ear_base',
  'composite_horn_tail_base',
  'composite_horn_tail_ear_base',
  'composite_wing_antenna_base',
  'composite_complete_insect_base',
  'composite_complete_animal_base',
  'composite_complete_winged_animal_base',
  'composite_generic_target_base',
  'vertical_book_fold',
  'horizontal_book_fold',
  'diagonal_fold',
] as const satisfies readonly GeneratedPlanKind[]

const INAPPLICABLE_KINDS = new Set<GeneratedPlanKind>([
  'vertical_book_fold',
  'horizontal_book_fold',
  'diagonal_fold',
])

describe('beginner generated-plan apply predicate', () => {
  it('preserves the complete legacy truth table', () => {
    expect(ALL_PLAN_KINDS).toHaveLength(28)
    expect(new Set(ALL_PLAN_KINDS).size).toBe(ALL_PLAN_KINDS.length)
    for (const kind of ALL_PLAN_KINDS) {
      expect(
        isBeginnerApplicableTemplate(kind),
        `unexpected apply predicate for ${kind}`,
      ).toBe(!INAPPLICABLE_KINDS.has(kind))
    }
  })
})
