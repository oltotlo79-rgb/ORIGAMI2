export const FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_VERSION =
  'topology_contact_policy_v1'

export const FOLD_PREVIEW_TOPOLOGY_CONTACT_POLICY_V2 =
  'topology_contact_policy_v2'

export type FoldPreviewTopologyRelation =
  | 'no_shared_feature'
  | 'shared_vertex'
  | 'shared_hinge_edge'
  | 'same_face'

export type FoldPreviewIntersectionEvidence =
  | 'separated'
  | 'point_contact'
  | 'boundary_line_contact'
  | 'shared_feature_contact'
  | 'shared_feature_thickness_overlap'
  | 'shared_feature_flat_stack'
  | 'coplanar_area_overlap'
  | 'transversal_crossing'
  | 'positive_volume_overlap'
  | 'indeterminate'

export type FoldPreviewIntersectionEvidenceV2 =
  | FoldPreviewIntersectionEvidence
  | 'boundary_area_contact'

export type FoldPreviewTopologyContactDecision =
  | 'separated'
  | 'touching'
  | 'allowed_shared_vertex_contact'
  | 'requires_hinge_model'
  | 'penetrating'
  | 'indeterminate'
  | 'ignored_self'

/**
 * Exhaustive topology × intersection policy for one pair of material faces.
 *
 * This function classifies evidence; it does not derive the evidence. In
 * particular, `shared_feature_thickness_overlap` may be supplied only after
 * the mid-surfaces have been rechecked and their complete intersection has
 * been proved to be the matching topology feature. A shared ID alone never
 * authorizes that evidence.
 *
 * Hinge contact still requires the separate finite-axis, opposing-half-plane,
 * thickness-corridor, and flat-stack checks. This table therefore returns
 * `requires_hinge_model` rather than granting a hinge exception itself.
 */
export function classifyFoldPreviewTopologyContact(
  topology: FoldPreviewTopologyRelation,
  evidence: FoldPreviewIntersectionEvidence,
): FoldPreviewTopologyContactDecision {
  if (topology === 'same_face') return 'ignored_self'
  if (evidence === 'indeterminate') return 'indeterminate'
  if (
    (topology === 'shared_vertex' && evidence === 'separated')
    || (
      topology === 'shared_hinge_edge'
      && (
        evidence === 'separated'
        || evidence === 'point_contact'
        || evidence === 'boundary_line_contact'
      )
    )
  ) return 'indeterminate'
  if (evidence === 'separated') return 'separated'

  if (topology === 'no_shared_feature') {
    if (
      evidence === 'point_contact'
      || evidence === 'boundary_line_contact'
    ) return 'touching'
    if (
      evidence === 'shared_feature_contact'
      || evidence === 'shared_feature_thickness_overlap'
      || evidence === 'shared_feature_flat_stack'
    ) return 'indeterminate'
    return 'penetrating'
  }

  if (topology === 'shared_vertex') {
    if (
      evidence === 'shared_feature_contact'
      || evidence === 'shared_feature_thickness_overlap'
    ) return 'allowed_shared_vertex_contact'
    if (evidence === 'shared_feature_flat_stack') return 'indeterminate'
    if (
      evidence === 'point_contact'
      || evidence === 'boundary_line_contact'
    ) return 'touching'
    return 'penetrating'
  }

  if (topology === 'shared_hinge_edge') {
    if (
      evidence === 'shared_feature_contact'
      || evidence === 'shared_feature_thickness_overlap'
      || evidence === 'shared_feature_flat_stack'
    ) return 'requires_hinge_model'
    if (
      evidence === 'point_contact'
      || evidence === 'boundary_line_contact'
    ) return 'touching'
    return 'penetrating'
  }

  return 'indeterminate'
}

/**
 * Exhaustive V2 policy. V1 remains frozen for existing runtime certificates;
 * new native evidence generators bind V2 and may additionally prove a
 * positive-area, zero-positive-volume material-boundary contact.
 */
export function classifyFoldPreviewTopologyContactV2(
  topology: FoldPreviewTopologyRelation,
  evidence: FoldPreviewIntersectionEvidenceV2,
): FoldPreviewTopologyContactDecision {
  if (evidence !== 'boundary_area_contact') {
    return classifyFoldPreviewTopologyContact(topology, evidence)
  }
  if (topology === 'same_face') return 'ignored_self'
  if (topology === 'shared_hinge_edge') return 'requires_hinge_model'
  return 'touching'
}
