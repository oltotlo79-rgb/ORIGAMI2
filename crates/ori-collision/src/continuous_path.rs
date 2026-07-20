//! Bounded observation of a collective-hinge path.
//!
//! Sampling is deliberately not presented as CCD proof.  The result can find
//! a blocking sampled pose and can recommend the authenticated initial pose as
//! a fail-closed hold, but it never certifies the open intervals between
//! samples or authorizes mutation.

use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
};
use thiserror::Error;

use crate::{
    StaticCollisionLimits, diagnose_static_collision_geometry,
    prepare_positive_thickness_pair_separation_v1, prepare_single_hinge_thickness_boundary_v1,
    prepare_tree_hinge_thickness_boundaries_v1, revalidate_positive_thickness_pair_separation_v1,
    revalidate_single_hinge_thickness_boundary_v1, revalidate_tree_hinge_thickness_boundaries_v1,
};

pub const STACKED_FOLD_BOUNDED_PATH_DIAGNOSTIC_MODEL_ID_V1: &str =
    "stacked_fold_bounded_path_diagnostic_v1";
pub const STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_single_hinge_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_single_hinge_positive_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_collinear_tree_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_two_hinge_positive_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_two_hinge_interval_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_tree_interval_zero_thickness_continuous_certificate_v1";
pub const MAX_STACKED_FOLD_PATH_SAMPLES_V1: usize = 64;
pub const MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1: usize = 8;
const MAX_STACKED_FOLD_INTERVAL_FACE_PAIRS_V1: usize = 36;
const MAX_STACKED_FOLD_INTERVAL_WORK_V1: usize = 64 * 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackedFoldPathDiagnosticLimitsV1 {
    /// Number of equal angle intervals. Both endpoints are observed.
    pub sample_intervals: usize,
    pub static_collision: StaticCollisionLimits,
}

impl Default for StackedFoldPathDiagnosticLimitsV1 {
    fn default() -> Self {
        Self {
            sample_intervals: 8,
            static_collision: StaticCollisionLimits::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedFoldBoundedPathDiagnosticV1 {
    sampled_pose_count: usize,
    sampled_nonblocking_pose_count: usize,
    first_sampled_blocking_angle_degrees: Option<f64>,
    requested_angle_degrees: f64,
    analytic_single_hinge_clearance: bool,
    analytic_collinear_tree_clearance: bool,
    analytic_positive_two_hinge_clearance: bool,
    interval_two_hinge_chain_clearance: bool,
    interval_tree_hinge_count: usize,
    positive_thickness_outer_shell: bool,
}

impl StackedFoldBoundedPathDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        STACKED_FOLD_BOUNDED_PATH_DIAGNOSTIC_MODEL_ID_V1
    }

    #[must_use]
    pub const fn sampled_pose_count(&self) -> usize {
        self.sampled_pose_count
    }

    #[must_use]
    pub const fn sampled_nonblocking_pose_count(&self) -> usize {
        self.sampled_nonblocking_pose_count
    }

    #[must_use]
    pub const fn first_sampled_blocking_angle_degrees(&self) -> Option<f64> {
        self.first_sampled_blocking_angle_degrees
    }

    #[must_use]
    pub const fn requested_angle_degrees(&self) -> f64 {
        self.requested_angle_degrees
    }

    /// Sampling cannot prove an open continuous interval.
    #[must_use]
    pub const fn continuous_clearance_certified(&self) -> bool {
        self.analytic_single_hinge_clearance
            || self.analytic_collinear_tree_clearance
            || self.analytic_positive_two_hinge_clearance
            || self.interval_two_hinge_chain_clearance
    }

    /// The only fail-closed recommendation supplied by this diagnostic is to
    /// retain the already authenticated initial pose.
    #[must_use]
    pub const fn safe_stop_angle_degrees(&self) -> f64 {
        if self.continuous_clearance_certified() {
            self.requested_angle_degrees
        } else {
            0.0
        }
    }

    #[must_use]
    pub const fn continuous_certificate_model_id(&self) -> Option<&'static str> {
        if self.interval_two_hinge_chain_clearance {
            Some(
                if self.sampled_pose_count > 0 && self.interval_tree_hinge_count() > 2 {
                    STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
                } else {
                    STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
                },
            )
        } else if self.analytic_positive_two_hinge_clearance {
            Some(STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
        } else if self.analytic_collinear_tree_clearance {
            Some(STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
        } else if self.analytic_single_hinge_clearance {
            Some(if self.positive_thickness_outer_shell {
                STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            } else {
                STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            })
        } else {
            None
        }
    }

    const fn interval_tree_hinge_count(&self) -> usize {
        // A certified tree has one more face than hinges. The diagnostic does
        // not otherwise expose topology, so this value is stored explicitly
        // below in the next field.
        self.interval_tree_hinge_count
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StackedFoldPathDiagnosticErrorV1 {
    #[error("the path diagnostic limits are invalid")]
    InvalidLimits,
    #[error("the requested angle or moving-hinge set is invalid")]
    InvalidPath,
    #[error("the initial pose is not owned by the supplied model")]
    PoseIssuerMismatch,
    #[error("one sampled pose could not be solved")]
    PoseUnavailable,
    #[error("one sampled static collision diagnosis failed")]
    StaticDiagnosisUnavailable,
}

pub fn diagnose_collective_hinge_path_v1(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving_hinges: &[EdgeId],
    requested_angle_degrees: f64,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    if limits.sample_intervals == 0 || limits.sample_intervals > MAX_STACKED_FOLD_PATH_SAMPLES_V1 {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidLimits);
    }
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || requested_angle_degrees > 180.0
        || moving_hinges.is_empty()
    {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    }
    model
        .bind_pose(initial_pose)
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch)?;
    let moving = moving_hinges.iter().copied().collect::<HashSet<_>>();
    if moving.len() != moving_hinges.len()
        || !moving
            .iter()
            .all(|edge| model.hinges().iter().any(|hinge| hinge.edge() == *edge))
    {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    }
    // Native narrow theorem: a simulation-ready material model containing
    // exactly two faces joined by its only hinge has exactly one unordered
    // face pair. Starting that hinge at bit-exact zero and rotating it
    // monotonically through [0, 180] cannot create a transversal intersection:
    // the two rigid material planes meet only on the shared axis until the
    // terminal flat-stack contact. Positive thickness and every larger graph
    // remain outside this theorem.
    let analytic_single_hinge_topology = model.face_ids().len() == 2
        && model.hinges().len() == 1
        && moving.len() == 1
        && initial_pose
            .hinge_angles()
            .iter()
            .find(|angle| moving.contains(&angle.edge()))
            .is_some_and(|angle| angle.angle_degrees().to_bits() == 0.0_f64.to_bits());
    let zero_thickness = paper_thickness_mm.to_bits() == 0.0_f64.to_bits();
    let analytic_collinear_tree_topology = zero_thickness
        && collinear_collective_tree_premises(
            model,
            initial_pose,
            &moving,
            requested_angle_degrees,
        );
    let positive_thickness = paper_thickness_mm.is_finite() && paper_thickness_mm > 0.0;
    let interval_two_hinge_chain_topology = zero_thickness
        && two_hinge_interval_clearance_premises(
            model,
            initial_pose,
            &moving,
            requested_angle_degrees,
            limits.sample_intervals,
        );
    let positive_two_hinge_topology = positive_thickness
        && model.face_ids().len() == 3
        && model.hinges().len() == 2
        && moving.len() == 2
        && initial_pose.hinge_angles().iter().all(|angle| {
            moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() == 0.0_f64.to_bits()
        });
    let mut all_positive_thickness_outer_shells = positive_thickness;

    let mut sampled_nonblocking_pose_count = 0;
    let mut first_sampled_blocking_angle_degrees = None;
    for index in 0..=limits.sample_intervals {
        let angle = requested_angle_degrees * index as f64 / limits.sample_intervals as f64;
        let angles = initial_pose
            .hinge_angles()
            .iter()
            .map(|hinge| {
                HingeAngle::new(
                    hinge.edge(),
                    if moving.contains(&hinge.edge()) {
                        angle
                    } else {
                        hinge.angle_degrees()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseUnavailable)?;
        let angles = CanonicalHingeAngles::new(angles)
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseUnavailable)?;
        let pose = model
            .solve(initial_pose.fixed_face(), &angles)
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseUnavailable)?;
        if positive_thickness && index > 0 && index < limits.sample_intervals {
            // For the strict two-triangle/one-hinge class up to a right angle,
            // radial separation changes monotonically. The requested endpoint
            // is therefore the worst finite-corridor case; intermediate
            // static recomputation would only duplicate that bounded proof.
            sampled_nonblocking_pose_count += 1;
            continue;
        }
        if positive_thickness && index == limits.sample_intervals {
            let bound = model
                .bind_pose(&pose)
                .map_err(|_| StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch)?;
            all_positive_thickness_outer_shells &=
                if positive_two_hinge_topology {
                    prepare_tree_hinge_thickness_boundaries_v1(bound, paper_thickness_mm)
                        .ok()
                        .flatten()
                        .is_some_and(|boundary| {
                            revalidate_tree_hinge_thickness_boundaries_v1(
                                &boundary,
                                bound,
                                paper_thickness_mm,
                            )
                            .is_some_and(|observations| observations.len() == 2)
                                && model.face_ids().iter().enumerate().all(|(index, first)| {
                                    model.face_ids().iter().skip(index + 1).all(|second| {
                                        let adjacent = model.hinges().iter().any(|hinge| {
                                            (hinge.left_face() == *first
                                                && hinge.right_face() == *second)
                                                || (hinge.left_face() == *second
                                                    && hinge.right_face() == *first)
                                        });
                                        adjacent
                                            || prepare_positive_thickness_pair_separation_v1(
                                                bound,
                                                paper_thickness_mm,
                                                *first,
                                                *second,
                                                limits.static_collision,
                                            )
                                            .is_ok_and(|capability| {
                                                capability.is_some_and(|capability| {
                                                revalidate_positive_thickness_pair_separation_v1(
                                                    &capability,
                                                    bound,
                                                    paper_thickness_mm,
                                                )
                                            })
                                            })
                                    })
                                })
                        })
                } else {
                    prepare_single_hinge_thickness_boundary_v1(bound, paper_thickness_mm)
                        .ok()
                        .flatten()
                        .is_some_and(|boundary| {
                            revalidate_single_hinge_thickness_boundary_v1(
                                &boundary,
                                bound,
                                paper_thickness_mm,
                            )
                            .is_some()
                        })
                };
            if all_positive_thickness_outer_shells {
                // The opaque boundary capability is issued only after the
                // complete shared-hinge solid classifier returns Allowed.
                // Re-running the general static entrypoint would duplicate
                // that exact work and can exhaust its independent meter.
                sampled_nonblocking_pose_count += 1;
                continue;
            }
            first_sampled_blocking_angle_degrees.get_or_insert(angle);
            continue;
        }
        if positive_thickness && index == 0 {
            sampled_nonblocking_pose_count += 1;
            continue;
        }
        let snapshot = diagnose_static_collision_geometry(
            model,
            &pose,
            paper_thickness_mm,
            limits.static_collision,
        )
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable)?;
        let narrow_shared_hinge_classified = analytic_single_hinge_topology
            && snapshot.expected_unordered_face_pairs() == 1
            && snapshot.pairs().len() == 1
            && snapshot.penetrating_pairs() == 0
            && snapshot.pairs().iter().all(|pair| {
                if positive_thickness {
                    pair.shared_hinge_solid_classified()
                } else {
                    pair.shared_hinge_boundary_contact_proven()
                }
            });
        if snapshot.has_prominent_blocking_hold()
            && !(zero_thickness && analytic_single_hinge_topology)
            && !(zero_thickness && analytic_collinear_tree_topology)
            && !(zero_thickness && interval_two_hinge_chain_topology)
            && !narrow_shared_hinge_classified
        {
            first_sampled_blocking_angle_degrees.get_or_insert(angle);
        } else {
            sampled_nonblocking_pose_count += 1;
        }
    }
    Ok(StackedFoldBoundedPathDiagnosticV1 {
        sampled_pose_count: limits.sample_intervals + 1,
        sampled_nonblocking_pose_count,
        first_sampled_blocking_angle_degrees,
        requested_angle_degrees,
        analytic_single_hinge_clearance: analytic_single_hinge_topology
            && (!positive_thickness || requested_angle_degrees <= 90.0)
            && (zero_thickness || all_positive_thickness_outer_shells)
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        analytic_collinear_tree_clearance: analytic_collinear_tree_topology
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        analytic_positive_two_hinge_clearance: positive_two_hinge_topology
            && requested_angle_degrees <= 90.0
            && all_positive_thickness_outer_shells
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        interval_two_hinge_chain_clearance: interval_two_hinge_chain_topology
            && first_sampled_blocking_angle_degrees.is_none()
            && sampled_nonblocking_pose_count == limits.sample_intervals + 1,
        interval_tree_hinge_count: if interval_two_hinge_chain_topology {
            moving.len()
        } else {
            0
        },
        positive_thickness_outer_shell: positive_thickness && all_positive_thickness_outer_shells,
    })
}

fn two_hinge_interval_clearance_premises(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    requested_angle_degrees: f64,
    interval_count: usize,
) -> bool {
    let hinge_count = model.hinges().len();
    let face_count = model.face_ids().len();
    let Some(pair_count) = face_count
        .checked_mul(face_count.saturating_sub(1))
        .map(|n| n / 2)
    else {
        return false;
    };
    if !(2..=MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1).contains(&hinge_count)
        || face_count != hinge_count + 1
        || moving.len() != hinge_count
        || interval_count == 0
        || pair_count > MAX_STACKED_FOLD_INTERVAL_FACE_PAIRS_V1
        || interval_count
            .checked_mul(pair_count)
            .is_none_or(|work| work > MAX_STACKED_FOLD_INTERVAL_WORK_V1)
        || initial_pose.fixed_face().is_none()
        || !initial_pose.hinge_angles().iter().all(|angle| {
            moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() == 0.0_f64.to_bits()
        })
    {
        return false;
    }
    let Some(first_line) = world_hinge_line(initial_pose, &model.hinges()[0]) else {
        return false;
    };
    if model.hinges()[1..].iter().all(|hinge| {
        world_hinge_line(initial_pose, hinge).is_some_and(|line| {
            exact_collinear_line(first_line.0, first_line.2, line.0, line.2)
                && exact_collinear_line(first_line.0, first_line.2, line.1, line.2)
        })
    }) {
        return false;
    }

    let Some(root) = initial_pose.fixed_face() else {
        return false;
    };
    let mut depth = HashMap::<FaceId, usize>::new();
    depth.insert(root, 0);
    let mut queue = VecDeque::from([root]);
    while let Some(face) = queue.pop_front() {
        let parent_depth = depth[&face];
        for hinge in model.hinges() {
            let next = if hinge.left_face() == face {
                Some(hinge.right_face())
            } else if hinge.right_face() == face {
                Some(hinge.left_face())
            } else {
                None
            };
            if let Some(next) = next {
                if let std::collections::hash_map::Entry::Vacant(entry) = depth.entry(next) {
                    let Some(next_depth) = parent_depth.checked_add(1) else {
                        return false;
                    };
                    entry.insert(next_depth);
                    queue.push_back(next);
                }
            }
        }
    }
    if depth.len() != face_count {
        return false;
    }

    let mut material_points = Vec::new();
    for face in model.face_ids() {
        let Some(boundary) = model.face_boundary(*face) else {
            return false;
        };
        for vertex in boundary.vertices() {
            let Some(point) = initial_pose.vertex_position(*vertex) else {
                return false;
            };
            material_points.push(point);
        }
    }
    let hinge_points = model
        .hinges()
        .iter()
        .flat_map(|hinge| [hinge.start(), hinge.end()])
        .collect::<Vec<_>>();
    let mut maximum_radius = 0.0_f64;
    for point in &material_points {
        for origin in &hinge_points {
            let distance = ((point.x() - origin.x()).powi(2)
                + (point.y() - origin.y()).powi(2)
                + (point.z() - origin.z()).powi(2))
            .sqrt();
            if !distance.is_finite() {
                return false;
            }
            maximum_radius = maximum_radius.max(distance);
        }
    }
    if maximum_radius == 0.0 {
        return false;
    }

    let adjacent = |first: ori_domain::FaceId, second: ori_domain::FaceId| {
        model.hinges().iter().any(|hinge| {
            (hinge.left_face() == first && hinge.right_face() == second)
                || (hinge.left_face() == second && hinge.right_face() == first)
        })
    };
    for interval in 0..interval_count {
        let lower = requested_angle_degrees * interval as f64 / interval_count as f64;
        let upper = requested_angle_degrees * (interval + 1) as f64 / interval_count as f64;
        let midpoint = (lower + upper) / 2.0;
        let half_width_radians = (upper - lower) * std::f64::consts::PI / 360.0;
        let Some(pose) = solve_collective_pose(model, initial_pose, moving, midpoint) else {
            return false;
        };
        let mut bounds = Vec::new();
        for face in model.face_ids() {
            let Some(face_depth) = depth.get(face) else {
                return false;
            };
            let expansion = *face_depth as f64 * maximum_radius * half_width_radians;
            if !expansion.is_finite() {
                return false;
            }
            let Some(transform) = pose.face_transform(*face) else {
                return false;
            };
            let Some(boundary) = model.face_boundary(*face) else {
                return false;
            };
            let mut minimum = [f64::INFINITY; 3];
            let mut maximum = [f64::NEG_INFINITY; 3];
            for vertex in boundary.vertices() {
                let Some(point) = initial_pose.vertex_position(*vertex) else {
                    return false;
                };
                let Ok(world) = transform.apply_point(point) else {
                    return false;
                };
                for (axis, value) in [world.x(), world.y(), world.z()].into_iter().enumerate() {
                    minimum[axis] = minimum[axis].min(value - expansion);
                    maximum[axis] = maximum[axis].max(value + expansion);
                }
            }
            bounds.push((*face, minimum, maximum));
        }
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                if adjacent(bounds[first].0, bounds[second].0) {
                    continue;
                }
                let separated = (0..3).any(|axis| {
                    bounds[first].2[axis] < bounds[second].1[axis]
                        || bounds[second].2[axis] < bounds[first].1[axis]
                });
                if !separated {
                    return false;
                }
            }
        }
    }
    true
}

fn solve_collective_pose(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    angle: f64,
) -> Option<MaterialTreePose> {
    let angles = initial_pose
        .hinge_angles()
        .iter()
        .map(|hinge| {
            HingeAngle::new(
                hinge.edge(),
                if moving.contains(&hinge.edge()) {
                    angle
                } else {
                    hinge.angle_degrees()
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .and_then(|angles| CanonicalHingeAngles::new(angles).ok())?;
    model.solve(initial_pose.fixed_face(), &angles).ok()
}

fn collinear_collective_tree_premises(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    requested_angle_degrees: f64,
) -> bool {
    if model.face_ids().len() < 3
        || model.hinges().len() < 2
        || moving.len() != model.hinges().len()
        || initial_pose.fixed_face().is_none()
        || !initial_pose.hinge_angles().iter().all(|angle| {
            moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() == 0.0_f64.to_bits()
        })
    {
        return false;
    }
    let Some(reference) = model.hinges().first() else {
        return false;
    };
    let Some(reference_line) = world_hinge_line(initial_pose, reference) else {
        return false;
    };
    if !model.hinges().iter().all(|hinge| {
        let Some((start, end, axis)) = world_hinge_line(initial_pose, hinge) else {
            return false;
        };
        exact_collinear_line(reference_line.0, reference_line.2, start, axis)
            && exact_collinear_line(reference_line.0, reference_line.2, end, axis)
    }) {
        return false;
    }
    [requested_angle_degrees / 2.0, requested_angle_degrees]
        .into_iter()
        .all(|angle| collective_pose_is_one_moving_body(model, initial_pose, moving, angle))
}

fn world_hinge_line(
    pose: &MaterialTreePose,
    hinge: &ori_kinematics::TreeHinge,
) -> Option<(
    ori_kinematics::Point3,
    ori_kinematics::Point3,
    ori_kinematics::Point3,
)> {
    let transform = pose.hinge_parent_transform(hinge.edge())?;
    Some((
        transform.apply_point(hinge.start()).ok()?,
        transform.apply_point(hinge.end()).ok()?,
        transform.apply_vector(hinge.axis()).ok()?,
    ))
}

fn exact_collinear_line(
    origin: ori_kinematics::Point3,
    axis: ori_kinematics::Point3,
    point: ori_kinematics::Point3,
    candidate_axis: ori_kinematics::Point3,
) -> bool {
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let reference = [axis.x(), axis.y(), axis.z()];
    let candidate = [candidate_axis.x(), candidate_axis.y(), candidate_axis.z()];
    let offset = [
        point.x() - origin.x(),
        point.y() - origin.y(),
        point.z() - origin.z(),
    ];
    cross(reference, candidate)
        .into_iter()
        .chain(cross(offset, reference))
        .all(|value| value == 0.0)
}

fn collective_pose_is_one_moving_body(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    angle: f64,
) -> bool {
    let Ok(angles) = initial_pose
        .hinge_angles()
        .iter()
        .map(|hinge| {
            HingeAngle::new(
                hinge.edge(),
                if moving.contains(&hinge.edge()) {
                    angle
                } else {
                    hinge.angle_degrees()
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .and_then(CanonicalHingeAngles::new)
    else {
        return false;
    };
    let Ok(pose) = model.solve(initial_pose.fixed_face(), &angles) else {
        return false;
    };
    let Some(fixed_face) = initial_pose.fixed_face() else {
        return false;
    };
    let Some(fixed_transform) = pose.face_transform(fixed_face) else {
        return false;
    };
    let mut moving_transform = None;
    for face in model
        .face_ids()
        .iter()
        .copied()
        .filter(|face| *face != fixed_face)
    {
        let Some(transform) = pose.face_transform(face) else {
            return false;
        };
        if transform == fixed_transform {
            return false;
        }
        if let Some(expected) = moving_transform {
            if transform != expected {
                return false;
            }
        } else {
            moving_transform = Some(transform);
        }
    }
    moving_transform.is_some()
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
    use ori_kinematics::TreeKinematicsLimits;
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use super::*;

    fn fixed_id<T: serde::de::DeserializeOwned>(prefix: &str, index: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{index:012x}\"")).unwrap()
    }

    fn one_hinge_model() -> MaterialTreeKinematicsModel {
        let points = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8100", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9100", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(Edge {
            id: fixed_id("9100", 6),
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Mountain,
        });
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let project: ProjectId = fixed_id("b100", 1);
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: project,
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.unwrap(),
            TreeKinematicsLimits::default(),
        )
        .unwrap()
    }

    fn two_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (450.0, 200.0),
            (250.0, 450.0),
            (0.0, 300.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8200", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9200", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend([
            Edge {
                id: fixed_id("9200", 6),
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: fixed_id("9200", 7),
                start: boundary[0],
                end: boundary[3],
                kind: EdgeKind::Valley,
            },
        ]);
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b200", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("three triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("two-hinge triangle model")
    }

    fn two_hinge_strip_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (1.0, 0.0),
            (3.0, 0.0),
            (4.0, 0.0),
            (4.0, 4.0),
            (3.0, 4.0),
            (1.0, 4.0),
            (0.0, 4.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8200", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9200", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend([
            Edge {
                id: fixed_id("9200", 20),
                start: boundary[1],
                end: boundary[6],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: fixed_id("9200", 21),
                start: boundary[2],
                end: boundary[5],
                kind: EdgeKind::Mountain,
            },
        ]);
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b200", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.unwrap(),
            TreeKinematicsLimits::default(),
        )
        .unwrap()
    }

    fn three_hinge_strip_model(narrow_gap: bool) -> MaterialTreeKinematicsModel {
        let middle = if narrow_gap { 2.01 } else { 3.0 };
        let points = [
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (middle, 0.0),
            (4.0, 0.0),
            (4.0, 4.0),
            (middle, 4.0),
            (2.0, 4.0),
            (1.0, 4.0),
            (0.0, 4.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8300", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9300", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend([(1, 8), (2, 7), (3, 6)].into_iter().enumerate().map(
            |(index, (start, end))| Edge {
                id: fixed_id("9300", 20 + index as u64),
                start: boundary[start],
                end: boundary[end],
                kind: EdgeKind::Mountain,
            },
        ));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b300", if narrow_gap { 2 } else { 1 }),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.unwrap(),
            TreeKinematicsLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn limits_fail_closed_before_geometry_access() {
        assert_eq!(MAX_STACKED_FOLD_PATH_SAMPLES_V1, 64);
        assert_eq!(
            StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
            8
        );
        assert_eq!(
            StackedFoldBoundedPathDiagnosticV1 {
                sampled_pose_count: 9,
                sampled_nonblocking_pose_count: 9,
                first_sampled_blocking_angle_degrees: None,
                requested_angle_degrees: 90.0,
                analytic_single_hinge_clearance: false,
                analytic_collinear_tree_clearance: false,
                analytic_positive_two_hinge_clearance: false,
                interval_two_hinge_chain_clearance: false,
                interval_tree_hinge_count: 0,
                positive_thickness_outer_shell: false,
            }
            .safe_stop_angle_degrees()
            .to_bits(),
            0.0_f64.to_bits()
        );
    }

    #[test]
    fn collinear_tree_gate_requires_one_exact_infinite_axis() {
        let origin = ori_kinematics::Point3::new(0.0, 0.0, 0.0).unwrap();
        let axis = ori_kinematics::Point3::new(1.0, 0.0, 0.0).unwrap();
        assert!(exact_collinear_line(
            origin,
            axis,
            ori_kinematics::Point3::new(4.0, 0.0, 0.0).unwrap(),
            ori_kinematics::Point3::new(-1.0, 0.0, 0.0).unwrap(),
        ));
        assert!(!exact_collinear_line(
            origin,
            axis,
            ori_kinematics::Point3::new(4.0, f64::from_bits(1), 0.0).unwrap(),
            ori_kinematics::Point3::new(1.0, 0.0, 0.0).unwrap(),
        ));
        assert!(!exact_collinear_line(
            origin,
            axis,
            origin,
            ori_kinematics::Point3::new(1.0, f64::from_bits(1), 0.0).unwrap(),
        ));
    }

    #[test]
    fn separated_two_hinge_strip_gets_interval_clearance_certificate() {
        let model = two_hinge_strip_model();
        assert_eq!(model.face_ids().len(), 3);
        assert_eq!(model.hinges().len(), 2);
        let middle = model
            .face_ids()
            .iter()
            .copied()
            .find(|face| {
                model
                    .hinges()
                    .iter()
                    .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                    .count()
                    == 2
            })
            .unwrap();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(middle), &angles).unwrap();
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let result = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &moving,
            10.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(result.continuous_clearance_certified());
        assert_eq!(
            result.continuous_certificate_model_id(),
            Some(STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
        );
        assert_eq!(result.safe_stop_angle_degrees(), 10.0);
    }

    #[test]
    fn separated_three_hinge_tree_gets_bounded_interval_certificate() {
        let model = three_hinge_strip_model(false);
        let fixed = model
            .face_ids()
            .iter()
            .copied()
            .find(|face| {
                model
                    .hinges()
                    .iter()
                    .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                    .count()
                    == 2
            })
            .unwrap();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(fixed), &angles).unwrap();
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let result = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &moving,
            5.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(result.continuous_clearance_certified());
        assert_eq!(
            result.continuous_certificate_model_id(),
            Some(STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
        );
    }

    #[test]
    fn near_collision_three_hinge_tree_fails_closed() {
        let model = three_hinge_strip_model(true);
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let result = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &moving,
            10.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!result.continuous_clearance_certified());
        assert_eq!(result.safe_stop_angle_degrees(), 0.0);
    }

    #[test]
    fn authenticated_two_face_zero_thickness_path_gets_narrow_certificate() {
        let model = one_hinge_model();
        let edge = model.hinges()[0].edge();
        let angles = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 0.0).unwrap()]).unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let result = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &[edge],
            90.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(result.continuous_clearance_certified());
        assert_eq!(
            result.continuous_certificate_model_id(),
            Some(STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
        );
        assert_eq!(result.safe_stop_angle_degrees(), 90.0);

        let positive_thickness = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &[edge],
            37.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("positive-thickness path");
        assert!(positive_thickness.continuous_clearance_certified());
        assert_eq!(
            positive_thickness.continuous_certificate_model_id(),
            Some(STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
        );
        assert_eq!(positive_thickness.safe_stop_angle_degrees(), 37.0);

        let requested =
            CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 37.0).expect("requested hinge")])
                .expect("canonical requested hinge");
        let first_pose = model
            .solve(Some(model.face_ids()[0]), &requested)
            .expect("first requested pose");
        let equal_but_distinct_pose = model
            .solve(Some(model.face_ids()[0]), &requested)
            .expect("ABA requested pose");
        let first_bound = model.bind_pose(&first_pose).expect("first bound");
        let boundary = prepare_single_hinge_thickness_boundary_v1(first_bound, 0.1)
            .expect("bounded classification")
            .expect("positive-thickness outer shell");
        assert!(
            revalidate_single_hinge_thickness_boundary_v1(
                &boundary,
                model
                    .bind_pose(&equal_but_distinct_pose)
                    .expect("distinct bound"),
                0.1,
            )
            .is_none()
        );
        assert!(
            revalidate_single_hinge_thickness_boundary_v1(
                &boundary,
                first_bound,
                f64::from_bits(0.1_f64.to_bits() + 1),
            )
            .is_none()
        );
    }

    #[test]
    fn three_triangle_positive_thickness_tree_stays_closed_without_all_pair_proof() {
        let model = two_hinge_triangle_model();
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let initial_angles = CanonicalHingeAngles::new(
            moving
                .iter()
                .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let initial = model
            .solve(Some(model.face_ids()[0]), &initial_angles)
            .expect("initial tree pose");
        for requested in [10.0, 30.0, 45.0, 60.0] {
            let diagnostic = diagnose_collective_hinge_path_v1(
                &model,
                &initial,
                &moving,
                requested,
                0.1,
                StackedFoldPathDiagnosticLimitsV1::default(),
            )
            .expect("bounded fail-closed diagnosis");
            assert!(!diagnostic.continuous_clearance_certified());
            assert_eq!(diagnostic.safe_stop_angle_degrees(), 0.0);
        }
    }
}
