use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};
use ori_topology::TopologySnapshot;
use sha2::{Digest, Sha256};

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

#[derive(Debug, Clone)]
pub struct DyadicMaterialHingeIntervalClosureCertificateV1 {
    issuer_geometry: MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule_binding_fingerprint: [u8; 32],
    graph_binding_fingerprint: [u8; 32],
    leaves: Vec<(u32, u64, MaterialHingeIntervalClosureCertificateV1)>,
}

impl PartialEq for DyadicMaterialHingeIntervalClosureCertificateV1 {
    fn eq(&self, other: &Self) -> bool {
        self.issuer_geometry.same_instance(&other.issuer_geometry)
            && self.fixed_face == other.fixed_face
            && self.schedule_binding_fingerprint == other.schedule_binding_fingerprint
            && self.graph_binding_fingerprint == other.graph_binding_fingerprint
            && self.leaves == other.leaves
    }
}

impl Eq for DyadicMaterialHingeIntervalClosureCertificateV1 {}

impl DyadicMaterialHingeIntervalClosureCertificateV1 {
    #[must_use]
    pub const fn fixed_face(&self) -> FaceId {
        self.fixed_face
    }

    #[must_use]
    pub fn leaves(&self) -> &[(u32, u64, MaterialHingeIntervalClosureCertificateV1)] {
        &self.leaves
    }

    /// Confirms that the leaves form one canonical, gap-free left-to-right
    /// partition of the complete schedule domain.
    #[must_use]
    pub fn has_canonical_complete_partition_v1(&self) -> bool {
        let mut cursor = 0_u128;
        for (depth, index, _) in &self.leaves {
            if *depth >= 64 || *index >= (1_u64 << depth) {
                return false;
            }
            let width = 1_u128 << (64 - depth);
            let start = u128::from(*index) * width;
            if start != cursor {
                return false;
            }
            cursor += width;
        }
        cursor == (1_u128 << 64)
    }

    /// Confirms that every partition leaf independently covers the complete
    /// canonical hinge carrier of the bound material graph.
    #[must_use]
    pub fn every_leaf_covers_graph_v1(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        let mut hinges = geometry
            .hinges()
            .iter()
            .map(TreeHinge::edge)
            .collect::<Vec<_>>();
        hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
        self.issuer_geometry.same_instance(geometry)
            && self.has_canonical_complete_partition_v1()
            && self.leaves.iter().all(|(_, _, leaf)| {
                leaf.fixed_face == self.fixed_face && leaf.checked_hinges == hinges
            })
    }

    /// Native binding for the exact ordered partition and every independent
    /// leaf proof. This does not grant pose or mutation authority.
    #[doc(hidden)]
    #[must_use]
    pub fn partition_binding_fingerprint_v1(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"ORIGAMI2_DYADIC_CLOSURE_PARTITION_BINDING_V1");
        hash.update(self.fixed_face.canonical_bytes());
        hash.update(self.schedule_binding_fingerprint);
        hash.update(self.graph_binding_fingerprint);
        hash.update((self.leaves.len() as u64).to_be_bytes());
        for (depth, index, leaf) in &self.leaves {
            hash.update(depth.to_be_bytes());
            hash.update(index.to_be_bytes());
            hash.update(leaf.fixed_face.canonical_bytes());
            hash.update((leaf.checked_hinges.len() as u64).to_be_bytes());
            for edge in &leaf.checked_hinges {
                hash.update(edge.canonical_bytes());
            }
        }
        hash.finalize().into()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleBasisLimitsV1 {
    pub max_cycles: usize,
    pub max_edges_per_cycle: usize,
    pub max_total_cycle_edges: usize,
}

impl Default for CycleBasisLimitsV1 {
    fn default() -> Self {
        Self {
            max_cycles: 64,
            max_edges_per_cycle: 128,
            max_total_cycle_edges: 8_192,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CanonicalCycleBasisV1 {
    issuer_geometry: MaterialHingeGraphGeometry,
    cycles: Vec<Vec<EdgeId>>,
}

impl CanonicalCycleBasisV1 {
    #[must_use]
    pub fn cycles(&self) -> &[Vec<EdgeId>] {
        &self.cycles
    }

    #[must_use]
    pub fn is_for_geometry(&self, geometry: &MaterialHingeGraphGeometry) -> bool {
        self.issuer_geometry.same_instance(geometry)
    }
}

#[derive(Debug, Clone)]
pub struct SimultaneousCycleBasisClosureCertificateV1 {
    basis: CanonicalCycleBasisV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
}

impl SimultaneousCycleBasisClosureCertificateV1 {
    #[must_use]
    pub const fn basis(&self) -> &CanonicalCycleBasisV1 {
        &self.basis
    }

    #[must_use]
    pub const fn closure(&self) -> &DyadicMaterialHingeIntervalClosureCertificateV1 {
        &self.closure
    }
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
    instance: std::sync::Arc<()>,
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
    pub fn same_instance(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.instance, &other.instance)
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
    pub fn extract_canonical_cycle_basis_v1(
        &self,
        audit: &MaterialHingeGraphAudit,
        limits: CycleBasisLimitsV1,
    ) -> Result<CanonicalCycleBasisV1, DyadicIntervalClosureErrorV1> {
        if audit.faces() != self.face_ids()
            || audit.closure_hinges().is_empty()
            || limits.max_edges_per_cycle < 2
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput);
        }
        if audit.closure_hinges().len() > limits.max_cycles {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
        }
        let spanning = audit
            .spanning_hinges()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut adjacency = HashMap::<FaceId, Vec<(FaceId, EdgeId)>>::new();
        for hinge in self
            .hinges()
            .iter()
            .filter(|hinge| spanning.contains(&hinge.edge()))
        {
            adjacency
                .entry(hinge.left_face())
                .or_default()
                .push((hinge.right_face(), hinge.edge()));
            adjacency
                .entry(hinge.right_face())
                .or_default()
                .push((hinge.left_face(), hinge.edge()));
        }
        for neighbors in adjacency.values_mut() {
            neighbors.sort_unstable_by_key(|(_, edge)| edge.canonical_bytes());
        }
        let mut total = 0usize;
        let mut cycles = Vec::with_capacity(audit.closure_hinges().len());
        for closure_edge in audit.closure_hinges() {
            let hinge = self
                .hinges()
                .iter()
                .find(|hinge| hinge.edge() == *closure_edge)
                .ok_or(DyadicIntervalClosureErrorV1::InvalidInput)?;
            let start = hinge.left_face();
            let target = hinge.right_face();
            let mut queue = VecDeque::from([start]);
            let mut parent = HashMap::<FaceId, (FaceId, EdgeId)>::new();
            parent.insert(start, (start, *closure_edge));
            while let Some(face) = queue.pop_front() {
                if face == target {
                    break;
                }
                for (next, edge) in adjacency.get(&face).into_iter().flatten() {
                    if !parent.contains_key(next) {
                        parent.insert(*next, (face, *edge));
                        queue.push_back(*next);
                    }
                }
            }
            if !parent.contains_key(&target) {
                return Err(DyadicIntervalClosureErrorV1::InvalidInput);
            }
            let mut path = Vec::new();
            let mut cursor = target;
            while cursor != start {
                let (previous, edge) = parent[&cursor];
                path.push(edge);
                cursor = previous;
            }
            path.reverse();
            path.push(*closure_edge);
            if path.len() > limits.max_edges_per_cycle {
                return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
            }
            total = total
                .checked_add(path.len())
                .filter(|total| *total <= limits.max_total_cycle_edges)
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            cycles.push(path);
        }
        Ok(CanonicalCycleBasisV1 {
            issuer_geometry: self.clone(),
            cycles,
        })
    }

    pub fn prove_simultaneous_cycle_basis_schedule_closure_v1(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        tolerance: f64,
        basis_limits: CycleBasisLimitsV1,
        closure_limits: DyadicIntervalClosureLimitsV1,
    ) -> Result<SimultaneousCycleBasisClosureCertificateV1, DyadicIntervalClosureErrorV1> {
        let basis = self.extract_canonical_cycle_basis_v1(audit, basis_limits)?;
        let closure = self.prove_dyadic_schedule_closure_v1(
            audit,
            fixed_face,
            schedule,
            tolerance,
            closure_limits,
        )?;
        Ok(SimultaneousCycleBasisClosureCertificateV1 { basis, closure })
    }

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
        if let Some(group_count) = composed_symmetric_rational_cycles_premises_v1(
            self, audit, fixed_face, schedule, tolerance,
        )
        .or_else(|| {
            coupled_figure_eight_rational_cycles_premises_v1(
                self, audit, fixed_face, schedule, tolerance,
            )
            .then_some(2)
        })
        .or_else(|| {
            rational_cactus_cycles_premises_v1(self, audit, fixed_face, schedule, tolerance)
        }) {
            let required_depth = usize::BITS - (group_count - 1).leading_zeros();
            if limits.max_leaves < group_count
                || limits.max_work < group_count
                || limits.max_depth < required_depth
            {
                return Err(DyadicIntervalClosureErrorV1::ResourceLimit);
            }
            let mut checked_hinges = self
                .hinges()
                .iter()
                .map(|hinge| hinge.edge())
                .collect::<Vec<_>>();
            checked_hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
            let certificate = MaterialHingeIntervalClosureCertificateV1 {
                version: MATERIAL_HINGE_INTERVAL_CLOSURE_CERTIFICATE_VERSION_V1,
                fixed_face,
                checked_hinges,
            };
            let base_depth = usize::BITS - 1 - group_count.leading_zeros();
            let base_count = 1_usize << base_depth;
            let split_from = base_count - (group_count - base_count);
            let mut partitions = Vec::with_capacity(group_count);
            for index in 0..base_count {
                if index < split_from {
                    partitions.push((base_depth, index as u64));
                } else {
                    partitions.push((base_depth + 1, (index * 2) as u64));
                    partitions.push((base_depth + 1, (index * 2 + 1) as u64));
                }
            }
            return Ok(DyadicMaterialHingeIntervalClosureCertificateV1 {
                issuer_geometry: self.clone(),
                fixed_face,
                schedule_binding_fingerprint: schedule.certificate_binding_fingerprint_v1(),
                graph_binding_fingerprint: schedule.graph_binding_fingerprint_v1(),
                leaves: partitions
                    .into_iter()
                    .map(|(depth, index)| (depth, index, certificate.clone()))
                    .collect(),
            });
        }
        let stationary_closed = self.hinges().iter().all(|hinge| {
            schedule
                .derivative_bound(hinge.edge())
                .is_some_and(|bound| bound.to_bits() == 0.0_f64.to_bits())
        }) && schedule.evaluate(0.0).is_some_and(|angles| {
            self.solve_closed(audit, fixed_face, &angles, tolerance)
                .is_ok()
        });
        if stationary_closed
            || dense_parallel_grid_cycle_closure_premises_v1(
                self, audit, fixed_face, schedule, tolerance,
            )
            || collective_flat_stack_cycle_closure_premises_v1(
                self, audit, fixed_face, schedule, tolerance,
            )
            || orthogonal_inverse_pair_cycle_closure_premises_v1(
                self, audit, fixed_face, schedule, tolerance,
            )
            || theta_opposite_pair_cycle_closure_premises_v1(
                self, audit, fixed_face, schedule, tolerance,
            )
            || symmetric_rational_kawasaki_cycle_closure_premises_v1(
                self, audit, fixed_face, schedule, tolerance,
            )
        {
            let mut checked_hinges = self
                .hinges()
                .iter()
                .map(|hinge| hinge.edge())
                .collect::<Vec<_>>();
            checked_hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
            return Ok(DyadicMaterialHingeIntervalClosureCertificateV1 {
                issuer_geometry: self.clone(),
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
            issuer_geometry: self.clone(),
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
            instance: std::sync::Arc::new(()),
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

// Exact two-carrier accordion identity for the smallest non-cactus square grid.
// Three collinear material segments on each of two parallel carrier lines share
// one canonical profile; the six transverse hinges remain exactly stationary.
fn dense_parallel_grid_cycle_closure_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    let face_count = geometry.face_ids().len();
    let Some((columns, rows)) = (3usize..=7).find_map(|columns| {
        (3usize..=7).find_map(|rows| {
            (columns * rows == face_count
                && geometry.hinges().len() == 2 * columns * rows - columns - rows
                && audit.closure_hinges().len() == (columns - 1) * (rows - 1))
                .then_some((columns, rows))
        })
    }) else {
        return false;
    };
    if !tolerance.is_finite() || tolerance < 0.0 {
        return false;
    }
    let Some(moving_edges) = schedule
        .collective_profile_edges_v1()
        .or_else(|| schedule.collective_half_angle_profile_edges_v1())
    else {
        return false;
    };
    if moving_edges.len() != rows * (columns - 1) && moving_edges.len() != columns * (rows - 1) {
        return false;
    }
    let moving = moving_edges.into_iter().collect::<HashSet<_>>();
    let (Some(initial), Some(midpoint), Some(target)) = (
        schedule.evaluate(0.0),
        schedule.evaluate(0.5),
        schedule.evaluate(1.0),
    ) else {
        return false;
    };
    if [initial.clone(), target.clone()].into_iter().any(|angles| {
        angles.as_slice().iter().any(|angle| {
            !moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() != 0.0_f64.to_bits()
        })
    }) {
        return false;
    }
    let Ok(_pose) = geometry.solve_closed(audit, fixed_face, &initial, tolerance) else {
        return false;
    };
    [midpoint, target].into_iter().all(|angles| {
        geometry
            .solve_closed(audit, fixed_face, &angles, tolerance)
            .is_ok()
    })
}

pub fn theta_opposite_pair_cycle_closure_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    if geometry.face_ids().len() != 6
        || geometry.hinges().len() != 7
        || audit.closure_hinges().len() != 2
        || !schedule.matches_binding(geometry, audit, fixed_face)
        || !tolerance.is_finite()
        || tolerance < 0.0
    {
        return false;
    }
    let Some(moving) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    if moving.len() != 3 {
        return false;
    }
    let Some(initial) = schedule.evaluate(0.0) else {
        return false;
    };
    if initial
        .as_slice()
        .iter()
        .any(|angle| angle.angle_degrees().to_bits() != 0.0_f64.to_bits())
    {
        return false;
    }
    let mut endpoints = geometry
        .hinges()
        .iter()
        .flat_map(|hinge| [hinge.start(), hinge.end()])
        .collect::<Vec<_>>();
    endpoints.sort_by(|a, b| {
        (a.x().to_bits(), a.y().to_bits(), a.z().to_bits()).cmp(&(
            b.x().to_bits(),
            b.y().to_bits(),
            b.z().to_bits(),
        ))
    });
    endpoints.dedup();
    let pivots = endpoints
        .into_iter()
        .filter(|point| {
            geometry
                .hinges()
                .iter()
                .filter(|hinge| hinge.start() == *point || hinge.end() == *point)
                .count()
                == 4
        })
        .collect::<Vec<_>>();
    if pivots.len() != 2 {
        return false;
    }
    pivots.into_iter().all(|pivot| {
        let incident = geometry
            .hinges()
            .iter()
            .filter(|hinge| {
                moving.contains(&hinge.edge()) && (hinge.start() == pivot || hinge.end() == pivot)
            })
            .collect::<Vec<_>>();
        incident.len() == 2 && {
            let outward = |hinge: &TreeHinge| {
                if hinge.start() == pivot {
                    Some(hinge.axis())
                } else {
                    crate::Point3::new(-hinge.axis().x(), -hinge.axis().y(), -hinge.axis().z()).ok()
                }
            };
            let (Some(first), Some(second)) = (outward(incident[0]), outward(incident[1])) else {
                return false;
            };
            first.x() == -second.x() && first.y() == -second.y() && first.z() == -second.z()
        }
    })
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

fn composed_symmetric_rational_cycles_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> Option<usize> {
    let group_count = audit.closure_hinges().len();
    if !(2..=32).contains(&group_count)
        || geometry.hinges().len() != group_count * 4
        || geometry.face_ids().len() != 1 + group_count * 3
    {
        return None;
    }
    let mut remaining = geometry
        .face_ids()
        .iter()
        .copied()
        .filter(|face| *face != fixed_face)
        .collect::<HashSet<_>>();
    let mut groups = Vec::new();
    while let Some(seed) = remaining.iter().next().copied() {
        let mut faces = HashSet::from([seed]);
        let mut queue = VecDeque::from([seed]);
        remaining.remove(&seed);
        while let Some(face) = queue.pop_front() {
            for hinge in geometry.hinges() {
                let next = if hinge.left_face() == face {
                    Some(hinge.right_face())
                } else if hinge.right_face() == face {
                    Some(hinge.left_face())
                } else {
                    None
                };
                if let Some(next) = next
                    && next != fixed_face
                    && remaining.remove(&next)
                {
                    faces.insert(next);
                    queue.push_back(next);
                }
            }
        }
        groups.push(faces);
    }
    if groups.len() != group_count || groups.iter().any(|group| group.len() != 3) {
        return None;
    }
    let valid_groups = groups.iter().all(|group| {
        let edges = geometry
            .hinges()
            .iter()
            .filter(|hinge| {
                [hinge.left_face(), hinge.right_face()]
                    .into_iter()
                    .all(|face| face == fixed_face || group.contains(&face))
            })
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        symmetric_rational_cycle_group_premises_v1(
            geometry, fixed_face, schedule, &edges, tolerance,
        )
    });
    (valid_groups
        && [0.0, 0.5, 1.0].into_iter().all(|u| {
            schedule.evaluate(u).is_some_and(|angles| {
                geometry
                    .solve_closed(audit, fixed_face, &angles, tolerance)
                    .is_ok()
            })
        }))
    .then_some(group_count)
}

fn coupled_figure_eight_rational_cycles_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    if geometry.hinges().len() != 8
        || geometry.face_ids().len() != 7
        || audit.closure_hinges().len() != 2
    {
        return false;
    }
    geometry
        .face_ids()
        .iter()
        .copied()
        .filter(|candidate| *candidate != fixed_face)
        .any(|shared| {
            let mut remaining = geometry
                .face_ids()
                .iter()
                .copied()
                .filter(|face| *face != shared)
                .collect::<HashSet<_>>();
            let mut groups = Vec::new();
            while let Some(seed) = remaining.iter().next().copied() {
                let mut faces = HashSet::from([seed]);
                let mut queue = VecDeque::from([seed]);
                remaining.remove(&seed);
                while let Some(face) = queue.pop_front() {
                    for hinge in geometry.hinges() {
                        let next = if hinge.left_face() == face {
                            Some(hinge.right_face())
                        } else if hinge.right_face() == face {
                            Some(hinge.left_face())
                        } else {
                            None
                        };
                        if let Some(next) = next
                            && next != shared
                            && remaining.remove(&next)
                        {
                            faces.insert(next);
                            queue.push_back(next);
                        }
                    }
                }
                groups.push(faces);
            }
            if groups.len() != 2 || groups.iter().any(|group| group.len() != 3) {
                return false;
            }
            let mut used_edges = HashSet::new();
            let valid = groups.iter().all(|group| {
                let edges = geometry
                    .hinges()
                    .iter()
                    .filter(|hinge| {
                        [hinge.left_face(), hinge.right_face()]
                            .into_iter()
                            .all(|face| face == shared || group.contains(&face))
                    })
                    .map(|hinge| hinge.edge())
                    .collect::<Vec<_>>();
                edges.len() == 4
                    && edges.iter().all(|edge| used_edges.insert(*edge))
                    && symmetric_rational_cycle_group_premises_v1(
                        geometry, shared, schedule, &edges, tolerance,
                    )
            });
            valid
                && used_edges.len() == geometry.hinges().len()
                && [0.0, 0.5, 1.0].into_iter().all(|u| {
                    schedule.evaluate(u).is_some_and(|angles| {
                        geometry
                            .solve_closed(audit, fixed_face, &angles, tolerance)
                            .is_ok()
                    })
                })
        })
}

fn rational_cactus_cycles_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> Option<usize> {
    let cycle_count = audit.closure_hinges().len();
    if !(3..=32).contains(&cycle_count)
        || geometry.hinges().len() != cycle_count * 4
        || geometry.face_ids().len() != 1 + cycle_count * 3
    {
        return None;
    }
    let mut cycles = HashSet::<Vec<EdgeId>>::new();
    for start in geometry.face_ids() {
        let mut stack = vec![(*start, Vec::<FaceId>::new(), Vec::<EdgeId>::new())];
        while let Some((face, mut visited_faces, mut edges)) = stack.pop() {
            if edges.len() == 4 {
                if face == *start && visited_faces.len() == 4 {
                    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
                    edges.dedup();
                    if edges.len() == 4 {
                        cycles.insert(edges);
                    }
                }
                continue;
            }
            if visited_faces.contains(&face) {
                continue;
            }
            visited_faces.push(face);
            for hinge in geometry.hinges() {
                let next = if hinge.left_face() == face {
                    Some(hinge.right_face())
                } else if hinge.right_face() == face {
                    Some(hinge.left_face())
                } else {
                    None
                };
                if let Some(next) = next {
                    let mut next_edges = edges.clone();
                    next_edges.push(hinge.edge());
                    stack.push((next, visited_faces.clone(), next_edges));
                }
            }
        }
    }
    if cycles.len() != cycle_count {
        return None;
    }
    let cycles = cycles.into_iter().collect::<Vec<_>>();
    let mut all_edges = HashSet::new();
    if !cycles
        .iter()
        .flat_map(|cycle| cycle.iter())
        .all(|edge| all_edges.insert(*edge))
        || all_edges.len() != geometry.hinges().len()
    {
        return None;
    }
    let cycle_faces = cycles
        .iter()
        .map(|cycle| {
            geometry
                .hinges()
                .iter()
                .filter(|hinge| cycle.contains(&hinge.edge()))
                .flat_map(|hinge| [hinge.left_face(), hinge.right_face()])
                .collect::<HashSet<_>>()
        })
        .collect::<Vec<_>>();
    if cycle_faces.iter().any(|faces| faces.len() != 4) {
        return None;
    }
    let mut face_incidence = HashMap::<FaceId, Vec<usize>>::new();
    for (block, faces) in cycle_faces.iter().enumerate() {
        for face in faces {
            face_incidence.entry(*face).or_default().push(block);
        }
    }
    for first in 0..cycle_count {
        for second in first + 1..cycle_count {
            let overlap = cycle_faces[first]
                .intersection(&cycle_faces[second])
                .count();
            if overlap > 1 {
                return None;
            }
        }
    }
    let articulation_faces = face_incidence
        .values()
        .filter(|blocks| blocks.len() > 1)
        .collect::<Vec<_>>();
    let incidence_edge_count = articulation_faces
        .iter()
        .map(|blocks| blocks.len())
        .sum::<usize>();
    let mut seen_blocks = HashSet::from([0_usize]);
    let mut seen_articulations = HashSet::new();
    let mut queue = VecDeque::from([0_usize]);
    while let Some(block) = queue.pop_front() {
        for (articulation, blocks) in articulation_faces.iter().enumerate() {
            if blocks.contains(&block) && seen_articulations.insert(articulation) {
                for next in *blocks {
                    if seen_blocks.insert(*next) {
                        queue.push_back(*next);
                    }
                }
            }
        }
    }
    let incidence_node_count = cycle_count + articulation_faces.len();
    if incidence_edge_count + 1 != incidence_node_count || seen_blocks.len() != cycle_count {
        return None;
    }
    let valid = cycles.iter().zip(&cycle_faces).all(|(edges, faces)| {
        let start = faces
            .iter()
            .copied()
            .min_by_key(FaceId::canonical_bytes)
            .expect("a four-cycle has faces");
        symmetric_rational_cycle_group_premises_v1(geometry, start, schedule, edges, tolerance)
    });
    (valid
        && [0.0, 0.5, 1.0].into_iter().all(|u| {
            schedule.evaluate(u).is_some_and(|angles| {
                geometry
                    .solve_closed(audit, fixed_face, &angles, tolerance)
                    .is_ok()
            })
        }))
    .then_some(cycle_count)
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
    let edges = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    symmetric_rational_cycle_group_premises_v1(geometry, fixed_face, schedule, &edges, tolerance)
        && [0.0, 0.5, 1.0].into_iter().all(|u| {
            schedule.evaluate(u).is_some_and(|angles| {
                geometry
                    .solve_closed(audit, fixed_face, &angles, tolerance)
                    .is_ok()
            })
        })
}

fn symmetric_rational_cycle_group_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    edges: &[EdgeId],
    tolerance: f64,
) -> bool {
    let Some((unit_edges, scaled_edges, numerator, denominator)) =
        schedule.bounded_symmetric_kawasaki_profile_for_edges_v1(edges)
    else {
        return false;
    };
    let sector_cosine = numerator as f64 / denominator as f64;
    let unit = unit_edges.into_iter().collect::<HashSet<_>>();
    let scaled = scaled_edges.into_iter().collect::<HashSet<_>>();
    let mut ordered = Vec::with_capacity(4);
    let mut face = fixed_face;
    let mut used = HashSet::new();
    for _ in 0..4 {
        let Some(hinge) = geometry.hinges().iter().find(|hinge| {
            edges.contains(&hinge.edge())
                && !used.contains(&hinge.edge())
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
    let candidates = [ordered[0].start(), ordered[0].end()];
    let Some(pivot) = candidates.into_iter().find(|candidate| {
        ordered
            .iter()
            .all(|hinge| hinge.start() == *candidate || hinge.end() == *candidate)
    }) else {
        return false;
    };
    let outward_axes = ordered
        .iter()
        .map(|hinge| {
            if hinge.start() == pivot {
                hinge.axis()
            } else {
                crate::Point3::new(-hinge.axis().x(), -hinge.axis().y(), -hinge.axis().z())
                    .expect("negating a finite unit axis remains representable")
            }
        })
        .collect::<Vec<_>>();
    let cosine = |a: crate::Point3, b: crate::Point3| a.x() * b.x() + a.y() * b.y() + a.z() * b.z();
    let adjacent_cosines = (0..4)
        .map(|index| cosine(outward_axes[index], outward_axes[(index + 1) % 4]))
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
    true
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
    fn theta_graph_partition_consumes_each_shared_path_hinge_once() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"theta-a");
        let b = FaceId::derive_v5(namespace, b"theta-b");
        let c = FaceId::derive_v5(namespace, b"theta-c");
        let d = FaceId::derive_v5(namespace, b"theta-d");
        let ab = EdgeId::derive_v5(namespace, b"theta-ab");
        let bd = EdgeId::derive_v5(namespace, b"theta-bd");
        let ac = EdgeId::derive_v5(namespace, b"theta-ac");
        let cd = EdgeId::derive_v5(namespace, b"theta-cd");
        let ad = EdgeId::derive_v5(namespace, b"theta-ad");
        let baseline = topology(
            &[a, b, c, d],
            &[(ab, a, b), (bd, b, d), (ac, a, c), (cd, c, d), (ad, a, d)],
        );
        let reordered = topology(
            &[d, c, b, a],
            &[(ad, d, a), (cd, d, c), (ac, c, a), (bd, d, b), (ab, b, a)],
        );
        let expected =
            MaterialHingeGraphAudit::prepare(&baseline, TreeKinematicsLimits::default()).unwrap();
        let actual =
            MaterialHingeGraphAudit::prepare(&reordered, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.spanning_hinges().len(), 3);
        assert_eq!(actual.closure_hinges().len(), 2);
        let consumed = actual
            .spanning_hinges()
            .iter()
            .chain(actual.closure_hinges())
            .copied()
            .collect::<HashSet<_>>();
        assert_eq!(
            consumed.len(),
            5,
            "shared theta path must not be consumed twice"
        );
        assert_eq!(consumed, HashSet::from([ab, bd, ac, cd, ad]));

        let start = Point3::new(0.0, 0.0, 0.0).unwrap();
        let end = Point3::new(1.0, 0.0, 0.0).unwrap();
        let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
        let hinges = [
            TreeHinge::new_for_test(ab, FoldAssignment::Mountain, a, b, start, end, axis),
            TreeHinge::new_for_test(bd, FoldAssignment::Mountain, b, d, start, end, axis),
            TreeHinge::new_for_test(ac, FoldAssignment::Mountain, a, c, start, end, axis),
            TreeHinge::new_for_test(cd, FoldAssignment::Mountain, c, d, start, end, axis),
            TreeHinge::new_for_test(ad, FoldAssignment::Mountain, a, d, start, end, axis),
        ];
        let geometry =
            MaterialHingeGraphGeometry::new_for_test(actual.faces().to_vec(), hinges.to_vec());
        let canonical_angles = |damaged: Option<EdgeId>| {
            let mut values = [ab, bd, ac, cd, ad]
                .into_iter()
                .map(|edge| {
                    HingeAngle::new(edge, if damaged == Some(edge) { 1.0 } else { 0.0 }).unwrap()
                })
                .collect::<Vec<_>>();
            values.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
            CanonicalHingeAngles::new(values).unwrap()
        };
        let closed = geometry
            .solve_closed(&actual, a, &canonical_angles(None), 0.0)
            .expect("both theta cycles close in one complete hinge observation");
        assert_eq!(closed.closure_certificate().checked_hinges().len(), 5);
        assert_eq!(
            closed
                .closure_certificate()
                .checked_hinges()
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            consumed
        );
        assert!(
            geometry
                .solve_closed(&actual, a, &canonical_angles(Some(ad)), 0.0)
                .is_err(),
            "damaging the shared direct path must break both-cycle closure"
        );

        assert_eq!(
            MaterialHingeGraphAudit::prepare(
                &baseline,
                TreeKinematicsLimits {
                    max_hinges: 4,
                    ..TreeKinematicsLimits::default()
                },
            ),
            Err(KinematicsError::ResourceLimitExceeded)
        );
        let duplicate_shared = topology(
            &[a, b, c, d],
            &[
                (ab, a, b),
                (bd, b, d),
                (ac, a, c),
                (cd, c, d),
                (ad, a, d),
                (ad, a, d),
            ],
        );
        assert_eq!(
            MaterialHingeGraphAudit::prepare(&duplicate_shared, TreeKinematicsLimits::default()),
            Err(KinematicsError::UnsupportedTopology)
        );
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
        numerator: i64,
        denominator: i64,
        pythagorean_leg: i64,
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
        let p = numerator as f64;
        let q = denominator as f64;
        let leg = pythagorean_leg as f64;
        let axes = [
            Point3::new(1.0, 0.0, 0.0).unwrap(),
            Point3::new(-p / q, leg / q, 0.0).unwrap(),
            Point3::new(
                (2.0 * p * p - q * q) / (q * q),
                -2.0 * p * leg / (q * q) + axis_perturbation,
                0.0,
            )
            .unwrap(),
            Point3::new(p / q, -leg / q, 0.0).unwrap(),
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
                        numerator: if index % 2 == 0 {
                            sign
                        } else {
                            numerator * sign
                        },
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: vec![RationalCoefficientV1 {
                    numerator: if index % 2 == 0 { 1 } else { denominator },
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
    fn bounded_rational_symmetric_sectors_close_exactly() {
        for (numerator, denominator, leg) in [(3, 5, 4), (6, 10, 8), (5, 13, 12), (8, 17, 15)] {
            let (geometry, audit, schedule) =
                rational_symmetric_cycle_fixture(numerator, denominator, leg, 0.0, 1).unwrap();
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
                .expect("exact bounded rational symmetric sector closure");
            assert_eq!(closure.leaves().len(), 1);
        }
    }

    #[test]
    fn rational_sector_rejects_near_degenerate_and_mixed_sign_profiles() {
        let (geometry, audit, schedule) =
            rational_symmetric_cycle_fixture(3, 5, 4, 1.0e-5, 1).unwrap();
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
        assert!(rational_symmetric_cycle_fixture(3, 5, 4, 0.0, -1).is_err());
        let (large_geometry, large_audit, large_schedule) =
            rational_symmetric_cycle_fixture(65, 97, 72, 0.0, 1).unwrap();
        assert!(
            large_schedule
                .bounded_symmetric_kawasaki_profile_v1()
                .is_none()
        );
        assert!(
            large_geometry
                .prove_dyadic_schedule_closure_v1(
                    &large_audit,
                    large_audit.faces()[0],
                    &large_schedule,
                    1.0e-9,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 0,
                        max_leaves: 1,
                        max_work: 1,
                        schedule_limits: CycleScheduleLimitsV1::default(),
                    },
                )
                .is_err()
        );
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

    fn composed_rational_cycles_fixture(
        group_count: usize,
        corrupt_group: Option<usize>,
        reverse_hinges: bool,
    ) -> (
        MaterialHingeGraphGeometry,
        MaterialHingeGraphAudit,
        CanonicalCycleScheduleV1,
        FaceId,
    ) {
        composed_rational_cycles_fixture_with_fixed(group_count, corrupt_group, reverse_hinges, 0)
    }

    fn composed_rational_cycles_fixture_with_fixed(
        group_count: usize,
        corrupt_group: Option<usize>,
        reverse_hinges: bool,
        fixed_face_index: usize,
    ) -> (
        MaterialHingeGraphGeometry,
        MaterialHingeGraphAudit,
        CanonicalCycleScheduleV1,
        FaceId,
    ) {
        let namespace = ProjectId::new();
        let faces = (0..(1 + group_count * 3))
            .map(|index| FaceId::derive_v5(namespace, &[index as u8]))
            .collect::<Vec<_>>();
        let edges = (0..(group_count * 4))
            .map(|index| EdgeId::derive_v5(namespace, &[index as u8]))
            .collect::<Vec<_>>();
        let pairs = (0..group_count)
            .flat_map(|group| {
                let first = 1 + group * 3;
                [
                    (0, first),
                    (first, first + 1),
                    (first + 1, first + 2),
                    (first + 2, 0),
                ]
            })
            .collect::<Vec<_>>();
        let mut source = topology(
            &faces,
            &pairs
                .iter()
                .enumerate()
                .map(|(index, (left, right))| (edges[index], faces[*left], faces[*right]))
                .collect::<Vec<_>>(),
        );
        for (index, adjacency) in source.hinge_adjacency.iter_mut().enumerate() {
            adjacency.assignment = if index % 4 == 3 {
                FoldAssignment::Mountain
            } else {
                FoldAssignment::Valley
            };
        }
        let audit =
            MaterialHingeGraphAudit::prepare(&source, TreeKinematicsLimits::default()).unwrap();
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
        let mut hinges = Vec::new();
        for (group, (p, q, leg)) in triples.into_iter().cycle().take(group_count).enumerate() {
            let origin = Point3::new(group as f64 * 10.0, 0.0, 0.0).unwrap();
            let axes = [
                Point3::new(1.0, 0.0, 0.0).unwrap(),
                Point3::new(-p / q, leg / q, 0.0).unwrap(),
                Point3::new(
                    (2.0 * p * p - q * q) / (q * q),
                    -2.0 * p * leg / (q * q)
                        + if corrupt_group == Some(group) {
                            1.0e-4
                        } else {
                            0.0
                        },
                    0.0,
                )
                .unwrap(),
                Point3::new(p / q, -leg / q, 0.0).unwrap(),
            ];
            for (local, axis) in axes.into_iter().enumerate() {
                let index = group * 4 + local;
                hinges.push(TreeHinge::new_for_test(
                    edges[index],
                    source.hinge_adjacency[index].assignment,
                    faces[pairs[index].0],
                    faces[pairs[index].1],
                    origin,
                    Point3::new(origin.x() + axis.x(), axis.y(), 0.0).unwrap(),
                    axis,
                ));
            }
        }
        if reverse_hinges {
            hinges.reverse();
        }
        let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
        let mut inputs = edges
            .iter()
            .copied()
            .enumerate()
            .map(|(index, edge)| {
                let (p, q, _) = triples[(index / 4) % triples.len()];
                let (p, q) = (p as i64, q as i64);
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
                    numerator_power_coefficients: vec![
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: if index % 2 == 0 { 1 } else { p },
                            denominator: 1,
                        },
                    ],
                    denominator_power_coefficients: vec![RationalCoefficientV1 {
                        numerator: if index % 2 == 0 { 1 } else { q },
                        denominator: 1,
                    }],
                }
            })
            .collect::<Vec<_>>();
        inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        let fixed = faces[fixed_face_index];
        let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs,
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        (geometry, audit, schedule, fixed)
    }

    #[test]
    fn two_to_thirty_two_independent_rational_cycles_use_canonical_balanced_leaves() {
        let cases = [
            (2, 1, vec![(1, 0), (1, 1)]),
            (3, 2, vec![(1, 0), (2, 2), (2, 3)]),
            (4, 2, vec![(2, 0), (2, 1), (2, 2), (2, 3)]),
            (5, 3, vec![(2, 0), (2, 1), (2, 2), (3, 6), (3, 7)]),
            (6, 3, vec![(2, 0), (2, 1), (3, 4), (3, 5), (3, 6), (3, 7)]),
            (
                7,
                3,
                vec![(2, 0), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6), (3, 7)],
            ),
            (8, 3, (0..8).map(|index| (3, index)).collect()),
            (16, 4, (0..16).map(|index| (4, index)).collect()),
            (32, 5, (0..32).map(|index| (5, index)).collect()),
        ];
        for (group_count, max_depth, expected) in cases {
            for reverse_hinges in [false, true] {
                let (geometry, audit, schedule, fixed) =
                    composed_rational_cycles_fixture(group_count, None, reverse_hinges);
                let closure = geometry
                    .prove_dyadic_schedule_closure_v1(
                        &audit,
                        fixed,
                        &schedule,
                        1.0e-9,
                        DyadicIntervalClosureLimitsV1 {
                            max_depth,
                            max_leaves: group_count,
                            max_work: group_count,
                            schedule_limits: CycleScheduleLimitsV1::default(),
                        },
                    )
                    .expect("independent cycle proof");
                assert!(closure.has_canonical_complete_partition_v1());
                assert_eq!(
                    closure
                        .leaves()
                        .iter()
                        .map(|leaf| (leaf.0, leaf.1))
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        }
        let (geometry, audit, schedule, fixed) = composed_rational_cycles_fixture(32, None, false);
        let exact = DyadicIntervalClosureLimitsV1 {
            max_depth: 5,
            max_leaves: 32,
            max_work: 32,
            schedule_limits: CycleScheduleLimitsV1::default(),
        };
        for short in [
            DyadicIntervalClosureLimitsV1 {
                max_depth: 4,
                ..exact
            },
            DyadicIntervalClosureLimitsV1 {
                max_leaves: 31,
                ..exact
            },
            DyadicIntervalClosureLimitsV1 {
                max_work: 31,
                ..exact
            },
        ] {
            assert_eq!(
                geometry.prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, short,),
                Err(DyadicIntervalClosureErrorV1::ResourceLimit)
            );
        }
        let (corrupt, corrupt_audit, corrupt_schedule, corrupt_fixed) =
            composed_rational_cycles_fixture(32, Some(31), false);
        assert!(
            corrupt
                .prove_dyadic_schedule_closure_v1(
                    &corrupt_audit,
                    corrupt_fixed,
                    &corrupt_schedule,
                    1.0e-9,
                    exact,
                )
                .is_err()
        );
    }

    #[test]
    fn composed_cycle_partition_rejects_reordered_gapped_and_stale_leaves() {
        let (geometry, audit, schedule, fixed) = composed_rational_cycles_fixture(4, None, false);
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 2,
                    max_leaves: 4,
                    max_work: 4,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        assert!(closure.every_leaf_covers_graph_v1(&geometry));

        let mut reordered = closure.clone();
        reordered.leaves.swap(1, 2);
        assert!(!reordered.has_canonical_complete_partition_v1());
        assert_ne!(
            reordered.partition_binding_fingerprint_v1(),
            closure.partition_binding_fingerprint_v1()
        );

        let mut gapped = closure.clone();
        gapped.leaves.remove(1);
        assert!(!gapped.has_canonical_complete_partition_v1());

        let mut stale = closure;
        let valid_binding = stale.partition_binding_fingerprint_v1();
        stale.leaves[2].2.checked_hinges.pop();
        assert!(stale.has_canonical_complete_partition_v1());
        assert!(!stale.every_leaf_covers_graph_v1(&geometry));
        assert_ne!(
            stale.leaves[2].2.checked_hinges(),
            stale.leaves[0].2.checked_hinges()
        );
        assert_ne!(stale.partition_binding_fingerprint_v1(), valid_binding);
    }

    #[test]
    fn composed_cycles_reject_partial_corruption_and_accumulated_resource_shortfall() {
        let (geometry, audit, schedule, fixed) =
            composed_rational_cycles_fixture(4, Some(2), false);
        assert!(
            geometry
                .prove_dyadic_schedule_closure_v1(
                    &audit,
                    fixed,
                    &schedule,
                    1.0e-9,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 2,
                        max_leaves: 4,
                        max_work: 4,
                        schedule_limits: CycleScheduleLimitsV1::default(),
                    },
                )
                .is_err()
        );
        let (geometry, audit, schedule, fixed) = composed_rational_cycles_fixture(4, None, false);
        for limits in [
            DyadicIntervalClosureLimitsV1 {
                max_depth: 1,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
            DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 3,
                max_work: 4,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
            DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 3,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        ] {
            assert_eq!(
                geometry
                    .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, limits,),
                Err(DyadicIntervalClosureErrorV1::ResourceLimit)
            );
        }
        let (foreign_geometry, foreign_audit, _foreign_schedule, foreign_fixed) =
            composed_rational_cycles_fixture(4, None, false);
        assert_eq!(
            foreign_geometry.prove_dyadic_schedule_closure_v1(
                &foreign_audit,
                foreign_fixed,
                &schedule,
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 2,
                    max_leaves: 4,
                    max_work: 4,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            ),
            Err(DyadicIntervalClosureErrorV1::InvalidInput)
        );
    }

    #[test]
    fn coupled_figure_eight_cycles_share_only_one_nonfixed_face() {
        let limits = DyadicIntervalClosureLimitsV1 {
            max_depth: 1,
            max_leaves: 2,
            max_work: 2,
            schedule_limits: CycleScheduleLimitsV1::default(),
        };
        for reverse in [false, true] {
            let (geometry, audit, schedule, fixed) =
                composed_rational_cycles_fixture_with_fixed(2, None, reverse, 1);
            let closure = geometry
                .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, limits)
                .expect("coupled figure-eight closure");
            assert_eq!(
                closure
                    .leaves()
                    .iter()
                    .map(|leaf| (leaf.0, leaf.1))
                    .collect::<Vec<_>>(),
                vec![(1, 0), (1, 1)]
            );
        }
        let (geometry, audit, schedule, fixed) =
            composed_rational_cycles_fixture_with_fixed(2, None, false, 1);
        for short in [
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                ..limits
            },
            DyadicIntervalClosureLimitsV1 {
                max_leaves: 1,
                ..limits
            },
            DyadicIntervalClosureLimitsV1 {
                max_work: 1,
                ..limits
            },
        ] {
            assert_eq!(
                geometry.prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, short,),
                Err(DyadicIntervalClosureErrorV1::ResourceLimit)
            );
        }
        let (corrupt, corrupt_audit, corrupt_schedule, corrupt_fixed) =
            composed_rational_cycles_fixture_with_fixed(2, Some(1), false, 1);
        assert!(
            corrupt
                .prove_dyadic_schedule_closure_v1(
                    &corrupt_audit,
                    corrupt_fixed,
                    &corrupt_schedule,
                    1.0e-9,
                    limits,
                )
                .is_err()
        );
    }

    #[test]
    fn three_cycle_cactus_accepts_shared_articulation_face_with_exact_limits() {
        let limits = DyadicIntervalClosureLimitsV1 {
            max_depth: 2,
            max_leaves: 3,
            max_work: 3,
            schedule_limits: CycleScheduleLimitsV1::default(),
        };
        for reverse in [false, true] {
            let (geometry, audit, schedule, fixed) =
                composed_rational_cycles_fixture_with_fixed(3, None, reverse, 1);
            let closure = geometry
                .prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, limits)
                .expect("three-cycle cactus closure");
            assert_eq!(
                closure
                    .leaves()
                    .iter()
                    .map(|leaf| (leaf.0, leaf.1))
                    .collect::<Vec<_>>(),
                vec![(1, 0), (2, 2), (2, 3)]
            );
        }
        let (geometry, audit, schedule, fixed) =
            composed_rational_cycles_fixture_with_fixed(3, None, false, 1);
        for short in [
            DyadicIntervalClosureLimitsV1 {
                max_depth: 1,
                ..limits
            },
            DyadicIntervalClosureLimitsV1 {
                max_leaves: 2,
                ..limits
            },
            DyadicIntervalClosureLimitsV1 {
                max_work: 2,
                ..limits
            },
        ] {
            assert_eq!(
                geometry.prove_dyadic_schedule_closure_v1(&audit, fixed, &schedule, 1.0e-9, short),
                Err(DyadicIntervalClosureErrorV1::ResourceLimit)
            );
        }
        let (corrupt, corrupt_audit, corrupt_schedule, corrupt_fixed) =
            composed_rational_cycles_fixture_with_fixed(3, Some(2), false, 1);
        assert!(
            corrupt
                .prove_dyadic_schedule_closure_v1(
                    &corrupt_audit,
                    corrupt_fixed,
                    &corrupt_schedule,
                    1.0e-9,
                    limits,
                )
                .is_err()
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

    #[test]
    fn rank_sixty_four_basis_fits_the_128_hinge_ceiling_and_cancels_one_short() {
        let namespace = ProjectId::new();
        let faces = (0..65)
            .map(|index| FaceId::derive_v5(namespace, &[0x60, index as u8]))
            .collect::<Vec<_>>();
        let mut carrier = Vec::new();
        for index in 1..65 {
            carrier.push((
                EdgeId::derive_v5(namespace, &[0x10, index as u8]),
                faces[0],
                faces[index],
            ));
        }
        for index in 1..65 {
            carrier.push((
                EdgeId::derive_v5(namespace, &[0x20, index as u8]),
                faces[index],
                faces[if index == 64 { 1 } else { index + 1 }],
            ));
        }
        let source = topology(&faces, &carrier);
        let audit = MaterialHingeGraphAudit::prepare(
            &source,
            TreeKinematicsLimits {
                max_hinges: 128,
                max_adjacency_entries: 256,
                ..TreeKinematicsLimits::default()
            },
        )
        .unwrap();
        assert_eq!(
            (audit.spanning_hinges().len(), audit.closure_hinges().len()),
            (64, 64)
        );
        let start = Point3::new(0.0, 0.0, 0.0).unwrap();
        let end = Point3::new(1.0, 0.0, 0.0).unwrap();
        let hinges = carrier
            .iter()
            .map(|(edge, left, right)| {
                TreeHinge::new_for_test(
                    *edge,
                    FoldAssignment::Mountain,
                    *left,
                    *right,
                    start,
                    end,
                    end,
                )
            })
            .collect::<Vec<_>>();
        let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
        let basis = geometry
            .extract_canonical_cycle_basis_v1(
                &audit,
                CycleBasisLimitsV1 {
                    max_cycles: 64,
                    max_edges_per_cycle: 65,
                    max_total_cycle_edges: 4_160,
                },
            )
            .unwrap();
        assert_eq!(basis.cycles().len(), 64);
        let total = basis.cycles().iter().map(Vec::len).sum::<usize>();
        assert!(total <= 4_160);
        assert!(matches!(
            geometry.extract_canonical_cycle_basis_v1(
                &audit,
                CycleBasisLimitsV1 {
                    max_cycles: 0,
                    max_edges_per_cycle: 65,
                    max_total_cycle_edges: 4_160,
                },
            ),
            Err(DyadicIntervalClosureErrorV1::ResourceLimit)
        ));
        assert!(matches!(
            geometry.extract_canonical_cycle_basis_v1(
                &audit,
                CycleBasisLimitsV1 {
                    max_cycles: 64,
                    max_edges_per_cycle: 65,
                    max_total_cycle_edges: total - 1,
                },
            ),
            Err(DyadicIntervalClosureErrorV1::ResourceLimit)
        ));
        let foreign = MaterialHingeGraphGeometry::new_for_test(
            audit.faces().to_vec(),
            geometry.hinges().to_vec(),
        );
        assert!(!basis.is_for_geometry(&foreign));
    }
}
