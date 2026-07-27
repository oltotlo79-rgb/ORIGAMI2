use std::collections::HashMap;

use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, HalfAngleRationalEntryInputV1, Point3,
    RationalCoefficientV1, TreeHinge, TreeKinematicsLimits,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TestProfileV1 {
    Collective,
    OtherNonconstant,
    Constant(u64),
}

pub(super) struct FreeWordFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cyclic_blocks: Vec<Vec<EdgeId>>,
    pub(super) bridge_edges: Vec<EdgeId>,
    pub(super) zero_edges: Vec<EdgeId>,
}

pub(super) struct FreeWordFixturePartsV1 {
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cyclic_blocks: Vec<Vec<EdgeId>>,
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
    parts: FreeWordFixturePartsV1,
    reverse_storage: bool,
) -> FreeWordFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    FreeWordFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        profiles: parts.profiles,
        cyclic_blocks: parts.cyclic_blocks,
        bridge_edges: parts.bridge_edges,
        zero_edges: parts.zero_edges,
    }
}

struct HingeInputV1 {
    name: String,
    profile: TestProfileV1,
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
    profiles: Vec<(EdgeId, TestProfileV1)>,
    reverse_every_other: bool,
}

impl FixtureBuilderV1 {
    fn push(&mut self, input: HingeInputV1) -> EdgeId {
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

fn line_v1(carrier: usize, inverse_slot: bool, offset: f64) -> (Point3, Point3, Point3) {
    let along = if inverse_slot { 4.0 } else { 0.0 };
    match carrier {
        0 => (
            Point3::new(along, offset, 0.0).unwrap(),
            Point3::new(along + 1.0, offset, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ),
        1 => (
            Point3::new(offset + 10.0, along, 0.0).unwrap(),
            Point3::new(offset + 10.0, along + 1.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
        ),
        2 => (
            Point3::new(offset + 20.0, 0.0, along).unwrap(),
            Point3::new(offset + 20.0, 0.0, along + 1.0).unwrap(),
            Point3::new(0.0, 0.0, 1.0).unwrap(),
        ),
        _ => (
            Point3::new(along, offset + 30.0, 5.0).unwrap(),
            Point3::new(along + 1.0, offset + 30.0, 5.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ),
    }
}

fn push_word_square_v1(
    builder: &mut FixtureBuilderV1,
    name: &str,
    faces: [FaceId; 4],
    carriers: [usize; 2],
    profiles: [TestProfileV1; 2],
    commutator: bool,
    offset: f64,
) -> Vec<EdgeId> {
    let carrier_order = if commutator {
        [carriers[0], carriers[1], carriers[0], carriers[1]]
    } else {
        [carriers[0], carriers[1], carriers[1], carriers[0]]
    };
    let profile_order = if commutator {
        [profiles[0], profiles[1], profiles[0], profiles[1]]
    } else {
        [profiles[0], profiles[1], profiles[1], profiles[0]]
    };
    (0..4)
        .map(|index| {
            let (start, end, axis) = line_v1(carrier_order[index], index >= 2, offset);
            builder.push(HingeInputV1 {
                name: format!("{name}:{index}"),
                profile: profile_order[index],
                assignment: if index < 2 {
                    FoldAssignment::Mountain
                } else {
                    FoldAssignment::Valley
                },
                left: faces[index],
                right: faces[(index + 1) % 4],
                start,
                end,
                axis,
            })
        })
        .collect()
}

pub(super) fn two_block_free_word_fixture_v1(
    reverse_every_other: bool,
    reverse_storage: bool,
    commutator: bool,
    collective_cycles: bool,
    distinct_nonconstant_bridges: bool,
) -> FreeWordFixtureV1 {
    let namespace = ProjectId::new();
    let center = FaceId::derive_v5(namespace, b"block-word-center");
    let x_faces = [
        center,
        FaceId::derive_v5(namespace, b"block-word-x1"),
        FaceId::derive_v5(namespace, b"block-word-x2"),
        FaceId::derive_v5(namespace, b"block-word-x3"),
    ];
    let y_faces = [
        center,
        FaceId::derive_v5(namespace, b"block-word-y1"),
        FaceId::derive_v5(namespace, b"block-word-y2"),
        FaceId::derive_v5(namespace, b"block-word-y3"),
    ];
    let bridge_first = FaceId::derive_v5(namespace, b"block-word-bridge1");
    let bridge_second = FaceId::derive_v5(namespace, b"block-word-bridge2");
    let faces = x_faces
        .into_iter()
        .chain(y_faces.into_iter().skip(1))
        .chain([bridge_first, bridge_second])
        .collect::<Vec<_>>();
    let mut builder = FixtureBuilderV1 {
        namespace,
        hinges: Vec::new(),
        profiles: Vec::new(),
        reverse_every_other,
    };
    let first_profile = if collective_cycles {
        TestProfileV1::Collective
    } else {
        TestProfileV1::Constant(30.0_f64.to_bits())
    };
    let x_block = push_word_square_v1(
        &mut builder,
        "block-word-x",
        x_faces,
        [0, 1],
        [first_profile, TestProfileV1::Constant(45.0_f64.to_bits())],
        commutator,
        0.0,
    );
    let y_block = push_word_square_v1(
        &mut builder,
        "block-word-y",
        y_faces,
        [2, 3],
        [first_profile, TestProfileV1::Constant(45.0_f64.to_bits())],
        commutator,
        100.0,
    );
    let bridge_edges = vec![
        builder.push(HingeInputV1 {
            name: "block-word-bridge:first".into(),
            profile: if distinct_nonconstant_bridges {
                TestProfileV1::Collective
            } else {
                TestProfileV1::Constant(73.0_f64.to_bits())
            },
            assignment: FoldAssignment::Mountain,
            left: center,
            right: bridge_first,
            start: Point3::new(0.0, 200.0, 0.0).unwrap(),
            end: Point3::new(0.0, 200.0, 1.0).unwrap(),
            axis: Point3::new(0.0, 0.0, 1.0).unwrap(),
        }),
        builder.push(HingeInputV1 {
            name: "block-word-bridge:second".into(),
            profile: if distinct_nonconstant_bridges {
                TestProfileV1::OtherNonconstant
            } else {
                TestProfileV1::Constant(90.0_f64.to_bits())
            },
            assignment: FoldAssignment::Valley,
            left: bridge_first,
            right: bridge_second,
            start: Point3::new(300.0, 0.0, 0.0).unwrap(),
            end: Point3::new(301.0, 0.0, 0.0).unwrap(),
            axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
        }),
    ];
    rebuild_fixture_v1(
        faces,
        builder.hinges,
        FreeWordFixturePartsV1 {
            fixed_face: center,
            profiles: builder.profiles,
            cyclic_blocks: vec![x_block, y_block],
            bridge_edges,
            zero_edges: Vec::new(),
        },
        reverse_storage,
    )
}

pub(super) fn parallel_cut_fixture_v1(correct_sign: bool) -> FreeWordFixtureV1 {
    let namespace = ProjectId::new();
    let faces = (0..4)
        .map(|index| {
            FaceId::derive_v5(namespace, format!("block-word-cut-face:{index}").as_bytes())
        })
        .collect::<Vec<_>>();
    let mut builder = FixtureBuilderV1 {
        namespace,
        hinges: Vec::new(),
        profiles: Vec::new(),
        reverse_every_other: false,
    };
    let zero_edges = vec![
        builder.push(HingeInputV1 {
            name: "block-word-cut-zero-a".into(),
            profile: TestProfileV1::Constant(0.0_f64.to_bits()),
            assignment: FoldAssignment::Mountain,
            left: faces[0],
            right: faces[1],
            start: Point3::new(0.0, 10.0, 0.0).unwrap(),
            end: Point3::new(1.0, 10.0, 0.0).unwrap(),
            axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
        }),
        builder.push(HingeInputV1 {
            name: "block-word-cut-zero-b".into(),
            profile: TestProfileV1::Constant((-0.0_f64).to_bits()),
            assignment: FoldAssignment::Valley,
            left: faces[2],
            right: faces[3],
            start: Point3::new(2.0, 10.0, 0.0).unwrap(),
            end: Point3::new(3.0, 10.0, 0.0).unwrap(),
            axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
        }),
    ];
    let moving = vec![
        builder.push(HingeInputV1 {
            name: "block-word-cut-moving-a".into(),
            profile: TestProfileV1::Collective,
            assignment: FoldAssignment::Mountain,
            left: faces[0],
            right: faces[2],
            start: Point3::new(0.0, 0.0, 0.0).unwrap(),
            end: Point3::new(1.0, 0.0, 0.0).unwrap(),
            axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
        }),
        builder.push(HingeInputV1 {
            name: "block-word-cut-moving-b".into(),
            profile: TestProfileV1::Collective,
            assignment: if correct_sign {
                FoldAssignment::Valley
            } else {
                FoldAssignment::Mountain
            },
            left: faces[3],
            right: faces[1],
            start: Point3::new(4.0, 0.0, 0.0).unwrap(),
            end: Point3::new(5.0, 0.0, 0.0).unwrap(),
            axis: Point3::new(1.0, 0.0, 0.0).unwrap(),
        }),
    ];
    let fixed_face = faces[0];
    rebuild_fixture_v1(
        faces,
        builder.hinges,
        FreeWordFixturePartsV1 {
            fixed_face,
            profiles: builder.profiles,
            cyclic_blocks: vec![moving],
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
    fixture: &FreeWordFixtureV1,
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
                                numerator: if second
                                    || profile == TestProfileV1::OtherNonconstant
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

pub(super) fn half_angle_schedule_v1(fixture: &FreeWordFixtureV1) -> CanonicalCycleScheduleV1 {
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
            let (numerator, denominator) = match *profiles.get(&edge).unwrap() {
                TestProfileV1::Collective => (vec![0, 1], vec![4]),
                TestProfileV1::OtherNonconstant => (vec![0, 1], vec![5]),
                TestProfileV1::Constant(bits) if f64::from_bits(bits) == 0.0 => (vec![0], vec![1]),
                TestProfileV1::Constant(bits) if bits == 30.0_f64.to_bits() => (vec![1], vec![3]),
                TestProfileV1::Constant(bits) if bits == 45.0_f64.to_bits() => (vec![1], vec![2]),
                TestProfileV1::Constant(_) => (vec![2], vec![3]),
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
                    .map(|numerator| RationalCoefficientV1 {
                        numerator,
                        denominator: 1,
                    })
                    .collect(),
                denominator_power_coefficients: denominator
                    .into_iter()
                    .map(|numerator| RationalCoefficientV1 {
                        numerator,
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
