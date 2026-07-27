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
    SampledHalfTurn,
    Constant(u64),
}

pub(super) struct FiniteHalfTurnFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cycle_edges: Vec<EdgeId>,
    pub(super) cycle_faces: Vec<FaceId>,
    pub(super) bridge_edges: Vec<EdgeId>,
}

pub(super) struct FiniteHalfTurnFixturePartsV1 {
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
    parts: FiniteHalfTurnFixturePartsV1,
    reverse_storage: bool,
) -> FiniteHalfTurnFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    FiniteHalfTurnFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        profiles: parts.profiles,
        cycle_edges: parts.cycle_edges,
        cycle_faces: parts.cycle_faces,
        bridge_edges: parts.bridge_edges,
    }
}

pub(super) fn normalized_v1(raw: [f64; 3]) -> Point3 {
    let length = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt();
    Point3::new(raw[0] / length, raw[1] / length, raw[2] / length).unwrap()
}

pub(super) fn carrier_v1(
    carrier: usize,
    slot: usize,
    center: [f64; 3],
) -> (Point3, Point3, Point3) {
    let raw = match carrier {
        0 => [1.0, 1.0, 0.0],
        1 => [1.0, 0.0, 1.0],
        _ => panic!("D3 carrier index must be 0 or 1"),
    };
    let along = slot as f64 * 2.0;
    let start = Point3::new(
        center[0] + along * raw[0],
        center[1] + along * raw[1],
        center[2] + along * raw[2],
    )
    .unwrap();
    let end = Point3::new(start.x() + raw[0], start.y() + raw[1], start.z() + raw[2]).unwrap();
    (start, end, normalized_v1(raw))
}

pub(super) fn d3_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
    fixed_face_index: usize,
    moving_bridge: bool,
) -> FiniteHalfTurnFixtureV1 {
    let namespace = ProjectId::new();
    let cycle_faces = (0..6)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("finite-half-turn-d3-face:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(6 + usize::from(moving_bridge));
    let mut profiles = Vec::with_capacity(hinges.capacity());
    let mut cycle_edges = Vec::with_capacity(6);
    let center = [3.0, 5.0, 7.0];
    for index in 0..6 {
        let edge = EdgeId::derive_v5(
            namespace,
            format!("finite-half-turn-d3-edge:{index}").as_bytes(),
        );
        let (mut left, mut right) = (cycle_faces[index], cycle_faces[(index + 1) % 6]);
        let (mut start, mut end, mut axis) = carrier_v1(index % 2, index, center);
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
        profiles.push((edge, TestProfileV1::Constant(180.0_f64.to_bits())));
        cycle_edges.push(edge);
    }

    let mut faces = cycle_faces.clone();
    let mut bridge_edges = Vec::new();
    if moving_bridge {
        let leaf = FaceId::derive_v5(namespace, b"finite-half-turn-moving-bridge-face");
        let edge = EdgeId::derive_v5(namespace, b"finite-half-turn-moving-bridge-edge");
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            cycle_faces[0],
            leaf,
            Point3::new(30.0, 0.0, 0.0).unwrap(),
            Point3::new(30.0, 1.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
        ));
        profiles.push((edge, TestProfileV1::Collective));
        bridge_edges.push(edge);
        faces.push(leaf);
    }
    rebuild_fixture_v1(
        faces,
        hinges,
        FiniteHalfTurnFixturePartsV1 {
            fixed_face: cycle_faces[fixed_face_index],
            profiles,
            cycle_edges,
            cycle_faces,
            bridge_edges,
        },
        reverse_storage,
    )
}

pub(super) fn odd_half_turn_fixture_v1() -> FiniteHalfTurnFixtureV1 {
    let namespace = ProjectId::new();
    let faces = (0..3)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("finite-half-turn-odd-face:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::with_capacity(3);
    let mut profiles = Vec::with_capacity(3);
    let mut cycle_edges = Vec::with_capacity(3);
    for index in 0..3 {
        let edge = EdgeId::derive_v5(
            namespace,
            format!("finite-half-turn-odd-edge:{index}").as_bytes(),
        );
        let (start, end, axis) = carrier_v1(0, index, [0.0, 0.0, 0.0]);
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            faces[index],
            faces[(index + 1) % 3],
            start,
            end,
            axis,
        ));
        profiles.push((edge, TestProfileV1::Constant(180.0_f64.to_bits())));
        cycle_edges.push(edge);
    }
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        FiniteHalfTurnFixturePartsV1 {
            fixed_face: faces[0],
            profiles,
            cycle_edges,
            cycle_faces: faces,
            bridge_edges: Vec::new(),
        },
        false,
    )
}

pub(super) fn replace_hinges_v1(
    fixture: FiniteHalfTurnFixtureV1,
    edges: &[EdgeId],
    replacement: impl Fn(&TreeHinge) -> TreeHinge,
) -> FiniteHalfTurnFixtureV1 {
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
        FiniteHalfTurnFixturePartsV1 {
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
    mut fixture: FiniteHalfTurnFixtureV1,
    edge: EdgeId,
    profile: TestProfileV1,
) -> FiniteHalfTurnFixtureV1 {
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

pub(super) fn polynomial_schedule_v1(
    fixture: &FiniteHalfTurnFixtureV1,
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
            let (initial, coefficients) = match *profiles.get(&edge).unwrap() {
                TestProfileV1::Constant(bits) => (bits, Vec::new()),
                TestProfileV1::Collective | TestProfileV1::OtherNonconstant => (
                    60.0_f64.to_bits(),
                    vec![
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: if profiles.get(&edge)
                                == Some(&TestProfileV1::OtherNonconstant)
                            {
                                14
                            } else {
                                15
                            },
                            denominator: 1,
                        },
                    ],
                ),
                TestProfileV1::SampledHalfTurn => (
                    165.0_f64.to_bits(),
                    vec![
                        RationalCoefficientV1 {
                            numerator: 0,
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
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 15,
                            denominator: 1,
                        },
                    ],
                ),
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
            max_degree: 4,
            max_coefficient_bits: 63,
            max_work,
        },
    )
    .unwrap()
}
