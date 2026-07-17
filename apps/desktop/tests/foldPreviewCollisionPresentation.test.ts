import assert from 'node:assert/strict'
import test from 'node:test'

import { summarizeFoldPreviewCollision } from '../src/lib/foldPreviewCollisionPresentation.ts'
import type {
  FoldPreviewNarrowPhaseInteraction,
  FoldPreviewNarrowPhaseResult,
} from '../src/lib/foldPreviewNarrowCollision.ts'

test('non-adjacent penetration outranks contact and indeterminate face highlights', () => {
  const presentation = summarizeFoldPreviewCollision(result([
    interaction('a', 'b', 'non_adjacent', 'touching'),
    interaction('a', 'c', 'non_adjacent', 'indeterminate'),
    interaction('a', 'd', 'non_adjacent', 'penetrating'),
  ]))
  assert.equal(presentation.faceSeverities.get('a'), 'penetrating')
  assert.equal(presentation.faceSeverities.get('b'), 'contact')
  assert.equal(presentation.faceSeverities.get('c'), 'indeterminate')
  assert.equal(presentation.faceSeverities.get('d'), 'penetrating')
})

test('hinge contact remains unhighlighted while hinge indeterminate stays visible', () => {
  const presentation = summarizeFoldPreviewCollision(result([
    interaction('a', 'b', 'hinge_adjacent', 'touching'),
    interaction('c', 'd', 'hinge_adjacent', 'penetrating'),
    interaction('e', 'f', 'hinge_adjacent', 'indeterminate'),
  ]))
  assert.equal(presentation.hingeInteractions, 3)
  assert.equal(presentation.faceSeverities.has('a'), false)
  assert.equal(presentation.faceSeverities.has('c'), false)
  assert.equal(presentation.faceSeverities.get('e'), 'indeterminate')
  assert.equal(presentation.faceSeverities.get('f'), 'indeterminate')
})

test('presentation counts preserve broad and narrow categories independently', () => {
  const presentation = summarizeFoldPreviewCollision({
    ...result([
      interaction('a', 'b', 'non_adjacent', 'penetrating'),
      interaction('c', 'd', 'non_adjacent', 'touching'),
      interaction('e', 'f', 'hinge_adjacent', 'indeterminate'),
    ]),
    broadPhaseCandidates: 7,
    broadPhaseNonAdjacentCandidates: 5,
    broadPhaseHingeAdjacentCandidates: 2,
  })
  assert.deepEqual({
    totalCandidates: presentation.totalCandidates,
    nonAdjacentCandidates: presentation.nonAdjacentCandidates,
    hingeAdjacentCandidates: presentation.hingeAdjacentCandidates,
    narrowInteractions: presentation.narrowInteractions,
    nonAdjacentPenetrations: presentation.nonAdjacentPenetrations,
    nonAdjacentContacts: presentation.nonAdjacentContacts,
    hingeInteractions: presentation.hingeInteractions,
    indeterminateInteractions: presentation.indeterminateInteractions,
  }, {
    totalCandidates: 7,
    nonAdjacentCandidates: 5,
    hingeAdjacentCandidates: 2,
    narrowInteractions: 3,
    nonAdjacentPenetrations: 1,
    nonAdjacentContacts: 1,
    hingeInteractions: 1,
    indeterminateInteractions: 1,
  })
})

function result(
  interactions: readonly FoldPreviewNarrowPhaseInteraction[],
): FoldPreviewNarrowPhaseResult {
  return {
    broadPhaseCandidates: interactions.length,
    broadPhaseNonAdjacentCandidates: interactions.filter(
      ({ relation }) => relation === 'non_adjacent',
    ).length,
    broadPhaseHingeAdjacentCandidates: interactions.filter(
      ({ relation }) => relation === 'hinge_adjacent',
    ).length,
    interactions,
    trianglePairTests: interactions.length,
    satTests: interactions.length,
    numericalMargin: Number.EPSILON * 64,
  }
}

function interaction(
  firstFaceId: string,
  secondFaceId: string,
  relation: FoldPreviewNarrowPhaseInteraction['relation'],
  geometryClass: FoldPreviewNarrowPhaseInteraction['geometryClass'],
): FoldPreviewNarrowPhaseInteraction {
  return {
    firstFaceId,
    secondFaceId,
    relation,
    hingeEdgeIds: relation === 'hinge_adjacent' ? ['hinge'] : [],
    geometryClass,
  }
}
