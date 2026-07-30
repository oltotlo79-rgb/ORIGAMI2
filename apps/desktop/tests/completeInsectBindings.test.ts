import assert from 'node:assert/strict'
import test from 'node:test'

import {
  resolveCompleteInsectBindings,
} from '../src/lib/completeInsectBindings.ts'

function target(
  id: number,
  direction: [number, number, number],
  y: number,
  priority: number,
) {
  return {
    id,
    count: 2,
    symmetry: 'bilateral' as const,
    direction_milli: direction,
    position_tenths_mm: [0, y, 0] as [number, number, number],
    priority,
  }
}

const wing = target(41, [1_000, 0, 0], 0, 60)
const antenna = target(7, [0, -1_000, 0], 0, 60)
const rearLegs = target(90, [1_000, 0, 0], -30, 50)
const middleLegs = target(3, [1_000, 0, 0], 0, 50)
const frontLegs = target(55, [1_000, 0, 0], 30, 50)
const targets = [wing, antenna, rearLegs, middleLegs, frontLegs]

function permutations<T>(values: readonly T[]): T[][] {
  if (values.length === 0) return [[]]
  return values.flatMap((value, index) => permutations([
    ...values.slice(0, index),
    ...values.slice(index + 1),
  ]).map((rest) => [value, ...rest]))
}

test('all 120 storage permutations resolve arbitrary IDs in semantic order', () => {
  for (const permutation of permutations(targets)) {
    const bindings = resolveCompleteInsectBindings(permutation)
    assert.deepEqual(
      bindings?.ordered.map(({ id }) => id),
      [41, 7, 90, 3, 55],
    )
  }
})

test('duplicate roles, IDs, priorities, tied legs, missing, and excess fail closed', () => {
  const duplicateWing = target(70, [-1_000, 0, 0], 15, 60)
  for (const invalid of [
    targets.slice(0, 4),
    [...targets, target(100, [1_000, 0, 0], 45, 50)],
    [wing, antenna, rearLegs, middleLegs, { ...frontLegs, id: middleLegs.id }],
    [wing, antenna, duplicateWing, middleLegs, frontLegs],
    [{ ...wing, priority: 50 }, antenna, rearLegs, middleLegs, frontLegs],
    [wing, antenna, rearLegs, middleLegs, {
      ...frontLegs,
      position_tenths_mm: [0, middleLegs.position_tenths_mm[1], 0],
    }],
  ]) {
    assert.equal(resolveCompleteInsectBindings(invalid), null)
  }
})
