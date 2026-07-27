use std::collections::HashSet;

use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, HalfAngleRationalEntryInputV1,
    RationalCoefficientV1, TreeKinematicsLimits,
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

pub(super) struct ExactGeneratorWordFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) moving_edges: Vec<EdgeId>,
    pub(super) constant_edges: Vec<EdgeId>,
    pub(super) zero_edges: Vec<EdgeId>,
    pub(super) groups: Vec<Vec<FaceId>>,
}

pub(super) struct ExactGeneratorWordFixturePartsV1 {
    pub(super) fixed_face: FaceId,
    pub(super) moving_edges: Vec<EdgeId>,
    pub(super) constant_edges: Vec<EdgeId>,
    pub(super) zero_edges: Vec<EdgeId>,
    pub(super) groups: Vec<Vec<FaceId>>,
}

pub(super) fn rebuild_fixture_v1(
    mut faces: Vec<FaceId>,
    mut hinges: Vec<TreeHinge>,
    parts: ExactGeneratorWordFixturePartsV1,
    reverse_storage: bool,
) -> ExactGeneratorWordFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    ExactGeneratorWordFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        moving_edges: parts.moving_edges,
        constant_edges: parts.constant_edges,
        zero_edges: parts.zero_edges,
        groups: parts.groups,
    }
}

#[derive(Clone, Copy)]
enum FixtureProfileV1 {
    Moving,
    Constant,
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
    constant_edges: Vec<EdgeId>,
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
            FixtureProfileV1::Constant => self.constant_edges.push(edge),
            FixtureProfileV1::Zero => self.zero_edges.push(edge),
        }
    }
}

pub(super) fn generator_word_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
) -> ExactGeneratorWordFixtureV1 {
    let namespace = ProjectId::new();
    let groups = (0..4)
        .map(|group| {
            (0..3)
                .map(|face| {
                    FaceId::derive_v5(
                        namespace,
                        format!("generator-word-face:{group}:{face}").as_bytes(),
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
        constant_edges: Vec::new(),
        zero_edges: Vec::new(),
        reverse_every_other,
    };

    for (group_index, group) in groups.iter().enumerate() {
        for left in 0..group.len() {
            for right in left + 1..group.len() {
                let coordinate = 100.0 + builder.hinges.len() as f64 * 2.0;
                builder.push(FixtureHingeInputV1 {
                    name: format!("zero:{group_index}:{left}:{right}"),
                    profile: FixtureProfileV1::Zero,
                    assignment: FoldAssignment::Mountain,
                    left: group[left],
                    right: group[right],
                    start: Point3::new(coordinate, 10.0, 0.0).unwrap(),
                    end: Point3::new(coordinate + 1.0, 10.0, 0.0).unwrap(),
                    axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
                });
            }
        }
    }

    for left in 0..groups[0].len() {
        for right in 0..groups[1].len() {
            let along = (left * groups[1].len() + right) as f64;
            builder.push(FixtureHingeInputV1 {
                name: format!("a:{left}:{right}"),
                profile: FixtureProfileV1::Moving,
                assignment: FoldAssignment::Mountain,
                left: groups[0][left],
                right: groups[1][right],
                start: Point3::new(along, 0.0, 0.0).unwrap(),
                end: Point3::new(along + 1.0, 0.0, 0.0).unwrap(),
                axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
            });
        }
    }
    for left in 0..groups[1].len() {
        for right in 0..groups[2].len() {
            let along = (left * groups[2].len() + right) as f64;
            builder.push(FixtureHingeInputV1 {
                name: format!("b:{left}:{right}"),
                profile: FixtureProfileV1::Moving,
                assignment: FoldAssignment::Mountain,
                left: groups[1][left],
                right: groups[2][right],
                start: Point3::new(4.0, along, 0.0).unwrap(),
                end: Point3::new(4.0, along + 1.0, 0.0).unwrap(),
                axis: Point3::new(0.0, 1.0, 0.0).unwrap(),
            });
        }
    }
    for left in 0..groups[2].len() {
        for right in 0..groups[3].len() {
            let along = (left * groups[3].len() + right) as f64;
            builder.push(FixtureHingeInputV1 {
                name: format!("c:{left}:{right}"),
                profile: FixtureProfileV1::Constant,
                assignment: FoldAssignment::Mountain,
                left: groups[2][left],
                right: groups[3][right],
                start: Point3::new(0.0, 6.0, along).unwrap(),
                end: Point3::new(0.0, 6.0, along + 1.0).unwrap(),
                axis: Point3::new(0.0, 0.0, 1.0).unwrap(),
            });
        }
    }

    rebuild_fixture_v1(
        faces,
        builder.hinges,
        ExactGeneratorWordFixturePartsV1 {
            fixed_face: groups[0][0],
            moving_edges: builder.moving_edges,
            constant_edges: builder.constant_edges,
            zero_edges: builder.zero_edges,
            groups,
        },
        reverse_storage,
    )
}

#[derive(Clone, Copy)]
pub(super) enum ScheduleMutationV1 {
    None,
    MovingProfile(EdgeId),
    ConstantAngle(EdgeId, u64),
}

pub(super) fn polynomial_schedule_v1(
    fixture: &ExactGeneratorWordFixtureV1,
    mutation: ScheduleMutationV1,
) -> CanonicalCycleScheduleV1 {
    let moving = fixture.moving_edges.iter().copied().collect::<HashSet<_>>();
    let constant = fixture
        .constant_edges
        .iter()
        .copied()
        .collect::<HashSet<_>>();
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
            let constant_edge = constant.contains(&edge);
            let mismatched =
                matches!(mutation, ScheduleMutationV1::MovingProfile(candidate) if candidate == edge);
            let constant_override =
                if let ScheduleMutationV1::ConstantAngle(candidate, bits) = mutation {
                    (candidate == edge).then_some(bits)
                } else {
                    None
                };
            CycleScheduleEntryInputV1 {
                edge,
                initial_angle_degrees_bits: constant_override.unwrap_or_else(|| {
                    if moving_edge {
                        60.0_f64.to_bits()
                    } else if constant_edge {
                        30.0_f64.to_bits()
                    } else {
                        (-0.0_f64).to_bits()
                    }
                }),
                chebyshev_coefficients: if moving_edge {
                    vec![
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: if mismatched { 14 } else { 15 },
                            denominator: 1,
                        },
                    ]
                } else {
                    Vec::new()
                },
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
            max_degree: 1,
            max_coefficient_bits: 63,
            max_work,
        },
    )
    .unwrap()
}

pub(super) fn half_angle_schedule_v1(
    fixture: &ExactGeneratorWordFixtureV1,
) -> CanonicalCycleScheduleV1 {
    let moving = fixture.moving_edges.iter().copied().collect::<HashSet<_>>();
    let constant = fixture
        .constant_edges
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut edges = fixture
        .geometry
        .hinges()
        .iter()
        .map(TreeHinge::edge)
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let entries = edges
        .into_iter()
        .map(|edge| HalfAngleRationalEntryInputV1 {
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
            numerator_power_coefficients: if moving.contains(&edge) {
                vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ]
            } else if constant.contains(&edge) {
                vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }]
            } else {
                vec![RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                }]
            },
            denominator_power_coefficients: vec![RationalCoefficientV1 {
                numerator: if moving.contains(&edge) { 4 } else { 3 },
                denominator: 1,
            }],
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
    fixture: ExactGeneratorWordFixtureV1,
    edge: EdgeId,
    replacement: impl FnOnce(&TreeHinge) -> TreeHinge,
) -> ExactGeneratorWordFixtureV1 {
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
        ExactGeneratorWordFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            moving_edges: fixture.moving_edges,
            constant_edges: fixture.constant_edges,
            zero_edges: fixture.zero_edges,
            groups: fixture.groups,
        },
        false,
    )
}
