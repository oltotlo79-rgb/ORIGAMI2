use super::*;

#[test]
fn n33_exact_profile_issues_and_revalidates_a_non_authorizing_pose() {
    let fixture = miura_fixture_v2(33);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N33 V2 pose authority");

    assert_eq!(authority.configured_max_blocks_v2(), 33);
    assert_eq!(authority.actual_block_count_v2(), 33);
    assert_eq!(authority.logical_work_v2(), 18_768);
    assert_eq!(authority.retained_bytes_upper_bound_v2(), 58_048);
    assert_eq!(
        fixture.decomposition.model_id_v2(),
        "common_articulation_edge_block_decomposition_v2"
    );
    assert_eq!(fixture.decomposition.logical_work_v2(), 30_720);
    assert_eq!(
        fixture.decomposition.storage_bytes_upper_bound_v2(),
        1_504_256
    );
    assert_eq!(
        authority.profile_binding_fingerprint_v2(),
        profile.binding_fingerprint_v2()
    );
    assert!(!authority.authorizes_continuous_motion());
    assert!(!authority.authorizes_collision_clearance());
    assert!(!authority.authorizes_project_mutation());
    assert!(!authority.authorizes_apply());
    assert!(!authority.authorizes_viewer());
    assert!(!authority.authorizes_layer_transport());
    let first_block = authority.block_v2(0).expect("first V2 restriction");
    assert!(first_block.is_for_geometry_v2(fixture.decomposition.blocks()[0].geometry()));
    assert!(!first_block.is_for_geometry_v2(&fixture.geometry));
    authority
        .revalidate_v2(fixture.input(&profile))
        .expect("same N33 V2 pose input");
}

#[test]
fn n34_profile_decomposition_and_pose_revalidate_with_independent_bounds() {
    let fixture = miura_fixture_v2(34);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(34).expect("N34 profile");
    let resources = profile.actual_v2();
    // Independent N=34 evaluations of the published canonical-Miura
    // formulae: F=8N+1, H=12N, decomposition work/storage, and
    // pose work/storage.  Do not derive these expected values through
    // another production getter.
    assert_eq!(resources.block_count_v2(), 34);
    assert_eq!(resources.face_count_v2(), 273);
    assert_eq!(resources.hinge_count_v2(), 408);
    assert_eq!(resources.decomposition_logical_work_v2(), 31_648);
    assert_eq!(resources.decomposition_storage_bytes_v2(), 1_549_312);
    assert_eq!(resources.pose_logical_work_v2(), 19_472);
    assert_eq!(resources.pose_retained_bytes_v2(), 59_792);
    assert_eq!(fixture.geometry.face_ids().len(), 273);
    assert_eq!(fixture.geometry.hinges().len(), 408);
    assert_eq!(fixture.decomposition.actual_block_count_v2(), 34);
    assert_eq!(fixture.decomposition.face_count_v2(), 273);
    assert_eq!(fixture.decomposition.hinge_count_v2(), 408);
    assert_eq!(fixture.decomposition.logical_work_v2(), 31_648);
    assert_eq!(
        fixture.decomposition.storage_bytes_upper_bound_v2(),
        1_549_312
    );

    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N34 V2 pose authority");
    assert_eq!(authority.configured_max_blocks_v2(), 34);
    assert_eq!(authority.actual_block_count_v2(), 34);
    assert_eq!(authority.logical_work_v2(), 19_472);
    assert_eq!(authority.retained_bytes_upper_bound_v2(), 59_792);
    authority
        .revalidate_v2(fixture.input(&profile))
        .expect("same N34 V2 pose input");
}

#[test]
fn n40_cap_n34_actual_profile_isolated_from_exact_n34_authorities() {
    let fixture = miura_fixture_v2(34);
    let exact = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(34)
        .expect("exact N34 profile");
    let cross_cap = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(40, 34)
        .expect("N40 cap, N34 actual profile");
    let actual = cross_cap.actual_v2();
    let maximum = cross_cap.maximum_v2();

    // Independently fixed cardinalities for this cross-cap case.  The
    // maximum is N=40 while every issued value is still constrained to the
    // N=34 source graph.
    assert_eq!(cross_cap.configured_max_blocks_v2(), 40);
    assert_eq!(cross_cap.actual_block_count_v2(), 34);
    assert_eq!(actual.block_count_v2(), 34);
    assert_eq!(actual.face_count_v2(), 273);
    assert_eq!(actual.hinge_count_v2(), 408);
    assert_eq!(maximum.block_count_v2(), 40);
    assert_eq!(maximum.face_count_v2(), 321);
    assert_eq!(maximum.hinge_count_v2(), 480);
    assert_eq!(fixture.geometry.face_ids().len(), 273);
    assert_eq!(fixture.geometry.hinges().len(), 408);
    assert!(actual.unordered_face_pair_count_v2() <= maximum.unordered_face_pair_count_v2());
    assert!(
        actual.raw_cross_block_pair_candidates_v2() <= maximum.raw_cross_block_pair_candidates_v2()
    );
    assert!(actual.canonical_cross_block_pairs_v2() <= maximum.canonical_cross_block_pairs_v2());
    assert!(
        actual.raw_sort_comparisons_per_item_v2() <= maximum.raw_sort_comparisons_per_item_v2()
    );
    assert!(
        actual.canonical_sort_comparisons_per_item_v2()
            <= maximum.canonical_sort_comparisons_per_item_v2()
    );
    assert!(actual.pose_logical_work_v2() <= maximum.pose_logical_work_v2());
    assert!(actual.pose_retained_bytes_v2() <= maximum.pose_retained_bytes_v2());
    assert!(actual.decomposition_logical_work_v2() <= maximum.decomposition_logical_work_v2());
    assert!(actual.decomposition_storage_bytes_v2() <= maximum.decomposition_storage_bytes_v2());
    assert!(actual.clearance_logical_work_v2() <= maximum.clearance_logical_work_v2());
    assert!(actual.clearance_storage_bytes_v2() <= maximum.clearance_storage_bytes_v2());

    let exact_authority = prove_common_articulation_pose_authority_v2(fixture.input(&exact))
        .expect("exact N34 authority");
    let cross_decomposition = fixture.decomposition_with_profile(&cross_cap);
    let cross_input = CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &fixture.pose,
        decomposition: &cross_decomposition,
        paper_thickness_mm: 0.1,
        profile: &cross_cap,
    };
    let cross_authority =
        prove_common_articulation_pose_authority_v2(cross_input).expect("N40-cap N34 authority");
    assert_eq!(cross_decomposition.limits().max_blocks, 40);
    assert_eq!(cross_decomposition.actual_block_count_v2(), 34);
    assert_eq!(cross_decomposition.face_count_v2(), 273);
    assert_eq!(cross_decomposition.hinge_count_v2(), 408);
    assert!(!cross_decomposition.is_for_profile_v2(&exact));
    assert!(!fixture.decomposition.is_for_profile_v2(&cross_cap));
    cross_authority
        .revalidate_v2(CommonArticulationPoseInputV2 {
            geometry: &fixture.geometry,
            pose: &fixture.pose,
            decomposition: &cross_decomposition,
            paper_thickness_mm: 0.1,
            profile: &cross_cap,
        })
        .expect("same cross-cap input");

    assert_eq!(
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            geometry: &fixture.geometry,
            pose: &fixture.pose,
            decomposition: &cross_decomposition,
            paper_thickness_mm: 0.1,
            profile: &exact,
        })
        .expect_err("cross-cap decomposition under exact profile"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    assert_eq!(
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            geometry: &fixture.geometry,
            pose: &fixture.pose,
            decomposition: &fixture.decomposition,
            paper_thickness_mm: 0.1,
            profile: &cross_cap,
        })
        .expect_err("exact decomposition under cross-cap profile"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    assert_eq!(
        exact_authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                geometry: &fixture.geometry,
                pose: &fixture.pose,
                decomposition: &cross_decomposition,
                paper_thickness_mm: 0.1,
                profile: &cross_cap,
            })
            .expect_err("cross-cap input must not revalidate exact authority"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );
    assert_eq!(
        cross_authority
            .revalidate_v2(fixture.input(&exact))
            .expect_err("exact input must not revalidate cross-cap authority"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );
}

#[test]
fn n33_one_short_and_cross_cap_profiles_are_rejected_or_bound() {
    let fixture = miura_fixture_v2(33);
    let exact =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let one_short = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(33, 32)
        .expect("one-short actual N profile");
    assert_eq!(
        fixture
            .geometry
            .decompose_canonical_edge_blocks_with_profile_v2(&fixture.audit, &one_short)
            .expect_err("one-short decomposition profile"),
        crate::CommonArticulationDecompositionErrorV2::ResourceLimit,
    );
    assert_eq!(
        prove_common_articulation_pose_authority_v2(fixture.input(&one_short))
            .expect_err("one-short profile"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );

    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&exact))
        .expect("exact N33 authority");
    let cross_cap = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, 33)
        .expect("N34 configured N33 actual profile");
    assert_eq!(
        prove_common_articulation_pose_authority_v2(fixture.input(&cross_cap))
            .expect_err("decomposition profile binding differs across configured caps"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    let cross_cap_decomposition = fixture.decomposition_with_profile(&cross_cap);
    assert_eq!(
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            decomposition: &cross_cap_decomposition,
            ..fixture.input(&exact)
        })
        .expect_err("configured-N34 decomposition cannot impersonate exact-N33 binding"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    assert!(authority.revalidate_v2(fixture.input(&exact)).is_ok());
}

#[test]
fn equal_total_face_counts_cannot_impersonate_canonical_miura_blocks() {
    assert_eq!(
        validate_canonical_miura_block_shape_v2(10, 12),
        Err(CommonArticulationPoseErrorV2::ResourceLimit),
    );
    assert_eq!(
        validate_canonical_miura_block_shape_v2(8, 12),
        Err(CommonArticulationPoseErrorV2::ResourceLimit),
    );
    assert_eq!(
        validate_canonical_miura_block_shape_v2(9, 11),
        Err(CommonArticulationPoseErrorV2::ResourceLimit),
    );
    assert!(validate_canonical_miura_block_shape_v2(9, 12).is_ok());
}

#[test]
fn revalidation_rejects_foreign_geometry_pose_and_decomposition() {
    let fixture = miura_fixture_v2(33);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N33 authority");
    assert!(
        authority
            .matches_live_input_with_checkpoint_v2(fixture.input(&profile), || Ok::<(), ()>(()))
            .unwrap()
    );

    let foreign = miura_fixture_v2(33);
    assert!(
        !authority
            .matches_live_input_with_checkpoint_v2(
                CommonArticulationPoseInputV2 {
                    geometry: &foreign.geometry,
                    ..fixture.input(&profile)
                },
                || Ok::<(), ()>(()),
            )
            .unwrap()
    );
    assert_eq!(
        authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                geometry: &foreign.geometry,
                ..fixture.input(&profile)
            })
            .expect_err("foreign geometry"),
        CommonArticulationPoseErrorV2::PoseIssuerMismatch,
    );

    let foreign_pose = fixture.new_pose_instance();
    assert!(
        !authority
            .matches_live_input_with_checkpoint_v2(
                CommonArticulationPoseInputV2 {
                    pose: &foreign_pose,
                    ..fixture.input(&profile)
                },
                || Ok::<(), ()>(()),
            )
            .unwrap()
    );
    assert_eq!(
        authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                pose: &foreign_pose,
                ..fixture.input(&profile)
            })
            .expect_err("foreign pose instance"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );

    let foreign_profile = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, 33)
        .expect("N34 configured N33 actual profile");
    let foreign_decomposition = fixture.decomposition_with_profile(&foreign_profile);
    assert!(
        !authority
            .matches_live_input_with_checkpoint_v2(
                CommonArticulationPoseInputV2 {
                    decomposition: &foreign_decomposition,
                    profile: &foreign_profile,
                    ..fixture.input(&profile)
                },
                || Ok::<(), ()>(()),
            )
            .unwrap()
    );
    assert_eq!(
        authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                decomposition: &foreign_decomposition,
                profile: &foreign_profile,
                ..fixture.input(&profile)
            })
            .expect_err("foreign decomposition binding"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );
}

#[test]
fn issuance_and_revalidation_honor_cancel_and_deadline() {
    let fixture = miura_fixture_v2(33);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    assert_eq!(
        prove_common_articulation_pose_authority_with_checkpoint_v2(
            fixture.input(&profile),
            || { Err(CommonArticulationPoseStopV2::Cancelled) }
        )
        .expect_err("cancelled issuance"),
        CommonArticulationPoseErrorV2::Cancelled,
    );
    assert_eq!(
        prove_common_articulation_pose_authority_with_checkpoint_v2(
            fixture.input(&profile),
            || { Err(CommonArticulationPoseStopV2::DeadlineExceeded) }
        )
        .expect_err("deadline issuance"),
        CommonArticulationPoseErrorV2::DeadlineExceeded,
    );
    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N33 authority");
    let mut live_match_polls = 0usize;
    assert_eq!(
        authority.matches_live_input_with_checkpoint_v2(fixture.input(&profile), || {
            live_match_polls += 1;
            if live_match_polls == 3 {
                Err("live-match stop")
            } else {
                Ok(())
            }
        }),
        Err("live-match stop")
    );
    assert_eq!(live_match_polls, 3);
    assert_eq!(
        authority
            .revalidate_with_checkpoint_v2(fixture.input(&profile), || {
                Err(CommonArticulationPoseStopV2::Cancelled)
            })
            .expect_err("cancelled revalidation"),
        CommonArticulationPoseErrorV2::Cancelled,
    );
}
