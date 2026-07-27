use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;
use crate::{Point3, TreeHinge};

fn native_upper_fixture_v1() -> CoaxialLatticeFixtureV1 {
    let count = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let namespace = ProjectId::new();
    let faces = (0..count)
        .map(|index| {
            FaceId::derive_v5(namespace, format!("coaxial-native-face:{index}").as_bytes())
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(count);
    let mut moving_edges = Vec::with_capacity(2);
    let mut constant_angles = Vec::with_capacity(2);
    let mut zero_edges = Vec::with_capacity(count - 4);
    for index in 0..count {
        let edge = EdgeId::derive_v5(namespace, format!("coaxial-native-edge:{index}").as_bytes());
        let next = (index + 1) % faces.len();
        let (assignment, y) = match index {
            0 | 1 => (FoldAssignment::Mountain, 0.0),
            2 | 3 => (FoldAssignment::Valley, 0.0),
            _ => (FoldAssignment::Mountain, 10.0),
        };
        hinges.push(TreeHinge::new_for_test(
            edge,
            assignment,
            faces[index],
            faces[next],
            Point3::new(index as f64, y, 0.0).unwrap(),
            Point3::new(index as f64 + 1.0, y, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ));
        match index {
            0 | 2 => moving_edges.push(edge),
            1 | 3 => constant_angles.push((edge, 30.0_f64.to_bits())),
            _ => zero_edges.push(edge),
        }
    }
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        CoaxialLatticeFixturePartsV1 {
            fixed_face: faces[0],
            moving_edges,
            constant_angles,
            zero_edges,
            groups: Vec::new(),
        },
        true,
    )
}

fn profile_cap_fixture_v1(constant_profile_count: usize) -> CoaxialLatticeFixtureV1 {
    let namespace = ProjectId::new();
    let edge_count = constant_profile_count * 2 + 2;
    let faces = (0..edge_count)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("coaxial-profile-face:{constant_profile_count}:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(edge_count);
    let mut moving_edges = Vec::with_capacity(2);
    let mut constant_angles = Vec::with_capacity(constant_profile_count * 2);
    for index in 0..edge_count {
        let edge = EdgeId::derive_v5(
            namespace,
            format!("coaxial-profile-edge:{constant_profile_count}:{index}").as_bytes(),
        );
        let (assignment, constant) = if index == 0 {
            (FoldAssignment::Mountain, None)
        } else if index <= constant_profile_count {
            (FoldAssignment::Mountain, Some(index))
        } else if index == constant_profile_count + 1 {
            (FoldAssignment::Valley, None)
        } else {
            (
                FoldAssignment::Valley,
                Some(edge_count.saturating_sub(index)),
            )
        };
        hinges.push(TreeHinge::new_for_test(
            edge,
            assignment,
            faces[index],
            faces[(index + 1) % faces.len()],
            Point3::new(index as f64, 0.0, 0.0).unwrap(),
            Point3::new(index as f64 + 1.0, 0.0, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ));
        if let Some(constant) = constant {
            constant_angles.push((edge, (constant as f64).to_bits()));
        } else {
            moving_edges.push(edge);
        }
    }
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        CoaxialLatticeFixturePartsV1 {
            fixed_face: faces[0],
            moving_edges,
            constant_angles,
            zero_edges: Vec::new(),
            groups: Vec::new(),
        },
        false,
    )
}

#[test]
fn coaxial_lattice_accepts_native_ten_thousand_hinges_and_rejects_one_over() {
    let fixture = native_upper_fixture_v1();
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    assert_eq!(fixture.geometry.face_ids().len(), native);
    assert_eq!(fixture.geometry.hinges().len(), native);
    assert_eq!(fixture.audit.closure_hinges().len(), 1);
    assert_eq!(fixture.zero_edges.len(), native - 4);
    let graph = authenticate_graph_v1(&fixture.geometry, &fixture.audit).unwrap();
    assert_eq!(graph.adjacency_entry_limit(), native * 2);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(coaxial_profile_lattice_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"coaxial-native-one-over"),
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

#[test]
fn coaxial_lattice_accepts_sixty_four_profiles_and_rejects_sixty_five() {
    let sixty_four = profile_cap_fixture_v1(63);
    let schedule = polynomial_schedule_v1(&sixty_four, ScheduleMutationV1::None);
    assert!(coaxial_profile_lattice_cycle_closure_premises_v1(
        &sixty_four.geometry,
        &sixty_four.audit,
        sixty_four.fixed_face,
        &schedule,
        0.0,
    ));

    let sixty_five = profile_cap_fixture_v1(64);
    let schedule = polynomial_schedule_v1(&sixty_five, ScheduleMutationV1::None);
    assert!(!coaxial_profile_lattice_cycle_closure_premises_v1(
        &sixty_five.geometry,
        &sixty_five.audit,
        sixty_five.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn coaxial_lattice_resource_products_are_checked_at_every_boundary() {
    let max_faces = 10_001;
    let max_adjacency = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2;
    let maximum = bounded_coaxial_lattice_work_v1(
        max_faces,
        max_adjacency,
        MAX_COAXIAL_PROFILE_LATTICE_GENERATORS_V1,
    )
    .unwrap();
    assert_eq!(maximum.storage, MAX_COAXIAL_PROFILE_LATTICE_STORAGE_V1);
    assert_eq!(maximum.work, MAX_COAXIAL_PROFILE_LATTICE_WORK_V1);

    assert!(bounded_coaxial_lattice_work_v1(max_faces + 1, max_adjacency, 64).is_none());
    assert!(bounded_coaxial_lattice_work_v1(max_faces, max_adjacency + 1, 64).is_none());
    assert!(bounded_coaxial_lattice_work_v1(max_faces, max_adjacency, 1).is_none());
    assert!(bounded_coaxial_lattice_work_v1(max_faces, max_adjacency, 65).is_none());
    assert!(bounded_coaxial_lattice_work_v1(usize::MAX, usize::MAX, 64).is_none());
}
