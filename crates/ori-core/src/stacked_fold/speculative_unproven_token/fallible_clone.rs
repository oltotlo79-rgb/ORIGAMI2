use ori_domain::{
    CreasePattern, InstructionPose, InstructionStep, InstructionTimeline, InstructionVisual, Paper,
    ProjectLayerDocumentV1,
};

use super::SpeculativeUnprovenFoldTokenIssueErrorV1;

pub(crate) fn try_clone_crease_pattern_v1(
    source: &CreasePattern,
) -> Result<CreasePattern, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut vertices = Vec::new();
    try_reserve_exact_v1(&mut vertices, source.vertices.len())?;
    for vertex in &source.vertices {
        vertices.push(ori_domain::Vertex {
            id: vertex.id,
            position: vertex.position,
        });
    }

    let mut edges = Vec::new();
    try_reserve_exact_v1(&mut edges, source.edges.len())?;
    for edge in &source.edges {
        edges.push(ori_domain::Edge {
            id: edge.id,
            start: edge.start,
            end: edge.end,
            kind: edge.kind,
        });
    }
    Ok(CreasePattern { vertices, edges })
}

pub(crate) fn try_clone_paper_v1(
    source: &Paper,
) -> Result<Paper, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut boundary_vertices = Vec::new();
    try_reserve_exact_v1(&mut boundary_vertices, source.boundary_vertices.len())?;
    boundary_vertices.extend(source.boundary_vertices.iter().copied());
    Ok(Paper {
        boundary_vertices,
        thickness_mm: source.thickness_mm,
        length_display_unit: source.length_display_unit,
        cutting_allowed: source.cutting_allowed,
        front: ori_domain::PaperAppearance {
            color: source.front.color,
            texture_asset: source.front.texture_asset,
        },
        back: ori_domain::PaperAppearance {
            color: source.back.color,
            texture_asset: source.back.texture_asset,
        },
    })
}

pub(crate) fn try_clone_instruction_timeline_v1(
    source: &InstructionTimeline,
) -> Result<InstructionTimeline, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut steps = Vec::new();
    try_reserve_exact_v1(&mut steps, source.steps.len())?;
    for step in &source.steps {
        steps.push(try_clone_instruction_step_v1(step)?);
    }
    Ok(InstructionTimeline { steps })
}

pub(super) fn try_clone_instruction_step_v1(
    source: &InstructionStep,
) -> Result<InstructionStep, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(InstructionStep {
        id: source.id,
        title: try_owned_string(&source.title)?,
        description: try_owned_string(&source.description)?,
        caution: try_owned_string(&source.caution)?,
        duration_ms: source.duration_ms,
        visual: try_clone_instruction_visual_v1(&source.visual)?,
        pose: try_clone_instruction_pose_v1(&source.pose)?,
    })
}

fn try_clone_instruction_visual_v1(
    source: &InstructionVisual,
) -> Result<InstructionVisual, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut arrows = Vec::new();
    try_reserve_exact_v1(&mut arrows, source.arrows.len())?;
    for arrow in &source.arrows {
        arrows.push(ori_domain::InstructionArrow {
            start: arrow.start,
            end: arrow.end,
            label: try_owned_string(&arrow.label)?,
        });
    }

    let mut focus_points = Vec::new();
    try_reserve_exact_v1(&mut focus_points, source.focus_points.len())?;
    for focus_point in &source.focus_points {
        focus_points.push(ori_domain::InstructionFocusPoint {
            position: focus_point.position,
            radius: focus_point.radius,
            label: try_owned_string(&focus_point.label)?,
        });
    }

    let mut hand_guides = Vec::new();
    try_reserve_exact_v1(&mut hand_guides, source.hand_guides.len())?;
    for hand_guide in &source.hand_guides {
        hand_guides.push(ori_domain::InstructionHandGuide {
            kind: hand_guide.kind,
            position: hand_guide.position,
            direction: hand_guide.direction,
            label: try_owned_string(&hand_guide.label)?,
        });
    }

    Ok(InstructionVisual {
        camera: source.camera,
        arrows,
        focus_points,
        hand_guides,
        cycle_layer_order_proof_v1: source
            .cycle_layer_order_proof_v1
            .as_ref()
            .map(try_clone_cycle_layer_order_proof_v1)
            .transpose()?,
        path_certificate_reference_v1: source
            .path_certificate_reference_v1
            .as_ref()
            .map(try_clone_path_certificate_reference_v1)
            .transpose()?,
        named_technique_compiler_v1: source
            .named_technique_compiler_v1
            .as_ref()
            .map(try_clone_named_technique_compiler_metadata_v1)
            .transpose()?,
    })
}

fn try_clone_cycle_layer_order_proof_v1(
    source: &ori_domain::CycleLayerOrderProofV1,
) -> Result<ori_domain::CycleLayerOrderProofV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut pairs = Vec::new();
    try_reserve_exact_v1(&mut pairs, source.pairs.len())?;
    pairs.extend(source.pairs.iter().copied());
    Ok(ori_domain::CycleLayerOrderProofV1 {
        version: source.version,
        model_id: try_owned_string(&source.model_id)?,
        target_order_sha256: source.target_order_sha256,
        transition_count: source.transition_count,
        pairs,
    })
}

fn try_clone_path_certificate_reference_v1(
    source: &ori_domain::PathCertificateReferenceV1,
) -> Result<ori_domain::PathCertificateReferenceV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::PathCertificateReferenceV1 {
        version: source.version,
        model_id: try_owned_string(&source.model_id)?,
        binding_sha256: source.binding_sha256,
        source_pose_sha256: source.source_pose_sha256,
        target_pose_sha256: source.target_pose_sha256,
        source_model_binding_sha256: source.source_model_binding_sha256,
        transition_count: source.transition_count,
    })
}

fn try_clone_named_technique_compiler_metadata_v1(
    source: &ori_domain::NamedTechniqueCompilerMetadataV1,
) -> Result<ori_domain::NamedTechniqueCompilerMetadataV1, SpeculativeUnprovenFoldTokenIssueErrorV1>
{
    Ok(ori_domain::NamedTechniqueCompilerMetadataV1 {
        version: source.version,
        model_id: try_owned_string(&source.model_id)?,
        technique_kind: try_owned_string(&source.technique_kind)?,
        segment_index: source.segment_index,
        segment_count: source.segment_count,
        compiler_output_sha256: source.compiler_output_sha256,
    })
}

fn try_clone_instruction_pose_v1(
    source: &InstructionPose,
) -> Result<InstructionPose, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut hinge_angles = Vec::new();
    try_reserve_exact_v1(&mut hinge_angles, source.hinge_angles.len())?;
    hinge_angles.extend(source.hinge_angles.iter().copied());
    Ok(InstructionPose {
        model: source.model,
        source_model_fingerprint: try_owned_string(&source.source_model_fingerprint)?,
        fixed_face: source.fixed_face,
        hinge_angles,
    })
}

pub(crate) fn try_clone_project_layer_document_v1(
    source: &ProjectLayerDocumentV1,
) -> Result<ProjectLayerDocumentV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut layers = Vec::new();
    try_reserve_exact_v1(&mut layers, source.layers.len())?;
    for layer in &source.layers {
        layers.push(ori_domain::LayerRecordV1 {
            id: layer.id,
            name: try_owned_string(&layer.name)?,
            content_kind: layer.content_kind,
            visible: layer.visible,
            locked: layer.locked,
            opacity: layer.opacity,
        });
    }

    let mut edge_assignments = Vec::new();
    try_reserve_exact_v1(&mut edge_assignments, source.edge_assignments.len())?;
    edge_assignments.extend(source.edge_assignments.iter().copied());
    Ok(ProjectLayerDocumentV1 {
        schema_version: source.schema_version,
        layers,
        edge_assignments,
    })
}

/// Clones every owned profile buffer only after reserving its destination.
///
/// The exhaustive struct literals are intentional: adding a new schema field
/// must fail compilation until its ownership and allocation behavior are
/// classified here.
pub(crate) fn try_clone_beginner_design_profile_v1(
    source: &ori_domain::BeginnerDesignProfileV1,
) -> Result<ori_domain::BeginnerDesignProfileV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerDesignProfileV1 {
        schema_version: source.schema_version,
        preset: source.preset,
        shape_fidelity_weight: source.shape_fidelity_weight,
        foldability_weight: source.foldability_weight,
        step_count_weight: source.step_count_weight,
        paper_efficiency_weight: source.paper_efficiency_weight,
        generation_constraints: try_clone_beginner_generation_constraints_v1(
            &source.generation_constraints,
        )?,
        generation_provenance: source
            .generation_provenance
            .as_ref()
            .map(try_clone_beginner_generation_provenance_v1)
            .transpose()?,
        reference_surface_landmarks_tenths_mm: source
            .reference_surface_landmarks_tenths_mm
            .as_deref()
            .map(try_clone_copy_slice_v1)
            .transpose()?,
        outline_edit_authority: source
            .outline_edit_authority
            .as_ref()
            .map(try_clone_beginner_outline_edit_authority_v1)
            .transpose()?,
        archived_reference_model_asset_ids: try_clone_copy_slice_v1(
            &source.archived_reference_model_asset_ids,
        )?,
        reference_consensus_v1: source
            .reference_consensus_v1
            .as_ref()
            .map(try_clone_beginner_reference_consensus_v1)
            .transpose()?,
    })
}

fn try_clone_beginner_generation_constraints_v1(
    source: &ori_domain::BeginnerGenerationConstraintsV1,
) -> Result<ori_domain::BeginnerGenerationConstraintsV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerGenerationConstraintsV1 {
        schema_version: source.schema_version,
        maximum_steps: source.maximum_steps,
        detail_level: source.detail_level,
        generic_body_size_tenths_mm: source.generic_body_size_tenths_mm,
        generic_body_outline_tenths_mm: source
            .generic_body_outline_tenths_mm
            .as_deref()
            .map(try_clone_copy_slice_v1)
            .transpose()?,
        generic_body_outline_mode: source.generic_body_outline_mode,
        target_category: source.target_category,
        custom_object_display_name: source
            .custom_object_display_name
            .as_deref()
            .map(try_owned_string)
            .transpose()?,
        target_parts: try_clone_copy_slice_v1(&source.target_parts)?,
        skeleton_segments: try_clone_copy_slice_v1(&source.skeleton_segments)?,
        component_bridge_override: source
            .component_bridge_override
            .as_ref()
            .map(try_clone_beginner_component_bridge_override_v1)
            .transpose()?,
        silhouette_thresholds: source.silhouette_thresholds,
        silhouette_crop_roi: source.silhouette_crop_roi,
        silhouette_orientation_degrees: source.silhouette_orientation_degrees,
        silhouette_mirror: source.silhouette_mirror,
        protrusions: try_clone_records_v1(
            &source.protrusions,
            try_clone_beginner_protrusion_target_v1,
        )?,
        bulge_targets: try_clone_records_v1(
            &source.bulge_targets,
            try_clone_beginner_bulge_target_v1,
        )?,
        target_asset: source.target_asset,
        allowed_techniques: try_clone_copy_slice_v1(&source.allowed_techniques)?,
    })
}

fn try_clone_beginner_component_bridge_override_v1(
    source: &ori_domain::BeginnerComponentBridgeOverrideV1,
) -> Result<ori_domain::BeginnerComponentBridgeOverrideV1, SpeculativeUnprovenFoldTokenIssueErrorV1>
{
    Ok(ori_domain::BeginnerComponentBridgeOverrideV1 {
        schema_version: source.schema_version,
        source_asset_sha256: source.source_asset_sha256,
        component_count: source.component_count,
        reviewed: source.reviewed,
        bridges: try_clone_copy_slice_v1(&source.bridges)?,
    })
}

fn try_clone_beginner_protrusion_target_v1(
    source: &ori_domain::BeginnerProtrusionTargetV1,
) -> Result<ori_domain::BeginnerProtrusionTargetV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerProtrusionTargetV1 {
        id: source.id,
        count: source.count,
        length_tenths_mm: source.length_tenths_mm,
        thickness_tenths_mm: source.thickness_tenths_mm,
        root_width_tenths_mm: source.root_width_tenths_mm,
        tip_width_tenths_mm: source.tip_width_tenths_mm,
        local_outline_tenths_mm: source
            .local_outline_tenths_mm
            .as_deref()
            .map(try_clone_copy_slice_v1)
            .transpose()?,
        position_tenths_mm: source.position_tenths_mm,
        direction_milli: source.direction_milli,
        symmetry: source.symmetry,
        curvature_degrees: source.curvature_degrees,
        joint: source.joint,
        motion_degrees: source.motion_degrees,
        side: source.side,
        priority: source.priority,
    })
}

fn try_clone_beginner_bulge_target_v1(
    source: &ori_domain::BeginnerBulgeTargetV1,
) -> Result<ori_domain::BeginnerBulgeTargetV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerBulgeTargetV1 {
        id: source.id,
        face_ids: try_clone_copy_slice_v1(&source.face_ids)?,
        range_min_tenths_mm: source.range_min_tenths_mm,
        range_max_tenths_mm: source.range_max_tenths_mm,
        direction_milli: source.direction_milli,
        amount_tenths_mm: source.amount_tenths_mm,
        source_fold_model_fingerprint: try_owned_string(&source.source_fold_model_fingerprint)?,
        reference_surface_binding: source
            .reference_surface_binding
            .as_ref()
            .map(try_clone_beginner_reference_surface_binding_v1)
            .transpose()?,
    })
}

fn try_clone_beginner_reference_surface_binding_v1(
    source: &ori_domain::BeginnerReferenceSurfaceBindingV1,
) -> Result<ori_domain::BeginnerReferenceSurfaceBindingV1, SpeculativeUnprovenFoldTokenIssueErrorV1>
{
    Ok(ori_domain::BeginnerReferenceSurfaceBindingV1 {
        asset_id: source.asset_id,
        range_id: source.range_id,
        protrusion_id: source.protrusion_id,
        triangle_indices: try_clone_copy_slice_v1(&source.triangle_indices)?,
        range_digest_sha256: source.range_digest_sha256,
    })
}

fn try_clone_beginner_generation_provenance_v1(
    source: &ori_domain::BeginnerGenerationProvenanceV1,
) -> Result<ori_domain::BeginnerGenerationProvenanceV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerGenerationProvenanceV1 {
        schema_version: source.schema_version,
        topology_authority_sha256: source.topology_authority_sha256,
        fold_path_certificate_sha256: source.fold_path_certificate_sha256,
        confidence_score: source.confidence_score,
        confidence_reasons: try_clone_strings_v1(&source.confidence_reasons)?,
        explicit_override: source.explicit_override,
        source_asset_fingerprint: try_owned_string(&source.source_asset_fingerprint)?,
        semantic_landmark_provenance: source
            .semantic_landmark_provenance
            .as_ref()
            .map(try_clone_beginner_semantic_landmark_provenance_v1)
            .transpose()?,
        generic_tree: source
            .generic_tree
            .as_ref()
            .map(try_clone_beginner_generic_tree_provenance_v1)
            .transpose()?,
        reference_consensus: source
            .reference_consensus
            .as_ref()
            .map(try_clone_beginner_reference_consensus_provenance_v1)
            .transpose()?,
        reference_consensus_summary: source
            .reference_consensus_summary
            .as_ref()
            .map(try_clone_beginner_reference_consensus_summary_v1)
            .transpose()?,
    })
}

fn try_clone_beginner_semantic_landmark_provenance_v1(
    source: &ori_domain::BeginnerSemanticLandmarkProvenanceV1,
) -> Result<
    ori_domain::BeginnerSemanticLandmarkProvenanceV1,
    SpeculativeUnprovenFoldTokenIssueErrorV1,
> {
    Ok(ori_domain::BeginnerSemanticLandmarkProvenanceV1 {
        schema_version: source.schema_version,
        ordered_bindings: try_clone_records_v1(&source.ordered_bindings, |binding| {
            Ok(ori_domain::BeginnerSemanticLandmarkBindingV1 {
                ordinal: binding.ordinal,
                role: try_owned_string(&binding.role)?,
                physical_ray: binding.physical_ray,
            })
        })?,
        physical_ray_group_sha256: source.physical_ray_group_sha256,
    })
}

fn try_clone_beginner_generic_tree_provenance_v1(
    source: &ori_domain::BeginnerGenericTreeProvenanceV1,
) -> Result<ori_domain::BeginnerGenericTreeProvenanceV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerGenericTreeProvenanceV1 {
        schema_version: source.schema_version,
        target_category: source.target_category,
        source: source.source,
        asset_content_sha256: source.asset_content_sha256,
        tree_topology_sha256: source.tree_topology_sha256,
        normalized_length_ratios: try_clone_copy_slice_v1(&source.normalized_length_ratios)?,
        orientation: source.orientation,
        generator_version: source.generator_version,
        authorizes_apply: source.authorizes_apply,
        instruction_proposal: source
            .instruction_proposal
            .as_ref()
            .map(try_clone_beginner_generic_tree_instruction_proposal_v1)
            .transpose()?,
    })
}

fn try_clone_beginner_generic_tree_instruction_proposal_v1(
    source: &ori_domain::BeginnerGenericTreeInstructionProposalV1,
) -> Result<
    ori_domain::BeginnerGenericTreeInstructionProposalV1,
    SpeculativeUnprovenFoldTokenIssueErrorV1,
> {
    Ok(ori_domain::BeginnerGenericTreeInstructionProposalV1 {
        schema_version: source.schema_version,
        topology_sha256: source.topology_sha256,
        generator_version: source.generator_version,
        authorizes_apply: source.authorizes_apply,
        physical_motion_proof: source.physical_motion_proof,
        steps: try_clone_records_v1(&source.steps, |step| {
            Ok(ori_domain::BeginnerGenericTreeInstructionStepV1 {
                canonical_crease_id: try_owned_string(&step.canonical_crease_id)?,
                tree_depth: step.tree_depth,
                assignment: try_owned_string(&step.assignment)?,
                target_branch: try_owned_string(&step.target_branch)?,
                fixed_side: try_owned_string(&step.fixed_side)?,
                caution: try_owned_string(&step.caution)?,
            })
        })?,
    })
}

fn try_clone_beginner_reference_consensus_provenance_v1(
    source: &ori_domain::BeginnerReferenceConsensusProvenanceV1,
) -> Result<
    ori_domain::BeginnerReferenceConsensusProvenanceV1,
    SpeculativeUnprovenFoldTokenIssueErrorV1,
> {
    Ok(ori_domain::BeginnerReferenceConsensusProvenanceV1 {
        schema_version: source.schema_version,
        source_revision: source.source_revision,
        bindings: try_clone_records_v1(&source.bindings, try_clone_beginner_reference_binding_v1)?,
        excluded_asset_id: source.excluded_asset_id,
        pair_digests_sha256: try_clone_copy_slice_v1(&source.pair_digests_sha256)?,
        summary: try_clone_beginner_reference_consensus_summary_v1(&source.summary)?,
    })
}

fn try_clone_beginner_reference_consensus_summary_v1(
    source: &ori_domain::BeginnerReferenceConsensusSummaryV1,
) -> Result<ori_domain::BeginnerReferenceConsensusSummaryV1, SpeculativeUnprovenFoldTokenIssueErrorV1>
{
    Ok(ori_domain::BeginnerReferenceConsensusSummaryV1 {
        schema_version: source.schema_version,
        model: try_owned_string(&source.model)?,
        source_count: source.source_count,
        excluded_count: source.excluded_count,
        agreement_score: source.agreement_score,
        component_subscore: source.component_subscore,
        extent_subscore: source.extent_subscore,
        branch_subscore: source.branch_subscore,
    })
}

fn try_clone_beginner_outline_edit_authority_v1(
    source: &ori_domain::BeginnerOutlineEditAuthorityV1,
) -> Result<ori_domain::BeginnerOutlineEditAuthorityV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerOutlineEditAuthorityV1 {
        schema_version: source.schema_version,
        source_asset_id: source.source_asset_id,
        source_sha256: source.source_sha256,
        edits: try_clone_records_v1(&source.edits, |edit| {
            Ok(match edit {
                ori_domain::BeginnerOutlineEditRecordV1::SplitVertical {
                    source_candidate_id,
                    split_x,
                    fragment_kinds,
                } => ori_domain::BeginnerOutlineEditRecordV1::SplitVertical {
                    source_candidate_id: *source_candidate_id,
                    split_x: *split_x,
                    fragment_kinds: *fragment_kinds,
                },
                ori_domain::BeginnerOutlineEditRecordV1::Merge {
                    source_candidate_ids,
                    merged_kind,
                } => ori_domain::BeginnerOutlineEditRecordV1::Merge {
                    source_candidate_ids: *source_candidate_ids,
                    merged_kind: *merged_kind,
                },
            })
        })?,
    })
}

fn try_clone_beginner_reference_consensus_v1(
    source: &ori_domain::BeginnerReferenceConsensusV1,
) -> Result<ori_domain::BeginnerReferenceConsensusV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerReferenceConsensusV1 {
        schema_version: source.schema_version,
        bindings: try_clone_records_v1(&source.bindings, try_clone_beginner_reference_binding_v1)?,
        excluded_asset_id: source.excluded_asset_id,
    })
}

fn try_clone_beginner_reference_binding_v1(
    source: &ori_domain::BeginnerReferenceBindingV1,
) -> Result<ori_domain::BeginnerReferenceBindingV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    Ok(ori_domain::BeginnerReferenceBindingV1 {
        kind: source.kind,
        asset_id: source.asset_id,
        sha256: source.sha256,
        quality: source.quality,
    })
}

fn try_clone_strings_v1(
    source: &[String],
) -> Result<Vec<String>, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    try_clone_records_v1(source, |value| try_owned_string(value))
}

fn try_clone_copy_slice_v1<T: Copy>(
    source: &[T],
) -> Result<Vec<T>, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut cloned = Vec::new();
    try_reserve_exact_v1(&mut cloned, source.len())?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn try_clone_records_v1<T>(
    source: &[T],
    mut clone_record: impl FnMut(&T) -> Result<T, SpeculativeUnprovenFoldTokenIssueErrorV1>,
) -> Result<Vec<T>, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut cloned = Vec::new();
    try_reserve_exact_v1(&mut cloned, source.len())?;
    for record in source {
        cloned.push(clone_record(record)?);
    }
    Ok(cloned)
}

pub(super) fn try_owned_string(
    value: &str,
) -> Result<String, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
    owned.push_str(value);
    Ok(owned)
}

fn try_reserve_exact_v1<T>(
    target: &mut Vec<T>,
    additional: usize,
) -> Result<(), SpeculativeUnprovenFoldTokenIssueErrorV1> {
    target
        .try_reserve_exact(additional)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_beginner_design_profile_is_deep_cloned_exactly() {
        let asset = ori_domain::AssetId::new();
        let second_asset = ori_domain::AssetId::new();
        let face = ori_domain::FaceId::new();
        let mut source = ori_domain::BeginnerDesignProfileV1::default();
        source.generation_constraints.generic_body_outline_tenths_mm =
            Some(vec![[0, 0], [100, 0], [50, 80]]);
        source.generation_constraints.custom_object_display_name =
            Some("折り紙オブジェクト".to_owned());
        source.generation_constraints.target_parts = vec![ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Wing,
            count: 2,
        }];
        source.generation_constraints.skeleton_segments =
            vec![ori_domain::BeginnerSkeletonSegmentV1 {
                id: 1,
                start: ori_domain::BeginnerSkeletonPointV1 {
                    x_tenths_mm: 0,
                    y_tenths_mm: 0,
                },
                end: ori_domain::BeginnerSkeletonPointV1 {
                    x_tenths_mm: 100,
                    y_tenths_mm: 50,
                },
                thickness_tenths_mm: 12,
            }];
        source.generation_constraints.component_bridge_override =
            Some(ori_domain::BeginnerComponentBridgeOverrideV1 {
                schema_version: 1,
                source_asset_sha256: [1; 32],
                component_count: 2,
                reviewed: true,
                bridges: vec![ori_domain::BeginnerComponentBridgeRecordV1 {
                    id: 0,
                    start_component_id: 0,
                    end_component_id: 1,
                    accepted: true,
                }],
            });
        source.generation_constraints.protrusions = vec![ori_domain::BeginnerProtrusionTargetV1 {
            id: 7,
            count: 2,
            length_tenths_mm: 300,
            thickness_tenths_mm: 20,
            root_width_tenths_mm: Some(30),
            tip_width_tenths_mm: Some(10),
            local_outline_tenths_mm: Some(vec![[0, 0], [30, 20], [10, 50]]),
            position_tenths_mm: [1, 2, 3],
            direction_milli: [1_000, 0, 0],
            symmetry: ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
            curvature_degrees: 15,
            joint: ori_domain::BeginnerProtrusionJointV1::Hinge,
            motion_degrees: [-30, 45],
            side: ori_domain::BeginnerProtrusionSideV1::Front,
            priority: 80,
        }];
        source.generation_constraints.bulge_targets = vec![ori_domain::BeginnerBulgeTargetV1 {
            id: 9,
            face_ids: vec![face],
            range_min_tenths_mm: [0, 0, 0],
            range_max_tenths_mm: [100, 100, 30],
            direction_milli: [0, 0, 1_000],
            amount_tenths_mm: 20,
            source_fold_model_fingerprint: "ab".repeat(32),
            reference_surface_binding: Some(ori_domain::BeginnerReferenceSurfaceBindingV1 {
                asset_id: asset,
                range_id: 4,
                protrusion_id: 7,
                triangle_indices: vec![0, 1, 2],
                range_digest_sha256: [2; 32],
            }),
        }];
        source.generation_constraints.allowed_techniques = vec![
            ori_domain::BeginnerFoldTechniqueV1::MountainFold,
            ori_domain::BeginnerFoldTechniqueV1::PetalFold,
        ];
        source.generation_constraints.target_asset =
            Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id: asset });

        let summary = || ori_domain::BeginnerReferenceConsensusSummaryV1 {
            schema_version: 1,
            model: "component_extent_branch_v1".to_owned(),
            source_count: 2,
            excluded_count: 0,
            agreement_score: 90,
            component_subscore: 91,
            extent_subscore: 92,
            branch_subscore: 93,
        };
        let binding = |asset_id| ori_domain::BeginnerReferenceBindingV1 {
            kind: ori_domain::BeginnerReferenceBindingKindV1::ReferenceModel,
            asset_id,
            sha256: [3; 32],
            quality: 95,
        };
        source.generation_provenance = Some(ori_domain::BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256: [4; 32],
            fold_path_certificate_sha256: Some([5; 32]),
            confidence_score: 88,
            confidence_reasons: vec!["形状一致".to_owned(), "bounded topology".to_owned()],
            explicit_override: false,
            source_asset_fingerprint: "asset:rich-profile".to_owned(),
            semantic_landmark_provenance: Some(ori_domain::BeginnerSemanticLandmarkProvenanceV1 {
                schema_version: 1,
                ordered_bindings: vec![ori_domain::BeginnerSemanticLandmarkBindingV1 {
                    ordinal: 0,
                    role: "head".to_owned(),
                    physical_ray: 0,
                }],
                physical_ray_group_sha256: [[6; 32]; 4],
            }),
            generic_tree: Some(ori_domain::BeginnerGenericTreeProvenanceV1 {
                schema_version: 1,
                target_category: Some(ori_domain::BeginnerTargetCategoryV1::CustomObject),
                source: ori_domain::BeginnerGenericTreeSourceV1::ManualSkeleton,
                asset_content_sha256: Some([7; 32]),
                tree_topology_sha256: [8; 32],
                normalized_length_ratios: vec![1_000_000, 1_500_000],
                orientation: ori_domain::BeginnerGenericTreeOrientationV1::Horizontal,
                generator_version: 1,
                authorizes_apply: false,
                instruction_proposal: Some(ori_domain::BeginnerGenericTreeInstructionProposalV1 {
                    schema_version: 1,
                    topology_sha256: [8; 32],
                    generator_version: 1,
                    authorizes_apply: false,
                    physical_motion_proof: false,
                    steps: vec![ori_domain::BeginnerGenericTreeInstructionStepV1 {
                        canonical_crease_id: "crease-0001".to_owned(),
                        tree_depth: 1,
                        assignment: "mountain".to_owned(),
                        target_branch: "left-wing".to_owned(),
                        fixed_side: "root".to_owned(),
                        caution: "ゆっくり折る".to_owned(),
                    }],
                }),
            }),
            reference_consensus: Some(ori_domain::BeginnerReferenceConsensusProvenanceV1 {
                schema_version: 1,
                source_revision: 3,
                bindings: vec![binding(asset), binding(second_asset)],
                excluded_asset_id: None,
                pair_digests_sha256: vec![[9; 32]],
                summary: summary(),
            }),
            reference_consensus_summary: Some(summary()),
        });
        source.reference_surface_landmarks_tenths_mm = Some(vec![[0, 0, 0], [100, 20, 30]]);
        source.outline_edit_authority = Some(ori_domain::BeginnerOutlineEditAuthorityV1 {
            schema_version: 1,
            source_asset_id: asset,
            source_sha256: [10; 32],
            edits: vec![
                ori_domain::BeginnerOutlineEditRecordV1::SplitVertical {
                    source_candidate_id: 1,
                    split_x: 50,
                    fragment_kinds: [
                        ori_domain::BeginnerTargetPartKindV1::Wing,
                        ori_domain::BeginnerTargetPartKindV1::Tail,
                    ],
                },
                ori_domain::BeginnerOutlineEditRecordV1::Merge {
                    source_candidate_ids: [1, 2],
                    merged_kind: ori_domain::BeginnerTargetPartKindV1::Torso,
                },
            ],
        });
        source.archived_reference_model_asset_ids = vec![asset, second_asset];
        source.reference_consensus_v1 = Some(ori_domain::BeginnerReferenceConsensusV1 {
            schema_version: 1,
            bindings: vec![binding(asset), binding(second_asset)],
            excluded_asset_id: Some(second_asset),
        });

        let cloned =
            try_clone_beginner_design_profile_v1(&source).expect("fallible rich profile clone");
        assert_eq!(cloned, source);
        assert_ne!(
            cloned.archived_reference_model_asset_ids.as_ptr(),
            source.archived_reference_model_asset_ids.as_ptr()
        );
        assert_ne!(
            cloned
                .generation_constraints
                .custom_object_display_name
                .as_ref()
                .expect("cloned custom name")
                .as_ptr(),
            source
                .generation_constraints
                .custom_object_display_name
                .as_ref()
                .expect("source custom name")
                .as_ptr()
        );
        assert_ne!(
            cloned
                .generation_provenance
                .as_ref()
                .expect("cloned provenance")
                .confidence_reasons[0]
                .as_ptr(),
            source
                .generation_provenance
                .as_ref()
                .expect("source provenance")
                .confidence_reasons[0]
                .as_ptr()
        );
    }
}
