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

fn native_carrier_free_product_cactus_v1() -> CarrierFreeProductFixtureV1 {
    const EDGES_PER_BLOCK: usize = 4;
    let edge_limit = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let block_count = edge_limit / EDGES_PER_BLOCK;
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"carrier-free-product-native-center");
    let mut faces = Vec::with_capacity(1 + block_count * 3);
    faces.push(center);
    let mut hinges = Vec::with_capacity(edge_limit);
    let mut profiles = Vec::with_capacity(edge_limit);
    let mut cycle_edges = Vec::with_capacity(edge_limit);
    for block in 0..block_count {
        let local = (0..3)
            .map(|vertex| {
                let face = FaceId::derive_v5(
                    namespace,
                    format!("carrier-free-product-native-face:{block}:{vertex}").as_bytes(),
                );
                faces.push(face);
                face
            })
            .collect::<Vec<_>>();
        let cycle = [center, local[0], local[1], local[2]];
        for edge_in_block in 0..EDGES_PER_BLOCK {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("carrier-free-product-native-edge:{block}:{edge_in_block}").as_bytes(),
            );
            let along = edge_in_block as f64 * 3.0;
            hinges.push(TreeHinge::new_for_test(
                edge,
                if edge_in_block < 2 {
                    FoldAssignment::Mountain
                } else {
                    FoldAssignment::Valley
                },
                cycle[edge_in_block],
                cycle[(edge_in_block + 1) % EDGES_PER_BLOCK],
                Point3::new(along, block as f64, 0.0).unwrap(),
                Point3::new(along + 1.0, block as f64, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            ));
            profiles.push((
                edge,
                TestProfileV1::Constant(
                    if edge_in_block % 2 == 0 {
                        30.0_f64
                    } else {
                        45.0_f64
                    }
                    .to_bits(),
                ),
            ));
            cycle_edges.push(edge);
        }
    }
    rebuild_fixture_v1(
        faces,
        hinges,
        CarrierFreeProductFixturePartsV1 {
            fixed_face: center,
            profiles,
            cycle_edges,
            bridge_edges: Vec::new(),
        },
        true,
    )
}

#[test]
fn carrier_free_product_accepts_native_ten_thousand_edge_cactus() {
    let fixture = native_carrier_free_product_cactus_v1();
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    assert_eq!(fixture.geometry.hinges().len(), native);
    assert_eq!(fixture.geometry.face_ids().len(), 7_501);
    assert_eq!(fixture.audit.closure_hinges().len(), 2_500);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_carrier_free_product_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let decomposition =
        prepare_contracted_block_cut_v1(&fixture.geometry, &fixture.audit, &schedule).unwrap();
    assert_eq!(decomposition.blocks().len(), 2_500);
    let (_, bounds, classifications) =
        preparation::prepare_carrier_free_product_blocks_v1(&schedule, &decomposition).unwrap();
    assert_eq!(
        bounds,
        CarrierFreeProductBoundsV1 {
            cyclic_edges: 10_000,
            cyclic_blocks: 2_500,
            node_capacity: 22_500,
            directed_appends: 20_000,
            vector_storage_limit: 40_000,
            vector_work: 40_000,
            key_classifications: 10_000,
        }
    );
    assert_eq!(classifications, bounds.key_classifications);
}

#[test]
fn carrier_free_product_resource_preflight_is_exact_and_rejects_one_over() {
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let maximum = bounded_carrier_free_product_counts_v1(&[(native, 64)]).unwrap();
    assert_eq!(maximum.cyclic_edges, native);
    assert_eq!(
        maximum.directed_appends,
        MAX_CARRIER_FREE_PRODUCT_DIRECTED_APPENDS_V1
    );
    assert_eq!(
        maximum.vector_work,
        MAX_CARRIER_FREE_PRODUCT_VECTOR_UNITS_V1
    );
    assert_eq!(
        maximum.vector_storage_limit,
        MAX_CARRIER_FREE_PRODUCT_VECTOR_UNITS_V1
    );
    assert_eq!(
        maximum.key_classifications,
        MAX_CARRIER_FREE_PRODUCT_KEY_CLASSIFICATIONS_V1
    );
    assert!(maximum.node_capacity <= MAX_CARRIER_FREE_PRODUCT_NODES_V1);

    assert!(bounded_carrier_free_product_counts_v1(&[(native + 1, 64)]).is_none());
    assert!(bounded_carrier_free_product_counts_v1(&[(native, 65)]).is_none());
    assert!(bounded_carrier_free_product_counts_v1(&[(usize::MAX, 64)]).is_none());
    assert!(bounded_carrier_free_product_counts_v1(&[]).is_none());
}

#[test]
fn carrier_free_product_rejects_actual_graph_one_over_before_block_allocations() {
    let fixture = native_carrier_free_product_cactus_v1();
    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"carrier-free-product-native-one-over"),
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
