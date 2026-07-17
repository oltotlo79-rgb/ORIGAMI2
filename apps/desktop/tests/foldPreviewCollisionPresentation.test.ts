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
  assert.equal(presentation.hingeUnresolvedInteractions, 3)
  assert.equal(presentation.faceSeverities.has('a'), false)
  assert.equal(presentation.faceSeverities.has('c'), false)
  assert.equal(presentation.faceSeverities.get('e'), 'indeterminate')
  assert.equal(presentation.faceSeverities.get('f'), 'indeterminate')
})

test('hinge policy separates model contact, corridor overlap, and outside collision', () => {
  const presentation = summarizeFoldPreviewCollision(result([
    interaction('a', 'b', 'hinge_adjacent', 'touching', {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: 'boundary_contact',
      thicknessRule: 'centered_mid_surface_v1',
    }),
    interaction('c', 'd', 'hinge_adjacent', 'penetrating', {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: 'corridor_overlap',
      thicknessRule: 'centered_mid_surface_v1',
    }),
    interaction('e', 'f', 'hinge_adjacent', 'penetrating', {
      kind: 'outside_hinge_penetration',
      hingeEdgeId: 'hinge',
    }),
    interaction('g', 'h', 'hinge_adjacent', 'touching', {
      kind: 'outside_hinge_contact',
      hingeEdgeId: 'hinge',
    }),
    interaction('i', 'j', 'hinge_adjacent', 'penetrating', {
      kind: 'indeterminate',
      hingeEdgeIds: ['hinge'],
      reason: 'corridor_boundary',
    }),
  ]))
  assert.deepEqual({
    allowed: presentation.hingeModelAllowedContacts,
    corridor: presentation.hingeModelCorridorOverlaps,
    penetrations: presentation.hingeOutsidePenetrations,
    contacts: presentation.hingeOutsideContacts,
    unresolved: presentation.hingeUnresolvedInteractions,
    indeterminate: presentation.indeterminateInteractions,
  }, {
    allowed: 1,
    corridor: 1,
    penetrations: 1,
    contacts: 1,
    unresolved: 1,
    indeterminate: 1,
  })
  assert.equal(presentation.faceSeverities.has('a'), false)
  assert.equal(presentation.faceSeverities.has('c'), false)
  assert.equal(presentation.faceSeverities.get('e'), 'penetrating')
  assert.equal(presentation.faceSeverities.get('g'), 'contact')
  assert.equal(presentation.faceSeverities.get('i'), 'indeterminate')
})

test('presentation fails closed for misplaced and contradictory hinge decisions', () => {
  const presentation = summarizeFoldPreviewCollision(result([
    interaction('a', 'b', 'non_adjacent', 'penetrating', {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: 'corridor_overlap',
      thicknessRule: 'centered_mid_surface_v1',
    }),
    interaction('c', 'd', 'hinge_adjacent', 'indeterminate', {
      kind: 'allowed_by_hinge_model',
      hingeEdgeId: 'hinge',
      geometry: 'boundary_contact',
      thicknessRule: 'centered_mid_surface_v1',
    }),
    interaction('e', 'f', 'hinge_adjacent', 'touching', {
      kind: 'outside_hinge_penetration',
      hingeEdgeId: 'hinge',
    }),
    interaction('g', 'h', 'hinge_adjacent', 'penetrating', {
      kind: 'outside_hinge_penetration',
      hingeEdgeId: 'different-hinge',
    }),
  ]))
  assert.equal(presentation.nonAdjacentPenetrations, 1)
  assert.equal(presentation.hingeOutsidePenetrations, 0)
  assert.equal(presentation.hingeModelAllowedContacts, 0)
  assert.equal(presentation.hingeModelCorridorOverlaps, 0)
  assert.equal(presentation.hingeUnresolvedInteractions, 3)
  assert.equal(presentation.indeterminateInteractions, 3)
  assert.equal(presentation.faceSeverities.get('a'), 'penetrating')
  assert.equal(presentation.faceSeverities.get('c'), 'indeterminate')
  assert.equal(presentation.faceSeverities.get('e'), 'indeterminate')
  assert.equal(presentation.faceSeverities.get('g'), 'indeterminate')
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
    hingeModelAllowedContacts: presentation.hingeModelAllowedContacts,
    hingeModelCorridorOverlaps: presentation.hingeModelCorridorOverlaps,
    hingeOutsidePenetrations: presentation.hingeOutsidePenetrations,
    hingeOutsideContacts: presentation.hingeOutsideContacts,
    hingeUnresolvedInteractions: presentation.hingeUnresolvedInteractions,
    indeterminateInteractions: presentation.indeterminateInteractions,
  }, {
    totalCandidates: 7,
    nonAdjacentCandidates: 5,
    hingeAdjacentCandidates: 2,
    narrowInteractions: 3,
    nonAdjacentPenetrations: 1,
    nonAdjacentContacts: 1,
    hingeInteractions: 1,
    hingeModelAllowedContacts: 0,
    hingeModelCorridorOverlaps: 0,
    hingeOutsidePenetrations: 0,
    hingeOutsideContacts: 0,
    hingeUnresolvedInteractions: 1,
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
  hingeDecision?: FoldPreviewNarrowPhaseInteraction['hingeDecision'],
): FoldPreviewNarrowPhaseInteraction {
  return {
    firstFaceId,
    secondFaceId,
    relation,
    hingeEdgeIds: relation === 'hinge_adjacent' ? ['hinge'] : [],
    geometryClass,
    ...(hingeDecision ? { hingeDecision } : {}),
  }
}
