use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};
use ori_topology::TopologySnapshot;

use crate::{
    CanonicalHingeAngles, KinematicsError, MaterialHingeGraphGeometry, RigidTransform, TreeHinge,
    TreeKinematicsLimits,
};

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
            let sign = match hinge.assignment() {
                ori_topology::FoldAssignment::Mountain => 1.0,
                ori_topology::FoldAssignment::Valley => -1.0,
            };
            let expected = left.compose(RigidTransform::around_axis(
                hinge.start(),
                hinge.axis(),
                angle.angle_degrees() * sign,
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

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeId, FaceId, ProjectId};
    use ori_topology::{
        BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot,
    };

    use super::*;
    use crate::{HingeAngle, Point3};

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
}
