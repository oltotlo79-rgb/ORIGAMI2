use std::{cell::Cell, time::Duration};

use ori_domain::{
    ConstraintId, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::*;

struct SemanticFixture {
    pattern: CreasePattern,
    records: Vec<GeometricConstraintRecordV1>,
}

fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
    GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    }
}

fn document(
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: records.into_iter().collect(),
    }
}

fn observer(
    cancellation: bool,
    deadline: std::time::Instant,
) -> GeometricConstraintAnalysisObserver {
    GeometricConstraintAnalysisObserver::new(GeometricConstraintAnalysisRuntime {
        cancellation: Arc::new(AtomicBool::new(cancellation)),
        deadline,
    })
}

fn continuing_observer() -> GeometricConstraintAnalysisObserver {
    observer(
        false,
        std::time::Instant::now()
            .checked_add(Duration::from_secs(60))
            .expect("future semantic-MUS test deadline"),
    )
}

fn semantic_fixture() -> SemanticFixture {
    let mut vertices = [VertexId::new(), VertexId::new(), VertexId::new()];
    vertices.sort_unstable_by_key(VertexId::canonical_bytes);
    let diagonal_endpoint = vertices[0];
    let origin = vertices[1];
    let horizontal_endpoint = vertices[2];
    let horizontal_edge = EdgeId::new();
    let diagonal_edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: horizontal_endpoint,
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: origin,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: diagonal_endpoint,
                position: Point2::new(1.0, 1.0),
            },
        ],
        edges: vec![
            Edge {
                id: diagonal_edge,
                start: origin,
                end: diagonal_endpoint,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: horizontal_edge,
                start: origin,
                end: horizontal_endpoint,
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let records = vec![
        record(GeometricConstraintKindV1::Horizontal {
            edge: horizontal_edge,
        }),
        record(GeometricConstraintKindV1::Horizontal {
            edge: diagonal_edge,
        }),
        record(GeometricConstraintKindV1::FixedAngle {
            vertex: origin,
            first_edge: horizontal_edge,
            second_edge: diagonal_edge,
            angle_degrees: 45.0,
        }),
    ];
    SemanticFixture { pattern, records }
}

fn prepared<'a>(
    pattern: &'a CreasePattern,
    records: impl IntoIterator<Item = GeometricConstraintRecordV1>,
) -> ori_core::GeometricConstraintSetV1<'a> {
    prepare_geometric_constraints_v1(
        pattern,
        &document(records),
        GeometricConstraintLimitsV1::default(),
    )
    .expect("semantic-MUS native fixture must prepare")
}

fn canonical_ids(records: &[GeometricConstraintRecordV1]) -> Vec<ConstraintId> {
    let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids
}

#[path = "geometric_constraint_analysis/semantic_mus_certified_tests.rs"]
mod certified_tests;

#[path = "geometric_constraint_analysis/semantic_mus_mapping_tests.rs"]
mod mapping_tests;

#[path = "geometric_constraint_analysis/semantic_mus_algebraic_tests.rs"]
mod algebraic_tests;

#[path = "geometric_constraint_analysis/semantic_mus_length_tests.rs"]
mod length_tests;
