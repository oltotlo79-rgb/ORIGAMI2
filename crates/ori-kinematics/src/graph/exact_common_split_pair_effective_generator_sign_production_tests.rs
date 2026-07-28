use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::{
    ExactCommonSplitPairEffectiveGeneratorSignLimitsV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry,
    tests::{Fixture, bind_schedule_and_profile, prove},
};
use crate::TreeKinematicsLimits;

fn production_split_fixture(edge_count: usize) -> Fixture {
    let namespace = ProjectId::schema_namespace([0x75; 16]);
    let boundary_points = [
        (0.0, 0.0),
        (5.0, 0.0),
        (10.0, 0.0),
        (10.0, 6.0),
        (5.0, 6.0),
        (0.0, 6.0),
    ];
    let mut vertices = boundary_points
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, format!("production-v-{index}").as_bytes()),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    vertices.extend((1..edge_count).map(|index| Vertex {
        id: VertexId::derive_v5(namespace, format!("production-split-v-{index}").as_bytes()),
        position: Point2::new(5.0, 6.0 * index as f64 / edge_count as f64),
    }));
    let boundary = vertices[..6]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, format!("production-boundary-{index}").as_bytes()),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let mut crease_vertices = vec![boundary[1]];
    crease_vertices.extend(vertices[6..].iter().map(|vertex| vertex.id));
    crease_vertices.push(boundary[4]);
    let mut hinge_edges = Vec::new();
    for index in 0..edge_count {
        let edge = EdgeId::derive_v5(namespace, format!("production-hinge-{index}").as_bytes());
        hinge_edges.push(edge);
        edges.push(Edge {
            id: edge,
            start: crease_vertices[index],
            end: crease_vertices[index + 1],
            kind: EdgeKind::Mountain,
        });
    }
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
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
    .unwrap();
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    assert_eq!(geometry.face_ids().len(), 2);
    assert_eq!(geometry.hinges().len(), edge_count);
    hinge_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let fixed_face = audit.faces()[0];
    let (schedule, profile) =
        bind_schedule_and_profile(&geometry, &audit, fixed_face, &hinge_edges, 10);
    Fixture {
        geometry,
        audit,
        fixed_face,
        schedule,
        profile,
    }
}

#[test]
fn production_two_and_three_segment_creases_are_recognized() {
    for edge_count in [2, 3] {
        let fixture = production_split_fixture(edge_count);
        let proof = prove(&fixture).unwrap();
        assert_eq!(proof.edge_ids(), fixture.profile.edge_ids());
        assert_eq!(fixture.audit.spanning_hinges(), &proof.edge_ids()[..1]);
        assert_eq!(fixture.audit.closure_hinges(), &proof.edge_ids()[1..]);
        proof
            .revalidate_issuers_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &fixture.schedule,
                &fixture.profile,
                ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
            )
            .unwrap();
    }
}
