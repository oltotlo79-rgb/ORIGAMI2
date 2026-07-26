//! Complete cache keys and trusted edit-impact aggregates.

use std::cmp::Ordering;

use ori_domain::{EdgeId, FaceId, ProjectId, VertexId};

use crate::continuous_path::{
    STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
};

use super::{
    MAX_PROOF_CACHE_INVALIDATION_WORK_V1, ProofCacheErrorV1, ProofCacheOperationControlV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofCacheCertificateModelV1 {
    SingleHingeZeroThickness,
    SingleHingePositiveThickness,
    CollinearTreeZeroThickness,
    TwoHingePositiveThickness,
    TwoHingeIntervalZeroThickness,
    TreeIntervalZeroThickness,
    CycleIntervalZeroThickness,
    CactusPositiveThickness,
}

impl ProofCacheCertificateModelV1 {
    #[must_use]
    pub const fn model_id(self) -> &'static str {
        match self {
            Self::SingleHingeZeroThickness => {
                STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
            Self::SingleHingePositiveThickness => {
                STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
            Self::CollinearTreeZeroThickness => {
                STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
            Self::TwoHingePositiveThickness => {
                STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
            Self::TwoHingeIntervalZeroThickness => {
                STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
            Self::TreeIntervalZeroThickness => {
                STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
            Self::CycleIntervalZeroThickness => {
                STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
            Self::CactusPositiveThickness => {
                STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
            }
        }
    }

    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::SingleHingeZeroThickness => 0,
            Self::SingleHingePositiveThickness => 1,
            Self::CollinearTreeZeroThickness => 2,
            Self::TwoHingePositiveThickness => 3,
            Self::TwoHingeIntervalZeroThickness => 4,
            Self::TreeIntervalZeroThickness => 5,
            Self::CycleIntervalZeroThickness => 6,
            Self::CactusPositiveThickness => 7,
        }
    }
}

pub struct ProofCacheKeyInputV1 {
    pub project_instance_id: ProjectId,
    pub project_id: ProjectId,
    pub revision: u64,
    pub geometry_fingerprint: [u8; 32],
    pub pose_generation: u64,
    pub paper_thickness_mm: f64,
    pub faces: [FaceId; 2],
    pub certificate_model: ProofCacheCertificateModelV1,
    pub issuer_context: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProofCacheKeyV1 {
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) geometry_fingerprint: [u8; 32],
    pub(super) pose_generation: u64,
    pub(super) paper_thickness_bits: u64,
    pub(super) faces: [FaceId; 2],
    pub(super) certificate_model: ProofCacheCertificateModelV1,
    pub(super) issuer_context: [u8; 32],
}

impl ProofCacheKeyV1 {
    pub fn new(input: ProofCacheKeyInputV1) -> Result<Self, ProofCacheErrorV1> {
        let mut faces = input.faces;
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        if input.project_instance_id.canonical_bytes() == [0; 16]
            || input.project_id.canonical_bytes() == [0; 16]
            || input.geometry_fingerprint == [0; 32]
            || input.pose_generation == 0
            || !input.paper_thickness_mm.is_finite()
            || input.paper_thickness_mm < 0.0
            || faces[0].canonical_bytes() == [0; 16]
            || faces[0] == faces[1]
            || input.issuer_context == [0; 32]
        {
            return Err(ProofCacheErrorV1::InvalidKey);
        }
        Ok(Self {
            project_instance_id: input.project_instance_id,
            project_id: input.project_id,
            revision: input.revision,
            geometry_fingerprint: input.geometry_fingerprint,
            pose_generation: input.pose_generation,
            paper_thickness_bits: input.paper_thickness_mm.to_bits(),
            faces,
            certificate_model: input.certificate_model,
            issuer_context: input.issuer_context,
        })
    }

    #[must_use]
    pub const fn faces(&self) -> [FaceId; 2] {
        self.faces
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn pose_generation(&self) -> u64 {
        self.pose_generation
    }

    #[must_use]
    pub const fn paper_thickness_bits(&self) -> u64 {
        self.paper_thickness_bits
    }

    #[must_use]
    pub const fn certificate_model(&self) -> ProofCacheCertificateModelV1 {
        self.certificate_model
    }
}

impl Ord for ProofCacheKeyV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.project_instance_id.canonical_bytes(),
            self.project_id.canonical_bytes(),
            self.revision,
            self.geometry_fingerprint,
            self.pose_generation,
            self.paper_thickness_bits,
            self.faces.map(|face| face.canonical_bytes()),
            self.certificate_model,
            self.issuer_context,
        )
            .cmp(&(
                other.project_instance_id.canonical_bytes(),
                other.project_id.canonical_bytes(),
                other.revision,
                other.geometry_fingerprint,
                other.pose_generation,
                other.paper_thickness_bits,
                other.faces.map(|face| face.canonical_bytes()),
                other.certificate_model,
                other.issuer_context,
            ))
    }
}

impl PartialOrd for ProofCacheKeyV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedEditImpactSetV1 {
    pub(super) source_revision: u64,
    pub(super) target_revision: u64,
    pub(super) vertices: Vec<VertexId>,
    pub(super) edges: Vec<EdgeId>,
    pub(super) faces: Vec<FaceId>,
    pub(super) preparation_work: usize,
    pub(super) upstream_preparation_work: usize,
}

impl AppliedEditImpactSetV1 {
    #[cfg(test)]
    pub(crate) fn from_complete_aggregate_v1(
        source_revision: u64,
        target_revision: u64,
        vertices: Vec<VertexId>,
        edges: Vec<EdgeId>,
        faces: Vec<FaceId>,
        control: &ProofCacheOperationControlV1<'_>,
    ) -> Result<Self, ProofCacheErrorV1> {
        Self::from_complete_aggregate_with_upstream_work_v1(
            source_revision,
            target_revision,
            vertices,
            edges,
            faces,
            0,
            control,
        )
    }

    pub(crate) fn from_complete_aggregate_with_upstream_work_v1(
        source_revision: u64,
        target_revision: u64,
        mut vertices: Vec<VertexId>,
        mut edges: Vec<EdgeId>,
        mut faces: Vec<FaceId>,
        upstream_preparation_work: usize,
        control: &ProofCacheOperationControlV1<'_>,
    ) -> Result<Self, ProofCacheErrorV1> {
        control.checkpoint()?;
        let aggregate_items = vertices
            .len()
            .checked_add(edges.len())
            .and_then(|value| value.checked_add(faces.len()))
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        let vertex_work = canonical_impact_work_v1(vertices.len())?;
        let edge_work = canonical_impact_work_v1(edges.len())?;
        let face_work = canonical_impact_work_v1(faces.len())?;
        let preparation_work = vertex_work
            .checked_add(edge_work)
            .and_then(|value| value.checked_add(face_work))
            .and_then(|value| value.checked_add(upstream_preparation_work))
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        if aggregate_items > MAX_PROOF_CACHE_INVALIDATION_WORK_V1
            || preparation_work > MAX_PROOF_CACHE_INVALIDATION_WORK_V1
        {
            return Err(ProofCacheErrorV1::ResourceLimitExceeded);
        }
        if target_revision <= source_revision
            || vertices.iter().any(|id| id.canonical_bytes() == [0; 16])
            || edges.iter().any(|id| id.canonical_bytes() == [0; 16])
            || faces.iter().any(|id| id.canonical_bytes() == [0; 16])
        {
            return Err(ProofCacheErrorV1::InvalidCandidate);
        }
        control.checkpoint()?;
        vertices.sort_unstable_by_key(VertexId::canonical_bytes);
        vertices.dedup();
        control.checkpoint()?;
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        edges.dedup();
        control.checkpoint()?;
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        faces.dedup();
        control.checkpoint()?;
        Ok(Self {
            source_revision,
            target_revision,
            vertices,
            edges,
            faces,
            preparation_work,
            upstream_preparation_work,
        })
    }
}

fn canonical_impact_work_v1(item_count: usize) -> Result<usize, ProofCacheErrorV1> {
    let sort_levels = if item_count <= 1 {
        0
    } else {
        usize::try_from(usize::BITS - (item_count - 1).leading_zeros())
            .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?
    };
    sort_levels
        .checked_add(2)
        .and_then(|work_per_item| item_count.checked_mul(work_per_item))
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)
}
