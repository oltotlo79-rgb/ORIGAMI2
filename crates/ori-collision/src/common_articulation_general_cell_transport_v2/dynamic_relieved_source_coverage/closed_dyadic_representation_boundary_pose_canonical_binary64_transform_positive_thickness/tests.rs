use super::*;

#[test]
fn phase3k_model_id_is_stable_v2() {
    assert_eq!(
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_REPRESENTATION_BOUNDARY_POSE_CANONICAL_BINARY64_TRANSFORM_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2,
        "common_articulation_dynamic_general_n_closed_dyadic_representation_boundary_pose_canonical_binary64_transform_positive_thickness_prerequisite_v2",
    );
}

#[test]
fn every_phase3k_limit_field_participates_in_replay_identity_v2() {
    let retained = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteLimitsV2 {
        max_blocks: 11,
        max_faces: 12,
        max_hinges: 13,
        max_pose_pair_deep_retained_bytes: 14,
        max_canonical_transform_logical_work: 15,
        max_canonical_transform_workspace_bytes: 16,
        max_retained_phase3j_prerequisite_bytes: 17,
        max_retained_transform_realization_evidence_bytes: 18,
        max_publication_bytes: 19,
        max_aggregate_peak_bytes: 20,
    };
    assert!(resources::limits_match_v2(retained, retained));
    for field in 0..10 {
        let mut drifted = retained;
        match field {
            0 => drifted.max_blocks += 1,
            1 => drifted.max_faces += 1,
            2 => drifted.max_hinges += 1,
            3 => drifted.max_pose_pair_deep_retained_bytes += 1,
            4 => drifted.max_canonical_transform_logical_work += 1,
            5 => drifted.max_canonical_transform_workspace_bytes += 1,
            6 => drifted.max_retained_phase3j_prerequisite_bytes += 1,
            7 => drifted.max_retained_transform_realization_evidence_bytes += 1,
            8 => drifted.max_publication_bytes += 1,
            9 => drifted.max_aggregate_peak_bytes += 1,
            _ => unreachable!(),
        }
        assert!(
            !resources::limits_match_v2(retained, drifted),
            "field {field}"
        );
    }
}
