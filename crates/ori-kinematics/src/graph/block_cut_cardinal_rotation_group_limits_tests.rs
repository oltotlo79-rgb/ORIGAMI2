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

fn native_cardinal_rotation_cactus_v1() -> CardinalRotationFixtureV1 {
    const EDGES_PER_BLOCK: usize = 4;
    let edge_limit = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let block_count = edge_limit / EDGES_PER_BLOCK;
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"cardinal-rotation-native-center");
    let mut faces = Vec::with_capacity(1 + block_count * (EDGES_PER_BLOCK - 1));
    faces.push(center);
    let mut hinges = Vec::with_capacity(edge_limit);
    let mut profiles = Vec::with_capacity(edge_limit);
    let mut cycle_edges = Vec::with_capacity(edge_limit);
    for block in 0..block_count {
        let local = (0..(EDGES_PER_BLOCK - 1))
            .map(|vertex| {
                let face = FaceId::derive_v5(
                    namespace,
                    format!("cardinal-rotation-native-face:{block}:{vertex}").as_bytes(),
                );
                faces.push(face);
                face
            })
            .collect::<Vec<_>>();
        let cycle = [center, local[0], local[1], local[2]];
        for edge_in_block in 0..EDGES_PER_BLOCK {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("cardinal-rotation-native-edge:{block}:{edge_in_block}").as_bytes(),
            );
            let along = edge_in_block as f64 * 2.0;
            hinges.push(TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                cycle[edge_in_block],
                cycle[(edge_in_block + 1) % EDGES_PER_BLOCK],
                Point3::new(along, 0.0, 0.0).unwrap(),
                Point3::new(along + 1.0, 0.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ));
            profiles.push((edge, TestProfileV1::Constant(90.0_f64.to_bits())));
            cycle_edges.push(edge);
        }
    }
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        CardinalRotationFixturePartsV1 {
            fixed_face: center,
            profiles,
            cycle_edges,
            cycle_faces: faces,
            bridge_edges: Vec::new(),
        },
        true,
    )
}

#[test]
fn cardinal_rotation_accepts_native_ten_thousand_edge_cactus() {
    let fixture = native_cardinal_rotation_cactus_v1();
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    assert_eq!(fixture.geometry.hinges().len(), native);
    assert_eq!(fixture.geometry.face_ids().len(), 7_501);
    assert_eq!(fixture.audit.closure_hinges().len(), 2_500);
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(block_cut_cardinal_rotation_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    assert_eq!(decomposition.blocks().len(), 2_500);
    let (prepared, bounds, classifications, exact_relations) =
        preparation::prepare_cardinal_rotation_blocks_v1(&decomposition).unwrap();
    assert_eq!(prepared.len(), 2_500);
    assert!(prepared.iter().all(|block| block.carrier_count == 1));
    assert_eq!(
        bounds,
        CardinalRotationBoundsV1 {
            cyclic_edges: 10_000,
            cyclic_blocks: 2_500,
            storage: 90_000,
            work: 540_000,
            directed_edges: 20_000,
            key_classifications: 10_000,
            exact_relations: 0,
        }
    );
    assert_eq!(classifications, bounds.key_classifications);
    assert_eq!(exact_relations, bounds.exact_relations);
}

#[test]
fn cardinal_rotation_resource_preflight_is_exact_and_rejects_one_over() {
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let maximum = bounded_cardinal_rotation_counts_v1(&[(native, native, 1, 0)]).unwrap();
    assert_eq!(maximum.cyclic_edges, native);
    assert_eq!(maximum.storage, MAX_CARDINAL_STORAGE_V1);
    assert_eq!(maximum.work, MAX_CARDINAL_WORK_V1);
    assert_eq!(maximum.directed_edges, MAX_CARDINAL_DIRECTED_EDGES_V1);
    assert_eq!(
        maximum.key_classifications,
        MAX_CARDINAL_KEY_CLASSIFICATIONS_V1
    );
    assert!(bounded_cardinal_rotation_counts_v1(&[(native, native + 1, 1, 0)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[(2, 1, 1, 0)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[(0, 2, 1, 0)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[(3, 2, 1, 0)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[(2, 2, 0, 0)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[(4, 4, 4, 6)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[(3, 3, 3, 2)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[(usize::MAX, usize::MAX, 1, 0)]).is_none());
    assert!(bounded_cardinal_rotation_counts_v1(&[]).is_none());
}

#[test]
fn cardinal_rotation_rejects_actual_graph_one_over_before_block_allocations() {
    let fixture = native_cardinal_rotation_cactus_v1();
    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"cardinal-rotation-native-one-over"),
        FoldAssignment::Mountain,
        fixture.geometry.face_ids()[1],
        fixture.geometry.face_ids()[4],
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
