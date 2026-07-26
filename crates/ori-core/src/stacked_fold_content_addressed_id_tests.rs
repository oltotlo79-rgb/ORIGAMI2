use std::cmp::Ordering;
use std::collections::HashSet;

use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, analyze_global_flat_foldability,
};
use ori_topology::{FaceKey, analyze_local_flat_foldability};

use super::*;
use crate::create_rectangular_sheet;

fn vertical_expected_crease(x: f64) -> ExpectedStackedFoldCreaseV1 {
    ExpectedStackedFoldCreaseV1 {
        start: Point2::new(x, 0.0),
        end: Point2::new(x, 400.0),
        kind: EdgeKind::Mountain,
    }
}

fn vertex_position(pattern: &CreasePattern, id: VertexId) -> Point2 {
    pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == id)
        .expect("fixture vertex")
        .position
}

fn segment_identity(
    pattern: &CreasePattern,
    start: Point2,
    end: Point2,
    kind: EdgeKind,
) -> (EdgeId, [VertexId; 2]) {
    let target = {
        let mut endpoints = [point_bits(start), point_bits(end)];
        endpoints.sort_unstable();
        endpoints
    };
    let edge = pattern
        .edges
        .iter()
        .find(|edge| {
            if edge.kind != kind {
                return false;
            }
            let mut endpoints = [
                point_bits(vertex_position(pattern, edge.start)),
                point_bits(vertex_position(pattern, edge.end)),
            ];
            endpoints.sort_unstable();
            endpoints == target
        })
        .expect("target segment exists exactly once");
    let mut endpoints = [edge.start, edge.end];
    endpoints.sort_unstable_by_key(VertexId::canonical_bytes);
    (edge.id, endpoints)
}

fn target_topology(
    identity: ProjectId,
    revision: Revision,
    candidate: &StackedFoldTopologyCandidateV1,
) -> TopologySnapshot {
    simulation_snapshot(
        identity,
        revision,
        &candidate.paper,
        &candidate.pattern,
        FaceLineageTopology::Target,
    )
    .expect("candidate has production simulation topology")
}

fn right_strip_face(
    topology: &TopologySnapshot,
    pattern: &CreasePattern,
    minimum_x: f64,
) -> (FaceId, FaceKey) {
    let matches = topology
        .faces
        .iter()
        .filter(|face| {
            !face.outer.half_edges.is_empty()
                && face
                    .outer
                    .half_edges
                    .iter()
                    .all(|half_edge| vertex_position(pattern, half_edge.origin).x >= minimum_x)
        })
        .map(|face| (face.id, face.key))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one unrelated right strip face");
    matches[0]
}

fn proven_layer_order(
    identity: ProjectId,
    revision: Revision,
    pattern: &CreasePattern,
    paper: &Paper,
) -> LayerOrderSnapshot {
    let source_topology = analyze_faces(FaceExtractionInput {
        identity_namespace: identity,
        source_revision: revision,
        paper,
        pattern,
    })
    .snapshot
    .expect("source topology");
    let local = analyze_local_flat_foldability(paper, pattern);
    let report = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            identity,
            paper,
            pattern,
            &source_topology,
            &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("global analysis");
    report.layer_order().expect("possible layer order").clone()
}

#[test]
fn topology_builder_v2_identities_ignore_revision_but_proofs_do_not() {
    let identity = ProjectId::new();
    let first_revision = 11;
    let second_revision = 29;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let expected = [vertical_expected_crease(300.0)];
    let build = |revision| {
        build_stacked_fold_topology_v1(
            identity,
            revision,
            &source_pattern,
            &source_paper,
            &expected,
            StackedFoldTopologyBuildLimitsV1::default(),
        )
        .expect("build content-addressed target")
    };
    let first = build(first_revision);
    let second = build(second_revision);
    assert_eq!(first, second);

    let source_vertex_ids = source_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<HashSet<_>>();
    let source_edge_ids = source_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<HashSet<_>>();
    for candidate in [&first, &second] {
        assert!(source_vertex_ids.iter().all(|id| {
            candidate
                .pattern
                .vertices
                .iter()
                .any(|vertex| vertex.id == *id)
        }));
        assert!(
            source_edge_ids.iter().all(|id| candidate
                .pattern
                .edges
                .iter()
                .any(|edge| edge.id == *id))
        );
    }
    let derived_vertices = |candidate: &StackedFoldTopologyCandidateV1| {
        let mut ids = candidate
            .pattern
            .vertices
            .iter()
            .filter(|vertex| !source_vertex_ids.contains(&vertex.id))
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        ids.sort_unstable_by_key(VertexId::canonical_bytes);
        ids
    };
    let derived_edges = |candidate: &StackedFoldTopologyCandidateV1| {
        let mut ids = candidate
            .pattern
            .edges
            .iter()
            .filter(|edge| !source_edge_ids.contains(&edge.id))
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        ids.sort_unstable_by_key(EdgeId::canonical_bytes);
        ids
    };
    assert!(!derived_vertices(&first).is_empty());
    assert!(!derived_edges(&first).is_empty());
    assert_eq!(derived_vertices(&first), derived_vertices(&second));
    assert_eq!(derived_edges(&first), derived_edges(&second));

    let first_topology = target_topology(identity, first_revision + 1, &first);
    let second_topology = target_topology(identity, second_revision + 1, &second);
    assert_ne!(
        first_topology.source_revision,
        second_topology.source_revision
    );
    let face_ids = |topology: &TopologySnapshot| {
        let mut ids = topology
            .faces
            .iter()
            .map(|face| face.id)
            .collect::<Vec<_>>();
        ids.sort_unstable_by_key(FaceId::canonical_bytes);
        ids
    };
    assert_eq!(face_ids(&first_topology), face_ids(&second_topology));

    let source_layer_order =
        proven_layer_order(identity, first_revision, &source_pattern, &source_paper);
    let stale_lineage = prepare_face_lineage_v1(
        FaceLineageInput {
            identity_namespace: identity,
            source_revision: first_revision,
            source_paper: &source_paper,
            source_pattern: &source_pattern,
            source_layer_order: &source_layer_order,
            target_revision: first_revision + 1,
            target_paper: &first.paper,
            target_pattern: &first.pattern,
        },
        FaceLineageLimits::default(),
    )
    .expect("prepare revision-bound lineage");
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                identity_namespace: identity,
                source_revision: second_revision,
                source_paper: &source_paper,
                source_pattern: &source_pattern,
                target_revision: second_revision + 1,
                target_paper: &second.paper,
                target_pattern: &second.pattern,
                face_lineage: &stale_lineage,
                expected_creases: &expected,
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::LineageRevisionMismatch),
        "stable content IDs must not create revision ABA authority"
    );
}

#[test]
fn topology_builder_v2_segment_survives_an_earlier_carrier_insertion() {
    let identity = ProjectId::new();
    let revision = 37;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let stable = vertical_expected_crease(300.0);
    let inserted = vertical_expected_crease(100.0);
    assert_eq!(
        compare_expected_crease(&inserted, &stable),
        Ordering::Less,
        "the new carrier must shift the old v1 carrier index"
    );
    let baseline = build_stacked_fold_topology_v1(
        identity,
        revision,
        &source_pattern,
        &source_paper,
        &[stable],
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("build baseline carrier");
    let expanded = build_stacked_fold_topology_v1(
        identity,
        revision,
        &source_pattern,
        &source_paper,
        &[stable, inserted],
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("insert canonically earlier carrier");
    assert_eq!(
        segment_identity(&baseline.pattern, stable.start, stable.end, stable.kind),
        segment_identity(&expanded.pattern, stable.start, stable.end, stable.kind)
    );
    let baseline_topology = target_topology(identity, revision + 1, &baseline);
    let expanded_topology = target_topology(identity, revision + 1, &expanded);
    assert_eq!(
        right_strip_face(&baseline_topology, &baseline.pattern, 300.0),
        right_strip_face(&expanded_topology, &expanded.pattern, 300.0)
    );
}

#[test]
fn topology_builder_v2_local_vertex_change_preserves_unrelated_face() {
    let identity = ProjectId::new();
    let revision = 43;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let left = vertical_expected_crease(100.0);
    let right = vertical_expected_crease(300.0);
    let mut changed_left = left;
    changed_left.start = Point2::new(120.0, 0.0);
    assert_eq!(changed_left.end, left.end);
    let build = |expected: &[ExpectedStackedFoldCreaseV1]| {
        build_stacked_fold_topology_v1(
            identity,
            revision,
            &source_pattern,
            &source_paper,
            expected,
            StackedFoldTopologyBuildLimitsV1::default(),
        )
        .expect("build local-change topology")
    };
    let baseline = build(&[left, right]);
    let changed = build(&[changed_left, right]);
    assert_ne!(baseline, changed);
    assert_eq!(
        segment_identity(&baseline.pattern, right.start, right.end, right.kind),
        segment_identity(&changed.pattern, right.start, right.end, right.kind)
    );
    let baseline_topology = target_topology(identity, revision + 1, &baseline);
    let changed_topology = target_topology(identity, revision + 1, &changed);
    assert_eq!(baseline_topology.faces.len(), 3);
    assert_eq!(changed_topology.faces.len(), 3);
    assert_eq!(
        right_strip_face(&baseline_topology, &baseline.pattern, 300.0),
        right_strip_face(&changed_topology, &changed.pattern, 300.0)
    );
}
