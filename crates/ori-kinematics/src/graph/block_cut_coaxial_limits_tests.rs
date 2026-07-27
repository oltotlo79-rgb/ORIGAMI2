use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;
use crate::{
    Point3, TreeHinge,
    graph::{
        block_cut_decomposition::prepare_contracted_block_cut_v1,
        exact_generator_word::authenticate_graph_v1,
    },
};

fn native_block_cactus_fixture_v1() -> BlockCutFixtureV1 {
    const EDGES_PER_BLOCK: usize = 4;
    let edge_limit = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let block_count = edge_limit / EDGES_PER_BLOCK;
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"block-cut-native-center");
    let mut faces = Vec::with_capacity(1 + block_count * 3);
    faces.push(center);
    let mut hinges = Vec::with_capacity(edge_limit);
    let mut profiles = Vec::with_capacity(edge_limit);
    for block in 0..block_count {
        let local = (0..3)
            .map(|vertex| {
                let face = FaceId::derive_v5(
                    namespace,
                    format!("block-cut-native-face:{block}:{vertex}").as_bytes(),
                );
                faces.push(face);
                face
            })
            .collect::<Vec<_>>();
        let cycle = [center, local[0], local[1], local[2]];
        for edge_in_block in 0..EDGES_PER_BLOCK {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("block-cut-native-edge:{block}:{edge_in_block}").as_bytes(),
            );
            let assignment = if edge_in_block < 2 {
                FoldAssignment::Mountain
            } else {
                FoldAssignment::Valley
            };
            let angle = if edge_in_block % 2 == 0 {
                30.0_f64
            } else {
                45.0_f64
            };
            let along = edge_in_block as f64 * 2.0;
            hinges.push(TreeHinge::new_for_test(
                edge,
                assignment,
                cycle[edge_in_block],
                cycle[(edge_in_block + 1) % EDGES_PER_BLOCK],
                Point3::new(along, block as f64, 0.0).unwrap(),
                Point3::new(along + 1.0, block as f64, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ));
            profiles.push((edge, TestScheduleProfileV1::Constant(angle.to_bits())));
        }
    }
    rebuild_fixture_v1(
        faces,
        hinges,
        BlockCutFixturePartsV1 {
            fixed_face: center,
            profiles,
            x_block_edges: Vec::new(),
            y_block_edges: Vec::new(),
            bridge_edges: Vec::new(),
            zero_edges: Vec::new(),
        },
        true,
    )
}

fn profile_count_fixture_v1(profile_count: usize) -> BlockCutFixtureV1 {
    let namespace = ProjectId::new();
    let edge_count = profile_count * 2;
    let faces = (0..edge_count)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("block-cut-profile-face:{profile_count}:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(edge_count);
    let mut profiles = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        let edge = EdgeId::derive_v5(
            namespace,
            format!("block-cut-profile-edge:{profile_count}:{index}").as_bytes(),
        );
        let (assignment, profile) = if index < profile_count {
            (FoldAssignment::Mountain, index + 1)
        } else {
            (FoldAssignment::Valley, edge_count.saturating_sub(index))
        };
        hinges.push(TreeHinge::new_for_test(
            edge,
            assignment,
            faces[index],
            faces[(index + 1) % edge_count],
            Point3::new(index as f64, 0.0, 0.0).unwrap(),
            Point3::new(index as f64 + 1.0, 0.0, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ));
        profiles.push((
            edge,
            TestScheduleProfileV1::Constant((profile as f64).to_bits()),
        ));
    }
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        BlockCutFixturePartsV1 {
            fixed_face: faces[0],
            profiles,
            x_block_edges: Vec::new(),
            y_block_edges: Vec::new(),
            bridge_edges: Vec::new(),
            zero_edges: Vec::new(),
        },
        false,
    )
}

#[test]
fn block_cut_accepts_native_ten_thousand_edge_cactus_and_rejects_one_over() {
    let fixture = native_block_cactus_fixture_v1();
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    assert_eq!(fixture.geometry.hinges().len(), native);
    assert_eq!(fixture.geometry.face_ids().len(), 7_501);
    assert_eq!(fixture.audit.closure_hinges().len(), 2_500);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_coaxial_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    assert_eq!(decomposition.blocks().len(), 2_500);
    assert!(decomposition.blocks().iter().all(|block| {
        block.edge_indices().len() == 4 && block.vertices().len() == 4 && !block.is_bridge()
    }));

    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"block-cut-native-one-over"),
        FoldAssignment::Mountain,
        fixture.geometry.face_ids()[1],
        fixture.geometry.face_ids()[5],
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
fn block_cut_accepts_sixty_four_profiles_and_rejects_sixty_five_per_block() {
    let sixty_four = profile_count_fixture_v1(64);
    let schedule = polynomial_schedule_v1(&sixty_four, ScheduleMutationV1::None);
    assert!(block_cut_coaxial_cycle_closure_premises_v1(
        &sixty_four.geometry,
        &sixty_four.audit,
        sixty_four.fixed_face,
        &schedule,
        0.0,
    ));

    let sixty_five = profile_count_fixture_v1(65);
    let schedule = polynomial_schedule_v1(&sixty_five, ScheduleMutationV1::None);
    assert!(!block_cut_coaxial_cycle_closure_premises_v1(
        &sixty_five.geometry,
        &sixty_five.audit,
        sixty_five.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn block_cut_resource_totals_are_checked_at_exact_products_and_one_over() {
    let mut maximum = BlockCutResourceTotalsV1::empty();
    assert!(
        maximum
            .charge_block(
                ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP,
                ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP,
                MAX_BLOCK_CUT_COAXIAL_PROFILES_PER_BLOCK_V1,
            )
            .is_some()
    );
    assert_eq!(maximum.storage, MAX_BLOCK_CUT_COAXIAL_STORAGE_V1);
    assert_eq!(maximum.work, MAX_BLOCK_CUT_COAXIAL_WORK_V1);
    assert!(
        maximum.charge_block(2, 2, 1).is_none(),
        "aggregate storage/work cannot exceed the native products"
    );

    let mut invalid = BlockCutResourceTotalsV1::empty();
    assert!(invalid.charge_block(3, 2, 1).is_none());
    assert!(invalid.charge_block(2, 2, 0).is_none());
    assert!(invalid.charge_block(2, 2, 65).is_none());
    assert!(invalid.charge_block(usize::MAX, usize::MAX, 64).is_none());

    let mut comparisons = BlockCutResourceTotalsV1::empty();
    comparisons.classification_work = MAX_BLOCK_CUT_COAXIAL_CLASSIFICATION_WORK_V1 - 1;
    assert!(comparisons.charge_profile_comparison().is_some());
    assert!(comparisons.charge_profile_comparison().is_none());
}
