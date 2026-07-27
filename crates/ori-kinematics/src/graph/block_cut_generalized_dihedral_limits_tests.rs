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

fn native_generalized_dihedral_cactus_v1() -> DihedralFixtureV1 {
    const EDGES_PER_BLOCK: usize = 4;
    let edge_limit = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let block_count = edge_limit / EDGES_PER_BLOCK;
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"generalized-dihedral-native-center");
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
                    format!("generalized-dihedral-native-face:{block}:{vertex}").as_bytes(),
                );
                faces.push(face);
                face
            })
            .collect::<Vec<_>>();
        let cycle = [center, local[0], local[1], local[2]];
        let pivot = block as f64 * 4.0;
        for edge_in_block in 0..EDGES_PER_BLOCK {
            let edge = EdgeId::derive_v5(
                namespace,
                format!("generalized-dihedral-native-edge:{block}:{edge_in_block}").as_bytes(),
            );
            let half_turn = edge_in_block.is_multiple_of(2);
            let along = edge_in_block as f64 * 3.0;
            let (start, end, axis) = if half_turn {
                (
                    Point3::new(pivot, along, 0.0).unwrap(),
                    Point3::new(pivot, along + 1.0, 0.0).unwrap(),
                    Point3::new(0.0, 1.0, 0.0).unwrap(),
                )
            } else {
                (
                    Point3::new(pivot + along, 0.0, 0.0).unwrap(),
                    Point3::new(pivot + along + 1.0, 0.0, 0.0).unwrap(),
                    Point3::new(1.0, 0.0, 0.0).unwrap(),
                )
            };
            hinges.push(TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                cycle[edge_in_block],
                cycle[(edge_in_block + 1) % EDGES_PER_BLOCK],
                start,
                end,
                axis,
            ));
            profiles.push((
                edge,
                if half_turn {
                    TestProfileV1::Constant(180.0_f64.to_bits())
                } else {
                    TestProfileV1::Collective
                },
            ));
            cycle_edges.push(edge);
        }
    }
    rebuild_fixture_v1(
        faces,
        hinges,
        DihedralFixturePartsV1 {
            fixed_face: center,
            profiles,
            cycle_edges,
            bridge_edges: Vec::new(),
        },
        true,
    )
}

#[test]
fn generalized_dihedral_accepts_native_ten_thousand_edge_cactus() {
    let fixture = native_generalized_dihedral_cactus_v1();
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    assert_eq!(fixture.geometry.hinges().len(), native);
    assert_eq!(fixture.geometry.face_ids().len(), 7_501);
    assert_eq!(fixture.audit.closure_hinges().len(), 2_500);
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(block_cut_generalized_dihedral_cycle_closure_premises_v1(
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
        preparation::prepare_generalized_dihedral_blocks_v1(&schedule, &decomposition).unwrap();
    assert_eq!(
        bounds,
        GeneralizedDihedralBoundsV1 {
            cyclic_edges: 10_000,
            cyclic_blocks: 2_500,
            storage: 10_000,
            work: 20_000,
            directed_edges: 20_000,
            key_classifications: 10_000,
        }
    );
    assert_eq!(classifications, bounds.key_classifications);
}

#[test]
fn generalized_dihedral_resource_preflight_is_exact_and_rejects_one_over() {
    let native = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
    let maximum = bounded_generalized_dihedral_counts_v1(&[(native, native, 64)]).unwrap();
    assert_eq!(maximum.storage, MAX_DIHEDRAL_STORAGE_V1);
    assert_eq!(maximum.work, MAX_DIHEDRAL_WORK_V1);
    assert_eq!(maximum.directed_edges, MAX_DIHEDRAL_DIRECTED_EDGES_V1);
    assert_eq!(
        maximum.key_classifications,
        MAX_DIHEDRAL_KEY_CLASSIFICATIONS_V1
    );

    assert!(bounded_generalized_dihedral_counts_v1(&[(native, native + 1, 64)]).is_none());
    assert!(bounded_generalized_dihedral_counts_v1(&[(native, native, 65)]).is_none());
    assert!(bounded_generalized_dihedral_counts_v1(&[(native, native - 1, 64)]).is_none());
    assert!(bounded_generalized_dihedral_counts_v1(&[(usize::MAX, usize::MAX, 64)]).is_none());
    assert!(bounded_generalized_dihedral_counts_v1(&[]).is_none());
}

#[test]
fn generalized_dihedral_rejects_actual_graph_one_over_before_block_allocations() {
    let fixture = native_generalized_dihedral_cactus_v1();
    let mut one_over_hinges = fixture.geometry.hinges().to_vec();
    one_over_hinges.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"generalized-dihedral-native-one-over"),
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
