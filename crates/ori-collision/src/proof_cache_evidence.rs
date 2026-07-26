//! Sealed pair evidence and its complete dependency footprint.

use std::sync::Arc;

use num_rational::BigRational;
use ori_domain::{EdgeId, FaceId, VertexId};

#[cfg(test)]
use super::MAX_PROOF_CACHE_STORAGE_BYTES_V1;
use super::{
    ProofCacheErrorV1, ProofCacheKeyV1, ProofCachePairWorkV1,
    encoding::{encode_exact_face_pose_v1, logical_entry_storage_bytes_v1, pair_proof_binding_v1},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedPairProofConclusionV1 {
    NonBlocking,
    Blocking,
}

#[derive(Debug, Clone)]
pub struct CachedPairProofResultV1 {
    pub(super) identity: Arc<()>,
    pub(super) conclusion: CachedPairProofConclusionV1,
    pub(super) binding: [u8; 32],
}

impl CachedPairProofResultV1 {
    pub(super) fn issue_bound_v1(
        key: &ProofCacheKeyV1,
        conclusion: CachedPairProofConclusionV1,
        work: &ProofCachePairWorkV1,
        dependencies: &PairProofDependenciesV1,
    ) -> Result<Self, ProofCacheErrorV1> {
        let binding = pair_proof_binding_v1(key, conclusion, work, dependencies)?;
        Ok(Self {
            identity: Arc::new(()),
            conclusion,
            binding,
        })
    }

    #[must_use]
    pub const fn conclusion(&self) -> CachedPairProofConclusionV1 {
        self.conclusion
    }

    #[must_use]
    pub const fn binding(&self) -> [u8; 32] {
        self.binding
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    pub(super) fn same_content(&self, other: &Self) -> bool {
        self.conclusion == other.conclusion && self.binding == other.binding
    }
}

impl PartialEq for CachedPairProofResultV1 {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
            && self.conclusion == other.conclusion
            && self.binding == other.binding
    }
}

impl Eq for CachedPairProofResultV1 {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactFacePoseCacheWitnessV1 {
    pub(super) face: FaceId,
    pub(super) canonical_exact_bytes: Vec<u8>,
}

pub(crate) struct ExactFacePoseComponentsV1<'a> {
    pub(crate) face: FaceId,
    pub(crate) rotation: &'a [[BigRational; 3]; 3],
    pub(crate) translation: &'a [BigRational; 3],
    pub(crate) boundary: &'a [(VertexId, [BigRational; 3])],
}

impl ExactFacePoseCacheWitnessV1 {
    pub(crate) fn from_exact_components_v1(
        components: ExactFacePoseComponentsV1<'_>,
    ) -> Result<Self, ProofCacheErrorV1> {
        if components.face.canonical_bytes() == [0; 16] || components.boundary.len() < 3 {
            return Err(ProofCacheErrorV1::InvalidCandidate);
        }
        let mut boundary_ids = components
            .boundary
            .iter()
            .map(|(vertex, _)| vertex.canonical_bytes())
            .collect::<Vec<_>>();
        boundary_ids.sort_unstable();
        if boundary_ids[0] == [0; 16] || boundary_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ProofCacheErrorV1::InvalidCandidate);
        }
        Ok(Self {
            face: components.face,
            canonical_exact_bytes: encode_exact_face_pose_v1(
                components.face,
                components.rotation,
                components.translation,
                components.boundary,
            )?,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_test_canonical_exact_bytes_v1(
        face: FaceId,
        canonical_exact_bytes: Vec<u8>,
    ) -> Result<Self, ProofCacheErrorV1> {
        (!canonical_exact_bytes.is_empty()
            && canonical_exact_bytes.len() <= MAX_PROOF_CACHE_STORAGE_BYTES_V1
            && face.canonical_bytes() != [0; 16])
            .then_some(Self {
                face,
                canonical_exact_bytes,
            })
            .ok_or(ProofCacheErrorV1::InvalidCandidate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProofMemoDependencyTokenV1 {
    pub(super) issuer: [u8; 32],
    pub(super) generation: u64,
    pub(super) content_fingerprint: [u8; 32],
    pub(super) entry_fingerprint: [u8; 32],
}

impl ProofMemoDependencyTokenV1 {
    #[cfg(test)]
    pub(crate) fn new_v1(
        issuer: [u8; 32],
        generation: u64,
        content_fingerprint: [u8; 32],
        entry_fingerprint: [u8; 32],
    ) -> Result<Self, ProofCacheErrorV1> {
        (issuer != [0; 32]
            && generation != 0
            && content_fingerprint != [0; 32]
            && entry_fingerprint != [0; 32])
            .then_some(Self {
                issuer,
                generation,
                content_fingerprint,
                entry_fingerprint,
            })
            .ok_or(ProofCacheErrorV1::InvalidCandidate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceDependencyFootprintV1 {
    pub(super) face: FaceId,
    pub(super) vertices: Vec<VertexId>,
    pub(super) edges: Vec<EdgeId>,
}

impl FaceDependencyFootprintV1 {
    pub(crate) fn from_complete_face_v1(
        face: FaceId,
        mut vertices: Vec<VertexId>,
        mut edges: Vec<EdgeId>,
    ) -> Result<Self, ProofCacheErrorV1> {
        vertices.sort_unstable_by_key(VertexId::canonical_bytes);
        vertices.dedup();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        edges.dedup();
        (face.canonical_bytes() != [0; 16]
            && !vertices.is_empty()
            && !edges.is_empty()
            && vertices.iter().all(|id| id.canonical_bytes() != [0; 16])
            && edges.iter().all(|id| id.canonical_bytes() != [0; 16]))
        .then_some(Self {
            face,
            vertices,
            edges,
        })
        .ok_or(ProofCacheErrorV1::InvalidCandidate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PairProofDependenciesV1 {
    pub(super) footprints: [FaceDependencyFootprintV1; 2],
    pub(super) exact_poses: [ExactFacePoseCacheWitnessV1; 2],
    pub(super) memo_dependencies: Vec<ProofMemoDependencyTokenV1>,
}

impl PairProofDependenciesV1 {
    pub(crate) fn new_v1(
        key: &ProofCacheKeyV1,
        mut footprints: [FaceDependencyFootprintV1; 2],
        mut exact_poses: [ExactFacePoseCacheWitnessV1; 2],
        mut memo_dependencies: Vec<ProofMemoDependencyTokenV1>,
    ) -> Result<Self, ProofCacheErrorV1> {
        footprints.sort_unstable_by_key(|item| item.face.canonical_bytes());
        exact_poses.sort_unstable_by_key(|item| item.face.canonical_bytes());
        memo_dependencies.sort_unstable();
        memo_dependencies.dedup();
        (footprints.each_ref().map(|item| item.face) == key.faces
            && exact_poses.each_ref().map(|item| item.face) == key.faces)
            .then_some(Self {
                footprints,
                exact_poses,
                memo_dependencies,
            })
            .ok_or(ProofCacheErrorV1::InvalidCandidate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PairProofCacheCandidateV1 {
    pub(super) key: ProofCacheKeyV1,
    pub(super) result: CachedPairProofResultV1,
    pub(super) work: ProofCachePairWorkV1,
    pub(super) dependencies: PairProofDependenciesV1,
    pub(super) logical_storage_bytes: usize,
}

impl PairProofCacheCandidateV1 {
    pub(crate) fn new_v1(
        key: ProofCacheKeyV1,
        conclusion: CachedPairProofConclusionV1,
        work: ProofCachePairWorkV1,
        dependencies: PairProofDependenciesV1,
    ) -> Result<Self, ProofCacheErrorV1> {
        let logical_storage_bytes = logical_entry_storage_bytes_v1(&dependencies)?;
        let result =
            CachedPairProofResultV1::issue_bound_v1(&key, conclusion, &work, &dependencies)?;
        Ok(Self {
            key,
            result,
            work,
            dependencies,
            logical_storage_bytes,
        })
    }

    pub(super) fn reauthenticate_v1(&self) -> Result<(), ProofCacheErrorV1> {
        let expected = CachedPairProofResultV1::issue_bound_v1(
            &self.key,
            self.result.conclusion,
            &self.work,
            &self.dependencies,
        )?;
        if self.result.same_content(&expected)
            && self.logical_storage_bytes == logical_entry_storage_bytes_v1(&self.dependencies)?
        {
            Ok(())
        } else {
            Err(ProofCacheErrorV1::InvalidCandidate)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PairProofCacheEntryV1 {
    pub(super) key: ProofCacheKeyV1,
    pub(super) result: CachedPairProofResultV1,
    pub(super) work: ProofCachePairWorkV1,
    pub(super) dependencies: PairProofDependenciesV1,
    pub(super) logical_storage_bytes: usize,
}

impl From<PairProofCacheCandidateV1> for PairProofCacheEntryV1 {
    fn from(candidate: PairProofCacheCandidateV1) -> Self {
        Self {
            key: candidate.key,
            result: candidate.result,
            work: candidate.work,
            dependencies: candidate.dependencies,
            logical_storage_bytes: candidate.logical_storage_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheHitV1 {
    pub(super) key: ProofCacheKeyV1,
    pub(super) result: CachedPairProofResultV1,
    pub(super) accounted_work: ProofCachePairWorkV1,
}

impl ProofCacheHitV1 {
    #[must_use]
    pub const fn key(&self) -> &ProofCacheKeyV1 {
        &self.key
    }

    #[must_use]
    pub const fn result(&self) -> &CachedPairProofResultV1 {
        &self.result
    }

    #[must_use]
    pub const fn accounted_work(&self) -> &ProofCachePairWorkV1 {
        &self.accounted_work
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCacheBatchLookupV1 {
    pub(super) hits: Vec<ProofCacheHitV1>,
    pub(super) missing_entries: usize,
    pub(super) total_accounted_work: ProofCachePairWorkV1,
    pub(super) cache_operation_work: usize,
    pub(super) runtime_operation_work: usize,
}

impl ProofCacheBatchLookupV1 {
    #[must_use]
    pub fn hits(&self) -> &[ProofCacheHitV1] {
        &self.hits
    }

    #[must_use]
    pub const fn missing_entries(&self) -> usize {
        self.missing_entries
    }

    #[must_use]
    pub const fn total_accounted_work(&self) -> &ProofCachePairWorkV1 {
        &self.total_accounted_work
    }

    #[must_use]
    pub const fn cache_operation_work(&self) -> usize {
        self.cache_operation_work
    }

    /// Complete bounded runtime-side work, including current-snapshot
    /// canonicalization, any differential rebind, lookup, and hit validation.
    #[must_use]
    pub const fn runtime_operation_work(&self) -> usize {
        self.runtime_operation_work
    }

    pub(crate) fn set_runtime_operation_work_v1(&mut self, work: usize) {
        self.runtime_operation_work = work;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofCachePublishReportV1 {
    pub admitted_entries: usize,
    pub already_present_entries: usize,
    pub unproven_due_to_capacity: usize,
    pub total_entries: usize,
    pub logical_storage_bytes: usize,
    pub cache_operation_work: usize,
}
