use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};
use ori_topology::TopologySnapshot;

use crate::{
    CanonicalCycleScheduleV1, CanonicalHingeAngles, CycleScheduleLimitsV1,
    IntervalRigidTransformV1, KinematicsError, MaterialHingeGraphGeometry, OutwardIntervalV1,
    RigidTransform, TreeHinge, TreeKinematicsLimits,
};

pub const MATERIAL_HINGE_INTERVAL_CLOSURE_CERTIFICATE_VERSION_V1: u32 = 1;

/// Bounded evidence that every angle in one canonical hinge box preserves all
/// material loop constraints. The interval face poses are intentionally not
/// exposed: loss of dependency correlation must reject instead of becoming
/// reusable pose authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialHingeIntervalClosureCertificateV1 {
    version: u32,
    fixed_face: FaceId,
    checked_hinges: Vec<EdgeId>,
}

impl MaterialHingeIntervalClosureCertificateV1 {
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn fixed_face(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub fn checked_hinges(&self) -> &[EdgeId] {
        &self.checked_hinges
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DyadicIntervalClosureLimitsV1 {
    pub max_depth: u32,
    pub max_leaves: usize,
    pub max_work: usize,
    pub schedule_limits: CycleScheduleLimitsV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DyadicMaterialHingeIntervalClosureCertificateV1 {
    fixed_face: FaceId,
    schedule_binding_fingerprint: [u8; 32],
    graph_binding_fingerprint: [u8; 32],
    leaves: Vec<(u32, u64, MaterialHingeIntervalClosureCertificateV1)>,
}

impl DyadicMaterialHingeIntervalClosureCertificateV1 {
    #[must_use]
    pub const fn fixed_face(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub fn leaves(&self) -> &[(u32, u64, MaterialHingeIntervalClosureCertificateV1)] {
        &self.leaves
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn schedule_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.schedule_binding_fingerprint
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn graph_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.graph_binding_fingerprint
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyadicIntervalClosureErrorV1 {
    InvalidInput,
    ResourceLimit,
    UnprovenClosure { depth: u32, index: u64 },
}

/// One caller-supplied face transform used only as closure evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateFaceTransform {
    face: FaceId,
    transform: RigidTransform,
}

impl CandidateFaceTransform {
    #[must_use]
    pub const fn new(face: FaceId, transform: RigidTransform) -> Self {
        Self { face, transform }
    }

    #[must_use]
    pub const fn face(self) -> FaceId {
        self.face
    }

    #[must_use]
    pub const fn transform(self) -> RigidTransform {
        self.transform
    }
}

/// A deterministic spanning-tree candidate whose every material hinge,
/// including loop-closure hinges, has been observed to close.
///
/// This remains observation-only and grants no applied-pose or mutation
/// authority.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedMaterialHingeGraphPose {
    fixed_face: FaceId,
    angles: CanonicalHingeAngles,
    transforms: Vec<CandidateFaceTransform>,
    closure: MaterialHingeClosureCertificate,
}

impl ClosedMaterialHingeGraphPose {
    #[must_use]
    pub const fn fixed_face(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub const fn hinge_angles(&self) -> &CanonicalHingeAngles {
        &self.angles
    }

    #[must_use]
    pub fn transforms(&self) -> &[CandidateFaceTransform] {
        &self.transforms
    }

    #[must_use]
    pub fn face_transform(&self, face: FaceId) -> Option<RigidTransform> {
        self.transforms
            .binary_search_by_key(&face.canonical_bytes(), |item| item.face.canonical_bytes())
            .ok()
            .map(|index| self.transforms[index].transform)
    }

    #[must_use]
    pub const fn closure_certificate(&self) -> &MaterialHingeClosureCertificate {
        &self.closure
    }
}

impl MaterialHingeGraphGeometry {
    /// Covers the complete schedule domain with deterministic left-first
    /// dyadic leaves and proves every leaf. A rejected leaf is subdivided;
    /// exhausting depth is reported separately from malformed input or work.
    pub fn prove_dyadic_schedule_closure_v1(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        tolerance: f64,
        limits: DyadicIntervalClosureLimitsV1,
    ) -> Result<DyadicMaterialHingeIntervalClosureCertificateV1, DyadicIntervalClosureErrorV1> {
        if !schedule.matches_binding(self, audit, fixed_face)
            || limits.max_depth >= 64
            || limits.max_leaves == 0
            || limits.max_work == 0
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput);
        }
        if collective_flat_stack_cycle_closure_premises_v1(
            self, audit, fixed_face, schedule, tolerance,
        ) || orthogonal_inverse_pair_cycle_closure_premises_v1(
            self, audit, fixed_face, schedule, tolerance,
        ) || symmetric_rational_kawasaki_cycle_closure_premises_v1(
            self, audit, fixed_face, schedule, tolerance,
        ) {
            let mut checked_hinges = self
                .hinges()
                .iter()
                .map(|hinge| hinge.edge())
                .collect::<Vec<_>>();
            checked_hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
            return Ok(DyadicMaterialHingeIntervalClosureCertificateV1 {
                fixed_face,
                schedule_binding_fingerprint: schedule.certificate_binding_fingerprint_v1(),
                graph_binding_fingerprint: schedule.graph_binding_fingerprint_v1(),
                leaves: vec![(
                    0,
                    0,
                    MaterialHingeIntervalClosureCertificateV1 {
                        version: MATERIAL_HINGE_INTERVAL_CLOSURE_CERTIFICATE_VERSION_V1,
                        fixed_face,
                        checked_hinges,
                    },
                )],
            });
        }
        let mut pending = vec![(0u32, 0u64)];
        let mut leaves = Vec::new();
        let mut work = 0usize;
        while let Some((depth, index)) = pending.pop() {
            work = work
                .checked_add(1)
                .filter(|value| *value <= limits.max_work)
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            let boxes =
                match schedule.evaluate_angle_box_dyadic(depth, index, limits.schedule_limits) {
                    Ok(boxes) => boxes,
                    Err(crate::CycleSchedulePrepareErrorV1::ResourceLimit)
                        if depth < limits.max_depth =>
                    {
                        let child_depth = depth + 1;
                        let left = index
                            .checked_mul(2)
                            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
                        pending.push((child_depth, left + 1));
                        pending.push((child_depth, left));
                        if pending.len().saturating_add(leaves.len()) > limits.max_leaves {
                            return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
                        }
                        continue;
                    }
                    Err(crate::CycleSchedulePrepareErrorV1::ResourceLimit) => {
                        return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
                    }
                    Err(_) => return Err(DyadicIntervalClosureErrorV1::InvalidInput),
                };
            match self.prove_interval_closure_v1(
                audit,
                fixed_face,
                &boxes,
                tolerance,
                limits.max_work,
            ) {
                Ok(certificate) => {
                    if leaves.len() >= limits.max_leaves {
                        return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
                    }
                    leaves.push((depth, index, certificate));
                }
                Err(KinematicsError::ResourceLimitExceeded) => {
                    return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
                }
                Err(_) if depth < limits.max_depth => {
                    let child_depth = depth + 1;
                    let left = index
                        .checked_mul(2)
                        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
                    // Stack order makes the deterministic traversal left-first.
                    pending.push((child_depth, left + 1));
                    pending.push((child_depth, left));
                    if pending.len().saturating_add(leaves.len()) > limits.max_leaves {
                        return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
                    }
                }
                Err(_) => {
                    return Err(DyadicIntervalClosureErrorV1::UnprovenClosure { depth, index });
                }
            }
        }
        Ok(DyadicMaterialHingeIntervalClosureCertificateV1 {
            fixed_face,
            schedule_binding_fingerprint: schedule.certificate_binding_fingerprint_v1(),
            graph_binding_fingerprint: schedule.graph_binding_fingerprint_v1(),
            leaves,
        })
    }

    /// Proves closure for every value in a canonical vector of angle boxes.
    ///
    /// The fixed material face is exactly identity. Each local material-hinge
    /// transform is right-composed into its parent's world pose. Traversing a
    /// hinge backwards reverses the assignment sign.
    pub fn prove_interval_closure_v1(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angle_boxes: &[(EdgeId, OutwardIntervalV1)],
        tolerance: f64,
        max_work: usize,
    ) -> Result<MaterialHingeIntervalClosureCertificateV1, KinematicsError> {
        if !tolerance.is_finite()
            || tolerance < 0.0
            || max_work == 0
            || self.face_ids() != audit.faces()
            || self.hinges().len() != angle_boxes.len()
            || self.hinges().len() != audit.spanning_hinges().len() + audit.closure_hinges().len()
            || !audit.faces().contains(&fixed_face)
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let mut hinges = self.hinges().iter().collect::<Vec<_>>();
        hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
        if hinges
            .iter()
            .map(|hinge| hinge.edge())
            .ne(angle_boxes.iter().map(|(edge, _)| *edge))
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let boxes = angle_boxes.iter().copied().collect::<HashMap<_, _>>();
        if boxes.len() != angle_boxes.len() {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let spanning = audit
            .spanning_hinges()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut adjacency = HashMap::<FaceId, Vec<(FaceId, usize, bool)>>::new();
        for face in audit.faces() {
            adjacency.insert(*face, Vec::new());
        }
        for (index, hinge) in self.hinges().iter().enumerate() {
            if spanning.contains(&hinge.edge()) {
                adjacency
                    .get_mut(&hinge.left_face())
                    .ok_or(KinematicsError::UnsupportedTopology)?
                    .push((hinge.right_face(), index, false));
                adjacency
                    .get_mut(&hinge.right_face())
                    .ok_or(KinematicsError::UnsupportedTopology)?
                    .push((hinge.left_face(), index, true));
            }
        }
        for neighbors in adjacency.values_mut() {
            neighbors.sort_unstable_by_key(|(_, index, _)| {
                self.hinges()[*index].edge().canonical_bytes()
            });
        }
        let interval_error = |error| match error {
            crate::OutwardIntervalErrorV1::ResourceLimit => KinematicsError::ResourceLimitExceeded,
            crate::OutwardIntervalErrorV1::InvalidEndpoint
            | crate::OutwardIntervalErrorV1::DivisionByZeroInterval => {
                KinematicsError::UnrepresentableGeometry
            }
        };
        let mut poses = HashMap::new();
        poses.insert(
            fixed_face,
            IntervalRigidTransformV1::identity().map_err(interval_error)?,
        );
        let mut queue = VecDeque::from([fixed_face]);
        let mut charged = 0usize;
        while let Some(parent_face) = queue.pop_front() {
            let parent = *poses
                .get(&parent_face)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            for &(child_face, hinge_index, reverse) in adjacency
                .get(&parent_face)
                .ok_or(KinematicsError::UnsupportedTopology)?
            {
                if poses.contains_key(&child_face) {
                    continue;
                }
                charged = charged
                    .checked_add(1)
                    .filter(|value| *value <= max_work)
                    .ok_or(KinematicsError::ResourceLimitExceeded)?;
                let hinge = &self.hinges()[hinge_index];
                let degrees = *boxes
                    .get(&hinge.edge())
                    .ok_or(KinematicsError::UnsupportedTopology)?;
                let mountain = hinge.assignment() == ori_topology::FoldAssignment::Mountain;
                let sign = if reverse ^ !mountain { -1.0 } else { 1.0 };
                let local = IntervalRigidTransformV1::about_axis(
                    [
                        sign * hinge.axis().x(),
                        sign * hinge.axis().y(),
                        sign * hinge.axis().z(),
                    ],
                    [hinge.start().x(), hinge.start().y(), hinge.start().z()],
                    degrees,
                    max_work,
                )
                .map_err(interval_error)?;
                poses.insert(
                    child_face,
                    parent.compose(local, max_work).map_err(interval_error)?,
                );
                queue.push_back(child_face);
            }
        }
        if poses.len() != audit.faces().len() {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let mut checked_hinges = Vec::with_capacity(hinges.len());
        for hinge in hinges {
            charged = charged
                .checked_add(1)
                .filter(|value| *value <= max_work)
                .ok_or(KinematicsError::ResourceLimitExceeded)?;
            if spanning.contains(&hinge.edge()) {
                // Spanning transforms were constructed from this exact
                // interval hinge. Recomputing them with fresh outward rounding
                // cannot add a closure premise and can only create a false
                // mismatch. Only non-spanning hinges constrain closure.
                checked_hinges.push(hinge.edge());
                continue;
            }
            let left = *poses
                .get(&hinge.left_face())
                .ok_or(KinematicsError::UnsupportedTopology)?;
            let right = *poses
                .get(&hinge.right_face())
                .ok_or(KinematicsError::UnsupportedTopology)?;
            let degrees = boxes[&hinge.edge()];
            let sign = if hinge.assignment() == ori_topology::FoldAssignment::Mountain {
                1.0
            } else {
                -1.0
            };
            let local = IntervalRigidTransformV1::about_axis(
                [
                    sign * hinge.axis().x(),
                    sign * hinge.axis().y(),
                    sign * hinge.axis().z(),
                ],
                [hinge.start().x(), hinge.start().y(), hinge.start().z()],
                degrees,
                max_work,
            )
            .map_err(interval_error)?;
            let expected = left.compose(local, max_work).map_err(interval_error)?;
            if !expected.universally_matches_within(right, tolerance) {
                return Err(KinematicsError::UnsupportedTopology);
            }
            checked_hinges.push(hinge.edge());
        }
        Ok(MaterialHingeIntervalClosureCertificateV1 {
            version: MATERIAL_HINGE_INTERVAL_CLOSURE_CERTIFICATE_VERSION_V1,
            fixed_face,
            checked_hinges,
        })
    }

    /// Measures the canonical spanning candidate against every retained hinge
    /// without promoting it to a closure certificate.
    pub fn measure_spanning_closure(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angles: &CanonicalHingeAngles,
    ) -> Result<MaterialHingeClosureResidual, KinematicsError> {
        let observed = self.solve_closed(audit, fixed_face, angles, f64::MAX)?;
        Ok(MaterialHingeClosureResidual {
            maximum_axis_point_error: observed.closure.maximum_axis_point_error,
            maximum_relative_transform_error: observed.closure.maximum_relative_transform_error,
        })
    }

    /// Validates a complete caller-derived embedding against every material
    /// hinge and packages it only when closure succeeds.
    pub fn observe_closed(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angles: &CanonicalHingeAngles,
        candidate: &[CandidateFaceTransform],
        tolerance: f64,
    ) -> Result<ClosedMaterialHingeGraphPose, KinematicsError> {
        if self.face_ids() != audit.faces()
            || self
                .face_ids()
                .binary_search_by_key(&fixed_face.canonical_bytes(), FaceId::canonical_bytes)
                .is_err()
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let transforms = canonical_transforms(audit, candidate)?;
        let closure = MaterialHingeClosureCertificate::observe(
            audit,
            self.hinges(),
            angles,
            &transforms,
            tolerance,
        )?;
        Ok(ClosedMaterialHingeGraphPose {
            fixed_face,
            angles: angles.clone(),
            transforms,
            closure,
        })
    }

    /// Propagates a canonical spanning tree and then verifies all retained
    /// material hinges. No closure constraint is discarded.
    pub fn solve_closed(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        angles: &CanonicalHingeAngles,
        tolerance: f64,
    ) -> Result<ClosedMaterialHingeGraphPose, KinematicsError> {
        if self.face_ids() != audit.faces()
            || self.hinges().len() != angles.as_slice().len()
            || self.hinges().len() != audit.spanning_hinges().len() + audit.closure_hinges().len()
            || self
                .face_ids()
                .binary_search_by_key(&fixed_face.canonical_bytes(), FaceId::canonical_bytes)
                .is_err()
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let angle_values = angles
            .as_slice()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect::<HashMap<_, _>>();
        if angle_values.len() != self.hinges().len()
            || self
                .hinges()
                .iter()
                .any(|hinge| !angle_values.contains_key(&hinge.edge()))
        {
            return Err(KinematicsError::UnsupportedTopology);
        }

        let spanning = audit
            .spanning_hinges()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut adjacency = HashMap::<FaceId, Vec<(FaceId, usize, f64)>>::new();
        adjacency
            .try_reserve(self.face_ids().len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        for face in self.face_ids() {
            adjacency.insert(*face, Vec::new());
        }
        for (index, hinge) in self.hinges().iter().enumerate() {
            if !spanning.contains(&hinge.edge()) {
                continue;
            }
            let assignment_sign = match hinge.assignment() {
                ori_topology::FoldAssignment::Mountain => 1.0,
                ori_topology::FoldAssignment::Valley => -1.0,
            };
            adjacency
                .get_mut(&hinge.left_face())
                .ok_or(KinematicsError::UnsupportedTopology)?
                .push((hinge.right_face(), index, assignment_sign));
            adjacency
                .get_mut(&hinge.right_face())
                .ok_or(KinematicsError::UnsupportedTopology)?
                .push((hinge.left_face(), index, -assignment_sign));
        }
        for neighbors in adjacency.values_mut() {
            neighbors.sort_unstable_by_key(|(_, index, _)| {
                self.hinges()[*index].edge().canonical_bytes()
            });
        }

        let mut solved = HashMap::new();
        solved
            .try_reserve(self.face_ids().len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        solved.insert(fixed_face, RigidTransform::identity());
        let mut queue = VecDeque::new();
        queue
            .try_reserve(self.face_ids().len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        queue.push_back(fixed_face);
        while let Some(parent_face) = queue.pop_front() {
            let parent = *solved
                .get(&parent_face)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            for &(child_face, hinge_index, rotation_sign) in adjacency
                .get(&parent_face)
                .ok_or(KinematicsError::UnsupportedTopology)?
            {
                if solved.contains_key(&child_face) {
                    continue;
                }
                let hinge = &self.hinges()[hinge_index];
                let angle = *angle_values
                    .get(&hinge.edge())
                    .ok_or(KinematicsError::UnsupportedTopology)?;
                let local = RigidTransform::around_axis(
                    hinge.start(),
                    hinge.axis(),
                    angle * rotation_sign,
                )?;
                solved.insert(child_face, parent.compose(local)?);
                queue.push_back(child_face);
            }
        }
        if solved.len() != self.face_ids().len() {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let transforms = self
            .face_ids()
            .iter()
            .map(|face| {
                solved
                    .get(face)
                    .copied()
                    .map(|transform| CandidateFaceTransform::new(*face, transform))
                    .ok_or(KinematicsError::UnsupportedTopology)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.observe_closed(audit, fixed_face, angles, &transforms, tolerance)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialHingeClosureResidual {
    maximum_axis_point_error: f64,
    maximum_relative_transform_error: f64,
}

impl MaterialHingeClosureResidual {
    #[must_use]
    pub const fn maximum_axis_point_error(self) -> f64 {
        self.maximum_axis_point_error
    }
    #[must_use]
    pub const fn maximum_relative_transform_error(self) -> f64 {
        self.maximum_relative_transform_error
    }
    #[must_use]
    pub fn maximum_error(self) -> f64 {
        self.maximum_axis_point_error
            .max(self.maximum_relative_transform_error)
    }
}

/// Observation-only evidence that every material hinge closes in a candidate
/// embedding. It grants neither solve nor mutation authority.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialHingeClosureCertificate {
    checked_hinges: Vec<EdgeId>,
    maximum_axis_point_error: f64,
    maximum_relative_transform_error: f64,
}

impl MaterialHingeClosureCertificate {
    /// Checks both material-axis endpoints and the complete expected relative
    /// rigid rotation on every hinge, including non-tree closure hinges.
    pub fn observe(
        audit: &MaterialHingeGraphAudit,
        hinges: &[TreeHinge],
        angles: &CanonicalHingeAngles,
        candidate: &[CandidateFaceTransform],
        tolerance: f64,
    ) -> Result<Self, KinematicsError> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(KinematicsError::UnrepresentableGeometry);
        }
        if hinges.len() != audit.spanning_hinges.len() + audit.closure_hinges.len()
            || candidate.len() != audit.faces.len()
            || angles.as_slice().len() != hinges.len()
        {
            return Err(KinematicsError::UnsupportedTopology);
        }
        let transforms = canonical_transforms(audit, candidate)?;
        let mut hinges = hinges.iter().collect::<Vec<_>>();
        hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
        let mut checked_hinges = Vec::new();
        checked_hinges
            .try_reserve_exact(hinges.len())
            .map_err(|_| KinematicsError::ResourceLimitExceeded)?;
        let mut maximum_axis_point_error = 0.0_f64;
        let mut maximum_relative_transform_error = 0.0_f64;

        for (hinge, angle) in hinges.into_iter().zip(angles.as_slice()) {
            if hinge.edge() != angle.edge()
                || !audit.spanning_hinges.contains(&hinge.edge())
                    && !audit.closure_hinges.contains(&hinge.edge())
            {
                return Err(KinematicsError::UnsupportedTopology);
            }
            let left = transform_for(&transforms, hinge.left_face())?;
            let right = transform_for(&transforms, hinge.right_face())?;
            for point in [hinge.start(), hinge.end()] {
                let error = point_distance(left.apply_point(point)?, right.apply_point(point)?)?;
                maximum_axis_point_error = maximum_axis_point_error.max(error);
                if error > tolerance {
                    return Err(KinematicsError::UnsupportedTopology);
                }
            }
            let signed_angle = crate::assignment_signed_angle_degrees_v1(
                hinge.edge(),
                hinge.assignment(),
                *angle,
            )?;
            let expected = left.compose(RigidTransform::around_axis(
                hinge.start(),
                hinge.axis(),
                signed_angle,
            )?)?;
            let error = transform_error(expected, right)?;
            maximum_relative_transform_error = maximum_relative_transform_error.max(error);
            if error > tolerance {
                return Err(KinematicsError::UnsupportedTopology);
            }
            checked_hinges.push(hinge.edge());
        }
        Ok(Self {
            checked_hinges,
            maximum_axis_point_error,
            maximum_relative_transform_error,
        })
    }

    #[must_use]
    pub fn checked_hinges(&self) -> &[EdgeId] {
        &self.checked_hinges
    }

    #[must_use]
    pub const fn maximum_axis_point_error(&self) -> f64 {
        self.maximum_axis_point_error
    }

    #[must_use]
    pub const fn maximum_relative_transform_error(&self) -> f64 {
        self.maximum_relative_transform_error
    }
}

fn canonical_transforms(
    audit: &MaterialHingeGraphAudit,
    candidate: &[CandidateFaceTransform],
) -> Result<Vec<CandidateFaceTransform>, KinematicsError> {
    let mut transforms = candidate.to_vec();
    transforms.sort_unstable_by_key(|item| item.face.canonical_bytes());
    if transforms
        .windows(2)
        .any(|pair| pair[0].face == pair[1].face)
        || transforms
            .iter()
            .map(|item| item.face)
            .ne(audit.faces.iter().copied())
    {
        return Err(KinematicsError::UnsupportedTopology);
    }
    Ok(transforms)
}

fn transform_for(
    transforms: &[CandidateFaceTransform],
    face: FaceId,
) -> Result<RigidTransform, KinematicsError> {
    transforms
        .binary_search_by_key(&face.canonical_bytes(), |item| item.face.canonical_bytes())
        .map(|index| transforms[index].transform)
        .map_err(|_| KinematicsError::UnsupportedTopology)
}

fn point_distance(first: crate::Point3, second: crate::Point3) -> Result<f64, KinematicsError> {
    let squared = (first.x() - second.x()).powi(2)
        + (first.y() - second.y()).powi(2)
        + (first.z() - second.z()).powi(2);
    let distance = libm::sqrt(squared);
    if distance.is_finite() {
        Ok(distance)
    } else {
        Err(KinematicsError::UnrepresentableGeometry)
    }
}

fn transform_error(first: RigidTransform, second: RigidTransform) -> Result<f64, KinematicsError> {
    let mut maximum = point_distance(first.translation(), second.translation())?;
    for (first_row, second_row) in first
        .rotation_rows()
        .into_iter()
        .zip(second.rotation_rows())
    {
        for (first_value, second_value) in first_row.into_iter().zip(second_row) {
            maximum = maximum.max((first_value - second_value).abs());
        }
    }
    Ok(maximum)
}

/// A bounded, geometry-free audit of a connected material hinge graph.
///
/// This is deliberately not a pose authority.  In particular, closure hinges
/// identify constraints that a future general graph solver must prove; they
/// must never be silently discarded and solved as an unconstrained tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialHingeGraphAudit {
    faces: Vec<FaceId>,
    spanning_hinges: Vec<EdgeId>,
    closure_hinges: Vec<EdgeId>,
}

impl MaterialHingeGraphAudit {
    /// Validates connectivity and deterministically partitions hinges into a
    /// canonical spanning tree and the remaining loop-closure constraints.
    pub fn prepare(
        topology: &TopologySnapshot,
        limits: TreeKinematicsLimits,
    ) -> Result<Self, KinematicsError> {
        if topology.faces.is_empty()
            || topology.faces.len() > limits.max_faces
            || topology.hinge_adjacency.len() > limits.max_hinges
            || topology
                .hinge_adjacency
                .len()
                .checked_mul(2)
                .is_none_or(|entries| entries > limits.max_adjacency_entries)
        {
            return Err(
                if topology.faces.len() > limits.max_faces
                    || topology.hinge_adjacency.len() > limits.max_hinges
                {
                    KinematicsError::ResourceLimitExceeded
                } else {
                    KinematicsError::UnsupportedTopology
                },
            );
        }

        let mut faces = topology
            .faces
            .iter()
            .map(|face| face.id)
            .collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        if faces.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(KinematicsError::UnsupportedTopology);
        }

        let indices = faces
            .iter()
            .copied()
            .enumerate()
            .map(|(index, face)| (face, index))
            .collect::<HashMap<_, _>>();
        let mut hinges = topology.hinge_adjacency.iter().collect::<Vec<_>>();
        hinges.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
        let mut edge_ids = HashSet::with_capacity(hinges.len());
        let mut parent = (0..faces.len()).collect::<Vec<_>>();
        let mut rank = vec![0_u8; faces.len()];
        let mut spanning_hinges = Vec::with_capacity(faces.len().saturating_sub(1));
        let mut closure_hinges = Vec::new();

        for hinge in hinges {
            if hinge.first == hinge.second || !edge_ids.insert(hinge.edge) {
                return Err(KinematicsError::UnsupportedTopology);
            }
            let first = *indices
                .get(&hinge.first)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            let second = *indices
                .get(&hinge.second)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            if union(&mut parent, &mut rank, first, second) {
                spanning_hinges.push(hinge.edge);
            } else {
                closure_hinges.push(hinge.edge);
            }
        }
        if spanning_hinges.len() != faces.len().saturating_sub(1) {
            return Err(KinematicsError::UnsupportedTopology);
        }
        Ok(Self {
            faces,
            spanning_hinges,
            closure_hinges,
        })
    }

    #[must_use]
    pub fn faces(&self) -> &[FaceId] {
        &self.faces
    }

    #[must_use]
    pub fn spanning_hinges(&self) -> &[EdgeId] {
        &self.spanning_hinges
    }

    #[must_use]
    pub fn closure_hinges(&self) -> &[EdgeId] {
        &self.closure_hinges
    }

    #[must_use]
    pub const fn is_tree(&self) -> bool {
        self.closure_hinges.is_empty()
    }
}

fn find(parent: &mut [usize], mut node: usize) -> usize {
    while parent[node] != node {
        parent[node] = parent[parent[node]];
        node = parent[node];
    }
    node
}

fn union(parent: &mut [usize], rank: &mut [u8], first: usize, second: usize) -> bool {
    let mut first = find(parent, first);
    let mut second = find(parent, second);
    if first == second {
        return false;
    }
    if rank[first] < rank[second] {
        std::mem::swap(&mut first, &mut second);
    }
    parent[second] = first;
    if rank[first] == rank[second] {
        rank[first] = rank[first].saturating_add(1);
    }
    true
}

// Narrow analytic identity for a flat stack cut by one collective world-axis
// fold. Exact profile equality and bounded revalidated initial-axis collinearity carry the
// loop identity over the full rational parameter domain; midpoint and target
// solves revalidate the claimed branch before a certificate is issued.
fn collective_flat_stack_cycle_closure_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    if audit.closure_hinges().is_empty() || !tolerance.is_finite() || tolerance < 0.0 {
        return false;
    }
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
    let Ok(initial_pose) = geometry.solve_closed(audit, fixed_face, &initial_angles, tolerance)
    else {
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
        bounded_same_infinite_line(reference_start, reference_axis, start, axis, tolerance)
            && bounded_same_infinite_line(reference_start, reference_axis, end, axis, tolerance)
            && bounded_same_infinite_line(
                reference_start,
                reference_axis,
                reference_end,
                axis,
                tolerance,
            )
    }) {
        return false;
    }
    [midpoint_angles, requested_angles]
        .into_iter()
        .all(|angles| {
            geometry
                .solve_closed(audit, fixed_face, &angles, tolerance)
                .is_ok()
        })
}

// Narrow non-collinear identity R(a)R(b)R(b)^-1R(a)^-1.  The four hinges
// share one pivot, the middle and outer axes pair respectively, and their
// assignments provide the inverse signs.  Exact collective scheduling makes
// the cancellation valid for every parameter value, while three solved poses
// revalidate the admitted branch and orientation convention.
fn orthogonal_inverse_pair_cycle_closure_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    if geometry.hinges().len() != 4 || audit.closure_hinges().len() != 1 {
        return false;
    }
    let Some(moving) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    if moving.len() != 4
        || geometry
            .hinges()
            .iter()
            .any(|hinge| !moving.contains(&hinge.edge()))
    {
        return false;
    }
    let mut ordered = Vec::with_capacity(4);
    let mut face = fixed_face;
    let mut used = HashSet::new();
    for _ in 0..4 {
        let Some(hinge) = geometry.hinges().iter().find(|hinge| {
            !used.contains(&hinge.edge())
                && (hinge.left_face() == face || hinge.right_face() == face)
        }) else {
            return false;
        };
        used.insert(hinge.edge());
        face = if hinge.left_face() == face {
            hinge.right_face()
        } else {
            hinge.left_face()
        };
        ordered.push(hinge);
    }
    if face != fixed_face
        || ordered[0].assignment() != ordered[1].assignment()
        || ordered[2].assignment() != ordered[3].assignment()
        || ordered[0].assignment() == ordered[3].assignment()
    {
        return false;
    }
    let same_point = |a: crate::Point3, b: crate::Point3| {
        (a.x() - b.x()).abs() <= tolerance
            && (a.y() - b.y()).abs() <= tolerance
            && (a.z() - b.z()).abs() <= tolerance
    };
    let parallel = |a: crate::Point3, b: crate::Point3| {
        let cross = [
            a.y() * b.z() - a.z() * b.y(),
            a.z() * b.x() - a.x() * b.z(),
            a.x() * b.y() - a.y() * b.x(),
        ];
        cross.into_iter().all(|value| value.abs() <= tolerance)
    };
    let pivot = ordered[0].start();
    if ordered
        .iter()
        .any(|hinge| !same_point(pivot, hinge.start()))
        || !parallel(ordered[0].axis(), ordered[3].axis())
        || !parallel(ordered[1].axis(), ordered[2].axis())
        || parallel(ordered[0].axis(), ordered[1].axis())
    {
        return false;
    }
    [0.0, 0.5, 1.0].into_iter().all(|u| {
        schedule.evaluate(u).is_some_and(|angles| {
            geometry
                .solve_closed(audit, fixed_face, &angles, tolerance)
                .is_ok()
        })
    })
}

fn symmetric_rational_kawasaki_cycle_closure_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    if geometry.hinges().len() != 4 || audit.closure_hinges().len() != 1 {
        return false;
    }
    let Some((unit_edges, scaled_edges, sector_cosine)) = [(1, 2, 0.5), (3, 5, 0.6)]
        .into_iter()
        .find_map(|(numerator, denominator, cosine)| {
            schedule
                .symmetric_kawasaki_half_angle_pairs_v1(numerator, denominator)
                .map(|(unit, scaled)| (unit, scaled, cosine))
        })
    else {
        return false;
    };
    let unit = unit_edges.into_iter().collect::<HashSet<_>>();
    let scaled = scaled_edges.into_iter().collect::<HashSet<_>>();
    let mut ordered = Vec::with_capacity(4);
    let mut face = fixed_face;
    let mut used = HashSet::new();
    for _ in 0..4 {
        let Some(hinge) = geometry.hinges().iter().find(|hinge| {
            !used.contains(&hinge.edge())
                && (hinge.left_face() == face || hinge.right_face() == face)
        }) else {
            return false;
        };
        used.insert(hinge.edge());
        face = if hinge.left_face() == face {
            hinge.right_face()
        } else {
            hinge.left_face()
        };
        ordered.push(hinge);
    }
    if face != fixed_face
        || unit.contains(&ordered[0].edge()) != unit.contains(&ordered[2].edge())
        || unit.contains(&ordered[1].edge()) != unit.contains(&ordered[3].edge())
        || unit.contains(&ordered[0].edge()) == unit.contains(&ordered[1].edge())
        || ordered
            .iter()
            .filter(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain)
            .count()
            != 1
        || ordered
            .iter()
            .find(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain)
            .is_none_or(|hinge| !scaled.contains(&hinge.edge()))
    {
        return false;
    }
    let cosine = |a: crate::Point3, b: crate::Point3| a.x() * b.x() + a.y() * b.y() + a.z() * b.z();
    let adjacent_cosines = (0..4)
        .map(|index| cosine(ordered[index].axis(), ordered[(index + 1) % 4].axis()))
        .collect::<Vec<_>>();
    let negative = adjacent_cosines
        .iter()
        .filter(|value| (**value + sector_cosine).abs() <= tolerance)
        .count();
    let positive = adjacent_cosines
        .iter()
        .filter(|value| (**value - sector_cosine).abs() <= tolerance)
        .count();
    if negative != 2 || positive != 2 {
        return false;
    }
    [0.0, 0.5, 1.0].into_iter().all(|u| {
        schedule.evaluate(u).is_some_and(|angles| {
            geometry
                .solve_closed(audit, fixed_face, &angles, tolerance)
                .is_ok()
        })
    })
}

fn bounded_same_infinite_line(
    origin: crate::Point3,
    axis: crate::Point3,
    point: crate::Point3,
    candidate_axis: crate::Point3,
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

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeId, FaceId, ProjectId};
    use ori_topology::{
        BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot,
    };

    use super::*;
    use crate::{HalfAngleRationalEntryInputV1, HingeAngle, Point3, RationalCoefficientV1};

    fn face(id: FaceId) -> Face {
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

    fn topology(faces: &[FaceId], hinges: &[(EdgeId, FaceId, FaceId)]) -> TopologySnapshot {
        TopologySnapshot {
            source_revision: 1,
            faces: faces.iter().copied().map(face).collect(),
            edge_incidence: Vec::new(),
            hinge_adjacency: hinges
                .iter()
                .map(|(edge, first, second)| FaceAdjacency {
                    edge: *edge,
                    first: *first,
                    second: *second,
                    assignment: FoldAssignment::Mountain,
                })
                .collect(),
            material_components: Vec::new(),
        }
    }

    #[test]
    fn canonical_partition_exposes_cycle_constraints_without_pose_authority() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"a");
        let b = FaceId::derive_v5(namespace, b"b");
        let c = FaceId::derive_v5(namespace, b"c");
        let ab = EdgeId::derive_v5(namespace, b"ab");
        let bc = EdgeId::derive_v5(namespace, b"bc");
        let ca = EdgeId::derive_v5(namespace, b"ca");
        let baseline = topology(&[a, b, c], &[(ab, a, b), (bc, b, c), (ca, c, a)]);
        let mut reordered = topology(&[c, a, b], &[(ca, c, a), (ab, a, b), (bc, b, c)]);

        let expected =
            MaterialHingeGraphAudit::prepare(&baseline, TreeKinematicsLimits::default()).unwrap();
        let actual =
            MaterialHingeGraphAudit::prepare(&reordered, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(actual, expected);
        assert!(!actual.is_tree());
        assert_eq!(actual.spanning_hinges().len(), 2);
        assert_eq!(actual.closure_hinges().len(), 1);

        reordered.hinge_adjacency.pop();
        let tree =
            MaterialHingeGraphAudit::prepare(&reordered, TreeKinematicsLimits::default()).unwrap();
        assert!(tree.is_tree());
    }

    #[test]
    fn disconnected_duplicate_and_bounded_inputs_fail_closed() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"a");
        let b = FaceId::derive_v5(namespace, b"b");
        let c = FaceId::derive_v5(namespace, b"c");
        let ab = EdgeId::derive_v5(namespace, b"ab");
        let disconnected = topology(&[a, b, c], &[(ab, a, b)]);
        assert_eq!(
            MaterialHingeGraphAudit::prepare(&disconnected, TreeKinematicsLimits::default()),
            Err(KinematicsError::UnsupportedTopology)
        );

        let duplicate = topology(&[a, b], &[(ab, a, b), (ab, a, b)]);
        assert_eq!(
            MaterialHingeGraphAudit::prepare(&duplicate, TreeKinematicsLimits::default()),
            Err(KinematicsError::UnsupportedTopology)
        );

        let limits = TreeKinematicsLimits {
            max_faces: 1,
            ..TreeKinematicsLimits::default()
        };
        assert_eq!(
            MaterialHingeGraphAudit::prepare(&topology(&[a, b], &[(ab, a, b)]), limits),
            Err(KinematicsError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn closure_certificate_checks_axis_and_relative_rotation_on_every_hinge() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"a");
        let b = FaceId::derive_v5(namespace, b"b");
        let ab = EdgeId::derive_v5(namespace, b"ab");
        let topology = topology(&[a, b], &[(ab, a, b)]);
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let start = Point3::new(0.0, 0.0, 0.0).unwrap();
        let end = Point3::new(1.0, 0.0, 0.0).unwrap();
        let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
        let hinge = TreeHinge::new_for_test(ab, FoldAssignment::Mountain, a, b, start, end, axis);
        let right = RigidTransform::around_axis(start, axis, 90.0).unwrap();
        let angles = CanonicalHingeAngles::new(vec![HingeAngle::new(ab, 90.0).unwrap()]).unwrap();
        let candidate = [
            CandidateFaceTransform::new(b, right),
            CandidateFaceTransform::new(a, RigidTransform::identity()),
        ];
        let certificate =
            MaterialHingeClosureCertificate::observe(&audit, &[hinge], &angles, &candidate, 0.0)
                .unwrap();
        assert_eq!(certificate.checked_hinges(), &[ab]);
        assert_eq!(certificate.maximum_axis_point_error(), 0.0);
        assert_eq!(certificate.maximum_relative_transform_error(), 0.0);
    }

    #[test]
    fn closure_certificate_rejects_incomplete_duplicate_and_nonclosing_evidence() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"a");
        let b = FaceId::derive_v5(namespace, b"b");
        let ab = EdgeId::derive_v5(namespace, b"ab");
        let topology = topology(&[a, b], &[(ab, a, b)]);
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let start = Point3::new(0.0, 0.0, 0.0).unwrap();
        let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
        let hinge = TreeHinge::new_for_test(ab, FoldAssignment::Mountain, a, b, start, axis, axis);
        let angles = CanonicalHingeAngles::new(vec![HingeAngle::new(ab, 90.0).unwrap()]).unwrap();
        let identity = RigidTransform::identity();
        let duplicate = [
            CandidateFaceTransform::new(a, identity),
            CandidateFaceTransform::new(a, identity),
        ];
        assert_eq!(
            MaterialHingeClosureCertificate::observe(
                &audit,
                std::slice::from_ref(&hinge),
                &angles,
                &duplicate,
                0.0,
            ),
            Err(KinematicsError::UnsupportedTopology)
        );
        let nonclosing = [
            CandidateFaceTransform::new(a, identity),
            CandidateFaceTransform::new(b, identity),
        ];
        assert_eq!(
            MaterialHingeClosureCertificate::observe(
                &audit,
                &[hinge],
                &angles,
                &nonclosing,
                1.0e-12,
            ),
            Err(KinematicsError::UnsupportedTopology)
        );
    }

    #[test]
    fn closure_certificate_does_not_skip_cycle_closure_hinges() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"a");
        let b = FaceId::derive_v5(namespace, b"b");
        let c = FaceId::derive_v5(namespace, b"c");
        let ab = EdgeId::derive_v5(namespace, b"ab");
        let bc = EdgeId::derive_v5(namespace, b"bc");
        let ca = EdgeId::derive_v5(namespace, b"ca");
        let topology = topology(&[a, b, c], &[(ab, a, b), (bc, b, c), (ca, c, a)]);
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(audit.closure_hinges().len(), 1);
        let start = Point3::new(0.0, 0.0, 0.0).unwrap();
        let end = Point3::new(1.0, 0.0, 0.0).unwrap();
        let axis = end;
        let hinges = [
            TreeHinge::new_for_test(ab, FoldAssignment::Mountain, a, b, start, end, axis),
            TreeHinge::new_for_test(bc, FoldAssignment::Mountain, b, c, start, end, axis),
            TreeHinge::new_for_test(ca, FoldAssignment::Mountain, c, a, start, end, axis),
        ];
        let mut raw_angles = [ab, bc, ca]
            .map(|edge| HingeAngle::new(edge, 0.0).unwrap())
            .to_vec();
        raw_angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
        let angles = CanonicalHingeAngles::new(raw_angles).unwrap();
        let candidate =
            [a, b, c].map(|face| CandidateFaceTransform::new(face, RigidTransform::identity()));
        let certificate =
            MaterialHingeClosureCertificate::observe(&audit, &hinges, &angles, &candidate, 0.0)
                .unwrap();
        assert_eq!(certificate.checked_hinges().len(), 3);
        assert!(
            audit
                .closure_hinges()
                .iter()
                .all(|edge| certificate.checked_hinges().contains(edge))
        );
        let geometry =
            MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges.to_vec());
        let zero = geometry
            .measure_spanning_closure(&audit, audit.faces()[0], &angles)
            .unwrap();
        assert_eq!(zero.maximum_error().to_bits(), 0.0_f64.to_bits());
        let mut nonzero = [ab, bc, ca]
            .map(|edge| HingeAngle::new(edge, 90.0).unwrap())
            .to_vec();
        nonzero.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
        let nonzero = CanonicalHingeAngles::new(nonzero).unwrap();
        assert!(
            geometry
                .measure_spanning_closure(&audit, audit.faces()[0], &nonzero)
                .unwrap()
                .maximum_error()
                > 0.0
        );
    }

    #[test]
    fn interval_closure_accepts_both_assignment_signs_and_checks_version() {
        let namespace = ProjectId::new();
        for (label, assignment) in [
            (b"mountain".as_slice(), FoldAssignment::Mountain),
            (b"valley".as_slice(), FoldAssignment::Valley),
        ] {
            let a = FaceId::derive_v5(namespace, &[label, b"a"].concat());
            let b = FaceId::derive_v5(namespace, &[label, b"b"].concat());
            let edge = EdgeId::derive_v5(namespace, label);
            let mut source = topology(&[a, b], &[(edge, a, b)]);
            source.hinge_adjacency[0].assignment = assignment;
            let audit =
                MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
            let start = Point3::new(0.0, 0.0, 0.0).unwrap();
            let end = Point3::new(1.0, 0.0, 0.0).unwrap();
            let hinge = TreeHinge::new_for_test(edge, assignment, a, b, start, end, end);
            let geometry =
                MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), vec![hinge]);
            let angle = OutwardIntervalV1::new(30.0, 30.0).unwrap();
            let certificate = geometry
                .prove_interval_closure_v1(&audit, a, &[(edge, angle)], 1.0e-10, 1_000_000)
                .unwrap();
            assert_eq!(
                certificate.version(),
                MATERIAL_HINGE_INTERVAL_CLOSURE_CERTIFICATE_VERSION_V1
            );
            assert_eq!(certificate.fixed_face(), a);
            assert_eq!(certificate.checked_hinges(), &[edge]);
        }
    }

    #[test]
    fn interval_closure_fails_closed_when_cycle_correlation_is_lost() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"interval-a");
        let b = FaceId::derive_v5(namespace, b"interval-b");
        let c = FaceId::derive_v5(namespace, b"interval-c");
        let ab = EdgeId::derive_v5(namespace, b"interval-ab");
        let bc = EdgeId::derive_v5(namespace, b"interval-bc");
        let ca = EdgeId::derive_v5(namespace, b"interval-ca");
        let source = topology(&[a, b, c], &[(ab, a, b), (bc, b, c), (ca, c, a)]);
        let audit =
            MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
        let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
        let x = Point3::new(1.0, 0.0, 0.0).unwrap();
        let y = Point3::new(0.0, 1.0, 0.0).unwrap();
        let z = Point3::new(0.0, 0.0, 1.0).unwrap();
        let hinges = vec![
            TreeHinge::new_for_test(ab, FoldAssignment::Mountain, a, b, origin, x, x),
            TreeHinge::new_for_test(bc, FoldAssignment::Mountain, b, c, origin, y, y),
            TreeHinge::new_for_test(ca, FoldAssignment::Mountain, c, a, origin, z, z),
        ];
        let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
        let mut boxes = [ab, bc, ca]
            .map(|edge| (edge, OutwardIntervalV1::new(10.0, 20.0).unwrap()))
            .to_vec();
        boxes.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
        assert!(
            geometry
                .prove_interval_closure_v1(&audit, a, &boxes, 1.0e-12, 1_000_000)
                .is_err()
        );
        assert_eq!(
            geometry.prove_interval_closure_v1(&audit, a, &boxes, 1.0e-12, 1),
            Err(KinematicsError::ResourceLimitExceeded)
        );
    }

    fn rational_symmetric_cycle_fixture(
        axis_perturbation: f64,
        sign: i64,
    ) -> Result<
        (
            MaterialHingeGraphGeometry,
            MaterialHingeGraphAudit,
            CanonicalCycleScheduleV1,
        ),
        crate::CycleSchedulePrepareErrorV1,
    > {
        let namespace = ProjectId::new();
        let faces = [b"a", b"b", b"c", b"d"].map(|name| FaceId::derive_v5(namespace, name));
        let edges = [b"ab", b"bc", b"cd", b"da"].map(|name| EdgeId::derive_v5(namespace, name));
        let mut source = topology(
            &faces,
            &[
                (edges[0], faces[0], faces[1]),
                (edges[1], faces[1], faces[2]),
                (edges[2], faces[2], faces[3]),
                (edges[3], faces[3], faces[0]),
            ],
        );
        for adjacency in &mut source.hinge_adjacency[..3] {
            adjacency.assignment = FoldAssignment::Valley;
        }
        let audit =
            MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
        let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
        let axes = [
            Point3::new(1.0, 0.0, 0.0).unwrap(),
            Point3::new(-0.6, 0.8, 0.0).unwrap(),
            Point3::new(-0.28, -0.96 + axis_perturbation, 0.0).unwrap(),
            Point3::new(0.6, -0.8, 0.0).unwrap(),
        ];
        let hinges = (0..4)
            .map(|index| {
                TreeHinge::new_for_test(
                    edges[index],
                    source.hinge_adjacency[index].assignment,
                    faces[index],
                    faces[(index + 1) % 4],
                    origin,
                    axes[index],
                    axes[index],
                )
            })
            .collect();
        let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
        let mut inputs = edges
            .into_iter()
            .enumerate()
            .map(|(index, edge)| HalfAngleRationalEntryInputV1 {
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
                numerator_power_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: if index % 2 == 0 { sign } else { 3 * sign },
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: if index % 2 == 0 { 1 } else { 5 },
                    denominator: 1,
                }],
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            audit.faces()[0],
            inputs,
            CycleScheduleLimitsV1::default(),
        )?;
        Ok((geometry, audit, schedule))
    }

    #[test]
    fn rational_three_fifths_sector_closes_exactly() {
        let (geometry, audit, schedule) = rational_symmetric_cycle_fixture(0.0, 1).unwrap();
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                audit.faces()[0],
                &schedule,
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 16,
                    max_leaves: 65_536,
                    max_work: 1_048_576,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .expect("exact 3/5 symmetric sector closure");
        assert_eq!(closure.leaves().len(), 1);
    }

    #[test]
    fn rational_sector_rejects_near_degenerate_and_mixed_sign_profiles() {
        let (geometry, audit, schedule) = rational_symmetric_cycle_fixture(1.0e-5, 1).unwrap();
        assert!(
            geometry
                .prove_dyadic_schedule_closure_v1(
                    &audit,
                    audit.faces()[0],
                    &schedule,
                    1.0e-9,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 16,
                        max_leaves: 65_536,
                        max_work: 1_048_576,
                        schedule_limits: CycleScheduleLimitsV1::default(),
                    },
                )
                .is_err()
        );
        assert!(rational_symmetric_cycle_fixture(0.0, -1).is_err());
        assert!(
            schedule
                .symmetric_kawasaki_half_angle_pairs_v1(3, 4)
                .is_none()
        );
        assert!(
            schedule
                .symmetric_kawasaki_half_angle_pairs_v1(3, 3)
                .is_none()
        );
    }

    #[test]
    fn noncommuting_four_hinge_inverse_pairs_close_as_an_interval_identity() {
        let namespace = ProjectId::new();
        let faces = [b"a", b"b", b"c", b"d"].map(|name| FaceId::derive_v5(namespace, name));
        let edges = [b"ab", b"bc", b"cd", b"da"].map(|name| EdgeId::derive_v5(namespace, name));
        let mut source = topology(
            &faces,
            &[
                (edges[0], faces[0], faces[1]),
                (edges[1], faces[1], faces[2]),
                (edges[2], faces[2], faces[3]),
                (edges[3], faces[3], faces[0]),
            ],
        );
        source.hinge_adjacency[2].assignment = FoldAssignment::Valley;
        source.hinge_adjacency[3].assignment = FoldAssignment::Valley;
        let audit =
            MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
        let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
        let x = Point3::new(1.0, 0.0, 0.0).unwrap();
        let y = Point3::new(0.0, 1.0, 0.0).unwrap();
        let hinges = vec![
            TreeHinge::new_for_test(
                edges[0],
                FoldAssignment::Mountain,
                faces[0],
                faces[1],
                origin,
                x,
                x,
            ),
            TreeHinge::new_for_test(
                edges[1],
                FoldAssignment::Mountain,
                faces[1],
                faces[2],
                origin,
                y,
                y,
            ),
            TreeHinge::new_for_test(
                edges[2],
                FoldAssignment::Valley,
                faces[2],
                faces[3],
                origin,
                y,
                y,
            ),
            TreeHinge::new_for_test(
                edges[3],
                FoldAssignment::Valley,
                faces[3],
                faces[0],
                origin,
                x,
                x,
            ),
        ];
        let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
        let mut boxes = edges
            .map(|edge| (edge, OutwardIntervalV1::new(37.0, 37.0).unwrap()))
            .to_vec();
        boxes.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
        let certificate = geometry
            .prove_interval_closure_v1(&audit, faces[0], &boxes, 1.0e-9, 1_000_000)
            .unwrap();
        assert_eq!(certificate.checked_hinges().len(), 4);

        let mut schedule_entries = edges
            .map(|edge| crate::HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [
                    crate::RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    crate::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![
                    crate::RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    crate::RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![crate::RationalCoefficientV1 {
                    numerator: 3,
                    denominator: 1,
                }],
            })
            .to_vec();
        schedule_entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let schedule = crate::CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            faces[0],
            schedule_entries,
            crate::CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let dyadic = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                faces[0],
                &schedule,
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 8,
                    max_leaves: 256,
                    max_work: 1_000_000,
                    schedule_limits: crate::CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        assert_eq!(dyadic.leaves().len(), 1);
    }
}
