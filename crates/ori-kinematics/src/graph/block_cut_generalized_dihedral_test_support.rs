use std::collections::HashMap;

use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, Point3, RationalCoefficientV1, TreeHinge,
    TreeKinematicsLimits,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TestProfileV1 {
    Collective,
    OtherNonconstant,
    Constant(u64),
}

pub(super) struct DihedralFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cycle_edges: Vec<EdgeId>,
    pub(super) bridge_edges: Vec<EdgeId>,
}

pub(super) struct DihedralFixturePartsV1 {
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cycle_edges: Vec<EdgeId>,
    pub(super) bridge_edges: Vec<EdgeId>,
}

fn face_v1(id: FaceId) -> Face {
    Face {
        id,
        key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
        outer: BoundaryWalk {
            half_edges: Vec::new(),
            signed_double_area: 1.0,
        },
        holes: Vec::new(),
        seams: Vec::new(),
        area: 0.5,
    }
}

fn topology_v1(faces: &[FaceId], hinges: &[TreeHinge]) -> TopologySnapshot {
    TopologySnapshot {
        source_revision: 1,
        faces: faces.iter().copied().map(face_v1).collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: hinges
            .iter()
            .map(|hinge| FaceAdjacency {
                edge: hinge.edge(),
                first: hinge.left_face(),
                second: hinge.right_face(),
                assignment: hinge.assignment(),
            })
            .collect(),
        material_components: Vec::new(),
    }
}

pub(super) fn rebuild_fixture_v1(
    mut faces: Vec<FaceId>,
    mut hinges: Vec<TreeHinge>,
    parts: DihedralFixturePartsV1,
    reverse_storage: bool,
) -> DihedralFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    DihedralFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        profiles: parts.profiles,
        cycle_edges: parts.cycle_edges,
        bridge_edges: parts.bridge_edges,
    }
}

fn carrier_v1(half_turn: bool, slot: usize) -> (Point3, Point3, Point3) {
    let along = slot as f64 * 3.0;
    if half_turn {
        (
            Point3::new(0.0, along, 0.0).unwrap(),
            Point3::new(0.0, along + 1.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
        )
    } else {
        (
            Point3::new(along, 0.0, 0.0).unwrap(),
            Point3::new(along + 1.0, 0.0, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        )
    }
}

pub(super) fn dihedral_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
    last_primary_inverse: bool,
    observer_tamper_bridge: bool,
) -> DihedralFixtureV1 {
    let namespace = ProjectId::new();
    let faces = (0..4)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("generalized-dihedral-face:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let bridge_capacity = if observer_tamper_bridge { 1 } else { 0 };
    let mut hinges = Vec::with_capacity(4 + bridge_capacity);
    let mut profiles = Vec::with_capacity(4 + bridge_capacity);
    let mut cycle_edges = Vec::with_capacity(4);
    for index in 0usize..4 {
        let half_turn = index.is_multiple_of(2);
        let edge = EdgeId::derive_v5(
            namespace,
            format!("generalized-dihedral-edge:{index}").as_bytes(),
        );
        let (mut left, mut right) = (faces[index], faces[(index + 1) % 4]);
        let (mut start, mut end, mut axis) = carrier_v1(half_turn, index);
        if reverse_every_other && matches!(index, 0 | 3) {
            std::mem::swap(&mut left, &mut right);
            std::mem::swap(&mut start, &mut end);
            axis = Point3::new(-axis.x(), -axis.y(), -axis.z()).unwrap();
        }
        hinges.push(TreeHinge::new_for_test(
            edge,
            if last_primary_inverse && index == 3 {
                FoldAssignment::Valley
            } else {
                FoldAssignment::Mountain
            },
            left,
            right,
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

    let mut all_faces = faces.clone();
    let mut bridge_edges = Vec::new();
    if observer_tamper_bridge {
        let leaf = FaceId::derive_v5(namespace, b"generalized-dihedral-bridge-face");
        let edge = EdgeId::derive_v5(namespace, b"generalized-dihedral-bridge-edge");
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            faces[0],
            leaf,
            Point3::new(20.0, 0.0, 0.0).unwrap(),
            Point3::new(20.0, 0.0, 1.0).unwrap(),
            Point3::new(0.0, 0.0, 1.0).unwrap(),
        ));
        profiles.push((edge, TestProfileV1::OtherNonconstant));
        bridge_edges.push(edge);
        all_faces.push(leaf);
    }
    rebuild_fixture_v1(
        all_faces,
        hinges,
        DihedralFixturePartsV1 {
            fixed_face: faces[0],
            profiles,
            cycle_edges,
            bridge_edges,
        },
        reverse_storage,
    )
}

pub(super) fn replace_hinges_v1(
    fixture: DihedralFixtureV1,
    edges: &[EdgeId],
    replacement: impl Fn(&TreeHinge) -> TreeHinge,
) -> DihedralFixtureV1 {
    let selected = edges
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let hinges = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            if selected.contains(&hinge.edge()) {
                replacement(hinge)
            } else {
                hinge.clone()
            }
        })
        .collect();
    rebuild_fixture_v1(
        fixture.geometry.face_ids().to_vec(),
        hinges,
        DihedralFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            profiles: fixture.profiles,
            cycle_edges: fixture.cycle_edges,
            bridge_edges: fixture.bridge_edges,
        },
        false,
    )
}

pub(super) fn replace_profile_v1(
    mut fixture: DihedralFixtureV1,
    edge: EdgeId,
    profile: TestProfileV1,
) -> DihedralFixtureV1 {
    let Some((_, current)) = fixture
        .profiles
        .iter_mut()
        .find(|(candidate, _)| *candidate == edge)
    else {
        panic!("test edge must exist");
    };
    *current = profile;
    fixture
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ScheduleMutationV1 {
    None,
    ThreeSampleMatch(EdgeId),
}

pub(super) fn polynomial_schedule_v1(
    fixture: &DihedralFixtureV1,
    mutation: ScheduleMutationV1,
) -> CanonicalCycleScheduleV1 {
    let profiles = fixture.profiles.iter().copied().collect::<HashMap<_, _>>();
    let mut edges = fixture
        .geometry
        .hinges()
        .iter()
        .map(TreeHinge::edge)
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let entries = edges
        .into_iter()
        .map(|edge| {
            let profile = *profiles.get(&edge).unwrap();
            let sample_match =
                matches!(mutation, ScheduleMutationV1::ThreeSampleMatch(value) if value == edge);
            let (initial, coefficients) = match profile {
                TestProfileV1::Constant(bits) => (bits, Vec::new()),
                TestProfileV1::Collective | TestProfileV1::OtherNonconstant => {
                    let coefficients = if sample_match {
                        vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 59,
                                denominator: 4,
                            },
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 1,
                                denominator: 4,
                            },
                        ]
                    } else {
                        vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: if profile == TestProfileV1::OtherNonconstant {
                                    14
                                } else {
                                    15
                                },
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                        ]
                    };
                    (60.0_f64.to_bits(), coefficients)
                }
            };
            CycleScheduleEntryInputV1 {
                edge,
                initial_angle_degrees_bits: initial,
                chebyshev_coefficients: coefficients,
            }
        })
        .collect::<Vec<_>>();
    let max_work = entries
        .iter()
        .map(|entry| entry.chebyshev_coefficients.len())
        .sum();
    CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        [0.0, 1.0],
        entries,
        CycleScheduleLimitsV1 {
            max_hinges: fixture.geometry.hinges().len(),
            max_degree: 3,
            max_coefficient_bits: 63,
            max_work,
        },
    )
    .unwrap()
}
