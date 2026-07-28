//! Canonical cycle-schedule wire conversion and bounded schedule generation.
//!
//! Native graph analysis and command orchestration remain in the parent
//! module. This module owns only deterministic schedule preparation and its
//! strict request vocabulary.

use ori_domain::FaceId;
use ori_kinematics::CycleScheduleLimitsV1;
use serde::Deserialize;

use super::{CYCLE_PATH_RESOURCE_MESSAGE, CYCLE_PATH_UNSUPPORTED_MESSAGE};

pub(super) fn prepare_requested_cycle_schedule_v1(
    request: &CycleScheduleRequestV1,
    geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    audit: &ori_kinematics::MaterialHingeGraphAudit,
    fixed_face: FaceId,
    live: &ori_kinematics::CanonicalHingeAngles,
) -> Result<ori_kinematics::CanonicalCycleScheduleV1, &'static str> {
    if request.version != 1 || request.entries.len() != live.as_slice().len() {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let rational = |value: RationalCoefficientRequestV1| {
        (value.denominator != 0)
            .then_some(ori_kinematics::RationalCoefficientV1 {
                numerator: value.numerator,
                denominator: value.denominator,
            })
            .ok_or(CYCLE_PATH_UNSUPPORTED_MESSAGE)
    };
    let inputs = request
        .entries
        .iter()
        .map(|entry| {
            Ok(ori_kinematics::HalfAngleRationalEntryInputV1 {
                edge: entry.edge,
                u_domain: [rational(entry.u_domain[0])?, rational(entry.u_domain[1])?],
                numerator_power_coefficients: entry
                    .numerator_power_coefficients
                    .iter()
                    .copied()
                    .map(rational)
                    .collect::<Result<Vec<_>, _>>()?,
                denominator_power_coefficients: entry
                    .denominator_power_coefficients
                    .iter()
                    .copied()
                    .map(rational)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let limits = production_cycle_schedule_limits_v1();
    let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
        geometry, audit, fixed_face, inputs, limits,
    )
    .map_err(|error| match error {
        ori_kinematics::CycleSchedulePrepareErrorV1::ResourceLimit => CYCLE_PATH_RESOURCE_MESSAGE,
        _ => CYCLE_PATH_UNSUPPORTED_MESSAGE,
    })?;
    for (upper, expected) in [false, true].into_iter().zip([
        live.as_slice()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect::<Vec<_>>(),
        request
            .entries
            .iter()
            .map(|entry| (entry.edge, entry.requested_angle_degrees))
            .collect(),
    ]) {
        let endpoint = schedule
            .evaluate_endpoint_angle_box(upper, limits)
            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?;
        if endpoint.len() != expected.len()
            || endpoint
                .iter()
                .zip(expected)
                .any(|((edge, interval), expected)| {
                    *edge != expected.0
                        || !expected.1.is_finite()
                        || expected.1 < interval.lower()
                        || expected.1 > interval.upper()
                        || interval.upper() - interval.lower()
                            > ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1
                })
        {
            return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
        }
    }
    Ok(schedule)
}

pub(super) fn generate_even_opposite_pair_schedule_v1(
    geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    audit: &ori_kinematics::MaterialHingeGraphAudit,
    fixed_face: FaceId,
    live: &ori_kinematics::CanonicalHingeAngles,
    target: &ori_kinematics::CanonicalHingeAngles,
) -> Result<ori_kinematics::CanonicalCycleScheduleV1, &'static str> {
    let changed = live
        .as_slice()
        .iter()
        .zip(target.as_slice())
        .filter_map(|(source, target)| {
            (source.angle_degrees().to_bits() != target.angle_degrees().to_bits())
                .then_some(target.edge())
        })
        .collect::<Vec<_>>();
    if changed.len() != 2
        || !ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(geometry, audit, 128)
            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?
            .iter()
            .any(|pair| pair.iter().all(|edge| changed.contains(edge)))
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let requested = target
        .as_slice()
        .iter()
        .find(|entry| entry.edge() == changed[0])
        .map(|entry| entry.angle_degrees())
        .ok_or(CYCLE_PATH_UNSUPPORTED_MESSAGE)?;
    let (endpoint_numerator, endpoint_denominator) =
        bounded_primitive_endpoint_ratio_for_angle_v1(requested)?;
    let entries = live
        .as_slice()
        .iter()
        .map(|source| {
            let active = changed.contains(&source.edge());
            CycleScheduleEntryRequestV1 {
                edge: source.edge(),
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
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: endpoint_numerator,
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
                    numerator: if active {
                        endpoint_denominator as i64
                    } else {
                        1
                    },
                    denominator: 1,
                }],
                requested_angle_degrees: if active {
                    requested
                } else {
                    source.angle_degrees()
                },
            }
        })
        .collect();
    prepare_requested_cycle_schedule_v1(
        &CycleScheduleRequestV1 {
            version: 1,
            entries,
            endpoint_denominator: None,
        },
        geometry,
        audit,
        fixed_face,
        live,
    )
}

pub(super) fn bounded_primitive_endpoint_ratio_v1(
    numerator: i64,
    denominator: u64,
) -> Result<(i64, u64), &'static str> {
    if numerator == 0 || denominator == 0 || denominator > 64 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let magnitude = numerator.unsigned_abs();
    if magnitude > 64 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let mut left = magnitude;
    let mut right = denominator;
    while right != 0 {
        (left, right) = (right, left % right);
    }
    if left != 1 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    Ok((numerator, denominator))
}

pub(super) fn bounded_primitive_endpoint_ratio_for_angle_v1(
    requested_angle_degrees: f64,
) -> Result<(i64, u64), &'static str> {
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees == 0.0
        || requested_angle_degrees.abs() >= 180.0
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let sign = if requested_angle_degrees.is_sign_positive() {
        1_i64
    } else {
        -1_i64
    };
    (1_u64..=64)
        .flat_map(|denominator| (1_i64..=64).map(move |n| (n * sign, denominator)))
        .filter_map(|ratio| bounded_primitive_endpoint_ratio_v1(ratio.0, ratio.1).ok())
        .find(|(numerator, denominator)| {
            ori_kinematics::deterministic_half_angle_ratio_degrees_v1(
                *numerator as f64,
                *denominator as f64,
            )
            .is_some_and(|candidate| (requested_angle_degrees - candidate).abs() <= 1.0e-12)
        })
        .ok_or(CYCLE_PATH_UNSUPPORTED_MESSAGE)
}

pub(crate) fn production_cycle_schedule_limits_v1() -> CycleScheduleLimitsV1 {
    let defaults = CycleScheduleLimitsV1::default();
    CycleScheduleLimitsV1 {
        max_hinges: 256,
        max_work: 1_152,
        ..defaults
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CycleScheduleRequestV1 {
    pub(super) version: u32,
    pub(super) entries: Vec<CycleScheduleEntryRequestV1>,
    #[serde(default)]
    pub(super) endpoint_denominator: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(Clone)]
pub(super) struct CycleScheduleEntryRequestV1 {
    pub(super) edge: ori_domain::EdgeId,
    pub(super) u_domain: [RationalCoefficientRequestV1; 2],
    pub(super) numerator_power_coefficients: Vec<RationalCoefficientRequestV1>,
    pub(super) denominator_power_coefficients: Vec<RationalCoefficientRequestV1>,
    pub(super) requested_angle_degrees: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RationalCoefficientRequestV1 {
    pub(super) numerator: i64,
    pub(super) denominator: u64,
}

#[cfg(test)]
pub(super) use tests::{
    advance_collective_schedule, dense_grid_schedule, dense_grid_schedule_ratio,
    four_bay_cycle_schedule, physical_four_vertex_cycle_schedule, theta_cycle_schedule,
};

#[cfg(test)]
#[path = "stacked_fold_cycle_schedule_tests.rs"]
mod tests;
