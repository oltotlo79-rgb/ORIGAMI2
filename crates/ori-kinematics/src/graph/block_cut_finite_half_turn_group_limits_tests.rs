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

fn native_finite_half_turn_cactus_v1() -> FiniteHalfTurnFixtureV1 {
    const EDGES_PER_BLOCK: usize = 4;
    let edge_limit = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let block_count = edge_limit / EDGES_PER_BLOCK;
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"finite-half-turn-native-center");
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
                    format!("finite-half-turn-native-face:{block}:{vertex}").as_bytes(),
                );
                faces.push(face);
                face
            })
            .collect::<Vec<_>>();
        let cycle = [center, local[0], local[1], local[2]];
        for edge_in_block in 0..EDGES_PER_BLOCK {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("finite-half-turn-native-edge:{block}:{edge_in_block}").as_bytes(),
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
            profiles.push((edge, TestProfileV1::Constant(180.0_f64.to_bits())));
            cycle_edges.push(edge);
        }
    }
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        FiniteHalfTurnFixturePartsV1 {
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
fn finite_half_turn_accepts_native_ten_thousand_edge_cactus() {
    let fixture = native_finite_half_turn_cactus_v1();
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    assert_eq!(fixture.geometry.hinges().len(), native);
    assert_eq!(fixture.geometry.face_ids().len(), 7_501);
    assert_eq!(fixture.audit.closure_hinges().len(), 2_500);
    let schedule = polynomial_schedule_v1(&fixture);
    assert!(block_cut_finite_half_turn_group_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    assert_eq!(decomposition.blocks().len(), 2_500);
    let (prepared, bounds, classifications) =
        preparation::prepare_finite_half_turn_blocks_v1(&decomposition).unwrap();
    assert_eq!(prepared.len(), 2_500);
    let first_group = &prepared[0].group;
    assert_eq!(first_group.order, 2);
    assert_eq!(first_group.carrier_count, 1);
    assert_eq!(first_group.products, 2);
    assert!(prepared.iter().all(|block| {
        block.group.order == first_group.order
            && block.group.carrier_count == first_group.carrier_count
            && block.group.products == first_group.products
            && block.group.exact_storage_bits == first_group.exact_storage_bits
            && block.group.exact_work_bits == first_group.exact_work_bits
    }));
    assert_eq!(
        bounds,
        FiniteHalfTurnBoundsV1 {
            cyclic_edges: 10_000,
            cyclic_blocks: 2_500,
            group_elements: 5_000,
            group_products: 5_000,
            exact_storage_bits: first_group.exact_storage_bits * 2_500,
            exact_work_bits: first_group.exact_work_bits * 2_500,
            potential_storage: 10_000,
            directed_work: 20_000,
            key_classifications: 10_000,
        }
    );
    assert_eq!(classifications, bounds.key_classifications);
}

#[test]
fn finite_half_turn_resource_preflight_is_exact_and_rejects_one_over() {
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let edge_maximum =
        bounded_finite_half_turn_counts_v1(&[(native, native, 1, 2, 2, 1, 1)]).unwrap();
    assert_eq!(edge_maximum.cyclic_edges, native);
    assert_eq!(
        edge_maximum.potential_storage,
        MAX_FINITE_HALF_TURN_POTENTIAL_STORAGE_V1
    );
    assert_eq!(
        edge_maximum.directed_work,
        MAX_FINITE_HALF_TURN_DIRECTED_WORK_V1
    );
    assert_eq!(
        edge_maximum.key_classifications,
        MAX_FINITE_HALF_TURN_KEY_CLASSIFICATIONS_V1
    );
    assert!(bounded_finite_half_turn_counts_v1(&[(native, native + 1, 1, 2, 2, 1, 1)]).is_none());

    let mut element_shapes = vec![(2, 2, 1, 256, 256, 1, 1); 234];
    element_shapes.push((2, 2, 1, 96, 96, 1, 1));
    let element_maximum = bounded_finite_half_turn_counts_v1(&element_shapes).unwrap();
    assert_eq!(
        element_maximum.group_elements,
        MAX_FINITE_HALF_TURN_GROUP_ELEMENTS_V1
    );
    *element_shapes.last_mut().unwrap() = (2, 2, 1, 97, 97, 1, 1);
    assert!(bounded_finite_half_turn_counts_v1(&element_shapes).is_none());

    let mut product_shapes = vec![(256, 256, 256, 256, 65_536, 1, 1); 39];
    product_shapes.push((16, 16, 16, 256, 4_096, 1, 1));
    let product_maximum = bounded_finite_half_turn_counts_v1(&product_shapes).unwrap();
    assert_eq!(
        product_maximum.group_products,
        MAX_FINITE_HALF_TURN_GROUP_PRODUCTS_V1
    );
    product_shapes.push((2, 2, 1, 2, 2, 1, 1));
    assert!(bounded_finite_half_turn_counts_v1(&product_shapes).is_none());

    let storage_maximum = bounded_finite_half_turn_counts_v1(&[(
        2,
        2,
        1,
        2,
        2,
        MAX_FINITE_HALF_TURN_EXACT_STORAGE_BITS_V1,
        1,
    )])
    .unwrap();
    assert_eq!(
        storage_maximum.exact_storage_bits,
        MAX_FINITE_HALF_TURN_EXACT_STORAGE_BITS_V1
    );
    assert!(
        bounded_finite_half_turn_counts_v1(&[(
            2,
            2,
            1,
            2,
            2,
            MAX_FINITE_HALF_TURN_EXACT_STORAGE_BITS_V1 + 1,
            1,
        )])
        .is_none()
    );

    let work_maximum = bounded_finite_half_turn_counts_v1(&[(
        2,
        2,
        1,
        2,
        2,
        1,
        MAX_FINITE_HALF_TURN_EXACT_WORK_BITS_V1,
    )])
    .unwrap();
    assert_eq!(
        work_maximum.exact_work_bits,
        MAX_FINITE_HALF_TURN_EXACT_WORK_BITS_V1
    );
    assert!(
        bounded_finite_half_turn_counts_v1(&[(
            2,
            2,
            1,
            2,
            2,
            1,
            MAX_FINITE_HALF_TURN_EXACT_WORK_BITS_V1 + 1,
        )])
        .is_none()
    );

    assert!(bounded_finite_half_turn_counts_v1(&[(2, 1, 1, 2, 2, 1, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(3, 2, 1, 2, 2, 1, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(2, 2, 0, 2, 0, 1, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(257, 257, 257, 257, 66_049, 1, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(2, 2, 2, 1, 2, 1, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(2, 2, 1, 257, 257, 1, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(2, 2, 1, 2, 1, 1, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(2, 2, 1, 2, 2, 0, 1)]).is_none());
    assert!(bounded_finite_half_turn_counts_v1(&[(2, 2, 1, 2, 2, 1, 0)]).is_none());
    assert!(
        bounded_finite_half_turn_counts_v1(&[(usize::MAX, usize::MAX, 1, 2, 2, 1, 1)]).is_none()
    );
    assert!(bounded_finite_half_turn_counts_v1(&[]).is_none());
}

#[test]
fn finite_half_turn_rejects_actual_graph_one_over_before_block_allocations() {
    let fixture = native_finite_half_turn_cactus_v1();
    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"finite-half-turn-native-one-over"),
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
