use super::*;

#[test]
fn resource_equation_accepts_exact_and_rejects_every_one_short_cap() {
    let source = SourceMetricsV2 {
        material_faces: 11,
        folded_faces: 12,
        overlap_cells: 13,
        face_pair_orders: 14,
        global_order_faces: 15,
        layer_records: 16,
        boundary_vertices: 17,
        boundary_layer_products: 18,
        projected_source_bytes: 96,
        charged_source_bytes: 128,
        traversal_work: 19,
    };
    let publication =
        size_of::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
    let clearance_peak = 2_048;
    let aggregate = publication + 128 + COVERAGE_WORKSPACE_BYTES_V2 + clearance_peak;
    let exact = CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
        max_blocks: 33,
        max_source_retained_bytes: 128,
        max_material_faces: 11,
        max_folded_faces: 12,
        max_overlap_cells: 13,
        max_face_pair_orders: 14,
        max_global_order_faces: 15,
        max_layer_records: 16,
        max_boundary_vertices: 17,
        max_source_logical_work: 19,
        max_publication_bytes: publication,
        max_aggregate_peak_bytes: aggregate,
    };
    let resources = checked_coverage_resources_v2(33, clearance_peak, source, exact)
        .expect("exact resource envelope");
    assert_eq!(resources.aggregate_peak_bytes, aggregate);

    let mut candidate_bound = exact;
    candidate_bound.max_source_retained_bytes = 256;
    candidate_bound.max_aggregate_peak_bytes =
        publication + 256 + COVERAGE_WORKSPACE_BYTES_V2 + clearance_peak;
    let resources = checked_coverage_resources_v2(33, clearance_peak, source, candidate_bound)
        .expect("candidate replay peak uses the policy cap");
    assert_eq!(resources.source_retained_bytes, 128);
    assert_eq!(
        resources.aggregate_peak_bytes,
        candidate_bound.max_aggregate_peak_bytes
    );

    for field in 0..12 {
        let mut one_short = exact;
        let cap = match field {
            0 => &mut one_short.max_blocks,
            1 => &mut one_short.max_source_retained_bytes,
            2 => &mut one_short.max_material_faces,
            3 => &mut one_short.max_folded_faces,
            4 => &mut one_short.max_overlap_cells,
            5 => &mut one_short.max_face_pair_orders,
            6 => &mut one_short.max_global_order_faces,
            7 => &mut one_short.max_layer_records,
            8 => &mut one_short.max_boundary_vertices,
            9 => &mut one_short.max_source_logical_work,
            10 => &mut one_short.max_publication_bytes,
            11 => &mut one_short.max_aggregate_peak_bytes,
            _ => unreachable!(),
        };
        *cap -= 1;
        assert_eq!(
            checked_coverage_resources_v2(33, clearance_peak, source, one_short),
            Err(CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::ResourceLimit),
            "cap {field} one-short"
        );
    }
}

#[test]
fn coverage_stop_mapping_is_exact() {
    for (stop, expected) in [
        (
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::Cancelled,
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Cancelled,
        ),
        (
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2::DeadlineExceeded,
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(checkpoint_v2(&mut || Err(stop)), Err(expected));
    }
}

#[test]
fn phase3g_preserves_phase3f_certificate_binding_mismatch() {
    let mismatch =
        CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::CertificateBindingMismatch;
    assert_eq!(
        map_clearance_error_v2(mismatch),
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::Clearance(mismatch)
    );
}
