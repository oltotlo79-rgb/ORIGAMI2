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

fn native_free_word_cactus_fixture_v1() -> FreeWordFixtureV1 {
    const EDGES_PER_BLOCK: usize = 4;
    let edge_limit = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let block_count = edge_limit / EDGES_PER_BLOCK;
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"block-cut-free-word-native-center");
    let mut faces = Vec::with_capacity(1 + block_count * 3);
    faces.push(center);
    let mut hinges = Vec::with_capacity(edge_limit);
    let mut profiles = Vec::with_capacity(edge_limit);
    let mut cyclic_blocks = Vec::with_capacity(block_count);
    for block in 0..block_count {
        let local = (0..3)
            .map(|vertex| {
                let face = FaceId::derive_v5(
                    namespace,
                    format!("block-cut-free-word-native-face:{block}:{vertex}").as_bytes(),
                );
                faces.push(face);
                face
            })
            .collect::<Vec<_>>();
        let cycle = [center, local[0], local[1], local[2]];
        let mut block_edges = Vec::with_capacity(EDGES_PER_BLOCK);
        for edge_in_block in 0..EDGES_PER_BLOCK {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("block-cut-free-word-native-edge:{block}:{edge_in_block}").as_bytes(),
            );
            let (assignment, start, end, axis, angle) = match edge_in_block {
                0 => (
                    FoldAssignment::Mountain,
                    Point3::new(0.0, block as f64, 0.0).unwrap(),
                    Point3::new(1.0, block as f64, 0.0).unwrap(),
                    Point3::new(1.0, 0.0, 0.0).unwrap(),
                    30.0_f64,
                ),
                1 => (
                    FoldAssignment::Mountain,
                    Point3::new(20_000.0 + block as f64, 0.0, 0.0).unwrap(),
                    Point3::new(20_000.0 + block as f64, 1.0, 0.0).unwrap(),
                    Point3::new(0.0, 1.0, 0.0).unwrap(),
                    45.0_f64,
                ),
                2 => (
                    FoldAssignment::Valley,
                    Point3::new(20_000.0 + block as f64, 4.0, 0.0).unwrap(),
                    Point3::new(20_000.0 + block as f64, 5.0, 0.0).unwrap(),
                    Point3::new(0.0, 1.0, 0.0).unwrap(),
                    45.0_f64,
                ),
                _ => (
                    FoldAssignment::Valley,
                    Point3::new(4.0, block as f64, 0.0).unwrap(),
                    Point3::new(5.0, block as f64, 0.0).unwrap(),
                    Point3::new(1.0, 0.0, 0.0).unwrap(),
                    30.0_f64,
                ),
            };
            hinges.push(TreeHinge::new_for_test(
                edge,
                assignment,
                cycle[edge_in_block],
                cycle[(edge_in_block + 1) % EDGES_PER_BLOCK],
                start,
                end,
                axis,
            ));
            profiles.push((edge, TestProfileV1::Constant(angle.to_bits())));
            block_edges.push(edge);
        }
        cyclic_blocks.push(block_edges);
    }
    rebuild_fixture_v1(
        faces,
        hinges,
        FreeWordFixturePartsV1 {
            fixed_face: center,
            profiles,
            cyclic_blocks,
            bridge_edges: Vec::new(),
            zero_edges: Vec::new(),
        },
        true,
    )
}

#[test]
fn block_cut_free_word_accepts_native_ten_thousand_edge_cactus() {
    let fixture = native_free_word_cactus_fixture_v1();
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    assert_eq!(fixture.geometry.hinges().len(), native);
    assert_eq!(fixture.geometry.face_ids().len(), 7_501);
    assert_eq!(fixture.audit.closure_hinges().len(), 2_500);
    assert_eq!(fixture.cyclic_blocks.len(), 2_500);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_free_word_cycle_closure_premises_v1(
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
        block.edge_indices().len() == 4 && block.vertices().len() == 4 && block.is_cyclic()
    }));
    let bounds = bounded_block_cut_free_word_resources_v1(decomposition.blocks()).unwrap();
    assert_eq!(
        bounds,
        BlockCutFreeWordBoundsV1 {
            node_capacity: 22_500,
            directed_work: 20_000,
            key_classifications: 10_000,
        }
    );
}

#[test]
fn block_cut_free_word_resource_preflight_is_exact_at_native_and_one_over() {
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let maximum = bounded_block_cut_free_word_counts_v1(native, native).unwrap();
    assert_eq!(maximum.node_capacity, MAX_BLOCK_CUT_FREE_WORD_NODES_V1);
    assert_eq!(
        maximum.directed_work,
        MAX_BLOCK_CUT_FREE_WORD_DIRECTED_WORK_V1
    );
    assert_eq!(
        maximum.key_classifications,
        MAX_BLOCK_CUT_FREE_WORD_KEY_CLASSIFICATIONS_V1
    );

    assert!(bounded_block_cut_free_word_counts_v1(native + 1, native).is_none());
    assert!(bounded_block_cut_free_word_counts_v1(native, native + 1).is_none());
    assert!(bounded_block_cut_free_word_counts_v1(0, 0).is_none());
    assert!(bounded_block_cut_free_word_counts_v1(usize::MAX, usize::MAX).is_none());
}

#[test]
fn block_cut_free_word_rejects_actual_graph_one_over_before_decomposition_allocations() {
    let fixture = native_free_word_cactus_fixture_v1();
    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"block-cut-free-word-native-one-over"),
        FoldAssignment::Mountain,
        fixture.geometry.face_ids()[1],
        fixture.geometry.face_ids()[5],
        Point3::new(40_000.0, 0.0, 0.0).unwrap(),
        Point3::new(40_001.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    let one_over = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        one_over_hinges,
    );
    assert!(authenticate_graph_v1(&one_over, &fixture.audit).is_none());
}
