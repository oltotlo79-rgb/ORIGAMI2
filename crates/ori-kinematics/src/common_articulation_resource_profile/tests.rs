use super::*;

#[test]
fn canonical_miura_n32_conforms_to_the_existing_extension_envelope() {
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(32).expect("N32 profile");
    let resources = profile.actual_v2();

    assert_eq!(profile.configured_max_blocks_v2(), 32);
    assert_eq!(profile.actual_block_count_v2(), 32);
    assert_eq!(resources.face_count_v2(), 257);
    assert_eq!(resources.hinge_count_v2(), 384);
    assert_eq!(resources.unordered_face_pair_count_v2(), 32_896);
    assert_eq!(resources.raw_cross_block_pair_candidates_v2(), 40_176);
    assert_eq!(resources.canonical_cross_block_pairs_v2(), 31_744);
    assert_eq!(resources.raw_sort_comparisons_per_item_v2(), 128);
    assert_eq!(resources.canonical_sort_comparisons_per_item_v2(), 120);
    assert_eq!(resources.pose_logical_work_v2(), 18_072);
    assert_eq!(resources.pose_retained_bytes_v2(), 56_304);
    assert_eq!(resources.decomposition_logical_work_v2(), 29_792);
    assert_eq!(resources.decomposition_storage_bytes_v2(), 1_459_200);
    assert_eq!(resources.clearance_logical_work_v2(), 9_154_601);
    assert_eq!(resources.clearance_storage_bytes_v2(), 2_347_648);
}

#[test]
fn canonical_miura_n33_has_the_exact_general_n_resource_envelope() {
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let resources = profile.actual_v2();

    assert_eq!(resources.block_count_v2(), 33);
    assert_eq!(resources.face_count_v2(), 265);
    assert_eq!(resources.hinge_count_v2(), 396);
    assert_eq!(resources.unordered_face_pair_count_v2(), 34_980);
    assert_eq!(resources.raw_cross_block_pair_candidates_v2(), 42_768);
    assert_eq!(resources.canonical_cross_block_pairs_v2(), 33_792);
    assert_eq!(resources.raw_sort_comparisons_per_item_v2(), 128);
    assert_eq!(resources.canonical_sort_comparisons_per_item_v2(), 128);
    assert_eq!(resources.pose_logical_work_v2(), 18_768);
    assert_eq!(resources.pose_retained_bytes_v2(), 58_048);
    assert_eq!(resources.decomposition_logical_work_v2(), 30_720);
    assert_eq!(resources.decomposition_storage_bytes_v2(), 1_504_256);
    assert_eq!(resources.clearance_logical_work_v2(), 10_015_062);
    assert_eq!(resources.clearance_storage_bytes_v2(), 2_497_536);
}

#[test]
fn general_n_profiles_match_the_closed_form_and_are_monotone_through_n128() {
    let mut previous: Option<CommonArticulationCanonicalMiuraResourcesV2> = None;
    for block_count in 33..=128 {
        let profile =
            CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(block_count)
                .expect("N33..=N128 profile arithmetic");
        let resources = profile.actual_v2();
        let expected_face_count = block_count
            .checked_mul(8)
            .and_then(|value| value.checked_add(1))
            .expect("bounded face formula");
        let expected_hinge_count = block_count.checked_mul(12).expect("bounded hinge formula");
        let expected_raw_pairs = checked_unordered_pair_count_v2(block_count)
            .expect("bounded block-pair formula")
            .checked_mul(81)
            .expect("bounded raw-pair formula");
        let expected_canonical_pairs = block_count
            .checked_mul(block_count.checked_sub(1).expect("positive N"))
            .and_then(|value| value.checked_mul(32))
            .expect("bounded canonical-pair formula");

        assert_eq!(resources.block_count_v2(), block_count);
        assert_eq!(resources.face_count_v2(), expected_face_count);
        assert_eq!(resources.hinge_count_v2(), expected_hinge_count);
        assert_eq!(
            resources.unordered_face_pair_count_v2(),
            checked_unordered_pair_count_v2(expected_face_count)
                .expect("bounded unordered-face-pair formula")
        );
        assert_eq!(
            resources.raw_cross_block_pair_candidates_v2(),
            expected_raw_pairs
        );
        assert_eq!(
            resources.canonical_cross_block_pairs_v2(),
            expected_canonical_pairs
        );
        assert_eq!(profile.maximum_v2(), resources);

        if let Some(previous) = previous {
            for (name, current, prior) in [
                ("block_count", resources.block_count, previous.block_count),
                ("face_count", resources.face_count, previous.face_count),
                ("hinge_count", resources.hinge_count, previous.hinge_count),
                (
                    "unordered_face_pair_count",
                    resources.unordered_face_pair_count,
                    previous.unordered_face_pair_count,
                ),
                (
                    "raw_cross_block_pair_candidates",
                    resources.raw_cross_block_pair_candidates,
                    previous.raw_cross_block_pair_candidates,
                ),
                (
                    "canonical_cross_block_pairs",
                    resources.canonical_cross_block_pairs,
                    previous.canonical_cross_block_pairs,
                ),
                (
                    "raw_sort_comparisons_per_item",
                    resources.raw_sort_comparisons_per_item,
                    previous.raw_sort_comparisons_per_item,
                ),
                (
                    "canonical_sort_comparisons_per_item",
                    resources.canonical_sort_comparisons_per_item,
                    previous.canonical_sort_comparisons_per_item,
                ),
                (
                    "pose_logical_work",
                    resources.pose_logical_work,
                    previous.pose_logical_work,
                ),
                (
                    "pose_retained_bytes",
                    resources.pose_retained_bytes,
                    previous.pose_retained_bytes,
                ),
                (
                    "decomposition_logical_work",
                    resources.decomposition_logical_work,
                    previous.decomposition_logical_work,
                ),
                (
                    "decomposition_storage_bytes",
                    resources.decomposition_storage_bytes,
                    previous.decomposition_storage_bytes,
                ),
                (
                    "clearance_logical_work",
                    resources.clearance_logical_work,
                    previous.clearance_logical_work,
                ),
                (
                    "clearance_storage_bytes",
                    resources.clearance_storage_bytes,
                    previous.clearance_storage_bytes,
                ),
            ] {
                assert!(current >= prior, "{name} regressed at N={block_count}");
            }
        }
        previous = Some(resources);
    }
}

#[test]
fn every_n33_resource_limit_rejects_its_one_short_boundary() {
    let exact = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33)
        .expect("N33 profile")
        .actual_v2();
    let one_short_limits = [
        CommonArticulationCanonicalMiuraResourcesV2 {
            block_count: exact.block_count - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            face_count: exact.face_count - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            hinge_count: exact.hinge_count - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            unordered_face_pair_count: exact.unordered_face_pair_count - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            raw_cross_block_pair_candidates: exact.raw_cross_block_pair_candidates - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            canonical_cross_block_pairs: exact.canonical_cross_block_pairs - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            raw_sort_comparisons_per_item: exact.raw_sort_comparisons_per_item - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            canonical_sort_comparisons_per_item: exact.canonical_sort_comparisons_per_item - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            pose_logical_work: exact.pose_logical_work - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            pose_retained_bytes: exact.pose_retained_bytes - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            decomposition_logical_work: exact.decomposition_logical_work - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            decomposition_storage_bytes: exact.decomposition_storage_bytes - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            clearance_logical_work: exact.clearance_logical_work - 1,
            ..exact
        },
        CommonArticulationCanonicalMiuraResourcesV2 {
            clearance_storage_bytes: exact.clearance_storage_bytes - 1,
            ..exact
        },
    ];

    for limits in one_short_limits {
        assert_eq!(
            validate_envelope_admission_v2(limits, exact),
            Err(CommonArticulationResourceProfileErrorV2::ResourceLimit),
        );
    }
}

#[test]
fn profile_binding_is_stable_and_binds_both_configured_and_actual_n() {
    let n33 =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let n33_again = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33)
        .expect("same N33 profile");
    let configured_33_actual_32 =
        CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(33, 32)
            .expect("configured N33, actual N32 profile");

    assert_eq!(
        n33.model_id_v2(),
        COMMON_ARTICULATION_RESOURCE_PROFILE_MODEL_ID_V2
    );
    assert_eq!(
        n33.binding_fingerprint_v2(),
        n33_again.binding_fingerprint_v2()
    );
    assert_eq!(
        n33.binding_fingerprint_v2(),
        [
            79, 174, 126, 238, 203, 235, 34, 222, 138, 72, 157, 197, 3, 110, 84, 149, 105, 191, 25,
            40, 156, 126, 142, 34, 137, 241, 145, 119, 75, 244, 79, 227,
        ],
    );
    assert_ne!(
        n33.binding_fingerprint_v2(),
        configured_33_actual_32.binding_fingerprint_v2(),
    );
    assert_eq!(configured_33_actual_32.maximum_v2().face_count_v2(), 265);
    assert_eq!(configured_33_actual_32.actual_v2().face_count_v2(), 257);
}

#[test]
fn invalid_and_overflowing_block_counts_fail_closed() {
    assert_eq!(
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(0),
        Err(CommonArticulationResourceProfileErrorV2::InvalidInput),
    );
    assert_eq!(
        CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(32, 33),
        Err(CommonArticulationResourceProfileErrorV2::InvalidInput),
    );
    for block_count in [usize::MAX, usize::MAX / 8, usize::MAX / 12] {
        let outcome = std::panic::catch_unwind(|| {
            CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(block_count)
        });
        assert!(
            matches!(
                outcome,
                Ok(Err(CommonArticulationResourceProfileErrorV2::ResourceLimit))
            ),
            "N={block_count} must return ResourceLimit without panicking",
        );
    }
}
