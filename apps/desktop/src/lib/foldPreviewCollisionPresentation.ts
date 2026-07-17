import type {
  FoldPreviewNarrowPhaseInteraction,
  FoldPreviewNarrowPhaseResult,
} from './foldPreviewNarrowCollision'

export type FoldPreviewFaceCollisionSeverity = 'contact' | 'indeterminate' | 'penetrating'

export type FoldPreviewCollisionPresentation = Readonly<{
  totalCandidates: number
  nonAdjacentCandidates: number
  hingeAdjacentCandidates: number
  narrowInteractions: number
  nonAdjacentPenetrations: number
  nonAdjacentContacts: number
  hingeInteractions: number
  indeterminateInteractions: number
  faceSeverities: ReadonlyMap<string, FoldPreviewFaceCollisionSeverity>
}>

export function summarizeFoldPreviewCollision(
  result: FoldPreviewNarrowPhaseResult,
): FoldPreviewCollisionPresentation {
  let nonAdjacentPenetrations = 0
  let nonAdjacentContacts = 0
  let hingeInteractions = 0
  let indeterminateInteractions = 0
  const faceSeverities = new Map<string, FoldPreviewFaceCollisionSeverity>()

  for (const interaction of result.interactions) {
    if (interaction.relation === 'hinge_adjacent') hingeInteractions += 1
    if (interaction.geometryClass === 'indeterminate') indeterminateInteractions += 1
    if (
      interaction.relation === 'non_adjacent'
      && interaction.geometryClass === 'penetrating'
    ) nonAdjacentPenetrations += 1
    if (
      interaction.relation === 'non_adjacent'
      && interaction.geometryClass === 'touching'
    ) nonAdjacentContacts += 1

    const severity = faceSeverity(interaction)
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
    indeterminateInteractions,
    faceSeverities,
  }
}

function faceSeverity(interaction: FoldPreviewNarrowPhaseInteraction) {
  if (interaction.geometryClass === 'indeterminate') return 'indeterminate'
  if (interaction.relation !== 'non_adjacent') return null
  return interaction.geometryClass === 'penetrating' ? 'penetrating' : 'contact'
}

function raiseSeverity(
  severities: Map<string, FoldPreviewFaceCollisionSeverity>,
  faceId: string,
  severity: FoldPreviewFaceCollisionSeverity,
) {
  const rank: Record<FoldPreviewFaceCollisionSeverity, number> = {
    contact: 1,
    indeterminate: 2,
    penetrating: 3,
  }
  const current = severities.get(faceId)
  if (!current || rank[severity] > rank[current]) severities.set(faceId, severity)
}
