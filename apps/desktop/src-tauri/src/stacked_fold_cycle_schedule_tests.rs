use super::super::{
    CYCLE_PATH_RESOURCE_MESSAGE, CYCLE_PATH_UNSUPPORTED_MESSAGE,
    MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1, dyadic_request_hinge_counts_are_bounded_v1,
    stacked_fold_read_wire::{StackedFoldReadRequest, validate_request_resource_shape_v1},
};
use super::*;

pub(crate) fn physical_four_vertex_cycle_schedule(
    _hinges: &[ori_domain::EdgeId],
) -> CycleScheduleRequestV1 {
    CycleScheduleRequestV1 {
        version: 2,
        entries: Vec::new(),
        endpoint_denominator: Some(1),
    }
}

pub(crate) fn dense_grid_schedule(
    hinges: &[ori_domain::EdgeId],
    moving: &[ori_domain::EdgeId],
    denominator: i64,
) -> CycleScheduleRequestV1 {
    dense_grid_schedule_ratio(hinges, moving, 1, denominator)
}

pub(crate) fn dense_grid_schedule_ratio(
    hinges: &[ori_domain::EdgeId],
    moving: &[ori_domain::EdgeId],
    numerator: i64,
    denominator: i64,
) -> CycleScheduleRequestV1 {
    let moving = moving
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut entries = hinges
        .iter()
        .copied()
        .map(|edge| {
            let active = moving.contains(&edge);
            CycleScheduleEntryRequestV1 {
                edge,
                u_domain: [
                    RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientRequestV1 {
                        numerator,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: if active {
                    vec![
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ]
                } else {
                    vec![RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    }]
                },
                denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                    numerator: if active { denominator } else { 1 },
                    denominator: 1,
                }],
                requested_angle_degrees: if active {
                    ori_kinematics::deterministic_half_angle_ratio_degrees_v1(
                        numerator as f64,
                        denominator as f64,
                    )
                    .unwrap()
                } else {
                    0.0
                },
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    CycleScheduleRequestV1 {
        version: 1,
        entries,
        endpoint_denominator: None,
    }
}

pub(crate) fn advance_collective_schedule(
    hinges: &[ori_domain::EdgeId],
    moving: &[ori_domain::EdgeId],
    denominator: i64,
) -> CycleScheduleRequestV1 {
    let moving = moving
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut entries = hinges
        .iter()
        .copied()
        .map(|edge| {
            let active = moving.contains(&edge);
            CycleScheduleEntryRequestV1 {
                edge,
                u_domain: [
                    RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientRequestV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: if active {
                    vec![
                        RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ]
                } else {
                    vec![RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    }]
                },
                denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                    numerator: if active { denominator } else { 1 },
                    denominator: 1,
                }],
                requested_angle_degrees: if active {
                    ori_kinematics::deterministic_half_angle_ratio_degrees_v1(
                        2.0,
                        denominator as f64,
                    )
                    .unwrap()
                } else {
                    0.0
                },
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    CycleScheduleRequestV1 {
        version: 1,
        entries,
        endpoint_denominator: None,
    }
}

pub(crate) fn four_bay_cycle_schedule(hinges: &[ori_domain::EdgeId]) -> CycleScheduleRequestV1 {
    let triples = [
        (3, 5),
        (5, 13),
        (8, 17),
        (7, 25),
        (3, 5),
        (5, 13),
        (8, 17),
        (7, 25),
        (3, 5),
        (5, 13),
        (8, 17),
        (7, 25),
        (3, 5),
        (5, 13),
        (8, 17),
        (7, 25),
    ];
    let mut entries = hinges
        .iter()
        .copied()
        .enumerate()
        .map(|(index, edge)| {
            let (p, q) = triples[(index / 4) % triples.len()];
            CycleScheduleEntryRequestV1 {
                edge,
                u_domain: [
                    RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientRequestV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![
                    RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientRequestV1 {
                        numerator: if index % 2 == 0 { 1 } else { p },
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                    numerator: if index % 2 == 0 { 1 } else { q },
                    denominator: 1,
                }],
                requested_angle_degrees: ori_kinematics::deterministic_half_angle_ratio_degrees_v1(
                    if index % 2 == 0 { 1.0 } else { p as f64 },
                    if index % 2 == 0 { 1.0 } else { q as f64 },
                )
                .unwrap(),
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    CycleScheduleRequestV1 {
        version: 1,
        entries,
        endpoint_denominator: None,
    }
}

pub(crate) fn theta_cycle_schedule(
    hinges: &[ori_domain::EdgeId],
    moving: &[ori_domain::EdgeId],
) -> CycleScheduleRequestV1 {
    let moving = moving
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut entries = hinges
        .iter()
        .copied()
        .map(|edge| {
            let moves = moving.contains(&edge);
            CycleScheduleEntryRequestV1 {
                edge,
                u_domain: [
                    RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientRequestV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: if moves {
                    vec![
                        RationalCoefficientRequestV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: 2,
                            denominator: 15,
                        },
                    ]
                } else {
                    vec![RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    }]
                },
                denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                    numerator: 1,
                    denominator: 1,
                }],
                requested_angle_degrees: if moves {
                    ori_kinematics::deterministic_half_angle_ratio_degrees_v1(2.0, 15.0).unwrap()
                } else {
                    0.0
                },
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    CycleScheduleRequestV1 {
        version: 1,
        entries,
        endpoint_denominator: None,
    }
}

#[test]
fn cycle_schedule_wire_rejects_unknown_fields_and_numeric_overflow() {
    let request = || {
        serde_json::json!({
            "expectedProjectInstanceId": "018f47a2-4b7a-7cc1-8abc-112233445566",
            "expectedProjectId": "018f47a2-4b7a-7cc1-8abc-665544332211",
            "expectedRevision": 3,
            "first": [0.0, 0.0, 0.0],
            "second": [1.0, 0.0, 0.0],
            "fixedSide": "left",
            "rotationDirection": "positive",
            "requestedAngleDegrees": 90.0,
            "cycleScheduleV1": {
                "version": 1,
                "entries": [{
                    "edge": "018f47a2-4b7a-7cc1-8abc-778899aabbcc",
                    "uDomain": [
                        {"numerator": 0, "denominator": 1},
                        {"numerator": 1, "denominator": 1}
                    ],
                    "numeratorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                    "denominatorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                    "requestedAngleDegrees": 90.0
                }]
            }
        })
    };
    let admitted = serde_json::from_value::<StackedFoldReadRequest>(request()).unwrap();
    assert_eq!(validate_request_resource_shape_v1(&admitted), Ok(()));
    let mut unknown = request();
    unknown["cycleScheduleV1"]["entries"][0]["authority"] = serde_json::json!(true);
    assert!(serde_json::from_value::<StackedFoldReadRequest>(unknown).is_err());
    let mut overflow = request();
    overflow["cycleScheduleV1"]["entries"][0]["uDomain"][0]["denominator"] = serde_json::json!(-1);
    assert!(serde_json::from_value::<StackedFoldReadRequest>(overflow).is_err());

    let mut coefficient_exhaustion = request();
    coefficient_exhaustion["cycleScheduleV1"]["entries"][0]["numeratorPowerCoefficients"] = serde_json::json!(
        (0..=MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1)
            .map(|_| serde_json::json!({"numerator": 1, "denominator": 1}))
            .collect::<Vec<_>>()
    );
    let coefficient_exhaustion =
        serde_json::from_value::<StackedFoldReadRequest>(coefficient_exhaustion).unwrap();
    assert_eq!(
        validate_request_resource_shape_v1(&coefficient_exhaustion),
        Err(CYCLE_PATH_RESOURCE_MESSAGE)
    );
}

#[test]
fn bounded_endpoint_ratios_and_dyadic_request_counts_are_admitted() {
    for denominator in 1..=64 {
        assert_eq!(
            bounded_primitive_endpoint_ratio_v1(1, denominator),
            Ok((1, denominator))
        );
    }
    for ratio in [(2, 3), (3, 7), (63, 64), (-2, 3), (4, 3), (7, 3), (64, 1)] {
        assert_eq!(
            bounded_primitive_endpoint_ratio_v1(ratio.0, ratio.1),
            Ok(ratio)
        );
    }
    for ratio in [(2, 3), (3, 7), (63, 64), (4, 3), (7, 3), (64, 1), (-4, 3)] {
        let angle = ori_kinematics::deterministic_half_angle_ratio_degrees_v1(
            ratio.0 as f64,
            ratio.1 as f64,
        )
        .unwrap();
        assert_eq!(
            bounded_primitive_endpoint_ratio_for_angle_v1(angle),
            Ok(ratio)
        );
    }
    let right_angle = ori_kinematics::deterministic_half_angle_ratio_degrees_v1(1.0, 1.0).unwrap();
    assert_eq!(right_angle.to_bits(), 90.0_f64.to_bits());
    for adjacent in [
        f64::from_bits(right_angle.to_bits() - 1),
        f64::from_bits(right_angle.to_bits() + 1),
    ] {
        assert_eq!(
            bounded_primitive_endpoint_ratio_for_angle_v1(adjacent),
            Ok((1, 1))
        );
    }
    for rejected_angle in [0.0, 180.0, -180.0, f64::INFINITY] {
        assert_eq!(
            bounded_primitive_endpoint_ratio_for_angle_v1(rejected_angle),
            Err(CYCLE_PATH_UNSUPPORTED_MESSAGE)
        );
    }
    for rejected in [(2, 4), (i64::MIN, 1), (1, 0), (1, 65), (65, 64)] {
        assert_eq!(
            bounded_primitive_endpoint_ratio_v1(rejected.0, rejected.1),
            Err(CYCLE_PATH_UNSUPPORTED_MESSAGE)
        );
    }
    assert!(dyadic_request_hinge_counts_are_bounded_v1(64, Some(64)));
    assert!(!dyadic_request_hinge_counts_are_bounded_v1(65, Some(64)));
    assert!(!dyadic_request_hinge_counts_are_bounded_v1(64, Some(65)));
}
