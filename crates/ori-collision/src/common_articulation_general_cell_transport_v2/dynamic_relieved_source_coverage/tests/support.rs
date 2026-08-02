use std::mem::size_of;

use ori_core::analyze_local_flat_foldability;
use ori_foldability::{
    DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2, GlobalFlatFoldabilityInput,
    GlobalFlatFoldabilityLimits, GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    GlobalFlatLayerOrderCompactPairAssignmentErrorV2,
    GlobalFlatLayerOrderCompactPairAssignmentInputV2,
    GlobalFlatLayerOrderCompactPairAssignmentLimitsV2, GlobalFlatLayerOrderRevalidationLimitsV2,
    GlobalFlatLayerOrderSourceAuthorityV2, LayerOrderSnapshot,
    issue_global_flat_layer_order_from_compact_pair_assignment_v2,
};
use ori_topology::{
    FaceExtractionInput, LocalFlatFoldabilityReport, TopologySnapshot, analyze_faces,
};

use super::super::*;
use crate::CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2;
use crate::dynamic_general_n_positive_thickness_v2::ordinary_interval::tests::{
    relief_public_api_tests::revalidation_input_v2, relief_support::ReliefFixtureInputV2,
    support::OrdinaryFixtureV2,
};

pub(super) struct LiveGlobalInputV2 {
    topology: TopologySnapshot,
    local: LocalFlatFoldabilityReport,
}

impl LiveGlobalInputV2 {
    pub(super) fn for_fixture_v2(fixture: &OrdinaryFixtureV2) -> Self {
        let namespace = fixture
            .fixture
            .geometry
            .source_identity_namespace_v1()
            .expect("golden N33 namespace");
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: fixture
                .fixture
                .geometry
                .source_revision_v1()
                .expect("golden N33 revision"),
            paper: &fixture.fixture.paper,
            pattern: &fixture.fixture.pattern,
        })
        .snapshot
        .expect("golden N33 topology");
        let local =
            analyze_local_flat_foldability(&fixture.fixture.paper, &fixture.fixture.pattern);
        Self { topology, local }
    }

    pub(super) fn input<'a>(
        &'a self,
        fixture: &'a OrdinaryFixtureV2,
    ) -> GlobalFlatFoldabilityInput<'a> {
        GlobalFlatFoldabilityInput::current_with_geometry(
            fixture
                .fixture
                .geometry
                .source_identity_namespace_v1()
                .expect("golden N33 namespace"),
            &fixture.fixture.paper,
            &fixture.fixture.pattern,
            &self.topology,
            &self.local,
        )
    }
}

pub(super) fn try_issue_compact_n33_source_v2(
    fixture: &OrdinaryFixtureV2,
    live: &LiveGlobalInputV2,
    direction_bits: &[u8],
    variable_count: usize,
    registry: [u8; 32],
) -> Result<
    GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    GlobalFlatLayerOrderCompactPairAssignmentErrorV2,
> {
    issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
            source: live.input(fixture),
            variable_count,
            variable_registry_sha256: registry,
            direction_bits_le: direction_bits,
        },
        GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
            analysis: GlobalFlatFoldabilityLimits {
                max_search_nodes: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
            ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
        },
    )
}

pub(super) fn source_revalidation_limits_v2(
    compact: &GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
) -> GlobalFlatLayerOrderRevalidationLimitsV2 {
    GlobalFlatLayerOrderRevalidationLimitsV2 {
        analysis: GlobalFlatFoldabilityLimits {
            max_search_nodes: 0,
            ..GlobalFlatFoldabilityLimits::default()
        },
        max_source_retained_bytes: compact.resources_v2().layer_order_retained_bytes,
        max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
    }
}

pub(super) fn exact_coverage_limits_v2(
    fixture: &OrdinaryFixtureV2,
    clearance: &CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    source: &LayerOrderSnapshot,
) -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
    let source_retained = source
        .checked_deep_retained_bytes_v1()
        .expect("checked N33 source bytes");
    let layout =
        super::super::super::source_binding::scan_source_layout_metrics_v2(source, &mut || Ok(()))
            .expect("checked N33 source layout");
    let source_logical = super::super::super::source_binding::source_traversal_work_v2(
        source,
        layout,
        source_retained,
        &mut || Ok(()),
    )
    .expect("checked N33 source work");
    let layer_records = source
        .overlap_cells
        .iter()
        .map(|cell| cell.bottom_to_top_faces.len())
        .sum();
    let boundary_vertices = source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len())
        .sum();
    let publication =
        size_of::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
    let aggregate = publication
        + source_retained
        + COVERAGE_WORKSPACE_BYTES_V2
        + clearance.aggregate_peak_bytes_upper_bound_v2();
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
        max_blocks: fixture.fixture.profile.configured_max_blocks_v2(),
        max_source_retained_bytes: source_retained,
        max_material_faces: source.material_faces.len(),
        max_folded_faces: source.folded_faces.len(),
        max_overlap_cells: source.overlap_cells.len(),
        max_face_pair_orders: source.face_pair_orders.len(),
        max_global_order_faces: source
            .global_bottom_to_top
            .as_ref()
            .map_or(1, |order| order.len().max(1)),
        max_layer_records: layer_records,
        max_boundary_vertices: boundary_vertices,
        max_source_logical_work: source_logical,
        max_publication_bytes: publication,
        max_aggregate_peak_bytes: aggregate,
    }
}

pub(super) fn replay_input_v2<'a>(
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    source: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
) -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2<'a> {
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2 {
        live: revalidation_input_v2(fixture, policies, public_limits),
        source_authority: source,
        limits,
    }
}

pub(super) fn limit_value_v2(
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    field: usize,
) -> usize {
    [
        limits.max_blocks,
        limits.max_source_retained_bytes,
        limits.max_material_faces,
        limits.max_folded_faces,
        limits.max_overlap_cells,
        limits.max_face_pair_orders,
        limits.max_global_order_faces,
        limits.max_layer_records,
        limits.max_boundary_vertices,
        limits.max_source_logical_work,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ][field]
}

pub(super) fn set_limit_v2(
    mut limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    field: usize,
    value: usize,
) -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
    match field {
        0 => limits.max_blocks = value,
        1 => limits.max_source_retained_bytes = value,
        2 => limits.max_material_faces = value,
        3 => limits.max_folded_faces = value,
        4 => limits.max_overlap_cells = value,
        5 => limits.max_face_pair_orders = value,
        6 => limits.max_global_order_faces = value,
        7 => limits.max_layer_records = value,
        8 => limits.max_boundary_vertices = value,
        9 => limits.max_source_logical_work = value,
        10 => limits.max_publication_bytes = value,
        11 => limits.max_aggregate_peak_bytes = value,
        _ => unreachable!(),
    }
    limits
}
