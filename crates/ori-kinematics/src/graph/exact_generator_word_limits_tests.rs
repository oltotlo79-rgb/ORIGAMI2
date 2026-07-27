use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;

fn native_upper_fixture_v1() -> ExactGeneratorWordFixtureV1 {
    let namespace = ProjectId::new();
    let faces = (0..MAX_EXACT_GENERATOR_WORD_HINGES_V1)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("generator-word-upper-face:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(MAX_EXACT_GENERATOR_WORD_HINGES_V1);
    let mut moving_edges = Vec::with_capacity(2);
    let mut zero_edges = Vec::with_capacity(MAX_EXACT_GENERATOR_WORD_HINGES_V1 - 2);
    for index in 0..MAX_EXACT_GENERATOR_WORD_HINGES_V1 {
        let edge = EdgeId::derive_v5(
            namespace,
            format!("generator-word-upper-edge:{index}").as_bytes(),
        );
        let next = (index + 1) % faces.len();
        let (assignment, start, end, axis) = if index < 2 {
            (
                if index == 0 {
                    FoldAssignment::Mountain
                } else {
                    FoldAssignment::Valley
                },
                Point3::new(index as f64, 0.0, 0.0).unwrap(),
                Point3::new(index as f64 + 1.0, 0.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            )
        } else {
            (
                FoldAssignment::Mountain,
                Point3::new(index as f64, 10.0, 0.0).unwrap(),
                Point3::new(index as f64 + 1.0, 10.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            )
        };
        hinges.push(TreeHinge::new_for_test(
            edge,
            assignment,
            faces[index],
            faces[next],
            start,
            end,
            axis,
        ));
        if index < 2 {
            moving_edges.push(edge);
        } else {
            zero_edges.push(edge);
        }
    }
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        ExactGeneratorWordFixturePartsV1 {
            fixed_face: faces[0],
            moving_edges,
            constant_edges: Vec::new(),
            zero_edges,
            groups: Vec::new(),
        },
        true,
    )
}

#[test]
fn generator_word_accepts_the_native_hinge_boundary_and_rejects_one_over() {
    let fixture = native_upper_fixture_v1();
    assert_eq!(
        fixture.geometry.hinges().len(),
        MAX_EXACT_GENERATOR_WORD_HINGES_V1
    );
    assert_eq!(fixture.audit.closure_hinges().len(), 1);
    let graph = authenticate_graph_v1(&fixture.geometry, &fixture.audit).unwrap();
    assert_eq!(
        graph.adjacency_entry_limit,
        MAX_EXACT_GENERATOR_WORD_ADJACENCY_ENTRIES_V1
    );
    assert_eq!(graph.word_node_limit, MAX_EXACT_GENERATOR_WORD_NODES_V1);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(exact_generator_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    assert!(
        bounded_exact_generator_word_counts_v1(
            MAX_EXACT_GENERATOR_WORD_FACES_V1,
            MAX_EXACT_GENERATOR_WORD_HINGES_V1,
        )
        .is_some()
    );
    assert!(
        bounded_exact_generator_word_counts_v1(
            MAX_EXACT_GENERATOR_WORD_FACES_V1 + 1,
            MAX_EXACT_GENERATOR_WORD_HINGES_V1,
        )
        .is_none()
    );
    assert!(
        bounded_exact_generator_word_counts_v1(
            MAX_EXACT_GENERATOR_WORD_FACES_V1,
            MAX_EXACT_GENERATOR_WORD_HINGES_V1 + 1,
        )
        .is_none()
    );
    assert!(ReducedWordInternerV1::prepare(MAX_EXACT_GENERATOR_WORD_NODES_V1).is_some());
    assert!(ReducedWordInternerV1::prepare(MAX_EXACT_GENERATOR_WORD_NODES_V1 + 1).is_none());

    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"generator-word-one-over"),
        FoldAssignment::Mountain,
        fixture.geometry.face_ids()[0],
        fixture.geometry.face_ids()[2],
        Point3::new(20_000.0, 0.0, 0.0).unwrap(),
        Point3::new(20_001.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    let one_over = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        one_over_hinges,
    );
    assert!(authenticate_graph_v1(&one_over, &fixture.audit).is_none());
}
