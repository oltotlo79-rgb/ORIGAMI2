//! Bounded observation of a collective-hinge path.
//!
//! Sampling is deliberately not presented as CCD proof.  The result can find
//! a blocking sampled pose and can recommend the authenticated initial pose as
//! a fail-closed hold, but it never certifies the open intervals between
//! samples or authorizes mutation.

use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{
    CanonicalHingeAngles, DyadicMaterialHingeIntervalClosureCertificateV1,
    GeneratedMultiHingePathCandidateV1, HingeAngle, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, MaterialTreeKinematicsModel, MaterialTreePose,
};
use thiserror::Error;

use crate::cayley::prepare_swept_tree_hinge_thickness_boundaries_v1;
use crate::{
    PositiveThicknessGraphLimitsV1, StaticCollisionLimits, diagnose_static_collision_geometry,
    prepare_positive_thickness_pair_separation_v1, prepare_single_hinge_thickness_boundary_v1,
    prove_positive_thickness_graph_geometry_v1, revalidate_positive_thickness_pair_separation_v1,
    revalidate_single_hinge_thickness_boundary_v1, revalidate_tree_hinge_thickness_boundaries_v1,
    static_collision::prepare_positive_thickness_tree_endpoint_topology_memo_v1,
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
    "stacked_fold_bounded_tree_positive_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_two_hinge_interval_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_tree_interval_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_cycle_interval_zero_thickness_continuous_certificate_v1";
pub const STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1: &str =
    "stacked_fold_cactus_positive_thickness_continuous_certificate_v1";
pub const MAX_STACKED_FOLD_PATH_SAMPLES_V1: usize = 64;
const MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1: usize = 120;
const MAX_POSITIVE_ENDPOINT_TREE_FACES_V1: usize = 64;
pub const MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1: usize = 64;
const MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1: usize = 2_048;
const MAX_STACKED_FOLD_INTERVAL_LEAVES_V1: usize = 128;
const MAX_STACKED_FOLD_INTERVAL_DEPTH_V1: usize = 7;
const MAX_STACKED_FOLD_INTERVAL_WORK_V1: usize =
    MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 * MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1;

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
    interval_leaf_count: usize,
    interval_pair_work: usize,
    positive_endpoint_memo_pair_entries: usize,
    positive_endpoint_exact_pair_calls: usize,
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
    pub const fn interval_leaf_count(&self) -> usize {
        self.interval_leaf_count
    }

    #[must_use]
    pub const fn interval_pair_work(&self) -> usize {
        self.interval_pair_work
    }

    #[must_use]
    pub const fn positive_endpoint_memo_pair_entries(&self) -> usize {
        self.positive_endpoint_memo_pair_entries
    }

    #[must_use]
    pub const fn positive_endpoint_exact_pair_calls(&self) -> usize {
        self.positive_endpoint_exact_pair_calls
    }

    #[must_use]
    pub const fn positive_endpoint_candidate_limit(&self) -> usize {
        MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1
    }

    #[must_use]
    pub const fn interval_candidate_limit(&self) -> usize {
        MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1
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

fn positive_tree_max_angle_degrees_v1(hinge_count: usize) -> Option<f64> {
    Some(match hinge_count {
        15 => 1.5,
        14 => 2.0,
        13 => 3.0,
        12 => 4.0,
        11 => 5.0,
        10 => 6.0,
        9 => 8.0,
        8 => 10.0,
        7 => 15.0,
        6 => 20.0,
        5 => 30.0,
        4 => 45.0,
        3 => 60.0,
        2 => 90.0,
        16..=63 => 0.1 / hinge_count as f64,
        _ => return None,
    })
}

fn positive_endpoint_pair_work_within_limit_v1(pair_count: usize) -> bool {
    pair_count <= MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1
}

fn positive_tree_resource_premises_v1(
    face_count: usize,
    hinge_count: usize,
    moving_count: usize,
) -> bool {
    let Some(_pair_count) = face_count
        .checked_mul(face_count.saturating_sub(1))
        .map(|product| product / 2)
    else {
        return false;
    };
    (3..=MAX_POSITIVE_ENDPOINT_TREE_FACES_V1).contains(&face_count)
        && hinge_count >= 2
        && hinge_count.checked_add(1) == Some(face_count)
        && moving_count == hinge_count
        && positive_tree_max_angle_degrees_v1(hinge_count).is_some()
}

fn positive_endpoint_candidates_v1(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    paper_thickness_mm: f64,
) -> Option<Vec<(FaceId, FaceId)>> {
    let expansion = paper_thickness_mm / 2.0;
    if !expansion.is_finite() || expansion <= 0.0 {
        return None;
    }
    let mut bounds = Vec::with_capacity(model.face_ids().len());
    let mut world_vertices = HashMap::with_capacity(model.face_ids().len());
    for face in model.face_ids() {
        let transform = pose.face_transform(*face)?;
        let boundary = model.face_boundary(*face)?;
        let mut minimum = [f64::INFINITY; 7];
        let mut maximum = [f64::NEG_INFINITY; 7];
        let mut points = Vec::with_capacity(boundary.vertices().len());
        for vertex in boundary.vertices() {
            let world = transform.apply_point(pose.vertex_position(*vertex)?).ok()?;
            points.push([world.x(), world.y(), world.z()]);
            for (axis, (value, radius)) in [
                (world.x(), expansion),
                (world.y(), expansion),
                (world.z(), expansion),
                (world.x() + world.y(), expansion * std::f64::consts::SQRT_2),
                (world.x() - world.y(), expansion * std::f64::consts::SQRT_2),
                (world.x() + 25.0 * world.y(), expansion * 626.0_f64.sqrt()),
                (world.x() - 25.0 * world.y(), expansion * 626.0_f64.sqrt()),
            ]
            .into_iter()
            .enumerate()
            {
                minimum[axis] = minimum[axis].min(value - radius);
                maximum[axis] = maximum[axis].max(value + radius);
            }
        }
        world_vertices.insert(*face, points);
        bounds.push((*face, minimum, maximum));
    }
    bounds.sort_by(|left, right| {
        left.1[0]
            .total_cmp(&right.1[0])
            .then_with(|| left.0.canonical_bytes().cmp(&right.0.canonical_bytes()))
    });
    let adjacent = |first: FaceId, second: FaceId| {
        model.hinges().iter().any(|hinge| {
            (hinge.left_face() == first && hinge.right_face() == second)
                || (hinge.left_face() == second && hinge.right_face() == first)
        })
    };
    let separated_by_planar_edge = |first: FaceId, second: FaceId| {
        let Some(first_points) = world_vertices.get(&first) else {
            return false;
        };
        let Some(second_points) = world_vertices.get(&second) else {
            return false;
        };
        [first_points, second_points]
            .into_iter()
            .any(|axes_source| {
                (0..axes_source.len()).any(|index| {
                    let start = axes_source[index];
                    let end = axes_source[(index + 1) % axes_source.len()];
                    let axis = [-(end[2] - start[2]), end[0] - start[0]];
                    let norm = axis[0].hypot(axis[1]);
                    if !norm.is_finite() || norm == 0.0 {
                        return false;
                    }
                    let project = |points: &Vec<[f64; 3]>| {
                        points.iter().fold(
                            (f64::INFINITY, f64::NEG_INFINITY),
                            |(minimum, maximum), point| {
                                let value = axis[0] * point[0] + axis[1] * point[2];
                                (minimum.min(value), maximum.max(value))
                            },
                        )
                    };
                    let (first_min, first_max) = project(first_points);
                    let (second_min, second_max) = project(second_points);
                    let radius = expansion * norm;
                    first_max + radius < second_min - radius
                        || second_max + radius < first_min - radius
                })
            })
    };
    let mut candidates = Vec::new();
    for first in 0..bounds.len() {
        for second in first + 1..bounds.len() {
            if bounds[second].1[0] > bounds[first].2[0] {
                break;
            }
            if adjacent(bounds[first].0, bounds[second].0)
                || separated_by_planar_edge(bounds[first].0, bounds[second].0)
                || (1..7).any(|axis| {
                    bounds[first].2[axis] < bounds[second].1[axis]
                        || bounds[second].2[axis] < bounds[first].1[axis]
                })
            {
                continue;
            }
            if candidates
                .len()
                .checked_add(1)
                .is_none_or(|work| !positive_endpoint_pair_work_within_limit_v1(work))
            {
                return None;
            }
            let mut pair = (bounds[first].0, bounds[second].0);
            if pair.1.canonical_bytes() < pair.0.canonical_bytes() {
                pair = (pair.1, pair.0);
            }
            candidates.push(pair);
        }
    }
    candidates.sort_by_key(|pair| (pair.0.canonical_bytes(), pair.1.canonical_bytes()));
    Some(candidates)
}

fn faces_share_material_vertex_v1(
    model: &MaterialTreeKinematicsModel,
    first: FaceId,
    second: FaceId,
) -> bool {
    model.face_boundary(first).is_some_and(|first_boundary| {
        model.face_boundary(second).is_some_and(|second_boundary| {
            first_boundary
                .vertices()
                .iter()
                .any(|vertex| second_boundary.vertices().contains(vertex))
        })
    })
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
    let mut interval_metrics = (0_usize, 0_usize);
    let interval_two_hinge_chain_topology = zero_thickness
        && two_hinge_interval_clearance_premises(
            model,
            initial_pose,
            &moving,
            requested_angle_degrees,
            limits.sample_intervals,
            &mut interval_metrics,
        );
    let positive_two_hinge_topology = positive_thickness
        && positive_tree_resource_premises_v1(
            model.face_ids().len(),
            model.hinges().len(),
            moving.len(),
        )
        && positive_tree_max_angle_degrees_v1(model.hinges().len())
            .is_some_and(|maximum| requested_angle_degrees <= maximum)
        && initial_pose.hinge_angles().iter().all(|angle| {
            moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() == 0.0_f64.to_bits()
        });
    let mut all_positive_thickness_outer_shells = positive_thickness;

    let mut sampled_nonblocking_pose_count = 0;
    let mut first_sampled_blocking_angle_degrees = None;
    let mut positive_endpoint_memo_pair_entries = 0;
    let mut positive_endpoint_exact_pair_calls = 0;
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
            let endpoint_candidates = positive_two_hinge_topology
                .then(|| positive_endpoint_candidates_v1(model, &pose, paper_thickness_mm))
                .flatten();
            let endpoint_topology = if endpoint_candidates.as_ref().is_some_and(|candidates| {
                candidates
                    .iter()
                    .any(|(first, second)| !faces_share_material_vertex_v1(model, *first, *second))
            }) {
                Some(
                    prepare_positive_thickness_tree_endpoint_topology_memo_v1(
                        model,
                        &pose,
                        paper_thickness_mm,
                        limits.static_collision,
                    )
                    .map_err(|_| StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable)?,
                )
            } else {
                None
            };
            all_positive_thickness_outer_shells &= if positive_two_hinge_topology {
                endpoint_candidates.as_ref().is_some()
                    && prepare_swept_tree_hinge_thickness_boundaries_v1(bound, paper_thickness_mm)
                    .ok()
                    .flatten()
                    .is_some_and(|boundary| {
                        revalidate_tree_hinge_thickness_boundaries_v1(
                            &boundary,
                            bound,
                            paper_thickness_mm,
                        )
                        .is_some_and(|observations| observations.len() == model.hinges().len())
                            && model.face_ids().iter().enumerate().all(|(index, first)| {
                                model.face_ids().iter().skip(index + 1).all(|second| {
                                    let adjacent = model.hinges().iter().any(|hinge| {
                                        (hinge.left_face() == *first
                                            && hinge.right_face() == *second)
                                            || (hinge.left_face() == *second
                                                && hinge.right_face() == *first)
                                    });
                                    adjacent
                                        || {
                                            let mut pair = (*first, *second);
                                            if pair.1.canonical_bytes() < pair.0.canonical_bytes() {
                                                pair = (pair.1, pair.0);
                                            }
                                            if !endpoint_candidates
                                                .as_ref()
                                                .is_some_and(|candidates| {
                                                    candidates.contains(&pair)
                                                })
                                            {
                                                return true;
                                            }
                                            positive_endpoint_memo_pair_entries += 1;
                                            faces_share_material_vertex_v1(
                                                model, *first, *second,
                                            ) || endpoint_topology.as_ref().is_some_and(|memo| {
                                                memo.enumerated_pairs()
                                                    == model.face_ids().len()
                                                        * model
                                                            .face_ids()
                                                            .len()
                                                            .saturating_sub(1)
                                                        / 2
                                                    && memo.proves_shared_vertex_pair(*first, *second)
                                            })
                                                || {
                                                    positive_endpoint_exact_pair_calls += 1;
                                                    prepare_positive_thickness_pair_separation_v1(
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
                                                }
                                        }
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
            && positive_tree_max_angle_degrees_v1(model.hinges().len())
                .is_some_and(|maximum| requested_angle_degrees <= maximum)
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
        interval_leaf_count: interval_metrics.0,
        interval_pair_work: interval_metrics.1,
        positive_endpoint_memo_pair_entries,
        positive_endpoint_exact_pair_calls,
        positive_thickness_outer_shell: positive_thickness && all_positive_thickness_outer_shells,
    })
}

fn two_hinge_interval_clearance_premises(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving: &HashSet<EdgeId>,
    requested_angle_degrees: f64,
    interval_count: usize,
    metrics: &mut (usize, usize),
) -> bool {
    let hinge_count = model.hinges().len();
    let face_count = model.face_ids().len();
    let Some(_pair_count) = face_count
        .checked_mul(face_count.saturating_sub(1))
        .map(|n| n / 2)
    else {
        return false;
    };
    if !(2..=MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1).contains(&hinge_count)
        || face_count != hinge_count + 1
        || moving.len() != hinge_count
        || interval_count == 0
        || interval_count > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
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
            if let Some(next) = next
                && let std::collections::hash_map::Entry::Vacant(entry) = depth.entry(next)
            {
                let Some(next_depth) = parent_depth.checked_add(1) else {
                    return false;
                };
                entry.insert(next_depth);
                queue.push_back(next);
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
    // Build one path-wide conservative candidate set. A face at ancestry
    // depth d moves by at most d*r*theta, so pairs omitted by this rest-order
    // sweep remain strictly x-separated throughout every adaptive leaf.
    let full_width_radians = requested_angle_degrees * std::f64::consts::PI / 180.0;
    let mut path_bounds = Vec::with_capacity(face_count);
    for face in model.face_ids() {
        let expansion =
            *depth.get(face).unwrap_or(&usize::MAX) as f64 * maximum_radius * full_width_radians;
        if !expansion.is_finite() {
            return false;
        }
        let Some(transform) = initial_pose.face_transform(*face) else {
            return false;
        };
        let Some(boundary) = model.face_boundary(*face) else {
            return false;
        };
        let mut minimum_x = f64::INFINITY;
        let mut maximum_x = f64::NEG_INFINITY;
        for vertex in boundary.vertices() {
            let Some(point) = initial_pose.vertex_position(*vertex) else {
                return false;
            };
            let Ok(world) = transform.apply_point(point) else {
                return false;
            };
            minimum_x = minimum_x.min(world.x() - expansion);
            maximum_x = maximum_x.max(world.x() + expansion);
        }
        path_bounds.push((*face, minimum_x, maximum_x));
    }
    path_bounds.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.canonical_bytes().cmp(&right.0.canonical_bytes()))
    });
    let mut canonical_candidates = Vec::new();
    for first in 0..path_bounds.len() {
        for second in first + 1..path_bounds.len() {
            if path_bounds[second].1 > path_bounds[first].2 {
                break;
            }
            let pair = (path_bounds[first].0, path_bounds[second].0);
            if !adjacent(pair.0, pair.1) {
                if canonical_candidates.len() >= MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1 {
                    return false;
                }
                canonical_candidates.push(pair);
            }
        }
    }
    canonical_candidates
        .sort_by_key(|(first, second)| (first.canonical_bytes(), second.canonical_bytes()));
    let mut pair_work = 0_usize;
    let mut evaluate = |lower: f64, upper: f64| -> Option<(bool, f64)> {
        let midpoint = (lower + upper) / 2.0;
        let half_width_radians = (upper - lower) * std::f64::consts::PI / 360.0;
        let pose = solve_collective_pose(model, initial_pose, moving, midpoint)?;
        let mut bounds = Vec::new();
        for face in model.face_ids() {
            let expansion = *depth.get(face)? as f64 * maximum_radius * half_width_radians;
            if !expansion.is_finite() {
                return None;
            }
            let transform = pose.face_transform(*face)?;
            let boundary = model.face_boundary(*face)?;
            let mut minimum = [f64::INFINITY; 3];
            let mut maximum = [f64::NEG_INFINITY; 3];
            for vertex in boundary.vertices() {
                let world = transform
                    .apply_point(initial_pose.vertex_position(*vertex)?)
                    .ok()?;
                for (axis, value) in [world.x(), world.y(), world.z()].into_iter().enumerate() {
                    minimum[axis] = minimum[axis].min(value - expansion);
                    maximum[axis] = maximum[axis].max(value + expansion);
                }
            }
            bounds.push((*face, minimum, maximum));
        }
        let bounds = bounds
            .into_iter()
            .map(|(face, minimum, maximum)| (face, (minimum, maximum)))
            .collect::<HashMap<_, _>>();
        let mut strict_margin = f64::INFINITY;
        for (first, second) in &canonical_candidates {
            let first = bounds.get(first)?;
            let second = bounds.get(second)?;
            pair_work = pair_work.checked_add(1)?;
            if pair_work > MAX_STACKED_FOLD_INTERVAL_WORK_V1 {
                return None;
            }
            let pair_margin = (0..3)
                .map(|axis| (second.0[axis] - first.1[axis]).max(first.0[axis] - second.1[axis]))
                .max_by(f64::total_cmp)?;
            strict_margin = strict_margin.min(pair_margin);
        }
        Some((strict_margin > 0.0, strict_margin))
    };
    let mut pending = Vec::with_capacity(interval_count);
    for interval in 0..interval_count {
        let lower = requested_angle_degrees * interval as f64 / interval_count as f64;
        let upper = requested_angle_degrees * (interval + 1) as f64 / interval_count as f64;
        let (certified, margin) = match evaluate(lower, upper) {
            Some(value) => value,
            None => return false,
        };
        pending.push((lower, upper, 0_usize, certified, margin));
    }
    let mut leaf_count = interval_count;
    while !pending.is_empty() {
        // The least separated leaf is refined first. Lower endpoint and depth
        // are stable tie-breakers, independent of model storage order.
        pending.sort_by(|left, right| {
            left.4
                .total_cmp(&right.4)
                .then_with(|| left.0.total_cmp(&right.0))
                .then_with(|| left.2.cmp(&right.2))
        });
        let (lower, upper, subdivision_depth, certified, _) = pending.remove(0);
        if certified {
            continue;
        }
        let midpoint = (lower + upper) / 2.0;
        if subdivision_depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1
            || leaf_count >= MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
            || !midpoint.is_finite()
            || midpoint <= lower
            || midpoint >= upper
        {
            return false;
        }
        leaf_count += 1;
        for (child_lower, child_upper) in [(lower, midpoint), (midpoint, upper)] {
            let (child_certified, child_margin) = match evaluate(child_lower, child_upper) {
                Some(value) => value,
                None => return false,
            };
            pending.push((
                child_lower,
                child_upper,
                subdivision_depth + 1,
                child_certified,
                child_margin,
            ));
        }
    }
    *metrics = (leaf_count, pair_work);
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedFoldCyclePathDiagnosticV1 {
    certified: bool,
    first_closure_failure_angle_degrees: Option<f64>,
    leaf_count: usize,
    pair_work: usize,
    positive_thickness_bits: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UniformCycleClosureRootsV1 {
    Roots(Vec<f64>),
    ProvenInfeasible { examined_leaves: usize },
    Indeterminate { examined_leaves: usize },
}

pub fn enumerate_uniform_cycle_closure_roots_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    initial_angles: &CanonicalHingeAngles,
    moving_edges: &[EdgeId],
    requested_angle_degrees: f64,
    max_leaves: usize,
) -> UniformCycleClosureRootsV1 {
    if !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || max_leaves == 0
        || max_leaves > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
        || audit.closure_hinges().is_empty()
        || moving_edges.is_empty()
    {
        return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
    }
    let moving = moving_edges.iter().copied().collect::<HashSet<_>>();
    let initial_by_edge = initial_angles
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<HashMap<_, _>>();
    if moving.len() != moving_edges.len()
        || initial_angles.as_slice().len() != geometry.hinges().len()
        || geometry.hinges().iter().any(|hinge| {
            !initial_by_edge.contains_key(&hinge.edge())
                || (moving.contains(&hinge.edge())
                    && initial_by_edge
                        .get(&hinge.edge())
                        .is_some_and(|angle| angle.to_bits() != 0.0_f64.to_bits()))
        })
    {
        return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
    }
    let residual = |angle: f64| -> Option<f64> {
        let values = initial_angles
            .as_slice()
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
            .ok()?;
        let angles = CanonicalHingeAngles::new(values).ok()?;
        geometry
            .measure_spanning_closure(audit, fixed_face, &angles)
            .ok()
            .map(|value| value.maximum_error())
    };
    let Some(requested_residual) = residual(requested_angle_degrees) else {
        return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
    };
    let mut scale = 1.0_f64;
    for face in geometry.face_ids() {
        let Some(boundary) = geometry.face_boundary_vertices(*face) else {
            return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
        };
        for vertex in boundary {
            let Some(point) = geometry.vertex_position(*vertex) else {
                return UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 0 };
            };
            scale = scale
                .max(point.x().abs())
                .max(point.y().abs())
                .max(point.z().abs());
        }
    }
    // Each spanning composition performs a bounded number of binary64
    // additions and multiplications per hinge. Gamma(n) with 64 operations
    // per hinge bounds their accumulated forward error at material scale.
    let operation_count = geometry.hinges().len().saturating_mul(64) as f64;
    let roundoff_bound =
        operation_count * f64::EPSILON / (1.0 - operation_count * f64::EPSILON) * scale.max(1.0);
    if requested_residual <= roundoff_bound {
        return UniformCycleClosureRootsV1::Roots(vec![requested_angle_degrees]);
    }
    let lipschitz = (geometry.hinges().len() as f64 * 2.0 + 1.0) * scale.max(1.0);
    let mut pending = vec![(0.0, requested_angle_degrees, 0_usize)];
    let mut roots = Vec::new();
    let mut leaves = 1_usize;
    let mut unresolved = false;
    while let Some((lower, upper, depth)) = pending.pop() {
        let midpoint = (lower + upper) / 2.0;
        let Some(value) = residual(midpoint) else {
            return UniformCycleClosureRootsV1::Indeterminate {
                examined_leaves: leaves,
            };
        };
        if midpoint > 0.0 && value <= roundoff_bound {
            roots.push(midpoint);
            continue;
        }
        let enclosure = lipschitz * (upper - lower) * std::f64::consts::PI / 360.0;
        if value > enclosure {
            continue;
        }
        if leaves >= max_leaves || depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1 {
            unresolved = true;
            continue;
        }
        leaves += 1;
        pending.push((midpoint, upper, depth + 1));
        pending.push((lower, midpoint, depth + 1));
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|a, b| a.to_bits() == b.to_bits());
    if !roots.is_empty() {
        UniformCycleClosureRootsV1::Roots(roots)
    } else if unresolved {
        UniformCycleClosureRootsV1::Indeterminate {
            examined_leaves: leaves,
        }
    } else {
        UniformCycleClosureRootsV1::ProvenInfeasible {
            examined_leaves: leaves,
        }
    }
}

impl StackedFoldCyclePathDiagnosticV1 {
    #[must_use]
    pub const fn continuous_certificate_model_id(&self) -> Option<&'static str> {
        if self.certified {
            Some(if self.positive_thickness_bits.is_some() {
                STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            } else {
                STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            })
        } else {
            None
        }
    }
    #[must_use]
    pub const fn first_closure_failure_angle_degrees(&self) -> Option<f64> {
        self.first_closure_failure_angle_degrees
    }
    #[must_use]
    pub const fn leaf_count(&self) -> usize {
        self.leaf_count
    }
    #[must_use]
    pub const fn pair_work(&self) -> usize {
        self.pair_work
    }
    #[must_use]
    pub const fn positive_thickness_bits(&self) -> Option<u64> {
        self.positive_thickness_bits
    }
}

/// Narrow cycle theorem for a collective, common-axis zero-thickness motion.
/// Closure at zero and one nonzero canonical spanning solution proves the
/// signed common-axis cycle identity; every adaptive midpoint/endpoint is
/// nevertheless revalidated before its swept boxes are admitted.
pub fn diagnose_collective_cycle_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    initial_angles: &CanonicalHingeAngles,
    moving_edges: &[EdgeId],
    requested_angle_degrees: f64,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    let failed = |angle| StackedFoldCyclePathDiagnosticV1 {
        certified: false,
        first_closure_failure_angle_degrees: angle,
        leaf_count: 0,
        pair_work: 0,
        positive_thickness_bits: None,
    };
    if audit.closure_hinges().is_empty()
        || geometry.hinges().len() > MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1
        || interval_count == 0
        || interval_count > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
        || !requested_angle_degrees.is_finite()
        || requested_angle_degrees <= 0.0
        || requested_angle_degrees > 180.0
        || moving_edges.is_empty()
    {
        return failed(None);
    }
    let moving = moving_edges.iter().copied().collect::<HashSet<_>>();
    let initial_by_edge = initial_angles
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<HashMap<_, _>>();
    if moving.len() != moving_edges.len()
        || initial_angles.as_slice().len() != geometry.hinges().len()
        || geometry.hinges().iter().any(|hinge| {
            !initial_by_edge.contains_key(&hinge.edge())
                || (moving.contains(&hinge.edge())
                    && initial_by_edge
                        .get(&hinge.edge())
                        .is_some_and(|angle| angle.to_bits() != 0.0_f64.to_bits()))
        })
    {
        return failed(None);
    }
    let angles_at = |angle: f64| {
        CanonicalHingeAngles::new(
            initial_angles
                .as_slice()
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
                .ok()?,
        )
        .ok()
    };
    let solve = |angle: f64| {
        geometry
            .solve_closed(audit, fixed_face, &angles_at(angle)?, 1.0e-9)
            .ok()
    };
    if solve(0.0).is_none() {
        return failed(Some(0.0));
    }
    let Some(reference) = geometry.hinges().first() else {
        return failed(None);
    };
    let direction = reference.axis();
    if geometry.hinges().iter().skip(1).any(|hinge| {
        !exact_collinear_line(reference.start(), direction, hinge.start(), hinge.axis())
            || !exact_collinear_line(reference.start(), direction, hinge.end(), hinge.axis())
    }) {
        return failed(None);
    }
    let mut maximum_radius = 0.0_f64;
    for face in geometry.face_ids() {
        let Some(boundary) = geometry.face_boundary_vertices(*face) else {
            return failed(None);
        };
        for vertex in boundary {
            let Some(point) = geometry.vertex_position(*vertex) else {
                return failed(None);
            };
            for hinge in geometry.hinges() {
                for origin in [hinge.start(), hinge.end()] {
                    maximum_radius = maximum_radius.max(
                        ((point.x() - origin.x()).powi(2)
                            + (point.y() - origin.y()).powi(2)
                            + (point.z() - origin.z()).powi(2))
                        .sqrt(),
                    );
                }
            }
        }
    }
    if !maximum_radius.is_finite() || maximum_radius == 0.0 {
        return failed(None);
    }
    let adjacent = |a: FaceId, b: FaceId| {
        geometry.hinges().iter().any(|hinge| {
            (hinge.left_face() == a && hinge.right_face() == b)
                || (hinge.left_face() == b && hinge.right_face() == a)
        })
    };
    let mut pending = (0..interval_count)
        .map(|index| {
            (
                requested_angle_degrees * index as f64 / interval_count as f64,
                requested_angle_degrees * (index + 1) as f64 / interval_count as f64,
                0_usize,
            )
        })
        .collect::<Vec<_>>();
    let mut leaves = interval_count;
    let mut work = 0_usize;
    while let Some((lower, upper, depth)) = pending.pop() {
        let midpoint = (lower + upper) / 2.0;
        for angle in [lower, midpoint, upper] {
            if solve(angle).is_none() {
                return failed(Some(angle));
            }
        }
        let Some(pose) = solve(midpoint) else {
            return failed(Some(midpoint));
        };
        let expansion = geometry.hinges().len() as f64
            * maximum_radius
            * (upper - lower)
            * std::f64::consts::PI
            / 360.0;
        let mut bounds = Vec::new();
        for face in geometry.face_ids() {
            let Some(transform) = pose.face_transform(*face) else {
                return failed(Some(midpoint));
            };
            let Some(boundary) = geometry.face_boundary_vertices(*face) else {
                return failed(None);
            };
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for vertex in boundary {
                let Some(point) = geometry.vertex_position(*vertex) else {
                    return failed(None);
                };
                let Ok(world) = transform.apply_point(point) else {
                    return failed(None);
                };
                for (axis, value) in [world.x(), world.y(), world.z()].into_iter().enumerate() {
                    min[axis] = min[axis].min(value - expansion);
                    max[axis] = max[axis].max(value + expansion);
                }
            }
            bounds.push((*face, min, max));
        }
        let mut clear = true;
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                if adjacent(bounds[first].0, bounds[second].0) {
                    continue;
                }
                work = match work.checked_add(1) {
                    Some(v) if v <= MAX_STACKED_FOLD_INTERVAL_WORK_V1 => v,
                    _ => return failed(None),
                };
                if !(0..3).any(|axis| {
                    bounds[first].2[axis] < bounds[second].1[axis]
                        || bounds[second].2[axis] < bounds[first].1[axis]
                }) {
                    clear = false;
                    break;
                }
            }
            if !clear {
                break;
            }
        }
        if !clear {
            if depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1
                || leaves >= MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
            {
                return failed(None);
            }
            leaves += 1;
            pending.push((lower, midpoint, depth + 1));
            pending.push((midpoint, upper, depth + 1));
        }
    }
    StackedFoldCyclePathDiagnosticV1 {
        certified: true,
        first_closure_failure_angle_degrees: None,
        leaf_count: leaves,
        pair_work: work,
        positive_thickness_bits: None,
    }
}

/// Conservatively certifies zero-thickness clearance for the exact same
/// per-hinge schedule already carrying a full-domain closure certificate.
/// This remains observation-only and never authorizes mutation.
pub fn diagnose_scheduled_cycle_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    candidate: &GeneratedMultiHingePathCandidateV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    diagnose_canonical_cycle_schedule_path_v1(
        geometry,
        audit,
        fixed_face,
        candidate.schedule(),
        closure,
        interval_count,
    )
}

/// Certifies a cactus schedule using thickness-expanded swept bounds and exact
/// positive-thickness endpoint/midpoint revalidation on every adaptive leaf.
pub fn diagnose_scheduled_positive_thickness_cycle_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    candidate: &GeneratedMultiHingePathCandidateV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    diagnose_canonical_cycle_schedule_path_internal_v1(
        geometry,
        audit,
        fixed_face,
        candidate.schedule(),
        closure,
        interval_count,
        Some(paper_thickness_mm),
    )
}

/// Runs the bounded cycle CCD oracle against a canonical schedule directly.
/// Schedule families without point evaluation or a finite derivative bound
/// remain explicitly uncertified; closure evidence alone is never clearance.
pub fn diagnose_canonical_cycle_schedule_path_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
) -> StackedFoldCyclePathDiagnosticV1 {
    diagnose_canonical_cycle_schedule_path_internal_v1(
        geometry,
        audit,
        fixed_face,
        schedule,
        closure,
        interval_count,
        None,
    )
}

fn diagnose_canonical_cycle_schedule_path_internal_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
    paper_thickness_mm: Option<f64>,
) -> StackedFoldCyclePathDiagnosticV1 {
    let failed = || StackedFoldCyclePathDiagnosticV1 {
        certified: false,
        first_closure_failure_angle_degrees: None,
        leaf_count: 0,
        pair_work: 0,
        positive_thickness_bits: None,
    };
    if interval_count == 0
        || interval_count > MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
        || !closure.every_leaf_covers_graph_v1(geometry)
        || closure.fixed_face() != fixed_face
        || closure.schedule_binding_fingerprint_v1()
            != schedule.certificate_binding_fingerprint_v1()
        || closure.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || !schedule.matches_binding(geometry, audit, fixed_face)
        || paper_thickness_mm.is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return failed();
    }
    let derivative_sum = geometry
        .hinges()
        .iter()
        .try_fold(0.0, |sum, hinge| {
            schedule
                .derivative_bound(hinge.edge())
                .map(|bound| sum + bound)
        })
        .filter(|value| value.is_finite());
    let Some(derivative_sum) = derivative_sum else {
        return failed();
    };
    if paper_thickness_mm.is_none()
        && scheduled_collinear_flat_stack_premises_v1(geometry, audit, fixed_face, schedule)
    {
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: 0,
            positive_thickness_bits: None,
        };
    }
    if paper_thickness_mm.is_none()
        && scheduled_kawasaki_120_120_60_60_premises_v1(geometry, audit, fixed_face, schedule)
    {
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: 0,
            positive_thickness_bits: None,
        };
    }
    let mut maximum_radius = 0.0_f64;
    for face in geometry.face_ids() {
        let Some(boundary) = geometry.face_boundary_vertices(*face) else {
            return failed();
        };
        for vertex in boundary {
            let Some(point) = geometry.vertex_position(*vertex) else {
                return failed();
            };
            for hinge in geometry.hinges() {
                let origin = hinge.start();
                maximum_radius = maximum_radius.max(
                    ((point.x() - origin.x()).powi(2)
                        + (point.y() - origin.y()).powi(2)
                        + (point.z() - origin.z()).powi(2))
                    .sqrt(),
                );
            }
        }
    }
    if !maximum_radius.is_finite() {
        return failed();
    }
    let adjacent = |a: FaceId, b: FaceId| {
        geometry.hinges().iter().any(|hinge| {
            (hinge.left_face() == a && hinge.right_face() == b)
                || (hinge.left_face() == b && hinge.right_face() == a)
        })
    };
    let local_symmetric_groups = composed_symmetric_rational_local_groups_v1(
        geometry, audit, fixed_face, schedule,
    )
    .or_else(|| rational_cactus_star_local_groups_v1(geometry, audit, fixed_face, schedule));
    if paper_thickness_mm.is_none()
        && local_symmetric_groups
            .as_ref()
            .is_some_and(|groups| symmetric_groups_have_disjoint_swept_balls_v1(geometry, groups))
    {
        let group_count = audit.closure_hinges().len();
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: group_count * (group_count - 1) / 2,
            positive_thickness_bits: None,
        };
    }
    if let (Some(thickness), Some(groups)) = (paper_thickness_mm, &local_symmetric_groups)
        && symmetric_groups_have_disjoint_positive_swept_balls_v1(geometry, groups, thickness)
    {
        let group_count = audit.closure_hinges().len();
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: group_count * (group_count - 1) / 2,
            positive_thickness_bits: Some(thickness.to_bits()),
        };
    }
    if let Some(thickness) = paper_thickness_mm
        && theta_collective_axis_positive_thickness_premises_v1(
            geometry, audit, fixed_face, schedule, closure, thickness,
        )
    {
        return StackedFoldCyclePathDiagnosticV1 {
            certified: true,
            first_closure_failure_angle_degrees: None,
            leaf_count: closure.leaves().len(),
            pair_work: geometry.face_ids().len() * (geometry.face_ids().len() - 1) / 2,
            positive_thickness_bits: Some(thickness.to_bits()),
        };
    }
    let mut pending = (0..interval_count)
        .map(|index| {
            (
                index as f64 / interval_count as f64,
                (index + 1) as f64 / interval_count as f64,
                0usize,
            )
        })
        .collect::<Vec<_>>();
    let mut leaves = interval_count;
    let mut work = 0usize;
    while let Some((lower, upper, depth)) = pending.pop() {
        let midpoint = (lower + upper) * 0.5;
        let Some(angles) = schedule.evaluate(midpoint) else {
            return failed();
        };
        let Ok(pose) = geometry.solve_closed(audit, fixed_face, &angles, 1.0e-9) else {
            return failed();
        };
        if let Some(thickness) = paper_thickness_mm {
            for progress in [lower, midpoint, upper] {
                let Some(exact_angles) = schedule.evaluate(progress) else {
                    return failed();
                };
                let Ok(exact_pose) =
                    geometry.solve_closed(audit, fixed_face, &exact_angles, 1.0e-9)
                else {
                    return failed();
                };
                if prove_positive_thickness_graph_geometry_v1(
                    geometry,
                    &exact_pose,
                    thickness,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .is_err()
                {
                    return failed();
                }
            }
        }
        let expansion = maximum_radius * derivative_sum * (upper - lower) * std::f64::consts::PI
            / 180.0
            + paper_thickness_mm.unwrap_or(0.0) * 0.5;
        let mut bounds = Vec::new();
        for face in geometry.face_ids() {
            let (Some(transform), Some(boundary)) = (
                pose.face_transform(*face),
                geometry.face_boundary_vertices(*face),
            ) else {
                return failed();
            };
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for vertex in boundary {
                let Some(point) = geometry.vertex_position(*vertex) else {
                    return failed();
                };
                let Ok(world) = transform.apply_point(point) else {
                    return failed();
                };
                for (axis, value) in [world.x(), world.y(), world.z()].into_iter().enumerate() {
                    min[axis] = min[axis].min(value - expansion);
                    max[axis] = max[axis].max(value + expansion);
                }
            }
            bounds.push((*face, min, max));
        }
        let mut clear = true;
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                if adjacent(bounds[first].0, bounds[second].0) {
                    continue;
                }
                if local_symmetric_groups.as_ref().is_some_and(|groups| {
                    !groups.contains_key(&bounds[first].0)
                        || !groups.contains_key(&bounds[second].0)
                        || groups.get(&bounds[first].0) == groups.get(&bounds[second].0)
                }) {
                    continue;
                }
                work = match work.checked_add(1) {
                    Some(value) if value <= MAX_STACKED_FOLD_INTERVAL_WORK_V1 => value,
                    _ => return failed(),
                };
                if !(0..3).any(|axis| {
                    bounds[first].2[axis] < bounds[second].1[axis]
                        || bounds[second].2[axis] < bounds[first].1[axis]
                }) {
                    clear = false;
                    break;
                }
            }
        }
        if !clear {
            if depth >= MAX_STACKED_FOLD_INTERVAL_DEPTH_V1
                || leaves >= MAX_STACKED_FOLD_INTERVAL_LEAVES_V1
            {
                return failed();
            }
            leaves += 1;
            pending.push((midpoint, upper, depth + 1));
            pending.push((lower, midpoint, depth + 1));
        }
    }
    StackedFoldCyclePathDiagnosticV1 {
        certified: true,
        first_closure_failure_angle_degrees: None,
        leaf_count: leaves,
        pair_work: work,
        positive_thickness_bits: paper_thickness_mm.map(f64::to_bits),
    }
}

fn theta_collective_axis_positive_thickness_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    paper_thickness_mm: f64,
) -> bool {
    if geometry.face_ids().len() != 6
        || geometry.hinges().len() != 7
        || audit.closure_hinges().len() != 2
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
        || !closure.every_leaf_covers_graph_v1(geometry)
        || closure.fixed_face() != fixed_face
    {
        return false;
    }
    let Some(moving) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    if moving.len() != 3 {
        return false;
    }
    let moving_hinges = geometry
        .hinges()
        .iter()
        .filter(|hinge| moving.contains(&hinge.edge()))
        .collect::<Vec<_>>();
    let Some(reference) = moving_hinges.first() else {
        return false;
    };
    moving_hinges.iter().skip(1).all(|hinge| {
        exact_collinear_line(
            reference.start(),
            reference.axis(),
            hinge.start(),
            hinge.axis(),
        ) && exact_collinear_line(
            reference.start(),
            reference.axis(),
            hinge.end(),
            hinge.axis(),
        )
    }) && [0.0, 1.0].into_iter().all(|progress| {
        schedule.evaluate(progress).is_some_and(|angles| {
            angles
                .as_slice()
                .iter()
                .all(|angle| angle.angle_degrees() >= 0.0 && angle.angle_degrees() < 90.0)
                && geometry
                    .solve_closed(audit, fixed_face, &angles, 1.0e-9)
                    .is_ok()
        })
    })
}

fn composed_symmetric_rational_local_groups_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> Option<HashMap<FaceId, usize>> {
    let count = audit.closure_hinges().len();
    if !(2..=16).contains(&count)
        || geometry.hinges().len() != count * 4
        || geometry.face_ids().len() != 1 + count * 3
    {
        return None;
    }
    let mut remaining = geometry
        .face_ids()
        .iter()
        .copied()
        .filter(|face| *face != fixed_face)
        .collect::<HashSet<_>>();
    let mut result = HashMap::new();
    for group_index in 0..count {
        let seed = *remaining.iter().next()?;
        let mut stack = vec![seed];
        let mut faces = HashSet::new();
        while let Some(face) = stack.pop() {
            if !remaining.remove(&face) {
                continue;
            }
            faces.insert(face);
            for hinge in geometry.hinges() {
                if hinge.left_face() == face && hinge.right_face() != fixed_face {
                    stack.push(hinge.right_face());
                } else if hinge.right_face() == face && hinge.left_face() != fixed_face {
                    stack.push(hinge.left_face());
                }
            }
        }
        if faces.len() != 3 {
            return None;
        }
        let edges = geometry
            .hinges()
            .iter()
            .filter(|hinge| {
                faces.contains(&hinge.left_face()) || faces.contains(&hinge.right_face())
            })
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        schedule.bounded_symmetric_kawasaki_profile_for_edges_v1(&edges)?;
        for face in faces {
            result.insert(face, group_index);
        }
    }
    (!result.is_empty() && remaining.is_empty()).then_some(result)
}

fn rational_cactus_star_local_groups_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> Option<HashMap<FaceId, usize>> {
    let count = audit.closure_hinges().len();
    if !(2..=16).contains(&count)
        || geometry.hinges().len() != count * 4
        || geometry.face_ids().len() != 1 + count * 3
    {
        return None;
    }
    for shared in geometry
        .face_ids()
        .iter()
        .copied()
        .filter(|face| *face != fixed_face)
    {
        let mut remaining = geometry
            .face_ids()
            .iter()
            .copied()
            .filter(|face| *face != shared)
            .collect::<HashSet<_>>();
        let mut result = HashMap::new();
        let mut valid = true;
        for group_index in 0..count {
            let Some(seed) = remaining.iter().next().copied() else {
                valid = false;
                break;
            };
            let mut stack = vec![seed];
            let mut faces = HashSet::new();
            while let Some(face) = stack.pop() {
                if !remaining.remove(&face) {
                    continue;
                }
                faces.insert(face);
                for hinge in geometry.hinges() {
                    if hinge.left_face() == face && hinge.right_face() != shared {
                        stack.push(hinge.right_face());
                    } else if hinge.right_face() == face && hinge.left_face() != shared {
                        stack.push(hinge.left_face());
                    }
                }
            }
            let edges = geometry
                .hinges()
                .iter()
                .filter(|hinge| {
                    faces.contains(&hinge.left_face()) || faces.contains(&hinge.right_face())
                })
                .map(|hinge| hinge.edge())
                .collect::<Vec<_>>();
            if faces.len() != 3
                || schedule
                    .bounded_symmetric_kawasaki_profile_for_edges_v1(&edges)
                    .is_none()
            {
                valid = false;
                break;
            }
            for face in faces {
                result.insert(face, group_index);
            }
        }
        if valid && remaining.is_empty() && result.len() == count * 3 {
            return Some(result);
        }
    }
    None
}

fn symmetric_groups_have_disjoint_swept_balls_v1(
    geometry: &MaterialHingeGraphGeometry,
    groups: &HashMap<FaceId, usize>,
) -> bool {
    let group_count = groups.values().copied().max().map_or(0, |value| value + 1);
    let mut balls = Vec::with_capacity(group_count);
    for group in 0..group_count {
        let hinges = geometry
            .hinges()
            .iter()
            .filter(|hinge| {
                groups.get(&hinge.left_face()) == Some(&group)
                    || groups.get(&hinge.right_face()) == Some(&group)
            })
            .collect::<Vec<_>>();
        if hinges.len() != 4 {
            return false;
        }
        let candidates = [hinges[0].start(), hinges[0].end()];
        let Some(pivot) = candidates.into_iter().find(|candidate| {
            hinges
                .iter()
                .all(|hinge| hinge.start() == *candidate || hinge.end() == *candidate)
        }) else {
            return false;
        };
        let mut radius = 0.0_f64;
        for face in geometry
            .face_ids()
            .iter()
            .filter(|face| groups.get(face) == Some(&group))
        {
            let Some(boundary) = geometry.face_boundary_vertices(*face) else {
                return false;
            };
            for vertex in boundary {
                let Some(point) = geometry.vertex_position(*vertex) else {
                    return false;
                };
                radius = radius.max(
                    ((point.x() - pivot.x()).powi(2)
                        + (point.y() - pivot.y()).powi(2)
                        + (point.z() - pivot.z()).powi(2))
                    .sqrt(),
                );
            }
        }
        balls.push((pivot, radius));
    }
    (0..balls.len()).all(|first| {
        (first + 1..balls.len()).all(|second| {
            let distance = ((balls[first].0.x() - balls[second].0.x()).powi(2)
                + (balls[first].0.y() - balls[second].0.y()).powi(2)
                + (balls[first].0.z() - balls[second].0.z()).powi(2))
            .sqrt();
            distance.is_finite()
                && balls[first].1.is_finite()
                && balls[second].1.is_finite()
                && distance > balls[first].1 + balls[second].1
        })
    })
}

fn symmetric_groups_have_disjoint_positive_swept_balls_v1(
    geometry: &MaterialHingeGraphGeometry,
    groups: &HashMap<FaceId, usize>,
    paper_thickness_mm: f64,
) -> bool {
    if !paper_thickness_mm.is_finite() || paper_thickness_mm <= 0.0 {
        return false;
    }
    let group_count = groups.values().copied().max().map_or(0, |value| value + 1);
    let mut balls = Vec::with_capacity(group_count);
    for group in 0..group_count {
        let hinges = geometry
            .hinges()
            .iter()
            .filter(|hinge| {
                groups.get(&hinge.left_face()) == Some(&group)
                    || groups.get(&hinge.right_face()) == Some(&group)
            })
            .collect::<Vec<_>>();
        if hinges.len() != 4 {
            return false;
        }
        let Some(pivot) = [hinges[0].start(), hinges[0].end()]
            .into_iter()
            .find(|candidate| {
                hinges
                    .iter()
                    .all(|hinge| hinge.start() == *candidate || hinge.end() == *candidate)
            })
        else {
            return false;
        };
        let mut radius = paper_thickness_mm * 0.5;
        for face in geometry
            .face_ids()
            .iter()
            .filter(|face| groups.get(face) == Some(&group))
        {
            let Some(boundary) = geometry.face_boundary_vertices(*face) else {
                return false;
            };
            for vertex in boundary {
                let Some(point) = geometry.vertex_position(*vertex) else {
                    return false;
                };
                radius = radius.max(
                    ((point.x() - pivot.x()).powi(2)
                        + (point.y() - pivot.y()).powi(2)
                        + (point.z() - pivot.z()).powi(2))
                    .sqrt()
                        + paper_thickness_mm * 0.5,
                );
            }
        }
        balls.push((pivot, radius));
    }
    (0..balls.len()).all(|first| {
        (first + 1..balls.len()).all(|second| {
            let distance = ((balls[first].0.x() - balls[second].0.x()).powi(2)
                + (balls[first].0.y() - balls[second].0.y()).powi(2)
                + (balls[first].0.z() - balls[second].0.z()).powi(2))
            .sqrt();
            distance.is_finite()
                && balls[first].1.is_finite()
                && balls[second].1.is_finite()
                && distance > balls[first].1 + balls[second].1
        })
    })
}

// Collision-free branch of the convex 120/120/60/60 bird-foot vertex for
// 0 <= tan(rho_BC/2) <= 1.  The exact rational schedule fixes the one-DOF
// mode, the single M crease selects its non-self-intersecting branch, and all
// material faces are triangles meeting at the one physical vertex.
fn scheduled_kawasaki_120_120_60_60_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> bool {
    if geometry.hinges().len() != 4
        || geometry.face_ids().len() != 4
        || audit.closure_hinges().len() != 1
    {
        return false;
    }
    let Some((unit, half)) = schedule.kawasaki_120_120_60_60_half_angle_pairs_v1() else {
        return false;
    };
    let half = half.into_iter().collect::<HashSet<_>>();
    if unit.len() != 2
        || geometry
            .hinges()
            .iter()
            .filter(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain)
            .count()
            != 1
        || geometry
            .hinges()
            .iter()
            .find(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain)
            .is_none_or(|hinge| !half.contains(&hinge.edge()))
    {
        return false;
    }
    let pivot = geometry.hinges()[0].end();
    let same = |a: ori_kinematics::Point3, b: ori_kinematics::Point3| {
        a.x().to_bits() == b.x().to_bits()
            && a.y().to_bits() == b.y().to_bits()
            && a.z().to_bits() == b.z().to_bits()
    };
    if geometry
        .hinges()
        .iter()
        .any(|hinge| !same(hinge.end(), pivot) || same(hinge.start(), pivot))
        || geometry.face_ids().iter().any(|face| {
            geometry
                .face_boundary_vertices(*face)
                .is_none_or(|boundary| boundary.len() != 3)
        })
    {
        return false;
    }
    [0.0, 0.5, 1.0].into_iter().all(|u| {
        schedule.evaluate(u).is_some_and(|angles| {
            geometry
                .solve_closed(audit, fixed_face, &angles, 1.0e-9)
                .is_ok()
        })
    })
}

// Narrow zero-thickness theorem for folding an already flat stack. Exact
// projective-profile equality prevents sampled schedules from impersonating a
// collective motion. Constant hinges must remain bit-exact 180 degrees, all
// moving hinges start at bit-exact zero, and their initial world axes must be
// one exact infinite line. The tree composition can therefore only rotate
// each flat layer about that same line; distinct layer planes meet on the fold
// line, while equal-angle layers preserve their pre-existing flat ordering.
fn scheduled_collinear_flat_stack_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &ori_kinematics::CanonicalCycleScheduleV1,
) -> bool {
    let Some(moving_edges) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    if moving_edges.len() < 2 {
        return false;
    }
    let moving = moving_edges.into_iter().collect::<HashSet<_>>();
    let (Some(initial_angles), Some(midpoint_angles), Some(requested_angles)) = (
        schedule.evaluate(0.0),
        schedule.evaluate(0.5),
        schedule.evaluate(1.0),
    ) else {
        return false;
    };
    let requested_moving = requested_angles
        .as_slice()
        .iter()
        .filter(|angle| moving.contains(&angle.edge()))
        .map(|angle| angle.angle_degrees().to_bits())
        .collect::<HashSet<_>>();
    let initial_moving = initial_angles
        .as_slice()
        .iter()
        .filter(|angle| moving.contains(&angle.edge()))
        .map(|angle| angle.angle_degrees().to_bits())
        .collect::<HashSet<_>>();
    if requested_moving.len() != 1
        || initial_moving.len() != 1
        || requested_moving.iter().next().is_none_or(|bits| {
            let angle = f64::from_bits(*bits);
            !angle.is_finite() || angle <= 0.0 || angle >= 180.0
        })
        || initial_angles.as_slice().iter().any(|angle| {
            !moving.contains(&angle.edge())
                && angle.angle_degrees().to_bits() != 180.0_f64.to_bits()
        })
        || requested_angles.as_slice().iter().any(|angle| {
            !moving.contains(&angle.edge())
                && angle.angle_degrees().to_bits() != 180.0_f64.to_bits()
        })
    {
        return false;
    }
    let Ok(initial_pose) = geometry.solve_closed(audit, fixed_face, &initial_angles, 1.0e-9) else {
        return false;
    };
    let mut moving_hinges = geometry
        .hinges()
        .iter()
        .filter(|hinge| moving.contains(&hinge.edge()));
    let Some(reference) = moving_hinges.next() else {
        return false;
    };
    let Some(reference_transform) = initial_pose.face_transform(reference.left_face()) else {
        return false;
    };
    let (Ok(reference_start), Ok(reference_end), Ok(reference_axis)) = (
        reference_transform.apply_point(reference.start()),
        reference_transform.apply_point(reference.end()),
        reference_transform.apply_vector(reference.axis()),
    ) else {
        return false;
    };
    if !moving_hinges.all(|hinge| {
        let Some(transform) = initial_pose.face_transform(hinge.left_face()) else {
            return false;
        };
        let (Ok(start), Ok(end), Ok(axis)) = (
            transform.apply_point(hinge.start()),
            transform.apply_point(hinge.end()),
            transform.apply_vector(hinge.axis()),
        ) else {
            return false;
        };
        bounded_collinear_line(reference_start, reference_axis, start, axis, 1.0e-9)
            && bounded_collinear_line(reference_start, reference_axis, end, axis, 1.0e-9)
            && bounded_collinear_line(reference_start, reference_axis, reference_end, axis, 1.0e-9)
    }) {
        return false;
    }
    [midpoint_angles, requested_angles]
        .into_iter()
        .all(|angles| {
            geometry
                .solve_closed(audit, fixed_face, &angles, 1.0e-9)
                .ok()
                .is_some_and(|pose| {
                    graph_pose_preserves_common_axis_layers(
                        geometry,
                        &initial_pose,
                        &pose,
                        reference_start,
                        reference_end,
                    )
                })
        })
}

fn bounded_collinear_line(
    origin: ori_kinematics::Point3,
    axis: ori_kinematics::Point3,
    point: ori_kinematics::Point3,
    candidate_axis: ori_kinematics::Point3,
    tolerance: f64,
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
    let axis_error = cross(reference, candidate);
    let offset_error = cross(offset, reference);
    let offset_scale = offset.into_iter().map(f64::abs).fold(1.0_f64, f64::max);
    axis_error.into_iter().all(|value| value.abs() <= tolerance)
        && offset_error
            .into_iter()
            .all(|value| value.abs() <= tolerance * offset_scale)
}

fn graph_pose_preserves_common_axis_layers(
    geometry: &MaterialHingeGraphGeometry,
    initial_pose: &ori_kinematics::ClosedMaterialHingeGraphPose,
    pose: &ori_kinematics::ClosedMaterialHingeGraphPose,
    axis_start: ori_kinematics::Point3,
    axis_end: ori_kinematics::Point3,
) -> bool {
    let tolerance = 1.0e-9
        * [
            axis_start.x().abs(),
            axis_start.y().abs(),
            axis_start.z().abs(),
            axis_end.x().abs(),
            axis_end.y().abs(),
            axis_end.z().abs(),
            1.0,
        ]
        .into_iter()
        .fold(1.0_f64, f64::max);
    let fixes = |actual: ori_kinematics::Point3, expected: ori_kinematics::Point3| {
        (actual.x() - expected.x()).abs() <= tolerance
            && (actual.y() - expected.y()).abs() <= tolerance
            && (actual.z() - expected.z()).abs() <= tolerance
    };
    let mut moved = false;
    for face in geometry.face_ids() {
        let (Some(initial_transform), Some(transform)) = (
            initial_pose.face_transform(*face),
            pose.face_transform(*face),
        ) else {
            return false;
        };
        let Ok(transform) = transform.relative_to(initial_transform) else {
            return false;
        };
        let (Ok(start), Ok(end)) = (
            transform.apply_point(axis_start),
            transform.apply_point(axis_end),
        ) else {
            return false;
        };
        if !fixes(start, axis_start) || !fixes(end, axis_end) {
            return false;
        }
        moved |= transform != ori_kinematics::RigidTransform::identity();
    }
    moved
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
#[path = "../../../test-support/four_bay_cycle.rs"]
mod four_bay_cycle_test_support;

#[cfg(test)]
mod tests {
    use crate::prepare_tree_hinge_thickness_boundaries_v1;
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
    use ori_kinematics::TreeKinematicsLimits;
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use super::*;

    fn fixed_id<T: serde::de::DeserializeOwned>(prefix: &str, index: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{index:012x}\"")).unwrap()
    }

    fn rational_cycle_bay_geometry(
        group_count: usize,
        reverse_hinges: bool,
    ) -> (
        MaterialHingeGraphGeometry,
        MaterialHingeGraphAudit,
        ori_kinematics::CanonicalCycleScheduleV1,
        FaceId,
    ) {
        let triples = [
            (3.0, 5.0, 4.0),
            (5.0, 13.0, 12.0),
            (8.0, 17.0, 15.0),
            (7.0, 25.0, 24.0),
            (3.0, 5.0, 4.0),
            (5.0, 13.0, 12.0),
            (8.0, 17.0, 15.0),
            (7.0, 25.0, 24.0),
            (3.0, 5.0, 4.0),
            (5.0, 13.0, 12.0),
            (8.0, 17.0, 15.0),
            (7.0, 25.0, 24.0),
            (3.0, 5.0, 4.0),
            (5.0, 13.0, 12.0),
            (8.0, 17.0, 15.0),
            (7.0, 25.0, 24.0),
        ];
        let (pattern, paper, hinges) = if reverse_hinges && group_count == 16 {
            super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern_with_reversed_hinges()
        } else if reverse_hinges && group_count == 8 {
            super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern_with_reversed_hinges()
        } else if reverse_hinges {
            super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern_with_reversed_hinges(
            )
        } else if group_count == 16 {
            super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern()
        } else if group_count == 8 {
            super::four_bay_cycle_test_support::eight_bay_rational_cycle_pattern()
        } else {
            super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern()
        };
        let analysis = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b600", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        let topology = analysis
            .snapshot
            .unwrap_or_else(|| panic!("four non-crossing rational bays: {:?}", analysis.issues));
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let fixed = topology
            .faces
            .iter()
            .max_by_key(|face| {
                topology
                    .hinge_adjacency
                    .iter()
                    .filter(|adjacency| adjacency.first == face.id || adjacency.second == face.id)
                    .count()
            })
            .unwrap()
            .id;
        let mut inputs = hinges
            .into_iter()
            .enumerate()
            .map(|(index, edge)| {
                let (p, q, _) = triples[index / 4];
                ori_kinematics::HalfAngleRationalEntryInputV1 {
                    edge,
                    u_domain: [
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: vec![
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: if index % 2 == 0 { 1 } else { p as i64 },
                            denominator: 1,
                        },
                    ],
                    denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                        numerator: if index % 2 == 0 { 1 } else { q as i64 },
                        denominator: 1,
                    }],
                }
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs,
            ori_kinematics::CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        (geometry, audit, schedule, fixed)
    }

    #[test]
    fn four_non_crossing_rational_bays_admit_real_geometry_and_four_leaf_closure() {
        let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
        assert_eq!(geometry.hinges().len(), 16);
        assert_eq!(geometry.face_ids().len(), 13);
        assert!(geometry.face_ids().iter().all(|face| {
            geometry
                .face_boundary_vertices(*face)
                .is_some_and(|boundary| boundary.len() >= 3)
        }));
        for u in [0.0, 0.5, 1.0] {
            let angles = schedule.evaluate(u).unwrap();
            geometry
                .solve_closed(&audit, fixed, &angles, 1.0e-8)
                .unwrap();
        }
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    max_depth: 2,
                    max_leaves: 4,
                    max_work: 4,
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
                },
            )
            .expect("four-leaf real-geometry closure");
        assert_eq!(closure.leaves().len(), 4);
        assert!(closure.every_leaf_covers_graph_v1(&geometry));
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
            schedule, &initial, &requested,
        )
        .unwrap();
        let groups = composed_symmetric_rational_local_groups_v1(
            &geometry,
            &audit,
            fixed,
            candidate.schedule(),
        )
        .unwrap();
        assert!(symmetric_groups_have_disjoint_swept_balls_v1(
            &geometry, &groups
        ));
        let mut cross_leaf_collision = groups.clone();
        let foreign_face = *groups.iter().find(|(_, group)| **group == 3).unwrap().0;
        cross_leaf_collision.insert(foreign_face, 2);
        assert!(!symmetric_groups_have_disjoint_swept_balls_v1(
            &geometry,
            &cross_leaf_collision
        ));
        let diagnostic = diagnose_scheduled_cycle_path_v1(
            &geometry,
            &audit,
            fixed,
            &candidate,
            &closure,
            MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 - 1,
        );
        assert!(
            diagnostic.continuous_certificate_model_id().is_some(),
            "real four-leaf CCD must certify: {diagnostic:?}"
        );
        assert_eq!(diagnostic.pair_work(), 6);
        for cancelled_or_excessive in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
            assert!(
                diagnose_scheduled_cycle_path_v1(
                    &geometry,
                    &audit,
                    fixed,
                    &candidate,
                    &closure,
                    cancelled_or_excessive,
                )
                .continuous_certificate_model_id()
                .is_none()
            );
        }
        let retried =
            diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
        assert_eq!(retried, diagnostic);
        for thickness in [0.1, 1.0, 3.0] {
            let positive = diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
            );
            assert_eq!(
                positive.continuous_certificate_model_id(),
                Some(STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
            );
            assert_eq!(
                positive.positive_thickness_bits(),
                Some(thickness.to_bits())
            );
            assert_ne!(
                positive,
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry,
                    &audit,
                    fixed,
                    &candidate,
                    &closure,
                    f64::from_bits(thickness.to_bits() + 1),
                    32,
                ),
                "bit-distinct thickness must not replay the same proof"
            );
            for one_short_or_cancelled in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
                assert!(
                    diagnose_scheduled_positive_thickness_cycle_path_v1(
                        &geometry,
                        &audit,
                        fixed,
                        &candidate,
                        &closure,
                        thickness,
                        one_short_or_cancelled,
                    )
                    .continuous_certificate_model_id()
                    .is_none()
                );
            }
        }
        for invalid_thickness in [0.0, -0.1, f64::NAN, f64::INFINITY] {
            assert!(
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry,
                    &audit,
                    fixed,
                    &candidate,
                    &closure,
                    invalid_thickness,
                    32,
                )
                .continuous_certificate_model_id()
                .is_none()
            );
        }
        let first = crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32, [0x41; 32], [0x42; 32],
        )
        .unwrap();
        let second = crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32, [0x41; 32], [0x42; 32],
        )
        .unwrap();
        assert_eq!(first.schedule_certificate(), second.schedule_certificate());
        assert_eq!(first.closure_certificate(), second.closure_certificate());
        assert_eq!(
            first.collision_certificate(),
            second.collision_certificate()
        );
        let (reversed_geometry, reversed_audit, reversed_schedule, reversed_fixed) =
            rational_cycle_bay_geometry(4, true);
        let reversed_closure = reversed_geometry
            .prove_dyadic_schedule_closure_v1(
                &reversed_audit,
                reversed_fixed,
                &reversed_schedule,
                1.0e-8,
                ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    max_depth: 2,
                    max_leaves: 4,
                    max_work: 4,
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        let reversed_initial = reversed_schedule.evaluate(0.0).unwrap();
        let reversed_requested = reversed_schedule.evaluate(1.0).unwrap();
        let reversed_candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
            reversed_schedule,
            &reversed_initial,
            &reversed_requested,
        )
        .unwrap();
        let reversed = crate::certify_scheduled_cycle_transition_v1(
            &reversed_geometry,
            &reversed_audit,
            reversed_fixed,
            &reversed_candidate,
            &reversed_closure,
            32,
            [0x41; 32],
            [0x42; 32],
        )
        .unwrap();
        assert_eq!(
            first.schedule_certificate(),
            reversed.schedule_certificate()
        );
        assert_eq!(first.closure_certificate(), reversed.closure_certificate());
        assert_eq!(
            first.collision_certificate(),
            reversed.collision_certificate()
        );
        for thickness in [0.1, 1.0, 3.0] {
            assert_eq!(
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
                ),
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &reversed_geometry,
                    &reversed_audit,
                    reversed_fixed,
                    &reversed_candidate,
                    &reversed_closure,
                    thickness,
                    32,
                )
            );
        }
    }

    #[test]
    fn cactus_star_groups_three_or_more_cycles_around_an_articulation_face() {
        let (geometry, audit, schedule, _common) = rational_cycle_bay_geometry(4, false);
        let exclusive = geometry
            .face_ids()
            .iter()
            .copied()
            .min_by_key(|face| {
                geometry
                    .hinges()
                    .iter()
                    .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                    .count()
            })
            .unwrap();
        let groups = rational_cactus_star_local_groups_v1(&geometry, &audit, exclusive, &schedule)
            .expect("four-cycle cactus block-cut star");
        assert_eq!(groups.len(), 12);
        assert_eq!(groups.values().copied().collect::<HashSet<_>>().len(), 4);
        assert!(symmetric_groups_have_disjoint_swept_balls_v1(
            &geometry, &groups
        ));
    }

    #[test]
    fn eight_bay_real_geometry_admits_exact_balanced_closure() {
        let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(8, false);
        let limits = ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_depth: 3,
            max_leaves: 8,
            max_work: 8,
            schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
        };
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, limits)
            .unwrap();
        assert_eq!(geometry.hinges().len(), 32);
        assert_eq!(closure.leaves().len(), 8);
        assert!(closure.leaves().iter().all(|leaf| leaf.0 == 3));
        for short in [
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                ..limits
            },
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_leaves: 7,
                ..limits
            },
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_work: 7,
                ..limits
            },
        ] {
            assert_eq!(
                geometry.prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, short,),
                Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
            );
        }
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
            schedule, &initial, &requested,
        )
        .unwrap();
        let groups = composed_symmetric_rational_local_groups_v1(
            &geometry,
            &audit,
            fixed,
            candidate.schedule(),
        )
        .unwrap();
        assert!(symmetric_groups_have_disjoint_swept_balls_v1(
            &geometry, &groups
        ));
        let mut collision = groups.clone();
        let foreign = *groups.iter().find(|(_, group)| **group == 7).unwrap().0;
        collision.insert(foreign, 6);
        assert!(!symmetric_groups_have_disjoint_swept_balls_v1(
            &geometry, &collision
        ));
        let diagnostic =
            diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
        assert!(diagnostic.continuous_certificate_model_id().is_some());
        assert_eq!(diagnostic.pair_work(), 28);
        for denied in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
            assert!(
                diagnose_scheduled_cycle_path_v1(
                    &geometry, &audit, fixed, &candidate, &closure, denied,
                )
                .continuous_certificate_model_id()
                .is_none()
            );
        }
        let first = crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32, [0x51; 32], [0x52; 32],
        )
        .unwrap();
        let (rg, ra, rs, rf) = rational_cycle_bay_geometry(8, true);
        let rc = rg
            .prove_dyadic_schedule_closure_v1(&ra, rf, &rs, 1.0e-8, limits)
            .unwrap();
        let ri = rs.evaluate(0.0).unwrap();
        let rr = rs.evaluate(1.0).unwrap();
        let reversed_candidate =
            ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(rs, &ri, &rr).unwrap();
        let reversed = crate::certify_scheduled_cycle_transition_v1(
            &rg,
            &ra,
            rf,
            &reversed_candidate,
            &rc,
            32,
            [0x51; 32],
            [0x52; 32],
        )
        .unwrap();
        assert_eq!(
            first.schedule_certificate(),
            reversed.schedule_certificate()
        );
        assert_eq!(first.closure_certificate(), reversed.closure_certificate());
        assert_eq!(
            first.collision_certificate(),
            reversed.collision_certificate()
        );
        for thickness in [0.1, 1.0, 3.0] {
            assert_eq!(
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
                ),
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &rg,
                    &ra,
                    rf,
                    &reversed_candidate,
                    &rc,
                    thickness,
                    32,
                )
            );
        }
    }

    #[test]
    fn sixteen_bay_geometry_certifies_all_cross_leaf_pairs_at_exact_caps() {
        let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(16, false);
        let limits = ori_kinematics::DyadicIntervalClosureLimitsV1 {
            max_depth: 4,
            max_leaves: 16,
            max_work: 16,
            schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
        };
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, limits)
            .unwrap();
        assert_eq!(geometry.hinges().len(), 64);
        assert_eq!(closure.leaves().len(), 16);
        assert!(closure.leaves().iter().all(|leaf| leaf.0 == 4));
        for short in [
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_depth: 3,
                ..limits
            },
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_leaves: 15,
                ..limits
            },
            ori_kinematics::DyadicIntervalClosureLimitsV1 {
                max_work: 15,
                ..limits
            },
        ] {
            assert_eq!(
                geometry.prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-8, short,),
                Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
            );
        }
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
            schedule, &initial, &requested,
        )
        .unwrap();
        let groups = composed_symmetric_rational_local_groups_v1(
            &geometry,
            &audit,
            fixed,
            candidate.schedule(),
        )
        .unwrap();
        assert!(symmetric_groups_have_disjoint_swept_balls_v1(
            &geometry, &groups
        ));
        let mut collision = groups.clone();
        let foreign = *groups.iter().find(|(_, group)| **group == 15).unwrap().0;
        collision.insert(foreign, 14);
        assert!(!symmetric_groups_have_disjoint_swept_balls_v1(
            &geometry, &collision
        ));
        let diagnostic =
            diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
        assert!(diagnostic.continuous_certificate_model_id().is_some());
        assert_eq!(diagnostic.pair_work(), 120);
        for denied in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
            assert!(
                diagnose_scheduled_cycle_path_v1(
                    &geometry, &audit, fixed, &candidate, &closure, denied,
                )
                .continuous_certificate_model_id()
                .is_none()
            );
        }
        let first = crate::certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed, &candidate, &closure, 32, [0x61; 32], [0x62; 32],
        )
        .unwrap();
        let (rg, ra, rs, rf) = rational_cycle_bay_geometry(16, true);
        let rc = rg
            .prove_dyadic_schedule_closure_v1(&ra, rf, &rs, 1.0e-8, limits)
            .unwrap();
        let ri = rs.evaluate(0.0).unwrap();
        let rr = rs.evaluate(1.0).unwrap();
        let reversed_candidate =
            ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(rs, &ri, &rr).unwrap();
        let reversed = crate::certify_scheduled_cycle_transition_v1(
            &rg,
            &ra,
            rf,
            &reversed_candidate,
            &rc,
            32,
            [0x61; 32],
            [0x62; 32],
        )
        .unwrap();
        assert_eq!(
            first.schedule_certificate(),
            reversed.schedule_certificate()
        );
        assert_eq!(first.closure_certificate(), reversed.closure_certificate());
        assert_eq!(
            first.collision_certificate(),
            reversed.collision_certificate()
        );
        for thickness in [0.1, 1.0, 3.0] {
            assert_eq!(
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry, &audit, fixed, &candidate, &closure, thickness, 32,
                ),
                diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &rg,
                    &ra,
                    rf,
                    &reversed_candidate,
                    &rc,
                    thickness,
                    32,
                )
            );
        }
    }

    #[test]
    fn genuine_two_hinge_tree_half_angle_schedule_has_closure_and_bounded_ccd() {
        let points = [
            (0.0, 0.0),
            (33.0, 0.0),
            (66.0, 0.0),
            (100.0, 0.0),
            (100.0, 100.0),
            (66.0, 100.0),
            (33.0, 100.0),
            (0.0, 100.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("7e00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("7f00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = [fixed_id("7f00", 20), fixed_id("7f00", 21)];
        edges.extend(hinges.iter().enumerate().map(|(index, hinge)| Edge {
            id: *hinge,
            start: boundary[index + 1],
            end: boundary[6 - index],
            kind: if index == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("7a00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("three material faces");
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let fixed = audit.faces()[0];
        let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            hinges
                .into_iter()
                .map(|hinge| ori_kinematics::HalfAngleRationalEntryInputV1 {
                    edge: hinge,
                    u_domain: [
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: vec![
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                        ori_kinematics::RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    denominator_power_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                        numerator: 10,
                        denominator: 1,
                    }],
                })
                .collect(),
            ori_kinematics::CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
            schedule, &initial, &requested,
        )
        .unwrap();
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                candidate.schedule(),
                1.0e-9,
                ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    max_depth: 16,
                    max_leaves: 65_536,
                    max_work: 1_048_576,
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
                },
            )
            .expect("two hinge tree closure");
        let diagnostic =
            diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 8);
        assert_eq!(
            diagnostic.continuous_certificate_model_id(),
            Some(STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1),
        );
        for cancelled_or_excessive in [0, MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1] {
            let rejected = diagnose_scheduled_cycle_path_v1(
                &geometry,
                &audit,
                fixed,
                &candidate,
                &closure,
                cancelled_or_excessive,
            );
            assert!(rejected.continuous_certificate_model_id().is_none());
            assert_eq!(rejected.leaf_count(), 0);
            assert_eq!(rejected.pair_work(), 0);
        }
        assert!(
            crate::certify_scheduled_cycle_transition_v1(
                &geometry, &audit, fixed, &candidate, &closure, 8, [0x11; 32], [0x22; 32],
            )
            .is_some()
        );
    }

    #[test]
    fn physical_four_vertex_cycle_has_four_radial_hinges_and_only_the_flat_uniform_root() {
        let points = [
            (0.0, 0.0),
            (400.0, 0.0),
            (400.0, 400.0),
            (0.0, 400.0),
            (200.0, 200.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8e00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..4]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let center = vertices[4].id;
        let mut edges = (0..4)
            .map(|index| Edge {
                id: fixed_id("9e00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let crease_edges = (0..4)
            .map(|index| fixed_id("9e00", index as u64 + 10))
            .collect::<Vec<_>>();
        edges.extend((0..4).map(|index| Edge {
            id: crease_edges[index],
            start: boundary[index],
            end: center,
            kind: if index % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("be00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("four triangular faces");
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .expect("cycle geometry");
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(geometry.hinges().len(), 4);
        assert_eq!(audit.closure_hinges().len(), 1);
        let directed_axes = geometry
            .hinges()
            .iter()
            .map(|hinge| {
                (
                    hinge.axis().x().to_bits(),
                    hinge.axis().y().to_bits(),
                    hinge.axis().z().to_bits(),
                )
            })
            .collect::<HashSet<_>>();
        assert_eq!(directed_axes.len(), 4);
        let mut flat = crease_edges
            .iter()
            .map(|edge| HingeAngle::new(*edge, 180.0).unwrap())
            .collect::<Vec<_>>();
        flat.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
        let flat = CanonicalHingeAngles::new(flat).unwrap();
        assert!(
            geometry
                .solve_closed(&audit, audit.faces()[0], &flat, 1.0e-9)
                .is_ok()
        );
        let moving = vec![crease_edges[1], crease_edges[3]];
        let mut initial = crease_edges
            .iter()
            .map(|edge| {
                HingeAngle::new(*edge, if moving.contains(edge) { 0.0 } else { 180.0 }).unwrap()
            })
            .collect::<Vec<_>>();
        initial.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
        let initial = CanonicalHingeAngles::new(initial).unwrap();
        assert_eq!(
            enumerate_uniform_cycle_closure_roots_v1(
                &geometry,
                &audit,
                audit.faces()[0],
                &initial,
                &moving,
                180.0,
                128,
            ),
            UniformCycleClosureRootsV1::Roots(vec![180.0])
        );
        let roots = enumerate_uniform_cycle_closure_roots_v1(
            &geometry,
            &audit,
            audit.faces()[0],
            &initial,
            &moving,
            90.0,
            128,
        );
        assert!(matches!(
            roots,
            UniformCycleClosureRootsV1::Indeterminate { .. }
        ));
        let mut reversed = moving.clone();
        reversed.reverse();
        assert_eq!(
            enumerate_uniform_cycle_closure_roots_v1(
                &geometry,
                &audit,
                audit.faces()[0],
                &initial,
                &reversed,
                90.0,
                128,
            ),
            roots
        );
        assert_eq!(
            enumerate_uniform_cycle_closure_roots_v1(
                &geometry,
                &audit,
                audit.faces()[0],
                &initial,
                &reversed,
                90.0,
                1,
            ),
            UniformCycleClosureRootsV1::Indeterminate { examined_leaves: 1 }
        );
        let path = diagnose_collective_cycle_path_v1(
            &geometry,
            &audit,
            audit.faces()[0],
            &initial,
            &moving,
            180.0,
            8,
        );
        assert_eq!(path.continuous_certificate_model_id(), None);
    }

    #[test]
    fn kawasaki_120_120_60_60_vertex_obeys_signed_half_angle_ratio() {
        let points = [
            (100.0, 0.0),
            (-50.0, 86.602_540_378_443_86),
            (-50.0, -86.602_540_378_443_86),
            (50.0, -86.602_540_378_443_86),
            (0.0, 0.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("ae00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..4]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let center = vertices[4].id;
        let hinges = (0..4)
            .map(|index| fixed_id("af00", index as u64 + 10))
            .collect::<Vec<_>>();
        let mut edges = (0..4)
            .map(|index| Edge {
                id: fixed_id("af00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend((0..4).map(|index| Edge {
            id: hinges[index],
            start: boundary[index],
            end: center,
            kind: if index == 3 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("aa00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("physical four-vertex topology");
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let fixed = audit.faces()[0];
        let schedule = ori_kinematics::CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            hinges
                .iter()
                .enumerate()
                .map(
                    |(index, edge)| ori_kinematics::HalfAngleRationalEntryInputV1 {
                        edge: *edge,
                        u_domain: [
                            ori_kinematics::RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            ori_kinematics::RationalCoefficientV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ],
                        numerator_power_coefficients: vec![
                            ori_kinematics::RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            ori_kinematics::RationalCoefficientV1 {
                                numerator: 1,
                                denominator: 1,
                            },
                        ],
                        denominator_power_coefficients: vec![
                            ori_kinematics::RationalCoefficientV1 {
                                numerator: if index % 2 == 0 { 1 } else { 2 },
                                denominator: 1,
                            },
                        ],
                    },
                )
                .collect(),
            ori_kinematics::CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        for u in [0.0, 0.5, 1.0] {
            let angles = schedule.evaluate(u).unwrap();
            geometry
                .solve_closed(&audit, fixed, &angles, 1.0e-8)
                .unwrap();
        }
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                ori_kinematics::DyadicIntervalClosureLimitsV1 {
                    max_depth: 16,
                    max_leaves: 65_536,
                    max_work: 1_048_576,
                    schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
                },
            )
            .expect("full-domain physical four-vertex closure");
        assert_eq!(closure.leaves().len(), 1);
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let candidate = ori_kinematics::admit_canonical_multi_hinge_path_candidate_v1(
            schedule, &initial, &requested,
        )
        .unwrap();
        let diagnostic =
            diagnose_scheduled_cycle_path_v1(&geometry, &audit, fixed, &candidate, &closure, 32);
        assert!(diagnostic.continuous_certificate_model_id().is_some());
        assert!(
            crate::certify_scheduled_cycle_transition_v1(
                &geometry, &audit, fixed, &candidate, &closure, 32, [0x31; 32], [0x32; 32],
            )
            .is_some()
        );
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

    fn three_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (500.0, 150.0),
            (500.0, 400.0),
            (250.0, 550.0),
            (0.0, 300.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8500", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9500", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in [2, 3, 4].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9500", 10 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b500", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("four triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("three-hinge triangular tree")
    }

    fn four_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (520.0, 120.0),
            (620.0, 350.0),
            (480.0, 580.0),
            (200.0, 650.0),
            (0.0, 320.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8600", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9600", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in [2, 3, 4, 5].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9600", 10 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b600", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("five triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("four-hinge triangular tree")
    }

    fn five_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (520.0, 90.0),
            (680.0, 280.0),
            (650.0, 500.0),
            (450.0, 680.0),
            (180.0, 700.0),
            (0.0, 340.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8700", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9700", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in [2, 3, 4, 5, 6].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9700", 10 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b700", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("six triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("five-hinge triangular tree")
    }

    fn six_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (530.0, 70.0),
            (700.0, 220.0),
            (760.0, 430.0),
            (620.0, 640.0),
            (380.0, 760.0),
            (140.0, 720.0),
            (0.0, 360.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8800", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9800", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in [2, 3, 4, 5, 6, 7].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9800", 10 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b800", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("seven triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("six-hinge triangular tree")
    }

    fn seven_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (540.0, 60.0),
            (730.0, 190.0),
            (840.0, 380.0),
            (810.0, 580.0),
            (650.0, 760.0),
            (410.0, 850.0),
            (150.0, 780.0),
            (0.0, 390.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8900", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9900", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in [2, 3, 4, 5, 6, 7, 8].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9900", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b900", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("eight triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("seven-hinge triangular tree")
    }

    fn eight_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (540.0, 60.0),
            (730.0, 190.0),
            (840.0, 380.0),
            (850.0, 570.0),
            (760.0, 750.0),
            (590.0, 880.0),
            (370.0, 930.0),
            (150.0, 850.0),
            (0.0, 430.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8a00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9a00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in [2, 3, 4, 5, 6, 7, 8, 9].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9a00", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("ba00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("nine triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("eight-hinge triangular tree")
    }

    fn nine_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (530.0, 45.0),
            (720.0, 140.0),
            (850.0, 300.0),
            (900.0, 490.0),
            (860.0, 680.0),
            (730.0, 840.0),
            (530.0, 940.0),
            (310.0, 960.0),
            (120.0, 850.0),
            (0.0, 460.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8c00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9c00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in [2, 3, 4, 5, 6, 7, 8, 9, 10].into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9c00", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("bc00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("ten triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("nine-hinge triangular tree")
    }

    fn ten_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (520.0, 35.0),
            (710.0, 110.0),
            (860.0, 240.0),
            (940.0, 410.0),
            (950.0, 590.0),
            (880.0, 760.0),
            (740.0, 900.0),
            (550.0, 980.0),
            (340.0, 990.0),
            (140.0, 880.0),
            (0.0, 480.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8d00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9d00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in (2..=11).enumerate() {
            edges.push(Edge {
                id: fixed_id("9d00", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("bd00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("eleven triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("ten-hinge triangular tree")
    }

    fn eleven_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0.0, 0.0),
            (300.0, 0.0),
            (520.0, 35.0),
            (710.0, 110.0),
            (860.0, 240.0),
            (940.0, 410.0),
            (950.0, 590.0),
            (880.0, 760.0),
            (740.0, 900.0),
            (550.0, 980.0),
            (340.0, 990.0),
            (140.0, 880.0),
            (60.0, 700.0),
            (0.0, 480.0),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8e00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9e00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in (2..=12).enumerate() {
            edges.push(Edge {
                id: fixed_id("9e00", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("be00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("twelve triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("eleven-hinge triangular tree")
    }

    fn twelve_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0., 0.),
            (300., 0.),
            (500., 20.),
            (680., 65.),
            (840., 145.),
            (970., 265.),
            (1050., 420.),
            (1070., 580.),
            (1030., 735.),
            (930., 870.),
            (780., 970.),
            (590., 1025.),
            (390., 1025.),
            (180., 930.),
            (0., 520.),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Vertex {
                id: fixed_id("8f00", i as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|i| Edge {
                id: fixed_id("9f00", i as u64 + 1),
                start: boundary[i],
                end: boundary[(i + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in (2..=13).enumerate() {
            edges.push(Edge {
                id: fixed_id("9f00", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("bf00", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("thirteen triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("twelve-hinge triangular tree")
    }

    fn thirteen_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0., 0.),
            (4., 0.),
            (7., 1.),
            (9., 3.),
            (11., 6.),
            (12., 9.),
            (12., 12.),
            (11., 15.),
            (9., 18.),
            (7., 20.),
            (4., 21.),
            (2., 20.),
            (1., 18.),
            (0., 15.),
            (-1., 11.),
            (-1., 6.),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Vertex {
                id: fixed_id("8a10", i as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|i| Edge {
                id: fixed_id("9a10", i as u64 + 1),
                start: boundary[i],
                end: boundary[(i + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in (2..=14).enumerate() {
            edges.push(Edge {
                id: fixed_id("9a10", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("ba10", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("fourteen triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("thirteen-hinge triangular tree")
    }

    fn fourteen_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        let points = [
            (0., 0.),
            (4., 0.),
            (7., 1.),
            (10., 3.),
            (12., 6.),
            (13., 9.),
            (13., 12.),
            (12., 15.),
            (10., 18.),
            (8., 20.),
            (5., 22.),
            (3., 22.),
            (1., 20.),
            (0., 18.),
            (-1., 15.),
            (-2., 10.),
            (-1., 4.),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Vertex {
                id: fixed_id("8b10", i as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|i| Edge {
                id: fixed_id("9b10", i as u64 + 1),
                start: boundary[i],
                end: boundary[(i + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in (2..=15).enumerate() {
            edges.push(Edge {
                id: fixed_id("9b10", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("bb10", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("fifteen triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("fourteen-hinge triangular tree")
    }

    fn fifteen_hinge_triangle_model_with_edge_order(
        reverse_edges: bool,
    ) -> MaterialTreeKinematicsModel {
        let points = [
            (0., 0.),
            (4., 0.),
            (8., 1.),
            (11., 3.),
            (14., 6.),
            (16., 10.),
            (17., 14.),
            (17., 18.),
            (16., 22.),
            (14., 26.),
            (11., 29.),
            (8., 31.),
            (5., 32.),
            (3., 31.),
            (1., 29.),
            (0., 26.),
            (-1., 20.),
            (-1., 10.),
        ];
        let vertices = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Vertex {
                id: fixed_id("8c10", i as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|v| v.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|i| Edge {
                id: fixed_id("9c10", i as u64 + 1),
                start: boundary[i],
                end: boundary[(i + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (offset, end) in (2..=16).enumerate() {
            edges.push(Edge {
                id: fixed_id("9c10", 20 + offset as u64),
                start: boundary[0],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        if reverse_edges {
            edges.reverse();
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("bc10", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("sixteen triangles"),
            TreeKinematicsLimits::default(),
        )
        .expect("fifteen-hinge triangular tree")
    }

    fn fifteen_hinge_triangle_model() -> MaterialTreeKinematicsModel {
        fifteen_hinge_triangle_model_with_edge_order(false)
    }

    fn branched_triangle_model(
        face_count: usize,
        reverse_edges: bool,
    ) -> MaterialTreeKinematicsModel {
        let vertex_count = face_count + 2;
        let vertices = (0..vertex_count)
            .map(|index| Vertex {
                id: fixed_id("8d10", index as u64 + 1),
                position: Point2::new(index as f64, (index * index) as f64),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..vertex_count)
            .map(|index| Edge {
                id: fixed_id("9d10", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % vertex_count],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let first_branch = vertex_count / 3;
        let second_branch = vertex_count * 2 / 3;
        let mut diagonals = vec![
            (0, first_branch),
            (first_branch, second_branch),
            (second_branch, 0),
        ];
        diagonals.extend((2..first_branch).map(|end| (0, end)));
        diagonals.extend((first_branch + 2..second_branch).map(|end| (first_branch, end)));
        diagonals.extend((second_branch + 2..vertex_count).map(|end| (second_branch, end)));
        diagonals.sort_unstable();
        diagonals.dedup();
        for (offset, (start, end)) in diagonals.into_iter().enumerate() {
            edges.push(Edge {
                id: fixed_id("9d10", 30 + offset as u64),
                start: boundary[start],
                end: boundary[end],
                kind: if offset % 2 == 0 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        if reverse_edges {
            edges.reverse();
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("bd10", face_count as u64),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("branched triangulation"),
            TreeKinematicsLimits::default(),
        )
        .expect("branched triangular tree")
    }

    fn zero_tree_pose(
        model: &MaterialTreeKinematicsModel,
    ) -> (Vec<EdgeId>, ori_kinematics::MaterialTreePose) {
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let angles = CanonicalHingeAngles::new(
            moving
                .iter()
                .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        (moving, pose)
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

    fn deep_strip_model(hinge_count: usize) -> MaterialTreeKinematicsModel {
        let column_count = hinge_count + 2;
        let mut points = (0..column_count)
            .map(|column| (column as f64 * 100.0, 0.0))
            .collect::<Vec<_>>();
        points.extend(
            (0..column_count)
                .rev()
                .map(|column| (column as f64 * 100.0, 4.0)),
        );
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8400", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9400", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend((1..=hinge_count).map(|column| Edge {
            id: fixed_id("9400", 1_000 + column as u64),
            start: boundary[column],
            end: boundary[2 * column_count - 1 - column],
            kind: EdgeKind::Mountain,
        }));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("b400", hinge_count as u64),
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

    fn sparse_triangle_strip_model(face_count: usize) -> MaterialTreeKinematicsModel {
        assert!((3..=64).contains(&face_count));
        let cell_count = face_count.div_ceil(2);
        let first_bottom = usize::from(face_count % 2 == 1);
        let mut points = (first_bottom..=cell_count)
            .map(|column| (column as f64 * 100.0, 0.0))
            .collect::<Vec<_>>();
        points.extend(
            (0..=cell_count)
                .rev()
                .map(|column| (column as f64 * 100.0, 4.0)),
        );
        let vertices = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| Vertex {
                id: fixed_id("8f20", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut bottom = vec![None; cell_count + 1];
        let mut top = vec![None; cell_count + 1];
        for vertex in &vertices {
            let column = (vertex.position.x / 100.0) as usize;
            if vertex.position.y == 0.0 {
                bottom[column] = Some(vertex.id);
            } else {
                top[column] = Some(vertex.id);
            }
        }
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixed_id("9f20", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let mut next_edge_id = boundary.len() as u64 + 1;
        for column in 1..cell_count {
            edges.push(Edge {
                id: fixed_id("9f20", next_edge_id),
                start: bottom[column].unwrap(),
                end: top[column].unwrap(),
                kind: EdgeKind::Mountain,
            });
            next_edge_id += 1;
        }
        for column in first_bottom..cell_count {
            edges.push(Edge {
                id: fixed_id("9f20", next_edge_id),
                start: bottom[column].unwrap(),
                end: top[column + 1].unwrap(),
                kind: EdgeKind::Valley,
            });
            next_edge_id += 1;
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("bf20", face_count as u64),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("sparse triangle strip"),
            TreeKinematicsLimits::default(),
        )
        .expect("sparse triangular tree")
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
                interval_leaf_count: 0,
                interval_pair_work: 0,
                positive_endpoint_memo_pair_entries: 0,
                positive_endpoint_exact_pair_calls: 0,
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
    fn canonical_sweep_matches_bruteforce_for_single_nonadjacent_pair() {
        for (model, expected) in [
            (three_hinge_strip_model(false), true),
            (three_hinge_strip_model(true), false),
        ] {
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
                .collect::<HashSet<_>>();
            let mut metrics = (0, 0);
            // For this four-face chain the exhaustive oracle has exactly the
            // three non-adjacent pairs; the established fixtures fix their
            // expected conjunction.
            assert_eq!(
                two_hinge_interval_clearance_premises(
                    &model,
                    &pose,
                    &moving,
                    if expected { 0.1 } else { 10.0 },
                    8,
                    &mut metrics,
                ),
                expected
            );
        }
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
    fn nine_hinge_deep_tree_is_certified_deterministically_across_input_permutation() {
        let model = deep_strip_model(9);
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
        let zero_candidate = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &moving,
            0.01,
            0.0,
            StackedFoldPathDiagnosticLimitsV1 {
                sample_intervals: 1,
                ..StackedFoldPathDiagnosticLimitsV1::default()
            },
        )
        .unwrap();
        assert!(zero_candidate.continuous_clearance_certified());
        assert_eq!(zero_candidate.interval_pair_work(), 0);
        let first = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &moving,
            5.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1 {
                sample_intervals: 1,
                ..StackedFoldPathDiagnosticLimitsV1::default()
            },
        )
        .unwrap();
        let mut reversed = moving;
        reversed.reverse();
        let second = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &reversed,
            5.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1 {
                sample_intervals: 1,
                ..StackedFoldPathDiagnosticLimitsV1::default()
            },
        )
        .unwrap();
        assert!(first.continuous_clearance_certified());
        assert_eq!(first, second);
        assert!(first.interval_leaf_count() >= 1);
        assert!(first.interval_pair_work() > 0);
    }

    #[test]
    fn sixteen_hinge_overlap_exhausts_adaptive_budget_fail_closed() {
        let model = sparse_triangle_strip_model(17);
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
            180.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1 {
                sample_intervals: 1,
                ..StackedFoldPathDiagnosticLimitsV1::default()
            },
        )
        .unwrap();
        assert!(!result.continuous_clearance_certified());
        assert_eq!(result.interval_leaf_count(), 0);
        assert_eq!(result.interval_pair_work(), 0);
    }

    #[test]
    fn twenty_four_hinge_sparse_tree_uses_complete_sweep_candidates() {
        let model = deep_strip_model(24);
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
            .collect::<HashSet<_>>();
        let mut metrics = (0, 0);
        assert!(two_hinge_interval_clearance_premises(
            &model,
            &pose,
            &moving,
            0.001,
            1,
            &mut metrics,
        ));
        assert_eq!(metrics.0, 1);
        assert!(metrics.1 < MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1);
    }

    #[test]
    fn thirty_two_hinge_dense_tree_exceeds_candidate_cap() {
        let model = deep_strip_model(32);
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
            .collect::<HashSet<_>>();
        let mut metrics = (0, 0);
        assert!(!two_hinge_interval_clearance_premises(
            &model,
            &pose,
            &moving,
            180.0,
            1,
            &mut metrics,
        ));
        assert_eq!(metrics, (0, 0));
    }

    #[test]
    fn forty_eight_hinge_sparse_tree_uses_one_canonical_candidate_scan() {
        let model = deep_strip_model(48);
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
            .collect::<HashSet<_>>();
        let mut metrics = (0, 0);
        assert!(two_hinge_interval_clearance_premises(
            &model,
            &pose,
            &moving,
            0.0001,
            1,
            &mut metrics,
        ));
        assert_eq!(metrics.0, 1);
        assert!(metrics.1 <= MAX_STACKED_FOLD_INTERVAL_CANDIDATES_V1);
    }

    #[test]
    fn sixty_four_hinge_dense_tree_fails_candidate_cap() {
        let model = deep_strip_model(64);
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
            .collect::<HashSet<_>>();
        let mut metrics = (0, 0);
        assert!(!two_hinge_interval_clearance_premises(
            &model,
            &pose,
            &moving,
            180.0,
            1,
            &mut metrics,
        ));
        assert_eq!(metrics, (0, 0));
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
    fn three_triangle_positive_thickness_tree_gets_bounded_certificate() {
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
            .expect("bounded positive-thickness diagnosis");
            assert!(diagnostic.continuous_clearance_certified());
            assert_eq!(
                diagnostic.continuous_certificate_model_id(),
                Some(STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
            );
            assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
        }
    }

    #[test]
    fn four_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = three_hinge_triangle_model();
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
        for requested in [10.0, 30.0, 60.0] {
            let diagnostic = diagnose_collective_hinge_path_v1(
                &model,
                &initial,
                &moving,
                requested,
                0.1,
                StackedFoldPathDiagnosticLimitsV1::default(),
            )
            .expect("bounded positive-thickness diagnosis");
            assert!(diagnostic.continuous_clearance_certified(), "{requested}");
            assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
        }
    }

    #[test]
    fn eight_triangle_positive_thickness_tree_rejects_over_angle() {
        let model = seven_hinge_triangle_model();
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
        let beyond_bound = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            15.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded hold");
        assert!(!beyond_bound.continuous_clearance_certified());
        assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
    }

    #[test]
    fn five_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = four_hinge_triangle_model();
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
        for requested in [10.0, 30.0, 45.0] {
            let diagnostic = diagnose_collective_hinge_path_v1(
                &model,
                &initial,
                &moving,
                requested,
                0.1,
                StackedFoldPathDiagnosticLimitsV1::default(),
            )
            .expect("bounded positive-thickness diagnosis");
            assert!(diagnostic.continuous_clearance_certified(), "{requested}");
            assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
        }
        let beyond_bound = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            45.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded hold");
        assert!(!beyond_bound.continuous_clearance_certified());
        assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
    }

    #[test]
    fn six_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = five_hinge_triangle_model();
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
        let requested = 30.0;
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            requested,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(diagnostic.continuous_clearance_certified(), "{requested}");
        assert_eq!(diagnostic.safe_stop_angle_degrees(), requested);
        let beyond_bound = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            30.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded hold");
        assert!(!beyond_bound.continuous_clearance_certified());
        assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
    }

    #[test]
    fn seven_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = six_hinge_triangle_model();
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
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            20.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.safe_stop_angle_degrees(), 20.0);
        let beyond_bound = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            20.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded hold");
        assert!(!beyond_bound.continuous_clearance_certified());
        assert_eq!(beyond_bound.safe_stop_angle_degrees(), 0.0);
    }

    #[test]
    fn eight_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = seven_hinge_triangle_model();
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
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            15.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.safe_stop_angle_degrees(), 15.0);
    }

    #[test]
    fn nine_triangle_positive_thickness_tree_rejects_over_angle() {
        let model = eight_hinge_triangle_model();
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
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            10.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(!diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.safe_stop_angle_degrees(), 0.0);
    }

    #[test]
    fn nine_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = eight_hinge_triangle_model();
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
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            10.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded positive-thickness diagnosis");
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.safe_stop_angle_degrees(), 10.0);
    }

    #[test]
    fn positive_endpoint_memo_cap_rejects_ten_face_tree() {
        let model = deep_strip_model(9);
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let angles = CanonicalHingeAngles::new(
            moving
                .iter()
                .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &pose,
            &moving,
            1.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!diagnostic.continuous_clearance_certified());
    }

    #[test]
    fn nine_triangle_boundary_rejects_aba_and_thickness_drift() {
        let model = eight_hinge_triangle_model();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 10.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let aba_bound = model.bind_pose(&aba_pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
            .unwrap()
            .expect("nine-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, aba_bound, 0.1).is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1)
                .is_none()
        );
    }

    #[test]
    fn ten_triangle_positive_thickness_tree_rejects_over_angle() {
        let model = nine_hinge_triangle_model();
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let angles = CanonicalHingeAngles::new(
            moving
                .iter()
                .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let initial = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            8.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!diagnostic.continuous_clearance_certified());
    }

    #[test]
    fn ten_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = nine_hinge_triangle_model();
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let angles = CanonicalHingeAngles::new(
            moving
                .iter()
                .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let initial = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            8.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.safe_stop_angle_degrees(), 8.0);
    }

    #[test]
    fn positive_endpoint_memo_cap_rejects_eleven_face_tree() {
        let model = deep_strip_model(10);
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let angles = CanonicalHingeAngles::new(
            moving
                .iter()
                .map(|edge| HingeAngle::new(*edge, 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let initial = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            1.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!diagnostic.continuous_clearance_certified());
    }

    #[test]
    fn ten_triangle_boundary_rejects_aba_and_thickness_drift() {
        let model = nine_hinge_triangle_model();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 8.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
            .unwrap()
            .expect("ten-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba_pose).unwrap(),
                0.1
            )
            .is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1)
                .is_none()
        );
    }

    #[test]
    fn eleven_triangle_positive_thickness_tree_rejects_over_angle() {
        let model = ten_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            6.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!diagnostic.continuous_clearance_certified());
    }

    #[test]
    fn eleven_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = ten_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            6.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.safe_stop_angle_degrees(), 6.0);
    }

    #[test]
    fn positive_endpoint_memo_cap_rejects_twelve_face_tree() {
        let model = deep_strip_model(11);
        let (moving, initial) = zero_tree_pose(&model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            1.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!diagnostic.continuous_clearance_certified());
    }

    #[test]
    fn eleven_triangle_boundary_rejects_aba_and_thickness_drift() {
        let model = ten_hinge_triangle_model();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 6.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
            .unwrap()
            .expect("eleven-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba_pose).unwrap(),
                0.1,
            )
            .is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1,)
                .is_none()
        );
    }

    #[test]
    fn twelve_triangle_positive_thickness_tree_rejects_over_angle() {
        let model = eleven_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            5.000_000_1,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!diagnostic.continuous_clearance_certified());
    }

    #[test]
    fn twelve_triangle_positive_thickness_tree_gets_bounded_certificate() {
        let model = eleven_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            5.0,
            0.01,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(diagnostic.continuous_clearance_certified());
        assert_eq!(diagnostic.safe_stop_angle_degrees(), 5.0);
    }

    #[test]
    fn positive_endpoint_memo_cap_rejects_thirteen_face_tree() {
        let model = deep_strip_model(12);
        let (moving, initial) = zero_tree_pose(&model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            1.0,
            0.1,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!diagnostic.continuous_clearance_certified());
    }

    #[test]
    fn twelve_triangle_boundary_rejects_aba_and_thickness_drift() {
        let model = eleven_hinge_triangle_model();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 5.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba_pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
            .unwrap()
            .expect("twelve-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba_pose).unwrap(),
                0.1,
            )
            .is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.100_000_000_1,)
                .is_none()
        );
    }

    #[test]
    fn thirteen_triangle_positive_thickness_bounds_and_binding() {
        let model = twelve_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let accepted = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            4.0,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(accepted.continuous_clearance_certified());
        let over = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            4.000_000_1,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!over.continuous_clearance_certified());

        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 4.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
            .unwrap()
            .expect("thirteen-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba).unwrap(),
                0.001,
            )
            .is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
                .is_none()
        );
    }

    #[test]
    fn fourteen_triangle_positive_thickness_bounds() {
        let model = thirteen_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let accepted = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            3.0,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(accepted.continuous_clearance_certified());
        let over = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            3.000_000_1,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!over.continuous_clearance_certified());
    }

    #[test]
    fn fourteen_triangle_boundary_rejects_aba_and_thickness_drift() {
        let model = thirteen_hinge_triangle_model();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 3.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
            .unwrap()
            .expect("fourteen-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba).unwrap(),
                0.001,
            )
            .is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
                .is_none()
        );
    }

    #[test]
    fn fifteen_triangle_positive_thickness_bounds_and_work_meter() {
        let model = fourteen_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let accepted = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            2.0,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(accepted.continuous_clearance_certified());
        assert_eq!(accepted.positive_endpoint_memo_pair_entries(), 91);
        assert_eq!(accepted.positive_endpoint_exact_pair_calls(), 0);
        assert!(
            accepted.positive_endpoint_memo_pair_entries()
                + accepted.positive_endpoint_exact_pair_calls()
                <= MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1
        );
        let over = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            2.000_000_1,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!over.continuous_clearance_certified());
        assert_eq!(over.positive_endpoint_memo_pair_entries(), 0);
        assert_eq!(over.positive_endpoint_exact_pair_calls(), 0);
    }

    #[test]
    fn fifteen_triangle_boundary_rejects_aba_and_thickness_drift() {
        let model = fourteen_hinge_triangle_model();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 2.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
            .unwrap()
            .expect("fifteen-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba).unwrap(),
                0.001,
            )
            .is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
                .is_none()
        );
    }

    #[test]
    fn sixteen_triangle_positive_thickness_bounds_and_work_meter() {
        let model = fifteen_hinge_triangle_model();
        let (moving, initial) = zero_tree_pose(&model);
        let accepted = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            1.5,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(accepted.continuous_clearance_certified());
        assert_eq!(accepted.positive_endpoint_memo_pair_entries(), 105);
        assert_eq!(accepted.positive_endpoint_exact_pair_calls(), 0);
        assert!(
            accepted.positive_endpoint_memo_pair_entries()
                + accepted.positive_endpoint_exact_pair_calls()
                <= MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1
        );
        let over = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            1.500_000_1,
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(!over.continuous_clearance_certified());
        assert_eq!(over.positive_endpoint_memo_pair_entries(), 0);
        assert_eq!(over.positive_endpoint_exact_pair_calls(), 0);
    }

    #[test]
    fn sixteen_triangle_boundary_rejects_aba_and_thickness_drift() {
        let model = fifteen_hinge_triangle_model();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 1.5).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let aba = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.001)
            .unwrap()
            .expect("sixteen-face boundary");
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba).unwrap(),
                0.001,
            )
            .is_none()
        );
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.001_000_000_1,)
                .is_none()
        );
    }

    #[test]
    fn sparse_seventeen_face_tree_is_not_rejected_by_total_pair_count() {
        assert_eq!(MAX_POSITIVE_ENDPOINT_MEMO_PAIR_ENTRIES_V1, 120);
        let model = sparse_triangle_strip_model(17);
        let (moving, initial) = zero_tree_pose(&model);
        let diagnostic = diagnose_collective_hinge_path_v1(
            &model,
            &initial,
            &moving,
            positive_tree_max_angle_degrees_v1(16).unwrap(),
            0.001,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .unwrap();
        assert!(diagnostic.continuous_clearance_certified());
        assert!(diagnostic.positive_endpoint_memo_pair_entries() <= 120);
        assert_eq!(diagnostic.positive_endpoint_exact_pair_calls(), 0);
    }

    #[test]
    fn positive_tree_resource_policy_is_branch_independent_and_pair_bounded() {
        for face_count in 10..=16 {
            assert!(positive_tree_resource_premises_v1(
                face_count,
                face_count - 1,
                face_count - 1,
            ));
            let model = branched_triangle_model(face_count, face_count % 2 == 0);
            assert_eq!(model.face_ids().len(), face_count);
            assert_eq!(model.hinges().len(), face_count - 1);
            let maximum_degree = model
                .face_ids()
                .iter()
                .map(|face| {
                    model
                        .hinges()
                        .iter()
                        .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                        .count()
                })
                .max()
                .unwrap();
            assert!(maximum_degree >= 3);
        }
        assert!(positive_endpoint_pair_work_within_limit_v1(120));
        assert!(!positive_endpoint_pair_work_within_limit_v1(121));
        assert!(positive_tree_resource_premises_v1(17, 16, 16));
        assert!(positive_tree_resource_premises_v1(64, 63, 63));
        assert!(!positive_tree_resource_premises_v1(65, 64, 64));
        assert!(!positive_tree_resource_premises_v1(
            usize::MAX,
            usize::MAX - 1,
            usize::MAX - 1
        ));
    }

    #[test]
    fn positive_tree_angle_policy_is_monotone_with_resource_growth() {
        let maxima = (2..=15)
            .map(|hinges| positive_tree_max_angle_degrees_v1(hinges).unwrap())
            .collect::<Vec<_>>();
        assert!(maxima.windows(2).all(|pair| pair[1] <= pair[0]));
        assert!(positive_tree_max_angle_degrees_v1(1).is_none());
        assert!(positive_tree_max_angle_degrees_v1(16).is_some());
        assert!(positive_tree_max_angle_degrees_v1(63).is_some());
        assert!(positive_tree_max_angle_degrees_v1(64).is_none());
    }

    #[test]
    fn sparse_positive_trees_scale_to_sixty_four_faces_with_zero_candidates() {
        for face_count in [17, 32, 64] {
            let model = sparse_triangle_strip_model(face_count);
            let (moving, initial) = zero_tree_pose(&model);
            let requested = positive_tree_max_angle_degrees_v1(face_count - 1).unwrap();
            let moving_set = moving.iter().copied().collect::<HashSet<_>>();
            let endpoint = solve_collective_pose(&model, &initial, &moving_set, requested).unwrap();
            let candidates = positive_endpoint_candidates_v1(&model, &endpoint, 0.001).unwrap();
            assert!(
                candidates.iter().all(|(first, second)| {
                    faces_share_material_vertex_v1(&model, *first, *second)
                }),
                "face_count={face_count}, candidates={}, nonjunction={}",
                candidates.len(),
                candidates
                    .iter()
                    .filter(|(first, second)| {
                        !faces_share_material_vertex_v1(&model, *first, *second)
                    })
                    .count()
            );
            let diagnostic = diagnose_collective_hinge_path_v1(
                &model,
                &initial,
                &moving,
                requested,
                0.001,
                StackedFoldPathDiagnosticLimitsV1::default(),
            )
            .unwrap();
            assert!(
                diagnostic.continuous_clearance_certified(),
                "face_count={face_count}, diagnostic={diagnostic:?}"
            );
            assert!(diagnostic.positive_endpoint_memo_pair_entries() <= 120);
            assert_eq!(diagnostic.positive_endpoint_exact_pair_calls(), 0);
        }
    }

    #[test]
    fn dense_sweep_candidate_cap_is_fail_closed_and_order_independent() {
        for reverse_edges in [false, true] {
            let model = branched_triangle_model(18, reverse_edges);
            let (moving, initial) = zero_tree_pose(&model);
            let moving = moving.into_iter().collect::<HashSet<_>>();
            let dense = solve_collective_pose(&model, &initial, &moving, 180.0).unwrap();
            assert!(positive_endpoint_candidates_v1(&model, &dense, 1_000_000.0).is_none());
        }
        assert!(positive_endpoint_pair_work_within_limit_v1(120));
        assert!(!positive_endpoint_pair_work_within_limit_v1(121));
    }

    #[test]
    fn endpoint_memo_is_stable_across_hinge_input_and_face_order_permutation() {
        let canonical = fifteen_hinge_triangle_model_with_edge_order(false);
        let reversed = fifteen_hinge_triangle_model_with_edge_order(true);
        for model in [&canonical, &reversed] {
            let (moving, initial) = zero_tree_pose(model);
            let diagnostic = diagnose_collective_hinge_path_v1(
                model,
                &initial,
                &moving,
                1.5,
                0.001,
                StackedFoldPathDiagnosticLimitsV1::default(),
            )
            .unwrap();
            assert!(diagnostic.continuous_clearance_certified());
            assert_eq!(diagnostic.positive_endpoint_memo_pair_entries(), 105);
            assert_eq!(diagnostic.positive_endpoint_exact_pair_calls(), 0);
        }
    }

    #[test]
    fn degenerate_tree_geometry_never_reaches_positive_resource_authority() {
        let vertices = (0..4)
            .map(|index| Vertex {
                id: fixed_id("8e10", index + 1),
                position: Point2::new(index as f64, 0.0),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let pattern = CreasePattern {
            vertices,
            edges: vec![
                Edge {
                    id: fixed_id("9e10", 1),
                    start: boundary[0],
                    end: boundary[1],
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: fixed_id("9e10", 2),
                    start: boundary[1],
                    end: boundary[2],
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: fixed_id("9e10", 3),
                    start: boundary[2],
                    end: boundary[3],
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: fixed_id("9e10", 4),
                    start: boundary[3],
                    end: boundary[0],
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: fixed_id("9e10", 5),
                    start: boundary[0],
                    end: boundary[2],
                    kind: EdgeKind::Mountain,
                },
            ],
        };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("be10", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.snapshot.is_none());

        let vertices = [(0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)]
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: fixed_id("8e20", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let edges = (0..4)
            .map(|index| Edge {
                id: fixed_id("9e20", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixed_id("be20", 1),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.snapshot.is_none());
    }
}
