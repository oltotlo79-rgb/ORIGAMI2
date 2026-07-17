import type {
  FoldPreviewNarrowPhaseInteraction,
  FoldPreviewNarrowPhaseResult,
} from './foldPreviewNarrowCollision'

export type FoldPreviewFaceCollisionSeverity = 'contact' | 'indeterminate' | 'penetrating'

const FACE_COLLISION_SEVERITY_RANK: Readonly<Record<
  FoldPreviewFaceCollisionSeverity,
  number
>> = {
  contact: 1,
  indeterminate: 2,
  penetrating: 3,
}

export type FoldPreviewCollisionPresentation = Readonly<{
  totalCandidates: number
  nonAdjacentCandidates: number
  hingeAdjacentCandidates: number
  narrowInteractions: number
  nonAdjacentPenetrations: number
  nonAdjacentContacts: number
  hingeInteractions: number
  hingeModelAllowedContacts: number
  hingeModelCorridorOverlaps: number
  hingeOutsidePenetrations: number
  hingeOutsideContacts: number
  hingeUnresolvedInteractions: number
  indeterminateInteractions: number
  faceSeverities: ReadonlyMap<string, FoldPreviewFaceCollisionSeverity>
}>

type HingePresentationKind =
  | 'not_hinge'
  | 'allowed_contact'
  | 'allowed_overlap'
  | 'outside_penetration'
  | 'outside_contact'
  | 'indeterminate'
  | 'unresolved'

export function summarizeFoldPreviewCollision(
  result: FoldPreviewNarrowPhaseResult,
): FoldPreviewCollisionPresentation {
  let nonAdjacentPenetrations = 0
  let nonAdjacentContacts = 0
  let hingeInteractions = 0
  let hingeModelAllowedContacts = 0
  let hingeModelCorridorOverlaps = 0
  let hingeOutsidePenetrations = 0
  let hingeOutsideContacts = 0
  let hingeUnresolvedInteractions = 0
  let indeterminateInteractions = 0
  const faceSeverities = new Map<string, FoldPreviewFaceCollisionSeverity>()

  for (const interaction of result.interactions) {
    const hingeKind = classifyHingePresentation(interaction)
    if (interaction.relation === 'hinge_adjacent') {
      hingeInteractions += 1
      if (hingeKind === 'allowed_contact') hingeModelAllowedContacts += 1
      else if (hingeKind === 'allowed_overlap') hingeModelCorridorOverlaps += 1
      else if (hingeKind === 'outside_penetration') hingeOutsidePenetrations += 1
      else if (hingeKind === 'outside_contact') hingeOutsideContacts += 1
      else hingeUnresolvedInteractions += 1
    }
    if (
      interaction.geometryClass === 'indeterminate'
      || hingeKind === 'indeterminate'
    ) indeterminateInteractions += 1
    if (
      interaction.relation === 'non_adjacent'
      && interaction.geometryClass === 'penetrating'
    ) nonAdjacentPenetrations += 1
    if (
      interaction.relation === 'non_adjacent'
      && interaction.geometryClass === 'touching'
    ) nonAdjacentContacts += 1

    const severity = faceSeverity(interaction, hingeKind)
    if (!severity) continue
    raiseSeverity(faceSeverities, interaction.firstFaceId, severity)
    raiseSeverity(faceSeverities, interaction.secondFaceId, severity)
  }

  return {
    totalCandidates: result.broadPhaseCandidates,
    nonAdjacentCandidates: result.broadPhaseNonAdjacentCandidates,
    hingeAdjacentCandidates: result.broadPhaseHingeAdjacentCandidates,
    narrowInteractions: result.interactions.length,
    nonAdjacentPenetrations,
    nonAdjacentContacts,
    hingeInteractions,
    hingeModelAllowedContacts,
    hingeModelCorridorOverlaps,
    hingeOutsidePenetrations,
    hingeOutsideContacts,
    hingeUnresolvedInteractions,
    indeterminateInteractions,
    faceSeverities,
  }
}

function classifyHingePresentation(
  interaction: FoldPreviewNarrowPhaseInteraction,
): HingePresentationKind {
  if (interaction.relation !== 'hinge_adjacent') return 'not_hinge'
  const decision = interaction.hingeDecision
  if (!decision) return 'unresolved'
  if (decision.kind === 'indeterminate') return 'indeterminate'
  if (!interaction.hingeEdgeIds.includes(decision.hingeEdgeId)) return 'indeterminate'
  if (decision.kind === 'outside_hinge_penetration') {
    return interaction.geometryClass === 'penetrating'
      ? 'outside_penetration'
      : 'indeterminate'
  }
  if (decision.kind === 'outside_hinge_contact') {
    return interaction.geometryClass === 'touching'
      ? 'outside_contact'
      : 'indeterminate'
  }
  if (
    decision.geometry === 'boundary_contact'
    && interaction.geometryClass === 'touching'
  ) return 'allowed_contact'
  if (
    decision.geometry === 'corridor_overlap'
    && interaction.geometryClass === 'penetrating'
  ) return 'allowed_overlap'
  return 'indeterminate'
}

function faceSeverity(
  interaction: FoldPreviewNarrowPhaseInteraction,
  hingeKind: HingePresentationKind,
) {
  if (interaction.relation === 'hinge_adjacent') {
    if (hingeKind === 'outside_penetration') return 'penetrating'
    if (hingeKind === 'outside_contact') return 'contact'
    if (
      hingeKind === 'indeterminate'
      || interaction.geometryClass === 'indeterminate'
    ) return 'indeterminate'
    return null
  }
  if (interaction.geometryClass === 'indeterminate') return 'indeterminate'
  return interaction.geometryClass === 'penetrating' ? 'penetrating' : 'contact'
}

function raiseSeverity(
  severities: Map<string, FoldPreviewFaceCollisionSeverity>,
  faceId: string,
  severity: FoldPreviewFaceCollisionSeverity,
) {
  const current = severities.get(faceId)
  if (
    !current
    || FACE_COLLISION_SEVERITY_RANK[severity] > FACE_COLLISION_SEVERITY_RANK[current]
  ) severities.set(faceId, severity)
}
