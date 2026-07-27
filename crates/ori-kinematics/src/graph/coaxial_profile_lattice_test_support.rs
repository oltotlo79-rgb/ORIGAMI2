use std::collections::{HashMap, HashSet};

use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, HalfAngleRationalEntryInputV1, Point3,
    RationalCoefficientV1, TreeHinge, TreeKinematicsLimits,
};

pub(super) fn face_v1(id: FaceId) -> Face {
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

pub(super) fn topology_v1(faces: &[FaceId], hinges: &[TreeHinge]) -> TopologySnapshot {
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

pub(super) struct CoaxialLatticeFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) moving_edges: Vec<EdgeId>,
    pub(super) constant_angles: Vec<(EdgeId, u64)>,
    pub(super) zero_edges: Vec<EdgeId>,
    pub(super) groups: Vec<Vec<FaceId>>,
}

pub(super) struct CoaxialLatticeFixturePartsV1 {
    pub(super) fixed_face: FaceId,
    pub(super) moving_edges: Vec<EdgeId>,
    pub(super) constant_angles: Vec<(EdgeId, u64)>,
    pub(super) zero_edges: Vec<EdgeId>,
    pub(super) groups: Vec<Vec<FaceId>>,
}

pub(super) fn rebuild_fixture_v1(
    mut faces: Vec<FaceId>,
    mut hinges: Vec<TreeHinge>,
    parts: CoaxialLatticeFixturePartsV1,
    reverse_storage: bool,
) -> CoaxialLatticeFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    CoaxialLatticeFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        moving_edges: parts.moving_edges,
        constant_angles: parts.constant_angles,
        zero_edges: parts.zero_edges,
        groups: parts.groups,
    }
}

#[derive(Clone, Copy)]
enum FixtureProfileV1 {
    Moving,
    Constant(u64),
    Zero,
}

struct FixtureHingeInputV1 {
    name: String,
    profile: FixtureProfileV1,
    assignment: FoldAssignment,
    left: FaceId,
    right: FaceId,
    start: Point3,
    end: Point3,
    axis: Point3,
}

struct FixtureBuilderV1 {
    namespace: ProjectId,
    hinges: Vec<TreeHinge>,
    moving_edges: Vec<EdgeId>,
    constant_angles: Vec<(EdgeId, u64)>,
    zero_edges: Vec<EdgeId>,
    reverse_every_other: bool,
}

impl FixtureBuilderV1 {
    fn push(&mut self, input: FixtureHingeInputV1) {
        let edge = EdgeId::derive_v5(self.namespace, input.name.as_bytes());
        let (left, right, start, end, axis) =
            if self.reverse_every_other && self.hinges.len() % 2 == 1 {
                (
                    input.right,
                    input.left,
                    input.end,
                    input.start,
                    Point3::new(-input.axis.x(), -input.axis.y(), -input.axis.z()).unwrap(),
                )
            } else {
                (input.left, input.right, input.start, input.end, input.axis)
            };
        self.hinges.push(TreeHinge::new_for_test(
            edge,
            input.assignment,
            left,
            right,
            start,
            end,
            axis,
        ));
        match input.profile {
            FixtureProfileV1::Moving => self.moving_edges.push(edge),
            FixtureProfileV1::Constant(bits) => self.constant_angles.push((edge, bits)),
            FixtureProfileV1::Zero => self.zero_edges.push(edge),
        }
    }
}

pub(super) fn coaxial_cube_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
) -> CoaxialLatticeFixtureV1 {
    let namespace = ProjectId::new();
    let groups = (0..8)
        .map(|state| {
            (0..2)
                .map(|copy| {
                    FaceId::derive_v5(
                        namespace,
                        format!("coaxial-cube-face:{state}:{copy}").as_bytes(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let faces = groups.iter().flatten().copied().collect::<Vec<_>>();
    let mut builder = FixtureBuilderV1 {
        namespace,
        hinges: Vec::new(),
        moving_edges: Vec::new(),
        constant_angles: Vec::new(),
        zero_edges: Vec::new(),
        reverse_every_other,
    };
    for (state, group) in groups.iter().enumerate() {
        let coordinate = 1_000.0 + state as f64 * 2.0;
        builder.push(FixtureHingeInputV1 {
            name: format!("coaxial-zero:{state}"),
            profile: FixtureProfileV1::Zero,
            assignment: FoldAssignment::Mountain,
            left: group[0],
            right: group[1],
            start: Point3::new(coordinate, 10.0, 0.0).unwrap(),
            end: Point3::new(coordinate + 1.0, 10.0, 0.0).unwrap(),
            axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
        });
    }
    for state in 0usize..8 {
        for dimension in 0usize..3 {
            if state & (1usize << dimension) != 0 {
                continue;
            }
            let next = state | (1usize << dimension);
            for left in 0..groups[state].len() {
                for right in 0..groups[next].len() {
                    let along = builder.hinges.len() as f64;
                    builder.push(FixtureHingeInputV1 {
                        name: format!("coaxial-edge:{state}:{dimension}:{left}:{right}"),
                        profile: match dimension {
                            0 => FixtureProfileV1::Moving,
                            1 => FixtureProfileV1::Constant(30.0_f64.to_bits()),
                            _ => FixtureProfileV1::Constant(45.0_f64.to_bits()),
                        },
                        assignment: FoldAssignment::Mountain,
                        left: groups[state][left],
                        right: groups[next][right],
                        start: Point3::new(along, 0.0, 0.0).unwrap(),
                        end: Point3::new(along + 1.0, 0.0, 0.0).unwrap(),
                        axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
                    });
                }
            }
        }
    }
    rebuild_fixture_v1(
        faces,
        builder.hinges,
        CoaxialLatticeFixturePartsV1 {
            fixed_face: groups[0][0],
            moving_edges: builder.moving_edges,
            constant_angles: builder.constant_angles,
            zero_edges: builder.zero_edges,
            groups,
        },
        reverse_storage,
    )
}

#[derive(Clone, Copy)]
pub(super) enum ScheduleMutationV1 {
    None,
    SecondNonconstant(EdgeId),
    ThreeSampleMatch(EdgeId),
    ConstantAngle(EdgeId, u64),
}

pub(super) fn polynomial_schedule_v1(
    fixture: &CoaxialLatticeFixtureV1,
    mutation: ScheduleMutationV1,
) -> CanonicalCycleScheduleV1 {
    let moving = fixture.moving_edges.iter().copied().collect::<HashSet<_>>();
    let constants = fixture
        .constant_angles
        .iter()
        .copied()
        .collect::<HashMap<_, _>>();
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
            let moving_edge = moving.contains(&edge);
            let second_profile =
                matches!(mutation, ScheduleMutationV1::SecondNonconstant(candidate) if candidate == edge);
            let sample_match =
                matches!(mutation, ScheduleMutationV1::ThreeSampleMatch(candidate) if candidate == edge);
            let constant_override =
                if let ScheduleMutationV1::ConstantAngle(candidate, bits) = mutation {
                    (candidate == edge).then_some(bits)
                } else {
                    None
                };
            let coefficients = if moving_edge {
                if sample_match {
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
                            numerator: if second_profile { 14 } else { 15 },
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
                }
            } else {
                Vec::new()
            };
            CycleScheduleEntryInputV1 {
                edge,
                initial_angle_degrees_bits: constant_override.unwrap_or_else(|| {
                    if moving_edge {
                        60.0_f64.to_bits()
                    } else {
                        constants.get(&edge).copied().unwrap_or(0.0_f64.to_bits())
                    }
                }),
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

pub(super) fn half_angle_schedule_v1(
    fixture: &CoaxialLatticeFixtureV1,
) -> CanonicalCycleScheduleV1 {
    let moving = fixture.moving_edges.iter().copied().collect::<HashSet<_>>();
    let constants = fixture
        .constant_angles
        .iter()
        .copied()
        .collect::<HashMap<_, _>>();
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
            let (numerator, denominator) = if moving.contains(&edge) {
                (
                    vec![
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    4,
                )
            } else if constants.get(&edge).copied() == Some(30.0_f64.to_bits()) {
                (
                    vec![RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    }],
                    3,
                )
            } else if constants.contains_key(&edge) {
                (
                    vec![RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    }],
                    2,
                )
            } else {
                (
                    vec![RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    }],
                    1,
                )
            };
            HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: numerator,
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: denominator,
                    denominator: 1,
                }],
            }
        })
        .collect::<Vec<_>>();
    CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        entries,
        CycleScheduleLimitsV1 {
            max_hinges: fixture.geometry.hinges().len(),
            max_degree: 1,
            max_coefficient_bits: 128,
            max_work: 16_384,
        },
    )
    .unwrap()
}

pub(super) fn replace_hinge_v1(
    fixture: CoaxialLatticeFixtureV1,
    edge: EdgeId,
    replacement: impl FnOnce(&TreeHinge) -> TreeHinge,
) -> CoaxialLatticeFixtureV1 {
    let mut replacement = Some(replacement);
    let hinges = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            if hinge.edge() == edge {
                replacement.take().unwrap()(hinge)
            } else {
                hinge.clone()
            }
        })
        .collect::<Vec<_>>();
    rebuild_fixture_v1(
        fixture.geometry.face_ids().to_vec(),
        hinges,
        CoaxialLatticeFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            moving_edges: fixture.moving_edges,
            constant_angles: fixture.constant_angles,
            zero_edges: fixture.zero_edges,
            groups: fixture.groups,
        },
        false,
    )
}
