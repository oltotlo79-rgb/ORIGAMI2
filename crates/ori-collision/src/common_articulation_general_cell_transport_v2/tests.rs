use super::test_support::transport_fixture_v2;
use super::*;

#[test]
fn n33_live_input_factory_regenerates_only_authenticated_source_artifacts() {
    let fixture = transport_fixture_v2();
    let live = fixture.n33_live_global_input_v2();
    let input = live.input(&fixture);
    assert_eq!(
        input.identity_namespace,
        fixture
            .clearance_fixture
            .geometry
            .source_identity_namespace_v1()
    );
    assert_eq!(
        input.source_revision,
        fixture
            .clearance_fixture
            .geometry
            .source_revision_v1()
            .expect("canonical N=33 revision")
    );
    assert_eq!(input.local_report_source_revision, input.source_revision);
    assert!(input.paper.is_some());
    assert!(input.crease_pattern.is_some());
}

#[test]
fn n33_source_limit_factory_refuses_the_foreign_small_live_snapshot() {
    let fixture = transport_fixture_v2();
    assert_eq!(
        super::test_support::exact_transport_limits_for_live_n33_source_v2(
            &fixture,
            &fixture.source,
        ),
        Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
    );
}

#[test]
fn n33_rejects_a_live_but_foreign_sealed_source() {
    let fixture = transport_fixture_v2();
    let authority = fixture
        .source_report
        .layer_order_source_authority_v2()
        .expect("small authority");
    let provenance = authority.provenance_v2();
    assert!(authority.is_current_v2());
    // This is the A/B-origin regression: a live, sealed certificate from B
    // must not authenticate the N=33 material graph A.
    assert_ne!(
        fixture
            .clearance_fixture
            .geometry
            .fold_model_fingerprint_v1(),
        provenance
            .source_fingerprint
            .map(|fingerprint| fingerprint.0),
    );
    assert_ne!(
        fixture
            .clearance_fixture
            .geometry
            .source_identity_namespace_v1(),
        provenance.identity_namespace,
    );
    assert!(matches!(
        issue_common_articulation_general_cell_transport_prerequisite_v2(fixture.input()),
        Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
    ));
}

#[test]
fn n33_component_origin_tamper_fails_the_source_provenance_gate() {
    let fixture = transport_fixture_v2();
    let canonical = &fixture.clearance_fixture;
    let namespace = canonical
        .geometry
        .source_identity_namespace_v1()
        .expect("canonical N=33 namespace");
    let provenance = ori_foldability::GlobalFlatFoldabilityProvenance::for_geometry(
        namespace,
        canonical
            .geometry
            .source_revision_v1()
            .expect("canonical N=33 revision"),
        &canonical.paper,
        &canonical.pattern,
    );
    let tampered = canonical.geometry_with_tampered_component_origin_for_test(
        ori_domain::ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x42, 0x5f, 0x4f, 0x52, 0x49,
            0x47, 0x49,
        ]),
    );

    // Only `sheet_origin` changed. The pattern, paper, revision, fold-model
    // fingerprint, and complete FaceId registry are still the A instance.
    assert_eq!(
        tampered.fold_model_fingerprint_v1(),
        canonical.geometry.fold_model_fingerprint_v1()
    );
    assert_eq!(
        tampered.source_revision_v1(),
        canonical.geometry.source_revision_v1()
    );
    assert_eq!(tampered.face_ids(), canonical.geometry.face_ids());
    assert!(super::validation::geometry_matches_source_provenance_v2(
        &canonical.geometry,
        provenance
    ));
    assert!(!super::validation::geometry_matches_source_provenance_v2(
        &tampered, provenance
    ));
}

#[test]
fn resource_equation_rejects_each_positive_one_short_transport_limit() {
    let source = SourceMetricsV2 {
        material_faces: 2,
        folded_faces: 2,
        overlap_cells: 1,
        face_pair_orders: 1,
        global_order_faces: 2,
        layer_records: 2,
        boundary_vertices: 3,
        boundary_layer_products: 6,
        projected_source_bytes: 96,
        charged_source_bytes: 128,
        traversal_work: 256,
    };
    let limits = CommonArticulationGeneralCellTransportLimitsV2 {
        max_blocks: 33,
        max_source_retained_bytes: 128,
        max_material_faces: 2,
        max_folded_faces: 2,
        max_overlap_cells: 1,
        max_face_pair_orders: 1,
        max_global_order_faces: 2,
        max_layer_records: 2,
        max_boundary_vertices: 3,
        max_boundary_samples: 12,
        max_transitions: 2,
        max_logical_work: 512,
        max_retained_bytes: 1_152,
        max_peak_bytes: 2_304,
    };
    let baseline = checked_transport_resource_work_v2(33, source, 64, 0, 1, limits)
        .expect("baseline resource equation");
    for field in [
        "blocks",
        "source",
        "material",
        "folded",
        "cells",
        "pairs",
        "global",
        "layers",
        "boundary",
        "samples",
        "transitions",
    ] {
        let mut one_short = limits;
        match field {
            "blocks" => one_short.max_blocks -= 1,
            "source" => one_short.max_source_retained_bytes -= 1,
            "material" => one_short.max_material_faces -= 1,
            "folded" => one_short.max_folded_faces -= 1,
            "cells" => one_short.max_overlap_cells -= 1,
            "pairs" => one_short.max_face_pair_orders -= 1,
            "global" => one_short.max_global_order_faces -= 1,
            "layers" => one_short.max_layer_records -= 1,
            "boundary" => one_short.max_boundary_vertices -= 1,
            "samples" => one_short.max_boundary_samples -= 1,
            "transitions" => one_short.max_transitions -= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            checked_transport_resource_work_v2(33, source, 64, 0, 1, one_short),
            Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit),
            "{field} one-short must fail closed"
        );
    }
    for (field, cap) in [
        ("logical work", baseline.logical_work),
        ("retained bytes", baseline.retained_bytes),
        ("peak bytes", baseline.peak_bytes),
    ] {
        let mut one_short = limits;
        match field {
            "logical work" => one_short.max_logical_work = cap - 1,
            "retained bytes" => one_short.max_retained_bytes = cap - 1,
            "peak bytes" => one_short.max_peak_bytes = cap - 1,
            _ => unreachable!(),
        }
        assert_eq!(
            checked_transport_resource_work_v2(33, source, 64, 0, 1, one_short),
            Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit),
            "{field} one-short must fail closed"
        );
    }
    assert_eq!(baseline.transitions, 2);
    assert_eq!(baseline.boundary_samples, 12);
    assert!(baseline.logical_work > 0);
}

#[test]
fn peak_charges_the_larger_of_clone_and_clearance_replay_phases() {
    let source = SourceMetricsV2 {
        material_faces: 2,
        folded_faces: 2,
        overlap_cells: 1,
        face_pair_orders: 1,
        global_order_faces: 2,
        layer_records: 2,
        boundary_vertices: 3,
        boundary_layer_products: 6,
        projected_source_bytes: 96,
        charged_source_bytes: 128,
        traversal_work: 256,
    };
    let clone_dominates =
        super::resource::transport_resource_totals_v2(source, 64, 64, 1).expect("clone phase peak");
    let phases_equal = super::resource::transport_resource_totals_v2(source, 64, 128, 1)
        .expect("equal phase peak");
    let clearance_dominates = super::resource::transport_resource_totals_v2(source, 64, 256, 1)
        .expect("clearance phase peak");
    assert_eq!(clone_dominates.peak_bytes, 2_304);
    assert_eq!(phases_equal.peak_bytes, 2_304);
    assert_eq!(clearance_dominates.peak_bytes, 2_432);
    assert_eq!(
        super::resource::transport_resource_totals_v2(source, 64, usize::MAX, 1),
        Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
    );

    let exact_limits = CommonArticulationGeneralCellTransportLimitsV2 {
        max_blocks: 33,
        max_source_retained_bytes: 128,
        max_material_faces: 2,
        max_folded_faces: 2,
        max_overlap_cells: 1,
        max_face_pair_orders: 1,
        max_global_order_faces: 2,
        max_layer_records: 2,
        max_boundary_vertices: 3,
        max_boundary_samples: clearance_dominates.boundary_samples,
        max_transitions: clearance_dominates.transitions,
        max_logical_work: clearance_dominates.logical_work,
        max_retained_bytes: clearance_dominates.retained_bytes,
        max_peak_bytes: clearance_dominates.peak_bytes,
    };
    assert!(checked_transport_resource_work_v2(33, source, 64, 256, 1, exact_limits).is_ok());
    assert_eq!(
        checked_transport_resource_work_v2(
            33,
            source,
            64,
            256,
            1,
            CommonArticulationGeneralCellTransportLimitsV2 {
                max_peak_bytes: clearance_dominates.peak_bytes - 1,
                ..exact_limits
            },
        ),
        Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
    );
}

#[test]
fn retained_source_work_charges_all_six_issue_path_capacity_passes() {
    let fixture = transport_fixture_v2();
    let source = &fixture.source;
    let layout = super::source_binding::scan_source_layout_metrics_v2(source, &mut || Ok(()))
        .expect("source layout metrics");
    let without_retention =
        super::source_binding::source_traversal_work_v2(source, layout, 0, &mut || Ok(()))
            .expect("source traversal baseline");
    let with_one_retained_byte =
        super::source_binding::source_traversal_work_v2(source, layout, 1, &mut || Ok(()))
            .expect("one retained byte traversal");

    assert_eq!(with_one_retained_byte - without_retention, 6);
}

#[test]
fn source_logical_preflight_rejects_one_short_before_structural_membership() {
    let fixture = transport_fixture_v2();
    let source = &fixture.source;
    let layout = super::source_binding::scan_source_layout_metrics_v2(source, &mut || Ok(()))
        .expect("linear source layout");
    let exact_work =
        super::source_binding::source_traversal_work_v2(source, layout, 1, &mut || Ok(()))
            .expect("source traversal work");
    super::source_binding::ensure_source_logical_work_within_limit_v2(exact_work, exact_work)
        .expect("exact source logical cap");
    assert_eq!(
        super::source_binding::ensure_source_logical_work_within_limit_v2(
            exact_work,
            exact_work - 1,
        ),
        Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
    );

    for stop in [
        CommonArticulationGeneralCellTransportStopV2::Cancelled,
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded,
    ] {
        assert_eq!(
            super::source_binding::scan_source_layout_metrics_v2(source, &mut || Err(stop)),
            Err(match stop {
                CommonArticulationGeneralCellTransportStopV2::Cancelled => {
                    CommonArticulationGeneralCellTransportErrorV2::Cancelled
                }
                CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
                    CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded
                }
            })
        );
    }
}

#[test]
fn invalid_or_foreign_source_fails_before_clearance_replay() {
    let fixture = transport_fixture_v2();
    let mut foreign_polls = 0usize;
    assert!(matches!(
        issue_common_articulation_general_cell_transport_prerequisite_with_checkpoint_v2(
            fixture.input(),
            || {
                foreign_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
    ));
    assert_eq!(
        foreign_polls, 2,
        "entry and input gates poll, but foreign provenance never enters clearance"
    );

    let retained_bytes = fixture
        .source
        .checked_deep_retained_bytes_v1()
        .expect("small source retained bytes");
    assert_eq!(
        super::source_binding::validate_source_retained_cap_v2(
            &fixture.source,
            retained_bytes - 1,
            &mut || Ok(()),
        ),
        Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
    );

    let input = fixture.input();
    let negative_zero = CommonArticulationGeneralCellTransportInputV2 {
        closure_tolerance: -0.0,
        ..input
    };
    let mut invalid_polls = 0usize;
    assert!(matches!(
        issue_common_articulation_general_cell_transport_prerequisite_with_checkpoint_v2(
            negative_zero,
            || {
                invalid_polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationGeneralCellTransportErrorV2::InvalidInput)
    ));
    assert_eq!(
        invalid_polls, 2,
        "invalid scalar input reaches neither source nor clearance replay"
    );
}

#[test]
fn cells_accept_layer_permutation_but_reject_duplicate_covering_face() {
    let fixture = transport_fixture_v2();
    let mut permuted = super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    let cell_index = permuted
        .overlap_cells
        .iter()
        .position(|cell| cell.covering_faces.len() >= 2)
        .expect("small source has a multi-face overlap cell");
    let canonical_faces = permuted.overlap_cells[cell_index].covering_faces.clone();
    permuted.overlap_cells[cell_index].bottom_to_top_faces = canonical_faces
        .iter()
        .rev()
        .map(|face| face.face_id)
        .collect();
    assert!(
        super::source_binding::validate_cells_v2(&permuted, &mut || Ok(())).is_ok(),
        "canonical covering order must not be mistaken for solved layer order"
    );

    let mut non_covering = super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    let duplicate_face = non_covering.overlap_cells[cell_index].covering_faces[0];
    non_covering.overlap_cells[cell_index].covering_faces[1] = duplicate_face;
    assert_eq!(
        super::source_binding::validate_cells_v2(&non_covering, &mut || Ok(())),
        Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
    );
}

#[test]
fn pair_orders_require_one_assignment_per_unordered_face_pair() {
    let n33_pair_registry_work = super::source_binding::checked_pair_registry_work_v2(34_980)
        .expect("N=33 pair-registry work fits usize");
    assert_eq!(
        n33_pair_registry_work, 594_660,
        "N=33 pair-registry validation stays O(P log P)"
    );
    assert!(n33_pair_registry_work < 1_223_600_400);
    let fixture = transport_fixture_v2();
    let source = super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    assert!(super::source_binding::validate_pair_orders_v2(&source, &mut || Ok(())).is_ok());
    let first = source
        .face_pair_orders
        .first()
        .expect("small source has a face-pair assignment")
        .clone();

    let mut same_direction = super::test_support::bounded_source_clone_for_test_v2(&source);
    same_direction.face_pair_orders.push(first.clone());
    assert_eq!(
        super::source_binding::validate_pair_orders_v2(&same_direction, &mut || Ok(())),
        Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
    );

    let mut reversed_direction = super::test_support::bounded_source_clone_for_test_v2(&source);
    let mut reversed = first;
    std::mem::swap(&mut reversed.lower_face, &mut reversed.upper_face);
    reversed_direction.face_pair_orders.push(reversed);
    reversed_direction
        .face_pair_orders
        .sort_unstable_by_key(|pair| {
            (
                pair.lower_face.face_key.0,
                pair.upper_face.face_key.0,
                pair.lower_face.face_id.canonical_bytes(),
                pair.upper_face.face_id.canonical_bytes(),
            )
        });
    assert_eq!(
        super::source_binding::validate_pair_orders_v2(&reversed_direction, &mut || Ok(())),
        Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
    );

    let base_layout = super::source_binding::scan_source_layout_metrics_v2(&source, &mut || Ok(()))
        .expect("base source layout");
    let base_work =
        super::source_binding::source_traversal_work_v2(&source, base_layout, 1, &mut || Ok(()))
            .expect("base source work");
    let duplicate_layout =
        super::source_binding::scan_source_layout_metrics_v2(&same_direction, &mut || Ok(()))
            .expect("duplicate source layout");
    let duplicate_work = super::source_binding::source_traversal_work_v2(
        &same_direction,
        duplicate_layout,
        1,
        &mut || Ok(()),
    )
    .expect("duplicate source work");
    assert!(
        duplicate_work > base_work,
        "canonical pair lookup growth is charged"
    );
    assert_eq!(
        super::source_binding::ensure_source_logical_work_within_limit_v2(
            duplicate_work,
            duplicate_work,
        ),
        Ok(()),
        "the exact duplicate-registry work cap is admitted"
    );
    assert_eq!(
        super::source_binding::ensure_source_logical_work_within_limit_v2(
            duplicate_work,
            duplicate_work - 1,
        ),
        Err(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
    );

    let mut polls = 0usize;
    super::source_binding::validate_pair_orders_v2(&source, &mut || {
        polls += 1;
        Ok(())
    })
    .expect("valid pair-order checkpoint traversal");
    assert!(polls > 1);
    for stop in [
        CommonArticulationGeneralCellTransportStopV2::Cancelled,
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded,
    ] {
        let mut observed = 0usize;
        assert_eq!(
            super::source_binding::validate_pair_orders_v2(&source, &mut || {
                observed += 1;
                if observed == polls / 2 {
                    Err(stop)
                } else {
                    Ok(())
                }
            }),
            Err(match stop {
                CommonArticulationGeneralCellTransportStopV2::Cancelled => {
                    CommonArticulationGeneralCellTransportErrorV2::Cancelled
                }
                CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
                    CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded
                }
            })
        );
    }
}

#[test]
fn rational_validation_accepts_canonical_zero_and_rejects_noncanonical_encodings() {
    let rational =
        |sign, numerator_magnitude_be, denominator_be| ori_foldability::ExactRationalValue {
            sign,
            numerator_magnitude_be,
            denominator_be,
        };
    for value in [
        rational(ori_foldability::ExactSign::Zero, vec![0], vec![1]),
        rational(ori_foldability::ExactSign::Positive, vec![1], vec![3]),
        rational(ori_foldability::ExactSign::Negative, vec![0x80, 1], vec![2]),
    ] {
        assert!(super::source_binding::validate_rational_v2(&value, &mut || Ok(())).is_ok());
    }
    for value in [
        rational(ori_foldability::ExactSign::Zero, vec![], vec![1]),
        rational(ori_foldability::ExactSign::Zero, vec![0, 0], vec![1]),
        rational(ori_foldability::ExactSign::Zero, vec![1], vec![1]),
        rational(ori_foldability::ExactSign::Zero, vec![0], vec![2]),
        rational(ori_foldability::ExactSign::Positive, vec![], vec![1]),
        rational(ori_foldability::ExactSign::Negative, vec![0, 1], vec![1]),
        rational(ori_foldability::ExactSign::Positive, vec![1], vec![]),
        rational(ori_foldability::ExactSign::Positive, vec![1], vec![0]),
        rational(ori_foldability::ExactSign::Positive, vec![1], vec![0, 1]),
    ] {
        assert_eq!(
            super::source_binding::validate_rational_v2(&value, &mut || Ok(())),
            Err(CommonArticulationGeneralCellTransportErrorV2::SourceBindingMismatch)
        );
    }
}

#[test]
fn cancellation_and_deadline_hide_the_candidate_before_clearance_replay() {
    let fixture = transport_fixture_v2();
    for stop in [
        CommonArticulationGeneralCellTransportStopV2::Cancelled,
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded,
    ] {
        let error =
            issue_common_articulation_general_cell_transport_prerequisite_with_checkpoint_v2(
                fixture.input(),
                || Err(stop),
            )
            .expect_err("entry stop must hide a candidate");
        assert_eq!(
            error,
            match stop {
                CommonArticulationGeneralCellTransportStopV2::Cancelled => {
                    CommonArticulationGeneralCellTransportErrorV2::Cancelled
                }
                CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
                    CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded
                }
            }
        );
    }
}

#[test]
fn retained_source_equality_is_checkpointable_through_exact_payloads() {
    let fixture = transport_fixture_v2();
    let expected = super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    let mut polls = 0usize;
    super::source_binding::source_equal_with_checkpoint_v2(&expected, &fixture.source, &mut || {
        polls += 1;
        Ok(())
    })
    .expect("equal source");
    assert!(polls > 4);
    for stop in [
        CommonArticulationGeneralCellTransportStopV2::Cancelled,
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded,
    ] {
        let mut observed = 0usize;
        assert_eq!(
            super::source_binding::source_equal_with_checkpoint_v2(
                &expected,
                &fixture.source,
                &mut || {
                    observed += 1;
                    if observed == polls / 2 {
                        Err(stop)
                    } else {
                        Ok(())
                    }
                },
            ),
            Err(match stop {
                CommonArticulationGeneralCellTransportStopV2::Cancelled => {
                    CommonArticulationGeneralCellTransportErrorV2::Cancelled
                }
                CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
                    CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded
                }
            })
        );
    }
}

#[test]
fn historical_search_node_telemetry_does_not_change_v2_source_identity() {
    let fixture = transport_fixture_v2();
    let expected = super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    let mut telemetry_changed =
        super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    let mut proof_summary = telemetry_changed
        .proof_summary
        .expect("sealed global source has a proof summary");
    proof_summary.search_nodes = proof_summary
        .search_nodes
        .checked_add(1)
        .expect("test search-node increment");
    telemetry_changed.proof_summary = Some(proof_summary);

    assert!(
        super::source_binding::source_equal_with_checkpoint_v2(
            &expected,
            &telemetry_changed,
            &mut || Ok(()),
        )
        .expect("search telemetry is deliberately unauthenticated")
    );
}

#[test]
fn spare_source_capacity_preserves_live_authority_and_transport_semantics() {
    let fixture = transport_fixture_v2();
    let baseline = super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    let mut spare_capacity = super::test_support::bounded_source_clone_for_test_v2(&fixture.source);
    let original_capacity = spare_capacity.material_faces.capacity();
    spare_capacity.material_faces.reserve_exact(8);
    assert!(spare_capacity.material_faces.capacity() > original_capacity);

    let baseline_digest = super::source_binding::source_digest_v2(&baseline, &mut || Ok(()))
        .expect("baseline semantic digest");
    let spare_digest = super::source_binding::source_digest_v2(&spare_capacity, &mut || Ok(()))
        .expect("spare-capacity semantic digest");
    let baseline_layout =
        super::source_binding::scan_source_layout_metrics_v2(&baseline, &mut || Ok(()))
            .expect("baseline semantic metrics");
    let spare_layout =
        super::source_binding::scan_source_layout_metrics_v2(&spare_capacity, &mut || Ok(()))
            .expect("spare-capacity semantic metrics");
    assert_eq!(baseline_digest, spare_digest);
    assert_eq!(baseline_layout, spare_layout);
    assert!(
        super::source_binding::source_equal_with_checkpoint_v2(
            &baseline,
            &spare_capacity,
            &mut || Ok(()),
        )
        .expect("allocator capacity is not source identity")
    );

    let spare_retained_bytes = spare_capacity
        .checked_deep_retained_bytes_v1()
        .expect("spare source retained capacity");
    let live = super::test_support::small_live_global_input_v2();
    let authority = ori_foldability::revalidate_global_flat_layer_order_source_v2(
        live.input(),
        &spare_capacity,
        ori_foldability::GlobalFlatLayerOrderRevalidationLimitsV2 {
            analysis: ori_foldability::GlobalFlatFoldabilityLimits::default(),
            max_source_retained_bytes: spare_retained_bytes,
            max_peak_bytes: 1_000_000,
        },
    )
    .expect("same semantic source replays despite spare capacity");
    assert!(authority.is_current_v2());
}
