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

pub(super) struct OrthogonalFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cycle_edges: Vec<EdgeId>,
    pub(super) cycle_faces: Vec<FaceId>,
    pub(super) bridge_edges: Vec<EdgeId>,
}

pub(super) struct OrthogonalFixturePartsV1 {
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cycle_edges: Vec<EdgeId>,
    pub(super) cycle_faces: Vec<FaceId>,
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
    parts: OrthogonalFixturePartsV1,
    reverse_storage: bool,
) -> OrthogonalFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    OrthogonalFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        profiles: parts.profiles,
        cycle_edges: parts.cycle_edges,
        cycle_faces: parts.cycle_faces,
        bridge_edges: parts.bridge_edges,
    }
}

pub(super) fn carrier_v1(axis_index: usize, slot: usize) -> (Point3, Point3, Point3) {
    let along = slot as f64 * 3.0;
    match axis_index {
        0 => (
            Point3::new(along, 0.0, 0.0).unwrap(),
            Point3::new(along + 1.0, 0.0, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ),
        1 => (
            Point3::new(0.0, along, 0.0).unwrap(),
            Point3::new(0.0, along + 1.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
        ),
        2 => (
            Point3::new(0.0, 0.0, along).unwrap(),
            Point3::new(0.0, 0.0, along + 1.0).unwrap(),
            Point3::new(0.0, 0.0, 1.0).unwrap(),
        ),
        _ => panic!("test carrier index must be 0..=2"),
    }
}

fn cycle_fixture_v1(
    label: &str,
    axes: &[usize],
    profiles_by_position: &[TestProfileV1],
    reverse_every_other: bool,
    reverse_storage: bool,
    fixed_face_index: usize,
    observer_tamper_bridge: bool,
) -> OrthogonalFixtureV1 {
    assert_eq!(axes.len(), profiles_by_position.len());
    let namespace = ProjectId::new();
    let cycle_faces = (0..axes.len())
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("{label}-orthogonal-face:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(axes.len() + usize::from(observer_tamper_bridge));
    let mut profiles = Vec::with_capacity(hinges.capacity());
    let mut cycle_edges = Vec::with_capacity(axes.len());
    for (index, (&axis_index, &profile)) in axes.iter().zip(profiles_by_position).enumerate() {
        let edge = EdgeId::derive_v5(
            namespace,
            format!("{label}-orthogonal-edge:{index}").as_bytes(),
        );
        let (mut left, mut right) = (
            cycle_faces[index],
            cycle_faces[(index + 1) % cycle_faces.len()],
        );
        let (mut start, mut end, mut axis) = carrier_v1(axis_index, index);
        if reverse_every_other && index.is_multiple_of(2) {
            std::mem::swap(&mut left, &mut right);
            std::mem::swap(&mut start, &mut end);
            axis = Point3::new(-axis.x(), -axis.y(), -axis.z()).unwrap();
        }
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            left,
            right,
            start,
            end,
            axis,
        ));
        profiles.push((edge, profile));
        cycle_edges.push(edge);
    }

    let mut all_faces = cycle_faces.clone();
    let mut bridge_edges = Vec::new();
    if observer_tamper_bridge {
        let leaf = FaceId::derive_v5(namespace, b"orthogonal-observer-bridge-face");
        let edge = EdgeId::derive_v5(namespace, b"orthogonal-observer-bridge-edge");
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            cycle_faces[0],
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
        OrthogonalFixturePartsV1 {
            fixed_face: cycle_faces[fixed_face_index],
            profiles,
            cycle_edges,
            cycle_faces,
            bridge_edges,
        },
        reverse_storage,
    )
}

pub(super) fn semidirect_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
    fixed_face_index: usize,
    observer_tamper_bridge: bool,
) -> OrthogonalFixtureV1 {
    cycle_fixture_v1(
        "semidirect",
        &[0, 1, 0, 2, 0],
        &[
            TestProfileV1::Collective,
            TestProfileV1::Constant(180.0_f64.to_bits()),
            TestProfileV1::Collective,
            TestProfileV1::Constant(180.0_f64.to_bits()),
            TestProfileV1::Constant(180.0_f64.to_bits()),
        ],
        reverse_every_other,
        reverse_storage,
        fixed_face_index,
        observer_tamper_bridge,
    )
}

pub(super) fn triangle_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
) -> OrthogonalFixtureV1 {
    cycle_fixture_v1(
        "triangle",
        &[0, 1, 2],
        &[TestProfileV1::Constant(180.0_f64.to_bits()); 3],
        reverse_every_other,
        reverse_storage,
        0,
        false,
    )
}

pub(super) fn half_turn_square_fixture_v1(carrier_count: usize) -> OrthogonalFixtureV1 {
    let axes = match carrier_count {
        1 => [0, 0, 0, 0],
        2 => [0, 0, 1, 1],
        _ => panic!("square fixture carrier count must be 1 or 2"),
    };
    cycle_fixture_v1(
        "half-turn-square",
        &axes,
        &[TestProfileV1::Constant(180.0_f64.to_bits()); 4],
        false,
        false,
        0,
        false,
    )
}

pub(super) fn replace_hinges_v1(
    fixture: OrthogonalFixtureV1,
    edges: &[EdgeId],
    replacement: impl Fn(&TreeHinge) -> TreeHinge,
) -> OrthogonalFixtureV1 {
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
        OrthogonalFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            profiles: fixture.profiles,
            cycle_edges: fixture.cycle_edges,
            cycle_faces: fixture.cycle_faces,
            bridge_edges: fixture.bridge_edges,
        },
        false,
    )
}

pub(super) fn replace_profile_v1(
    mut fixture: OrthogonalFixtureV1,
    edge: EdgeId,
    profile: TestProfileV1,
) -> OrthogonalFixtureV1 {
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
    CollectiveTouchesHalfTurn,
}

pub(super) fn polynomial_schedule_v1(
    fixture: &OrthogonalFixtureV1,
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
            let touches_half_turn =
                matches!(mutation, ScheduleMutationV1::CollectiveTouchesHalfTurn);
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
                    } else if touches_half_turn && profile == TestProfileV1::Collective {
                        vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 90,
                                denominator: 1,
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
                        ]
                    };
                    let initial = if touches_half_turn && profile == TestProfileV1::Collective {
                        90.0_f64.to_bits()
                    } else {
                        60.0_f64.to_bits()
                    };
                    (initial, coefficients)
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
