//! Contract and fixture tests for the general-N V2 pose issuer.
use std::collections::{BTreeMap, BTreeSet, HashSet};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;
use crate::{
    CanonicalCycleScheduleV1, CanonicalEdgeBlockLimitsV1, CanonicalHingeAngles,
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, CycleSchedulePrepareErrorV1,
    CycleScheduleRestrictionErrorV1, CycleScheduleRestrictionStopV1, HingeAngle,
    MaterialHingeGraphAudit, RationalCoefficientV1, TreeKinematicsLimits,
};

#[path = "schedule_closure_tests.rs"]
mod schedule_closure_tests;

#[path = "block_closure_set_tests.rs"]
mod block_closure_set_tests;

#[path = "whole_parent_closure_tests.rs"]
mod whole_parent_closure_tests;

#[path = "dynamic_closure_bundle_tests.rs"]
mod dynamic_closure_bundle_tests;

#[path = "dynamic_closure_bridge_tests.rs"]
mod dynamic_closure_bridge_tests;

#[path = "issuance_and_revalidation_tests.rs"]
mod issuance_and_revalidation_tests;

#[path = "resource_and_binding_tests.rs"]
mod resource_and_binding_tests;

struct MiuraFixtureV2 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    pose: ClosedMaterialHingeGraphPose,
    decomposition: CanonicalMaterialEdgeBlockDecompositionV2,
}

impl MiuraFixtureV2 {
    fn input<'a>(
        &'a self,
        profile: &'a CommonArticulationResourceProfileV2,
    ) -> CommonArticulationPoseInputV2<'a> {
        CommonArticulationPoseInputV2 {
            geometry: &self.geometry,
            pose: &self.pose,
            decomposition: &self.decomposition,
            paper_thickness_mm: 0.1,
            profile,
        }
    }

    fn new_pose_instance(&self) -> ClosedMaterialHingeGraphPose {
        let angles = zero_angles_v2(&self.geometry);
        self.geometry
            .solve_closed(&self.audit, self.geometry.face_ids()[0], &angles, 0.0)
            .expect("same-geometry fresh pose")
    }

    fn decomposition_with_profile(
        &self,
        profile: &CommonArticulationResourceProfileV2,
    ) -> CanonicalMaterialEdgeBlockDecompositionV2 {
        self.geometry
            .decompose_canonical_edge_blocks_with_profile_v2(&self.audit, profile)
            .expect("same geometry with a profile-bound decomposition")
    }
}

fn miura_fixture_v2(block_count: usize) -> MiuraFixtureV2 {
    miura_fixture_with_namespace_v2(block_count, ProjectId::new())
}

fn miura_fixture_with_namespace_v2(block_count: usize, namespace: ProjectId) -> MiuraFixtureV2 {
    let (geometry, audit) = miura_geometry_and_audit_v2(block_count, namespace);
    let profile = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(block_count)
        .expect("canonical Miura V2 profile");
    let decomposition = geometry
        .decompose_canonical_edge_blocks_with_profile_v2(&audit, &profile)
        .expect("N33 canonical decomposition");
    let angles = zero_angles_v2(&geometry);
    let pose = geometry
        .solve_closed(&audit, geometry.face_ids()[0], &angles, 0.0)
        .expect("N33 closed pose");
    assert_eq!(geometry.face_ids().len(), 8 * block_count + 1);
    assert_eq!(geometry.hinges().len(), 12 * block_count);
    assert_eq!(decomposition.blocks().len(), block_count);
    MiuraFixtureV2 {
        geometry,
        audit,
        pose,
        decomposition,
    }
}

fn miura_geometry_and_audit_v2(
    block_count: usize,
    namespace: ProjectId,
) -> (MaterialHingeGraphGeometry, MaterialHingeGraphAudit) {
    let cells = canonical_miura_cells_v2(block_count);
    let (pattern, paper) = miura_pattern_v2(&cells, namespace);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("N33 Miura topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("N33 Miura geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("N33 Miura audit");
    (geometry, audit)
}

fn canonical_miura_cells_v2(block_count: usize) -> Vec<(i32, i32)> {
    (0..block_count)
        .flat_map(|index| {
            let x = i32::try_from(index)
                .expect("fixture block index fits i32")
                .checked_mul(2)
                .expect("fixture block x fits i32");
            let y = if index % 2 == 0 { 0_i32 } else { -2_i32 };
            let maximum_x = x.checked_add(2).expect("fixture maximum x fits i32");
            let maximum_y = y.checked_add(2).expect("fixture maximum y fits i32");
            (x..=maximum_x).flat_map(move |x| (y..=maximum_y).map(move |y| (x, y)))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn zero_angles_v2(geometry: &MaterialHingeGraphGeometry) -> CanonicalHingeAngles {
    CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero angle"))
            .collect(),
    )
    .expect("canonical zero angles")
}

fn zero_cycle_schedule_entries_v2(
    geometry: &MaterialHingeGraphGeometry,
) -> Vec<CycleScheduleEntryInputV1> {
    let mut entries = geometry
        .hinges()
        .iter()
        .map(|hinge| CycleScheduleEntryInputV1 {
            edge: hinge.edge(),
            initial_angle_degrees_bits: 0.0_f64.to_bits(),
            chebyshev_coefficients: vec![RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    entries
}

fn miura_pattern_v2(cells: &[(i32, i32)], namespace: ProjectId) -> (CreasePattern, Paper) {
    let mut points = BTreeSet::new();
    let mut incidence =
        BTreeMap::<((i32, i32), (i32, i32)), (usize, (i32, i32), (i32, i32))>::new();
    for &(x, y) in cells {
        let next_x = x.checked_add(1).expect("fixture cell x fits i32");
        let next_y = y.checked_add(1).expect("fixture cell y fits i32");
        let corners = [(x, y), (next_x, y), (next_x, next_y), (x, next_y)];
        points.extend(corners);
        for index in 0..4 {
            let start = corners[index];
            let end = corners[(index + 1) % 4];
            let key = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            incidence
                .entry(key)
                .and_modify(|entry| {
                    entry.0 = entry
                        .0
                        .checked_add(1)
                        .expect("fixture edge incidence fits usize");
                })
                .or_insert((1, start, end));
        }
    }
    let vertices = points
        .iter()
        .map(|&(x, y)| Vertex {
            id: vertex_id_for_fixture_v2(namespace, (x, y)),
            position: Point2::new(f64::from(x) * 20.0, f64::from(y) * 20.0),
        })
        .collect::<Vec<_>>();
    let vertex = |point: (i32, i32)| {
        vertices[points
            .iter()
            .position(|candidate| *candidate == point)
            .expect("cell corner")]
        .id
    };
    let edges = incidence
        .iter()
        .map(|(&(first, second), &(count, start, end))| Edge {
            id: edge_id_for_fixture_v2(namespace, first, second),
            start: vertex(start),
            end: vertex(end),
            kind: if count == 1 {
                EdgeKind::Boundary
            } else if first.1 == second.1 {
                EdgeKind::Mountain
            } else if first.1.rem_euclid(2) == 0 {
                EdgeKind::Valley
            } else {
                EdgeKind::Mountain
            },
        })
        .collect::<Vec<_>>();
    let directed = incidence
        .values()
        .filter(|(count, _, _)| *count == 1)
        .map(|(_, start, end)| (*start, *end))
        .collect::<Vec<_>>();
    let mut boundary = vec![directed[0].0];
    while boundary.len() < directed.len() {
        let cursor = *boundary.last().expect("boundary start");
        boundary.push(
            directed
                .iter()
                .find(|(start, _)| *start == cursor)
                .expect("next boundary edge")
                .1,
        );
    }
    let boundary_vertices = boundary.into_iter().map(vertex).collect();
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices,
            thickness_mm: 0.1,
            ..Paper::default()
        },
    )
}

fn vertex_id_for_fixture_v2(namespace: ProjectId, point: (i32, i32)) -> VertexId {
    let mut preimage = [0_u8; 9];
    preimage[0] = 0xc1;
    preimage[1..].copy_from_slice(&fixture_point_bytes_v2(point));
    VertexId::derive_v5(namespace, &preimage)
}

fn edge_id_for_fixture_v2(namespace: ProjectId, first: (i32, i32), second: (i32, i32)) -> EdgeId {
    let mut preimage = [0_u8; 17];
    preimage[0] = 0xc2;
    preimage[1..9].copy_from_slice(&fixture_point_bytes_v2(first));
    preimage[9..].copy_from_slice(&fixture_point_bytes_v2(second));
    EdgeId::derive_v5(namespace, &preimage)
}

fn fixture_point_bytes_v2(point: (i32, i32)) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    bytes[..4].copy_from_slice(&point.0.to_le_bytes());
    bytes[4..].copy_from_slice(&point.1.to_le_bytes());
    bytes
}
