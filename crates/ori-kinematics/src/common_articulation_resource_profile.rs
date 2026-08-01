//! Primitive-only resource envelopes for the general-N common-articulation
//! extension family.
//!
//! This module deliberately knows neither an authority nor a collision
//! certificate.  It supplies the checked cardinality arithmetic shared by
//! later V2 issuers without creating a V2-to-V1 conversion route.

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable domain identifier for [`CommonArticulationResourceProfileV2`].
pub const COMMON_ARTICULATION_RESOURCE_PROFILE_MODEL_ID_V2: &str =
    "common_articulation_resource_profile_v2";

const CANONICAL_MIURA_CLEARANCE_BASE_WORK_V2: usize = 32;
const CANONICAL_MIURA_CROSS_BLOCK_PAIR_BYTES_V2: usize = 32;
const CANONICAL_MIURA_CLEARANCE_BASE_BYTES_V2: usize = 1_024;
const CANONICAL_MIURA_FACE_BYTES_V2: usize = 128;
const CANONICAL_MIURA_HINGE_BYTES_V2: usize = 32;
/// The raw registry pipeline performs a pollable heap sort, local-registry
/// sort/dedup, explicit write-index compaction, and sorted local membership.
/// Eight times the bit length safely covers those comparisons without relying
/// on opaque library sorting or non-pollable bulk compaction.
const CANONICAL_MIURA_HEAPSORT_COMPARISON_FACTOR_V2: usize = 8;
const CANONICAL_MIURA_DECOMPOSITION_BASE_WORK_V2: usize = 64;
const CANONICAL_MIURA_DECOMPOSITION_FACE_WORK_V2: usize = 32;
const CANONICAL_MIURA_DECOMPOSITION_HINGE_WORK_V2: usize = 48;
const CANONICAL_MIURA_DECOMPOSITION_BLOCK_WORK_V2: usize = 96;
const CANONICAL_MIURA_DECOMPOSITION_BASE_BYTES_V2: usize = 16_384;
const CANONICAL_MIURA_DECOMPOSITION_FACE_BYTES_V2: usize = 1_024;
const CANONICAL_MIURA_DECOMPOSITION_HINGE_BYTES_V2: usize = 1_024;
const CANONICAL_MIURA_DECOMPOSITION_BLOCK_BYTES_V2: usize = 24_576;

/// Failure while constructing or checking a general-N resource profile.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationResourceProfileErrorV2 {
    #[error("the canonical Miura resource profile input is invalid")]
    InvalidInput,
    #[error("the canonical Miura resource profile exceeds a checked resource limit")]
    ResourceLimit,
}

/// Read-only resource envelope for one canonical 3x3 Miura block count.
///
/// All fields are private so an issuer cannot forge a smaller limit while
/// retaining a larger binding.  The envelope contains only primitive counts
/// and byte/work estimates; it is not an authority and is not serializable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationCanonicalMiuraResourcesV2 {
    block_count: usize,
    face_count: usize,
    hinge_count: usize,
    unordered_face_pair_count: usize,
    raw_cross_block_pair_candidates: usize,
    canonical_cross_block_pairs: usize,
    raw_sort_comparisons_per_item: usize,
    canonical_sort_comparisons_per_item: usize,
    pose_logical_work: usize,
    pose_retained_bytes: usize,
    decomposition_logical_work: usize,
    decomposition_storage_bytes: usize,
    clearance_logical_work: usize,
    clearance_storage_bytes: usize,
}

impl CommonArticulationCanonicalMiuraResourcesV2 {
    #[must_use]
    pub const fn block_count_v2(&self) -> usize {
        self.block_count
    }

    #[must_use]
    pub const fn face_count_v2(&self) -> usize {
        self.face_count
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.hinge_count
    }

    /// Safe common cap for each graph pair category.
    #[must_use]
    pub const fn unordered_face_pair_count_v2(&self) -> usize {
        self.unordered_face_pair_count
    }

    #[must_use]
    pub const fn raw_cross_block_pair_candidates_v2(&self) -> usize {
        self.raw_cross_block_pair_candidates
    }

    #[must_use]
    pub const fn canonical_cross_block_pairs_v2(&self) -> usize {
        self.canonical_cross_block_pairs
    }

    #[must_use]
    pub const fn raw_sort_comparisons_per_item_v2(&self) -> usize {
        self.raw_sort_comparisons_per_item
    }

    #[must_use]
    pub const fn canonical_sort_comparisons_per_item_v2(&self) -> usize {
        self.canonical_sort_comparisons_per_item
    }

    #[must_use]
    pub const fn pose_logical_work_v2(&self) -> usize {
        self.pose_logical_work
    }

    #[must_use]
    pub const fn pose_retained_bytes_v2(&self) -> usize {
        self.pose_retained_bytes
    }

    /// Checked work budget for the iterative general-N edge-block issuer.
    #[must_use]
    pub const fn decomposition_logical_work_v2(&self) -> usize {
        self.decomposition_logical_work
    }

    /// Conservative retained and transient allocation budget for the
    /// compact general-N edge-block issuer.
    #[must_use]
    pub const fn decomposition_storage_bytes_v2(&self) -> usize {
        self.decomposition_storage_bytes
    }

    #[must_use]
    pub const fn clearance_logical_work_v2(&self) -> usize {
        self.clearance_logical_work
    }

    #[must_use]
    pub const fn clearance_storage_bytes_v2(&self) -> usize {
        self.clearance_storage_bytes
    }
}

/// Checked, immutable resource envelope for a canonical 3x3 Miura family.
///
/// `maximum_v2` is the configured-N envelope and `actual_v2` is the live-N
/// envelope.  A future authority must bind both values and still bind its
/// geometry/decomposition/source identities separately.
///
/// N=32 remains constructible only to check arithmetic conformance with the
/// legacy extension.  This primitive profile admits no authority by itself;
/// every general-N V2 authority must require configured and actual N >= 33.
///
/// General-cell-transport and final authority memory are intentionally absent:
/// those limits depend on the live overlap-cell source and cannot soundly be
/// inferred from N alone.  Their V2 issuers must add and bind observed source
/// metrics rather than invent an N-only formula.
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationResourceProfileV2;
///
/// let _forged = CommonArticulationResourceProfileV2 {
///     configured_max_blocks: 33,
///     actual_block_count: 33,
///     maximum: todo!(),
///     actual: todo!(),
///     binding_fingerprint: [0; 32],
/// };
/// ```
///
/// ```compile_fail
/// use ori_kinematics::{
///     CommonArticulationPoseExtensionLimitsV1, CommonArticulationResourceProfileV2,
/// };
///
/// let profile = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33)
///     .unwrap();
/// let _: CommonArticulationPoseExtensionLimitsV1 = profile.into();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CommonArticulationResourceProfileV2;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationResourceProfileV2>();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationResourceProfileV2 {
    configured_max_blocks: usize,
    actual_block_count: usize,
    maximum: CommonArticulationCanonicalMiuraResourcesV2,
    actual: CommonArticulationCanonicalMiuraResourcesV2,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationResourceProfileV2 {
    /// Builds an exact configured/actual profile for the canonical 3x3 Miura
    /// family using checked arithmetic only.
    pub fn for_canonical_miura_3x3_v2(
        configured_max_blocks: usize,
        actual_block_count: usize,
    ) -> Result<Self, CommonArticulationResourceProfileErrorV2> {
        if configured_max_blocks == 0
            || actual_block_count == 0
            || actual_block_count > configured_max_blocks
        {
            return Err(CommonArticulationResourceProfileErrorV2::InvalidInput);
        }

        let maximum = canonical_miura_resources_v2(configured_max_blocks)?;
        let actual = canonical_miura_resources_v2(actual_block_count)?;
        validate_envelope_admission_v2(maximum, actual)?;
        let binding_fingerprint = resource_profile_binding_fingerprint_v2(
            configured_max_blocks,
            actual_block_count,
            maximum,
            actual,
        )?;
        Ok(Self {
            configured_max_blocks,
            actual_block_count,
            maximum,
            actual,
            binding_fingerprint,
        })
    }

    /// Convenience constructor for a profile whose configured and actual N
    /// are identical.
    pub fn exact_canonical_miura_3x3_v2(
        block_count: usize,
    ) -> Result<Self, CommonArticulationResourceProfileErrorV2> {
        Self::for_canonical_miura_3x3_v2(block_count, block_count)
    }

    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_RESOURCE_PROFILE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn configured_max_blocks_v2(&self) -> usize {
        self.configured_max_blocks
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn maximum_v2(&self) -> CommonArticulationCanonicalMiuraResourcesV2 {
        self.maximum
    }

    #[must_use]
    pub const fn actual_v2(&self) -> CommonArticulationCanonicalMiuraResourcesV2 {
        self.actual
    }

    /// Returns the stable SHA-256 binding over every configured and actual
    /// primitive field in this profile.
    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }
}

fn canonical_miura_resources_v2(
    block_count: usize,
) -> Result<CommonArticulationCanonicalMiuraResourcesV2, CommonArticulationResourceProfileErrorV2> {
    let block_count_minus_one = block_count
        .checked_sub(1)
        .ok_or(CommonArticulationResourceProfileErrorV2::InvalidInput)?;
    let block_count_squared = block_count
        .checked_mul(block_count)
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let face_count = block_count
        .checked_mul(8)
        .and_then(|value| value.checked_add(1))
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let hinge_count = block_count
        .checked_mul(12)
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let unordered_face_pair_count = checked_unordered_pair_count_v2(face_count)?;
    let raw_cross_block_pair_candidates = checked_unordered_pair_count_v2(block_count)?
        .checked_mul(81)
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let canonical_cross_block_pairs = block_count
        .checked_mul(block_count_minus_one)
        .and_then(|value| value.checked_mul(32))
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let raw_sort_comparisons_per_item =
        heap_sort_comparisons_per_item_v2(raw_cross_block_pair_candidates)?;
    let canonical_sort_comparisons_per_item =
        heap_sort_comparisons_per_item_v2(canonical_cross_block_pairs)?;
    let pose_logical_work = block_count_squared
        .checked_mul(4)
        .and_then(|value| value.checked_add(block_count.checked_mul(436)?))
        .and_then(|value| value.checked_add(24))
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let pose_retained_bytes = block_count
        .checked_mul(1_744)
        .and_then(|value| value.checked_add(496))
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let (decomposition_logical_work, decomposition_storage_bytes) =
        canonical_miura_decomposition_resources_v2(block_count)?;
    let clearance_logical_work = CANONICAL_MIURA_CLEARANCE_BASE_WORK_V2
        .checked_add(pose_logical_work)
        .and_then(|value| value.checked_add(face_count))
        .and_then(|value| value.checked_add(hinge_count))
        .and_then(|value| value.checked_add(block_count))
        .and_then(|value| value.checked_add(raw_cross_block_pair_candidates.checked_mul(3)?))
        .and_then(|value| value.checked_add(canonical_cross_block_pairs.checked_mul(2)?))
        .and_then(|value| {
            value.checked_add(
                raw_cross_block_pair_candidates.checked_mul(raw_sort_comparisons_per_item)?,
            )
        })
        .and_then(|value| {
            value.checked_add(
                canonical_cross_block_pairs.checked_mul(canonical_sort_comparisons_per_item)?,
            )
        })
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let clearance_storage_bytes = raw_cross_block_pair_candidates
        .checked_add(canonical_cross_block_pairs)
        .and_then(|value| value.checked_mul(CANONICAL_MIURA_CROSS_BLOCK_PAIR_BYTES_V2))
        .and_then(|value| value.checked_add(CANONICAL_MIURA_CLEARANCE_BASE_BYTES_V2))
        .and_then(|value| value.checked_add(face_count.checked_mul(CANONICAL_MIURA_FACE_BYTES_V2)?))
        .and_then(|value| {
            value.checked_add(hinge_count.checked_mul(CANONICAL_MIURA_HINGE_BYTES_V2)?)
        })
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;

    Ok(CommonArticulationCanonicalMiuraResourcesV2 {
        block_count,
        face_count,
        hinge_count,
        unordered_face_pair_count,
        raw_cross_block_pair_candidates,
        canonical_cross_block_pairs,
        raw_sort_comparisons_per_item,
        canonical_sort_comparisons_per_item,
        pose_logical_work,
        pose_retained_bytes,
        decomposition_logical_work,
        decomposition_storage_bytes,
        clearance_logical_work,
        clearance_storage_bytes,
    })
}

/// Checked bounds used by the V2 iterative Tarjan issuer.  The source path
/// additionally requires every canonical Miura face to be quadrilateral and
/// every emitted block to contain exactly 9 faces and 12 hinges, which makes
/// the per-record storage term a safe upper bound rather than an N-only
/// guess about arbitrary graph geometry.
pub(crate) fn canonical_miura_decomposition_resources_v2(
    block_count: usize,
) -> Result<(usize, usize), CommonArticulationResourceProfileErrorV2> {
    let face_count = block_count
        .checked_mul(8)
        .and_then(|value| value.checked_add(1))
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let hinge_count = block_count
        .checked_mul(12)
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let work = CANONICAL_MIURA_DECOMPOSITION_BASE_WORK_V2
        .checked_add(
            face_count
                .checked_mul(CANONICAL_MIURA_DECOMPOSITION_FACE_WORK_V2)
                .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?,
        )
        .and_then(|value| {
            value.checked_add(hinge_count.checked_mul(CANONICAL_MIURA_DECOMPOSITION_HINGE_WORK_V2)?)
        })
        .and_then(|value| {
            value.checked_add(block_count.checked_mul(CANONICAL_MIURA_DECOMPOSITION_BLOCK_WORK_V2)?)
        })
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    let storage = CANONICAL_MIURA_DECOMPOSITION_BASE_BYTES_V2
        .checked_add(
            face_count
                .checked_mul(CANONICAL_MIURA_DECOMPOSITION_FACE_BYTES_V2)
                .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?,
        )
        .and_then(|value| {
            value
                .checked_add(hinge_count.checked_mul(CANONICAL_MIURA_DECOMPOSITION_HINGE_BYTES_V2)?)
        })
        .and_then(|value| {
            value
                .checked_add(block_count.checked_mul(CANONICAL_MIURA_DECOMPOSITION_BLOCK_BYTES_V2)?)
        })
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
    Ok((work, storage))
}

fn checked_unordered_pair_count_v2(
    count: usize,
) -> Result<usize, CommonArticulationResourceProfileErrorV2> {
    count
        .checked_mul(
            count
                .checked_sub(1)
                .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)
}

fn bit_length_v2(value: usize) -> usize {
    usize::BITS as usize - value.max(1).leading_zeros() as usize
}

fn heap_sort_comparisons_per_item_v2(
    value: usize,
) -> Result<usize, CommonArticulationResourceProfileErrorV2> {
    bit_length_v2(value)
        .checked_mul(CANONICAL_MIURA_HEAPSORT_COMPARISON_FACTOR_V2)
        .ok_or(CommonArticulationResourceProfileErrorV2::ResourceLimit)
}

fn validate_envelope_admission_v2(
    maximum: CommonArticulationCanonicalMiuraResourcesV2,
    actual: CommonArticulationCanonicalMiuraResourcesV2,
) -> Result<(), CommonArticulationResourceProfileErrorV2> {
    if actual.block_count > maximum.block_count
        || actual.face_count > maximum.face_count
        || actual.hinge_count > maximum.hinge_count
        || actual.unordered_face_pair_count > maximum.unordered_face_pair_count
        || actual.raw_cross_block_pair_candidates > maximum.raw_cross_block_pair_candidates
        || actual.canonical_cross_block_pairs > maximum.canonical_cross_block_pairs
        || actual.raw_sort_comparisons_per_item > maximum.raw_sort_comparisons_per_item
        || actual.canonical_sort_comparisons_per_item > maximum.canonical_sort_comparisons_per_item
        || actual.pose_logical_work > maximum.pose_logical_work
        || actual.pose_retained_bytes > maximum.pose_retained_bytes
        || actual.decomposition_logical_work > maximum.decomposition_logical_work
        || actual.decomposition_storage_bytes > maximum.decomposition_storage_bytes
        || actual.clearance_logical_work > maximum.clearance_logical_work
        || actual.clearance_storage_bytes > maximum.clearance_storage_bytes
    {
        return Err(CommonArticulationResourceProfileErrorV2::ResourceLimit);
    }
    Ok(())
}

fn resource_profile_binding_fingerprint_v2(
    configured_max_blocks: usize,
    actual_block_count: usize,
    maximum: CommonArticulationCanonicalMiuraResourcesV2,
    actual: CommonArticulationCanonicalMiuraResourcesV2,
) -> Result<[u8; 32], CommonArticulationResourceProfileErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_RESOURCE_PROFILE_MODEL_ID_V2.as_bytes());
    hash.update([0]);
    for value in [
        configured_max_blocks,
        actual_block_count,
        maximum.block_count,
        maximum.face_count,
        maximum.hinge_count,
        maximum.unordered_face_pair_count,
        maximum.raw_cross_block_pair_candidates,
        maximum.canonical_cross_block_pairs,
        maximum.raw_sort_comparisons_per_item,
        maximum.canonical_sort_comparisons_per_item,
        maximum.pose_logical_work,
        maximum.pose_retained_bytes,
        maximum.decomposition_logical_work,
        maximum.decomposition_storage_bytes,
        maximum.clearance_logical_work,
        maximum.clearance_storage_bytes,
        actual.block_count,
        actual.face_count,
        actual.hinge_count,
        actual.unordered_face_pair_count,
        actual.raw_cross_block_pair_candidates,
        actual.canonical_cross_block_pairs,
        actual.raw_sort_comparisons_per_item,
        actual.canonical_sort_comparisons_per_item,
        actual.pose_logical_work,
        actual.pose_retained_bytes,
        actual.decomposition_logical_work,
        actual.decomposition_storage_bytes,
        actual.clearance_logical_work,
        actual.clearance_storage_bytes,
    ] {
        // The canonical preimage is fixed-width u64 little-endian even on a
        // hypothetical platform whose usize is wider than u64.  Refuse a
        // lossy conversion instead of producing a platform-dependent hash.
        let value = u64::try_from(value)
            .map_err(|_| CommonArticulationResourceProfileErrorV2::ResourceLimit)?;
        hash.update(value.to_le_bytes());
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
#[path = "common_articulation_resource_profile/tests.rs"]
mod tests;
