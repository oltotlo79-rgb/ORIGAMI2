//! N=33 prerequisite plus a small, live sealed foldability-source fixture.

use ori_core::{analyze_global_flat_foldability, analyze_local_flat_foldability};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityReport,
    LayerOrderSnapshot,
};
use ori_topology::{
    FaceExtractionInput, LocalFlatFoldabilityReport, TopologySnapshot, analyze_faces,
};

use crate::{
    CommonArticulationClearanceOutcomeV2, issue_common_articulation_clearance_prerequisite_v2,
};

use super::*;
use crate::common_articulation_clearance_v2::test_support::{
    MiuraFixtureV2, golden_n33_miura_fixture_v2, miura_fixture_v2,
};

pub(super) struct TransportFixtureV2 {
    pub(super) clearance_fixture: MiuraFixtureV2,
    pub(super) clearance: CommonArticulationClearancePrerequisiteV2,
    pub(super) source_report: GlobalFlatFoldabilityReport,
    pub(super) source: LayerOrderSnapshot,
    pub(super) limits: CommonArticulationGeneralCellTransportLimitsV2,
}

/// Test-only source copy through the same retained-byte cap required by the
/// transport issuer. This keeps equality and telemetry-mutation regressions
/// from introducing an unbounded deep clone outside the production path.
pub(super) fn bounded_source_clone_for_test_v2(source: &LayerOrderSnapshot) -> LayerOrderSnapshot {
    let retained_bytes = source
        .checked_deep_retained_bytes_v1()
        .expect("test source retained bytes");
    source
        .try_clone_with_retained_byte_limit_v1(retained_bytes)
        .expect("bounded test source clone")
}

impl TransportFixtureV2 {
    pub(super) fn input(&self) -> CommonArticulationGeneralCellTransportInputV2<'_> {
        CommonArticulationGeneralCellTransportInputV2 {
            geometry: &self.clearance_fixture.geometry,
            audit: &self.clearance_fixture.audit,
            pose: &self.clearance_fixture.pose,
            decomposition: &self.clearance_fixture.decomposition,
            common_pose: &self.clearance_fixture.common_pose,
            parent_fixed_face: self.clearance_fixture.parent_fixed_face,
            parent_schedule: &self.clearance_fixture.parent_schedule,
            profile: &self.clearance_fixture.profile,
            paper_thickness_mm: 0.1,
            closure_tolerance: self.clearance_fixture.closure_tolerance,
            block_closure_set: &self.clearance_fixture.block_closure_set,
            whole_parent_closure: &self.clearance_fixture.whole_parent_closure,
            whole_parent_closure_limits: self.clearance_fixture.whole_parent_closure_limits,
            clearance: &self.clearance,
            source_authority: self
                .source_report
                .layer_order_source_authority_v2()
                .expect("live sealed source"),
            limits: self.limits,
        }
    }

    /// Forms a normal issue input only from an authority that was live-
    /// revalidated or solver-issued for this exact N=33 source. Its resource
    /// caps are measured internally from that source; callers cannot inject
    /// permissive limits.
    pub(super) fn input_from_n33_live_authority_v2<'a>(
        &'a self,
        source_authority: ori_foldability::GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    ) -> Result<
        CommonArticulationGeneralCellTransportInputV2<'a>,
        CommonArticulationGeneralCellTransportErrorV2,
    > {
        let limits = exact_transport_limits_for_live_n33_source_v2(
            self,
            source_authority.layer_order_snapshot_v2(),
        )?;
        Ok(CommonArticulationGeneralCellTransportInputV2 {
            geometry: &self.clearance_fixture.geometry,
            audit: &self.clearance_fixture.audit,
            pose: &self.clearance_fixture.pose,
            decomposition: &self.clearance_fixture.decomposition,
            common_pose: &self.clearance_fixture.common_pose,
            parent_fixed_face: self.clearance_fixture.parent_fixed_face,
            parent_schedule: &self.clearance_fixture.parent_schedule,
            profile: &self.clearance_fixture.profile,
            paper_thickness_mm: 0.1,
            closure_tolerance: self.clearance_fixture.closure_tolerance,
            block_closure_set: &self.clearance_fixture.block_closure_set,
            whole_parent_closure: &self.clearance_fixture.whole_parent_closure,
            whole_parent_closure_limits: self.clearance_fixture.whole_parent_closure_limits,
            clearance: &self.clearance,
            source_authority,
            limits,
        })
    }

    /// Forms the matching replay input under the same source-derived exact
    /// caps. Kept separate because the issuing input consumes its authority.
    pub(super) fn revalidation_input_from_n33_live_authority_v2<'a>(
        &'a self,
        source_authority: ori_foldability::GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    ) -> Result<
        CommonArticulationGeneralCellTransportRevalidationInputV2<'a>,
        CommonArticulationGeneralCellTransportErrorV2,
    > {
        let limits = exact_transport_limits_for_live_n33_source_v2(
            self,
            source_authority.layer_order_snapshot_v2(),
        )?;
        Ok(CommonArticulationGeneralCellTransportRevalidationInputV2 {
            geometry: &self.clearance_fixture.geometry,
            audit: &self.clearance_fixture.audit,
            pose: &self.clearance_fixture.pose,
            decomposition: &self.clearance_fixture.decomposition,
            common_pose: &self.clearance_fixture.common_pose,
            parent_fixed_face: self.clearance_fixture.parent_fixed_face,
            parent_schedule: &self.clearance_fixture.parent_schedule,
            profile: &self.clearance_fixture.profile,
            paper_thickness_mm: 0.1,
            closure_tolerance: self.clearance_fixture.closure_tolerance,
            block_closure_set: &self.clearance_fixture.block_closure_set,
            whole_parent_closure: &self.clearance_fixture.whole_parent_closure,
            whole_parent_closure_limits: self.clearance_fixture.whole_parent_closure_limits,
            clearance: &self.clearance,
            source_authority,
            limits,
        })
    }

    /// Rebuilds only the bounded live foldability artifacts required to
    /// revalidate a genuine compact N=33 assignment fixture. No solved
    /// snapshot is cloned or retained here.
    pub(super) fn n33_live_global_input_v2(&self) -> N33LiveGlobalInputV2 {
        let namespace = self
            .clearance_fixture
            .geometry
            .source_identity_namespace_v1()
            .expect("canonical N=33 namespace");
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: self
                .clearance_fixture
                .geometry
                .source_revision_v1()
                .expect("canonical N=33 revision"),
            paper: &self.clearance_fixture.paper,
            pattern: &self.clearance_fixture.pattern,
        })
        .snapshot
        .expect("canonical N=33 topology");
        let local = analyze_local_flat_foldability(
            &self.clearance_fixture.paper,
            &self.clearance_fixture.pattern,
        );
        N33LiveGlobalInputV2 { topology, local }
    }
}

/// Regenerated live N=33 inputs kept beside a compact assignment fixture only
/// for its no-search certificate revalidation.
pub(super) struct N33LiveGlobalInputV2 {
    topology: TopologySnapshot,
    local: LocalFlatFoldabilityReport,
}

/// Regenerated small-source artifacts used only to prove that a semantically
/// identical snapshot with different spare `Vec` capacity still earns a live
/// foldability authority. It is intentionally separate from the N=33
/// transport geometry and its checked compact-assignment fixture.
pub(super) struct SmallLiveGlobalInputV2 {
    namespace: ProjectId,
    paper: Paper,
    pattern: CreasePattern,
    topology: TopologySnapshot,
    local: LocalFlatFoldabilityReport,
}

impl SmallLiveGlobalInputV2 {
    pub(super) fn input(&self) -> GlobalFlatFoldabilityInput<'_> {
        GlobalFlatFoldabilityInput::current_with_geometry(
            self.namespace,
            &self.paper,
            &self.pattern,
            &self.topology,
            &self.local,
        )
    }
}

impl N33LiveGlobalInputV2 {
    pub(super) fn input<'a>(
        &'a self,
        fixture: &'a TransportFixtureV2,
    ) -> GlobalFlatFoldabilityInput<'a> {
        GlobalFlatFoldabilityInput::current_with_geometry(
            fixture
                .clearance_fixture
                .geometry
                .source_identity_namespace_v1()
                .expect("canonical N=33 namespace"),
            &fixture.clearance_fixture.paper,
            &fixture.clearance_fixture.pattern,
            &self.topology,
            &self.local,
        )
    }
}

/// Builds the only admissible transport caps for a live revalidated N=33
/// source: every source, logical, retained, and peak cap equals its measured
/// value. Intermediate source validation receives only exact direct caps and
/// never reaches issue.
pub(super) fn exact_transport_limits_for_live_n33_source_v2(
    fixture: &TransportFixtureV2,
    source: &LayerOrderSnapshot,
) -> Result<
    CommonArticulationGeneralCellTransportLimitsV2,
    CommonArticulationGeneralCellTransportErrorV2,
> {
    let layer_records = source
        .overlap_cells
        .iter()
        .try_fold(0usize, |total, cell| {
            total
                .checked_add(cell.bottom_to_top_faces.len())
                .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
        })?;
    let boundary_vertices = source
        .overlap_cells
        .iter()
        .try_fold(0usize, |total, cell| {
            total
                .checked_add(cell.exact_boundary.len())
                .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)
        })?;
    let source_retained_bytes = source
        .checked_deep_retained_bytes_v1()
        .ok_or(CommonArticulationGeneralCellTransportErrorV2::ResourceLimit)?;
    let source_layout =
        super::source_binding::scan_source_layout_metrics_v2(source, &mut || Ok(()))?;
    let source_logical_work = super::source_binding::source_traversal_work_v2(
        source,
        source_layout,
        source_retained_bytes,
        &mut || Ok(()),
    )?;
    let direct_source_caps = CommonArticulationGeneralCellTransportLimitsV2 {
        max_blocks: fixture.clearance_fixture.profile.configured_max_blocks_v2(),
        max_source_retained_bytes: source_retained_bytes,
        max_material_faces: source.material_faces.len(),
        max_folded_faces: source.folded_faces.len(),
        max_overlap_cells: source.overlap_cells.len(),
        max_face_pair_orders: source.face_pair_orders.len(),
        max_global_order_faces: source.global_bottom_to_top.as_ref().map_or(0, Vec::len),
        max_layer_records: layer_records,
        max_boundary_vertices: boundary_vertices,
        // These three are not consumed by source validation. They are filled
        // from `transport_resource_totals_v2` before the final issue limits
        // are formed, so no unlimited resource cap exists on the issue path.
        max_boundary_samples: 0,
        max_transitions: 0,
        // This finite pre-admission cap is exactly the deterministic source
        // traversal charge. Full transport work is set from `measured` below.
        max_logical_work: source_logical_work,
        max_retained_bytes: 0,
        max_peak_bytes: 0,
    };
    let (_, source_metrics) = super::source_binding::source_digest_and_metrics_v2(
        source,
        &fixture.clearance_fixture.geometry,
        &fixture.clearance_fixture.decomposition,
        &fixture.clearance_fixture.profile,
        source.provenance.source,
        direct_source_caps,
        &mut || Ok(()),
    )?;
    let measured = super::resource::transport_resource_totals_v2(
        source_metrics,
        fixture.clearance.logical_work_v2(),
        fixture.clearance.storage_bytes_upper_bound_v2(),
        fixture
            .clearance_fixture
            .whole_parent_closure
            .parent_closure_leaves_v2(),
    )?;
    Ok(CommonArticulationGeneralCellTransportLimitsV2 {
        max_boundary_samples: measured.boundary_samples,
        max_transitions: measured.transitions,
        max_logical_work: measured.logical_work,
        max_retained_bytes: measured.retained_bytes,
        max_peak_bytes: measured.peak_bytes,
        ..direct_source_caps
    })
}

pub(super) fn transport_fixture_v2() -> TransportFixtureV2 {
    transport_fixture_from_clearance_v2(miura_fixture_v2())
}

/// Deterministic transport fixture whose material namespace is exactly the
/// one bound by the shared checked N=33 compact pair assignment.
pub(super) fn golden_n33_transport_fixture_v2() -> TransportFixtureV2 {
    transport_fixture_from_clearance_v2(golden_n33_miura_fixture_v2())
}

fn transport_fixture_from_clearance_v2(clearance_fixture: MiuraFixtureV2) -> TransportFixtureV2 {
    let source_report = small_live_source_report_v2();
    let source_authority = source_report
        .layer_order_source_authority_v2()
        .expect("small live sealed source");
    assert!(source_authority.is_current_v2());
    let source_retained_bytes = source_authority
        .layer_order_snapshot_v2()
        .checked_deep_retained_bytes_v1()
        .expect("small source retained bytes");
    let source = source_authority
        .layer_order_snapshot_v2()
        .try_clone_with_retained_byte_limit_v1(source_retained_bytes)
        .expect("bounded small source clone");
    let transitions = clearance_fixture
        .whole_parent_closure
        .parent_closure_leaves_v2()
        + 1;
    let layer_records = source
        .overlap_cells
        .iter()
        .map(|cell| cell.bottom_to_top_faces.len())
        .sum::<usize>();
    let boundary_vertices = source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len())
        .sum::<usize>();
    let boundary_samples = source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len() * cell.bottom_to_top_faces.len())
        .sum::<usize>()
        * transitions;
    let limits = CommonArticulationGeneralCellTransportLimitsV2 {
        max_blocks: clearance_fixture.profile.configured_max_blocks_v2(),
        max_source_retained_bytes: source_retained_bytes,
        max_material_faces: source.material_faces.len(),
        max_folded_faces: source.folded_faces.len(),
        max_overlap_cells: source.overlap_cells.len(),
        max_face_pair_orders: source.face_pair_orders.len(),
        max_global_order_faces: source.global_bottom_to_top.as_ref().map_or(0, Vec::len),
        max_layer_records: layer_records,
        max_boundary_vertices: boundary_vertices,
        max_boundary_samples: boundary_samples,
        max_transitions: transitions,
        // This fixture is deliberately foreign and must fail before resource
        // admission, so keep non-source caps finite rather than disguising a
        // test-only unbounded route.
        max_logical_work: 1,
        max_retained_bytes: 1,
        max_peak_bytes: 1,
    };
    let clearance =
        match issue_common_articulation_clearance_prerequisite_v2(clearance_fixture.input())
            .expect("N=33 clearance fixture")
        {
            CommonArticulationClearanceOutcomeV2::Unpromoted(value) => *value,
        };
    TransportFixtureV2 {
        clearance_fixture,
        clearance,
        source_report,
        source,
        limits,
    }
}

pub(super) fn small_live_global_input_v2() -> SmallLiveGlobalInputV2 {
    let namespace = ProjectId::schema_namespace([
        0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x54, 0x52, 0x41, 0x4e, 0x53, 0x50, 3,
    ]);
    let vertices = [
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(2.0, 2.0),
        Point2::new(1.0, 2.0),
        Point2::new(0.0, 2.0),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, position)| Vertex {
        id: VertexId::derive_v5(namespace, &[index as u8]),
        position,
    })
    .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[index as u8]),
            start: vertices[index].id,
            end: vertices[(index + 1) % vertices.len()].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: EdgeId::derive_v5(namespace, b"hinge"),
        start: vertices[1].id,
        end: vertices[4].id,
        kind: EdgeKind::Mountain,
    });
    let paper = Paper {
        boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
        ..Paper::default()
    };
    let pattern = CreasePattern { vertices, edges };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("small topology");
    let local = analyze_local_flat_foldability(&paper, &pattern);
    SmallLiveGlobalInputV2 {
        namespace,
        paper,
        pattern,
        topology,
        local,
    }
}

fn small_live_source_report_v2() -> GlobalFlatFoldabilityReport {
    let live = small_live_global_input_v2();
    analyze_global_flat_foldability(live.input(), GlobalFlatFoldabilityLimits::default())
        .expect("small live global foldability")
}
