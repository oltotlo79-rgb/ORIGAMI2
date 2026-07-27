use std::collections::HashMap;

use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, HalfAngleRationalEntryInputV1, Point3,
    RationalCoefficientV1, TreeHinge, TreeKinematicsLimits,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TestScheduleProfileV1 {
    Collective,
    OtherNonconstant,
    Constant(u64),
}

pub(super) struct BlockCutFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestScheduleProfileV1)>,
    pub(super) x_block_edges: Vec<EdgeId>,
    pub(super) y_block_edges: Vec<EdgeId>,
    pub(super) bridge_edges: Vec<EdgeId>,
    pub(super) zero_edges: Vec<EdgeId>,
}

pub(super) struct BlockCutFixturePartsV1 {
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestScheduleProfileV1)>,
    pub(super) x_block_edges: Vec<EdgeId>,
    pub(super) y_block_edges: Vec<EdgeId>,
    pub(super) bridge_edges: Vec<EdgeId>,
    pub(super) zero_edges: Vec<EdgeId>,
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
    parts: BlockCutFixturePartsV1,
    reverse_storage: bool,
) -> BlockCutFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    BlockCutFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        profiles: parts.profiles,
        x_block_edges: parts.x_block_edges,
        y_block_edges: parts.y_block_edges,
        bridge_edges: parts.bridge_edges,
        zero_edges: parts.zero_edges,
    }
}

struct TestHingeInputV1 {
    name: String,
    profile: TestScheduleProfileV1,
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
    profiles: Vec<(EdgeId, TestScheduleProfileV1)>,
    reverse_every_other: bool,
}

impl FixtureBuilderV1 {
    fn push(&mut self, input: TestHingeInputV1) -> EdgeId {
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
        self.profiles.push((edge, input.profile));
        edge
    }
}

fn push_square_v1(
    builder: &mut FixtureBuilderV1,
    name: &str,
    faces: [FaceId; 4],
    first_profile: TestScheduleProfileV1,
    second_profile: TestScheduleProfileV1,
    x_carrier: bool,
    carrier_offset: f64,
) -> Vec<EdgeId> {
    let assignments = [
        FoldAssignment::Mountain,
        FoldAssignment::Mountain,
        FoldAssignment::Valley,
        FoldAssignment::Valley,
    ];
    let profiles = [first_profile, second_profile, first_profile, second_profile];
    (0..4)
        .map(|index| {
            let along = index as f64 * 2.0;
            let (start, end, axis) = if x_carrier {
                (
                    Point3::new(along, carrier_offset, 0.0).unwrap(),
                    Point3::new(along + 1.0, carrier_offset, 0.0).unwrap(),
                    Point3::new(1.0, 0.0, 0.0).unwrap(),
                )
            } else {
                (
                    Point3::new(carrier_offset, along, 0.0).unwrap(),
                    Point3::new(carrier_offset, along + 1.0, 0.0).unwrap(),
                    Point3::new(0.0, 1.0, 0.0).unwrap(),
                )
            };
            builder.push(TestHingeInputV1 {
                name: format!("{name}:{index}"),
                profile: profiles[index],
                assignment: assignments[index],
                left: faces[index],
                right: faces[(index + 1) % 4],
                start,
                end,
                axis,
            })
        })
        .collect()
}

pub(super) fn two_block_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
    constant_only_cycles: bool,
    distinct_nonconstant_bridges: bool,
) -> BlockCutFixtureV1 {
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"block-cut-center");
    let x_faces = [
        center,
        FaceId::derive_v5(namespace, b"block-cut-x-1"),
        FaceId::derive_v5(namespace, b"block-cut-x-2"),
        FaceId::derive_v5(namespace, b"block-cut-x-3"),
    ];
    let y_faces = [
        center,
        FaceId::derive_v5(namespace, b"block-cut-y-1"),
        FaceId::derive_v5(namespace, b"block-cut-y-2"),
        FaceId::derive_v5(namespace, b"block-cut-y-3"),
    ];
    let bridge_first = FaceId::derive_v5(namespace, b"block-cut-bridge-1");
    let bridge_second = FaceId::derive_v5(namespace, b"block-cut-bridge-2");
    let mut faces = x_faces
        .into_iter()
        .chain(y_faces.into_iter().skip(1))
        .chain([bridge_first])
        .collect::<Vec<_>>();
    if distinct_nonconstant_bridges {
        faces.push(bridge_second);
    }
    let mut builder = FixtureBuilderV1 {
        namespace,
        hinges: Vec::new(),
        profiles: Vec::new(),
        reverse_every_other,
    };
    let x_first = if constant_only_cycles {
        TestScheduleProfileV1::Constant(20.0_f64.to_bits())
    } else {
        TestScheduleProfileV1::Collective
    };
    let y_first = if constant_only_cycles {
        TestScheduleProfileV1::Constant(25.0_f64.to_bits())
    } else {
        TestScheduleProfileV1::Collective
    };
    let x_block_edges = push_square_v1(
        &mut builder,
        "block-cut-x",
        x_faces,
        x_first,
        TestScheduleProfileV1::Constant(30.0_f64.to_bits()),
        true,
        0.0,
    );
    let y_block_edges = push_square_v1(
        &mut builder,
        "block-cut-y",
        y_faces,
        y_first,
        TestScheduleProfileV1::Constant(45.0_f64.to_bits()),
        false,
        10.0,
    );
    let mut bridge_edges = vec![builder.push(TestHingeInputV1 {
        name: "block-cut-bridge:first".into(),
        profile: if distinct_nonconstant_bridges {
            TestScheduleProfileV1::Collective
        } else {
            TestScheduleProfileV1::Constant(73.0_f64.to_bits())
        },
        assignment: FoldAssignment::Mountain,
        left: center,
        right: bridge_first,
        start: Point3::new(20.0, 20.0, 0.0).unwrap(),
        end: Point3::new(20.0, 20.0, 1.0).unwrap(),
        axis: Point3::new(0.0, 0.0, 1.0).unwrap(),
    })];
    if distinct_nonconstant_bridges {
        bridge_edges.push(builder.push(TestHingeInputV1 {
            name: "block-cut-bridge:second".into(),
            profile: TestScheduleProfileV1::OtherNonconstant,
            assignment: FoldAssignment::Valley,
            left: bridge_first,
            right: bridge_second,
            start: Point3::new(30.0, 0.0, 0.0).unwrap(),
            end: Point3::new(31.0, 0.0, 0.0).unwrap(),
            axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
        }));
    }
    rebuild_fixture_v1(
        faces,
        builder.hinges,
        BlockCutFixturePartsV1 {
            fixed_face: center,
            profiles: builder.profiles,
            x_block_edges,
            y_block_edges,
            bridge_edges,
            zero_edges: Vec::new(),
        },
        reverse_storage,
    )
}

pub(super) fn exact_cut_fixture_v1(correct_reverse_sign: bool) -> BlockCutFixtureV1 {
    let namespace = ProjectId::new();
    let a0 = FaceId::derive_v5(namespace, b"block-cut-cut-a0");
    let a1 = FaceId::derive_v5(namespace, b"block-cut-cut-a1");
    let b0 = FaceId::derive_v5(namespace, b"block-cut-cut-b0");
    let b1 = FaceId::derive_v5(namespace, b"block-cut-cut-b1");
    let mut builder = FixtureBuilderV1 {
        namespace,
        hinges: Vec::new(),
        profiles: Vec::new(),
        reverse_every_other: false,
    };
    let zero_a = builder.push(TestHingeInputV1 {
        name: "block-cut-cut-zero-a".into(),
        profile: TestScheduleProfileV1::Constant(0.0_f64.to_bits()),
        assignment: FoldAssignment::Mountain,
        left: a0,
        right: a1,
        start: Point3::new(0.0, 10.0, 0.0).unwrap(),
        end: Point3::new(1.0, 10.0, 0.0).unwrap(),
        axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
    });
    let zero_b = builder.push(TestHingeInputV1 {
        name: "block-cut-cut-zero-b".into(),
        profile: TestScheduleProfileV1::Constant((-0.0_f64).to_bits()),
        assignment: FoldAssignment::Valley,
        left: b0,
        right: b1,
        start: Point3::new(2.0, 10.0, 0.0).unwrap(),
        end: Point3::new(3.0, 10.0, 0.0).unwrap(),
        axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
    });
    let first = builder.push(TestHingeInputV1 {
        name: "block-cut-cut-moving-first".into(),
        profile: TestScheduleProfileV1::Collective,
        assignment: FoldAssignment::Mountain,
        left: a0,
        right: b0,
        start: Point3::new(0.0, 0.0, 0.0).unwrap(),
        end: Point3::new(1.0, 0.0, 0.0).unwrap(),
        axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
    });
    let second = builder.push(TestHingeInputV1 {
        name: "block-cut-cut-moving-second".into(),
        profile: TestScheduleProfileV1::Collective,
        assignment: if correct_reverse_sign {
            FoldAssignment::Valley
        } else {
            FoldAssignment::Mountain
        },
        left: b1,
        right: a1,
        start: Point3::new(4.0, 0.0, 0.0).unwrap(),
        end: Point3::new(5.0, 0.0, 0.0).unwrap(),
        axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
    });
    rebuild_fixture_v1(
        vec![a0, a1, b0, b1],
        builder.hinges,
        BlockCutFixturePartsV1 {
            fixed_face: a0,
            profiles: builder.profiles,
            x_block_edges: vec![first, second],
            y_block_edges: Vec::new(),
            bridge_edges: Vec::new(),
            zero_edges: vec![zero_a, zero_b],
        },
        false,
    )
}

pub(super) fn contracted_self_loop_fixture_v1() -> BlockCutFixtureV1 {
    let namespace = ProjectId::new();
    let faces = (0..3)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("block-cut-self-loop-face:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let mut builder = FixtureBuilderV1 {
        namespace,
        hinges: Vec::new(),
        profiles: Vec::new(),
        reverse_every_other: false,
    };
    let zero_edges = (0..2)
        .map(|index| {
            builder.push(TestHingeInputV1 {
                name: format!("block-cut-self-loop-zero:{index}"),
                profile: TestScheduleProfileV1::Constant(0.0_f64.to_bits()),
                assignment: FoldAssignment::Mountain,
                left: faces[index],
                right: faces[index + 1],
                start: Point3::new(index as f64, 10.0, 0.0).unwrap(),
                end: Point3::new(index as f64 + 1.0, 10.0, 0.0).unwrap(),
                axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
            })
        })
        .collect::<Vec<_>>();
    let active = builder.push(TestHingeInputV1 {
        name: "block-cut-self-loop-active".into(),
        profile: TestScheduleProfileV1::Constant(30.0_f64.to_bits()),
        assignment: FoldAssignment::Mountain,
        left: faces[0],
        right: faces[2],
        start: Point3::new(0.0, 0.0, 0.0).unwrap(),
        end: Point3::new(1.0, 0.0, 0.0).unwrap(),
        axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
    });
    rebuild_fixture_v1(
        faces.clone(),
        builder.hinges,
        BlockCutFixturePartsV1 {
            fixed_face: faces[0],
            profiles: builder.profiles,
            x_block_edges: vec![active],
            y_block_edges: Vec::new(),
            bridge_edges: Vec::new(),
            zero_edges,
        },
        false,
    )
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ScheduleMutationV1 {
    None,
    SecondNonconstant(EdgeId),
    ThreeSampleMatch(EdgeId),
}

pub(super) fn polynomial_schedule_v1(
    fixture: &BlockCutFixtureV1,
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
            let second =
                matches!(mutation, ScheduleMutationV1::SecondNonconstant(candidate) if candidate == edge);
            let sample_match =
                matches!(mutation, ScheduleMutationV1::ThreeSampleMatch(candidate) if candidate == edge);
            let (initial, coefficients) = match profile {
                TestScheduleProfileV1::Constant(bits) => (bits, Vec::new()),
                TestScheduleProfileV1::Collective | TestScheduleProfileV1::OtherNonconstant => {
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
                                numerator: if second
                                    || profile == TestScheduleProfileV1::OtherNonconstant
                                {
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

pub(super) fn half_angle_schedule_v1(fixture: &BlockCutFixtureV1) -> CanonicalCycleScheduleV1 {
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
            let (numerator, denominator) = match profile {
                TestScheduleProfileV1::Collective => (vec![0, 1], vec![4]),
                TestScheduleProfileV1::OtherNonconstant => (vec![0, 1], vec![5]),
                TestScheduleProfileV1::Constant(bits) if f64::from_bits(bits) == 0.0 => {
                    (vec![0], vec![1])
                }
                TestScheduleProfileV1::Constant(bits) if bits == 20.0_f64.to_bits() => {
                    (vec![1], vec![5])
                }
                TestScheduleProfileV1::Constant(bits) if bits == 25.0_f64.to_bits() => {
                    (vec![1], vec![4])
                }
                TestScheduleProfileV1::Constant(bits) if bits == 30.0_f64.to_bits() => {
                    (vec![1], vec![3])
                }
                TestScheduleProfileV1::Constant(bits) if bits == 45.0_f64.to_bits() => {
                    (vec![1], vec![2])
                }
                TestScheduleProfileV1::Constant(_) => (vec![2], vec![3]),
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
                numerator_power_coefficients: numerator
                    .into_iter()
                    .map(|value| RationalCoefficientV1 {
                        numerator: value,
                        denominator: 1,
                    })
                    .collect(),
                denominator_power_coefficients: denominator
                    .into_iter()
                    .map(|value| RationalCoefficientV1 {
                        numerator: value,
                        denominator: 1,
                    })
                    .collect(),
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
    fixture: BlockCutFixtureV1,
    edge: EdgeId,
    replacement: impl FnOnce(&TreeHinge) -> TreeHinge,
) -> BlockCutFixtureV1 {
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
        BlockCutFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            profiles: fixture.profiles,
            x_block_edges: fixture.x_block_edges,
            y_block_edges: fixture.y_block_edges,
            bridge_edges: fixture.bridge_edges,
            zero_edges: fixture.zero_edges,
        },
        false,
    )
}
