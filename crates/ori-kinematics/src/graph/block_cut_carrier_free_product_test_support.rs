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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TestWordV1 {
    SameCarrierCommutatorThenInverse,
    DifferentCarrierCommutator,
}

pub(super) struct CarrierFreeProductFixtureV1 {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) profiles: Vec<(EdgeId, TestProfileV1)>,
    pub(super) cycle_edges: Vec<EdgeId>,
    pub(super) bridge_edges: Vec<EdgeId>,
}

pub(super) struct CarrierFreeProductFixturePartsV1 {
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
    parts: CarrierFreeProductFixturePartsV1,
    reverse_storage: bool,
) -> CarrierFreeProductFixtureV1 {
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if reverse_storage {
        hinges.reverse();
    }
    let source = topology_v1(&faces, &hinges);
    let audit = MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
    CarrierFreeProductFixtureV1 {
        geometry: MaterialHingeGraphGeometry::new_for_test(faces, hinges),
        audit,
        fixed_face: parts.fixed_face,
        profiles: parts.profiles,
        cycle_edges: parts.cycle_edges,
        bridge_edges: parts.bridge_edges,
    }
}

fn carrier_v1(carrier: usize, slot: usize) -> (Point3, Point3, Point3) {
    let along = slot as f64 * 3.0;
    match carrier {
        0 => (
            Point3::new(along, 0.0, 0.0).unwrap(),
            Point3::new(along + 1.0, 0.0, 0.0).unwrap(),
            Point3::new(1.0, 0.0, 0.0).unwrap(),
        ),
        1 => (
            Point3::new(10.0, along, 0.0).unwrap(),
            Point3::new(10.0, along + 1.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
        ),
        _ => (
            Point3::new(20.0, 0.0, along).unwrap(),
            Point3::new(20.0, 0.0, along + 1.0).unwrap(),
            Point3::new(0.0, 0.0, 1.0).unwrap(),
        ),
    }
}

pub(super) fn word_fixture_v1(
    word: TestWordV1,
    collective: bool,
    observer_tamper_bridge: bool,
    reverse_every_other: bool,
    reverse_storage: bool,
) -> CarrierFreeProductFixtureV1 {
    let namespace = ProjectId::new();
    let edge_count = match word {
        TestWordV1::SameCarrierCommutatorThenInverse => 6,
        TestWordV1::DifferentCarrierCommutator => 4,
    };
    let faces = (0..edge_count)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("carrier-free-product-face:{word:?}:{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let collective_profile = if collective {
        TestProfileV1::Collective
    } else {
        TestProfileV1::Constant(30.0_f64.to_bits())
    };
    let (carriers, profiles) = match word {
        TestWordV1::SameCarrierCommutatorThenInverse => (
            vec![0, 0, 0, 0, 1, 1],
            vec![
                collective_profile,
                TestProfileV1::Constant(45.0_f64.to_bits()),
                collective_profile,
                TestProfileV1::Constant(45.0_f64.to_bits()),
                TestProfileV1::Constant(60.0_f64.to_bits()),
                TestProfileV1::Constant(60.0_f64.to_bits()),
            ],
        ),
        TestWordV1::DifferentCarrierCommutator => (
            vec![0, 1, 0, 1],
            vec![
                collective_profile,
                TestProfileV1::Constant(45.0_f64.to_bits()),
                collective_profile,
                TestProfileV1::Constant(45.0_f64.to_bits()),
            ],
        ),
    };
    let bridge_capacity = if observer_tamper_bridge { 1 } else { 0 };
    let mut hinges = Vec::with_capacity(edge_count + bridge_capacity);
    let mut schedule_profiles = Vec::with_capacity(edge_count + bridge_capacity);
    let mut cycle_edges = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        let edge = EdgeId::derive_v5(
            namespace,
            format!("carrier-free-product-edge:{word:?}:{index}").as_bytes(),
        );
        let (mut left, mut right) = (faces[index], faces[(index + 1) % edge_count]);
        let (mut start, mut end, mut axis) = carrier_v1(carriers[index], index);
        if reverse_every_other && index % 2 == 1 {
            std::mem::swap(&mut left, &mut right);
            std::mem::swap(&mut start, &mut end);
            axis = Point3::new(-axis.x(), -axis.y(), -axis.z()).unwrap();
        }
        hinges.push(TreeHinge::new_for_test(
            edge,
            if match word {
                TestWordV1::SameCarrierCommutatorThenInverse => {
                    matches!(index, 0 | 1 | 4)
                }
                TestWordV1::DifferentCarrierCommutator => index < 2,
            } {
                FoldAssignment::Mountain
            } else {
                FoldAssignment::Valley
            },
            left,
            right,
            start,
            end,
            axis,
        ));
        schedule_profiles.push((edge, profiles[index]));
        cycle_edges.push(edge);
    }

    let mut all_faces = faces.clone();
    let mut bridge_edges = Vec::new();
    if observer_tamper_bridge {
        let leaf = FaceId::derive_v5(namespace, b"carrier-free-product-observer-leaf");
        let edge = EdgeId::derive_v5(namespace, b"carrier-free-product-observer-bridge");
        let (start, end, axis) = carrier_v1(2, 0);
        hinges.push(TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            faces[0],
            leaf,
            start,
            end,
            axis,
        ));
        schedule_profiles.push((edge, TestProfileV1::OtherNonconstant));
        bridge_edges.push(edge);
        all_faces.push(leaf);
    }
    rebuild_fixture_v1(
        all_faces,
        hinges,
        CarrierFreeProductFixturePartsV1 {
            fixed_face: faces[0],
            profiles: schedule_profiles,
            cycle_edges,
            bridge_edges,
        },
        reverse_storage,
    )
}

pub(super) fn replace_hinge_v1(
    fixture: CarrierFreeProductFixtureV1,
    edge: EdgeId,
    replacement: impl FnOnce(&TreeHinge) -> TreeHinge,
) -> CarrierFreeProductFixtureV1 {
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
        .collect();
    rebuild_fixture_v1(
        fixture.geometry.face_ids().to_vec(),
        hinges,
        CarrierFreeProductFixturePartsV1 {
            fixed_face: fixture.fixed_face,
            profiles: fixture.profiles,
            cycle_edges: fixture.cycle_edges,
            bridge_edges: fixture.bridge_edges,
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
    fixture: &CarrierFreeProductFixtureV1,
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
                matches!(mutation, ScheduleMutationV1::SecondNonconstant(value) if value == edge);
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
                                numerator: if second || profile == TestProfileV1::OtherNonconstant {
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

pub(super) fn half_angle_schedule_v1(
    fixture: &CarrierFreeProductFixtureV1,
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
            let (numerator, denominator) = match *profiles.get(&edge).unwrap() {
                TestProfileV1::Collective => (vec![0, 1], vec![4]),
                TestProfileV1::OtherNonconstant => (vec![0, 1], vec![5]),
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
        .collect();
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
