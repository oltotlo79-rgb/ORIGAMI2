import assert from 'node:assert/strict'
import test from 'node:test'

import {
  resolveCompleteAnimalBindings,
} from '../src/lib/completeAnimalBindings.ts'
import { normalizeBeginnerDesignProfile } from '../src/lib/coreClient.ts'

type Target = ReturnType<typeof target>

function target(
  id: number,
  count: number,
  direction: [number, number, number],
  symmetry: 'none' | 'bilateral',
) {
  return {
    id,
    count,
    length_tenths_mm: 100,
    thickness_tenths_mm: 10,
    position_tenths_mm: [0, 0, 0] as [number, number, number],
    direction_milli: direction,
    symmetry,
    curvature_degrees: 0,
    joint: 'fixed' as const,
    motion_degrees: [0, 0] as [number, number],
    side: 'either' as const,
    priority: 50,
  }
}

const horn = target(1, 1, [0, -1_000, 0], 'none')
const tail = target(2, 1, [1_000, 0, 0], 'none')
const ears = target(3, 2, [1_000, 0, 0], 'bilateral')
const legs = target(4, 4, [0, 1_000, 0], 'bilateral')
const wing = target(5, 2, [1_000, 0, 0], 'bilateral')
const animal = [horn, tail, ears, legs]

function permutations<T>(values: readonly T[]): T[][] {
  if (values.length === 0) return [[]]
  return values.flatMap((value, index) => permutations([
    ...values.slice(0, index),
    ...values.slice(index + 1),
  ]).map((rest) => [value, ...rest]))
}

function profile(protrusions: readonly Target[], hasWing: boolean) {
  return {
    schema_version: 1,
    preset: 'balanced',
    shape_fidelity_weight: 35,
    foldability_weight: 35,
    step_count_weight: 15,
    paper_efficiency_weight: 15,
    generation_constraints: {
      schema_version: 1,
      maximum_steps: 60,
      detail_level: 'standard',
      target_category: 'animal',
      target_parts: [
        { kind: 'head', count: 1 },
        { kind: 'torso', count: 1 },
        { kind: 'horn', count: 1 },
        { kind: 'tail', count: 1 },
        { kind: 'ear', count: 2 },
        { kind: 'leg', count: 4 },
        ...(hasWing ? [{ kind: 'wing', count: 2 }] : []),
      ],
      skeleton_segments: [],
      protrusions,
      bulge_targets: [],
      target_asset: null,
      allowed_techniques: ['valley_fold'],
    },
  }
}

test('all storage permutations resolve the same four complete-animal roles', () => {
  for (const permutation of permutations(animal)) {
    const bindings = resolveCompleteAnimalBindings(permutation, false)
    assert.deepEqual(bindings?.ordered.map(({ id }) => id), [1, 2, 3, 4])
    assert.ok(normalizeBeginnerDesignProfile(profile(permutation, false)))

    const winged = [...permutation, wing]
    const wingedBindings = resolveCompleteAnimalBindings(winged, true)
    assert.deepEqual(
      wingedBindings?.ordered.map(({ id }) => id),
      [1, 2, 3, 4, 5],
    )
    assert.ok(normalizeBeginnerDesignProfile(profile(winged, true)))
  }
})

test('missing, duplicate, ambiguous, and non-final wing bindings fail closed', () => {
  const ambiguousHorn = target(6, 1, [0, 1_000, 0], 'none')
  for (const invalid of [
    animal.slice(0, 3),
    [horn, tail, ears, { ...legs, id: ears.id }],
    [horn, ambiguousHorn, ears, legs],
  ]) {
    assert.equal(resolveCompleteAnimalBindings(invalid, false), null)
    assert.equal(normalizeBeginnerDesignProfile(profile(invalid, false)), null)
  }

  const nonFinalWing = [wing, horn, tail, ears, legs]
  assert.equal(resolveCompleteAnimalBindings(nonFinalWing, true), null)
  assert.equal(
    normalizeBeginnerDesignProfile(profile(nonFinalWing, true)),
    null,
  )
})
