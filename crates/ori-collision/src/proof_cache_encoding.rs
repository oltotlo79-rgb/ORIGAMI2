//! Canonical exact encoding, sealed bindings and logical byte accounting.

use num_rational::BigRational;
use ori_domain::{FaceId, VertexId};
use sha2::{Digest, Sha256};

use super::{
    CANONICAL_ID_BYTES_V1, FINGERPRINT_BYTES_V1, MAX_PROOF_CACHE_STORAGE_BYTES_V1,
    MODEL_TAG_BYTES_V1, PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1,
    PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1, ProofCacheErrorV1, ProofCacheKeyV1, ProofCachePairWorkV1,
    U64_BYTES_V1,
    evidence::{CachedPairProofConclusionV1, PairProofDependenciesV1},
};

const EXACT_FACE_POSE_ENCODING_DOMAIN_V1: &[u8] = b"ori-proof-cache-exact-face-pose-v1";
const PAIR_PROOF_BINDING_DOMAIN_V1: &[u8] = b"ori-proof-cache-pair-binding-v1";

pub(super) fn encode_exact_face_pose_v1(
    face: FaceId,
    rotation: &[[BigRational; 3]; 3],
    translation: &[BigRational; 3],
    boundary: &[(VertexId, [BigRational; 3])],
) -> Result<Vec<u8>, ProofCacheErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve(256)
        .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
    append_bounded_v1(&mut output, EXACT_FACE_POSE_ENCODING_DOMAIN_V1)?;
    append_bounded_v1(&mut output, &face.canonical_bytes())?;
    for row in rotation {
        for value in row {
            append_canonical_rational_v1(&mut output, value)?;
        }
    }
    for value in translation {
        append_canonical_rational_v1(&mut output, value)?;
    }
    append_count_v1(&mut output, boundary.len())?;
    for (vertex, point) in boundary {
        append_bounded_v1(&mut output, &vertex.canonical_bytes())?;
        for value in point {
            append_canonical_rational_v1(&mut output, value)?;
        }
    }
    Ok(output)
}

fn append_bounded_v1(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ProofCacheErrorV1> {
    let next_len = output
        .len()
        .checked_add(bytes.len())
        .filter(|len| *len <= MAX_PROOF_CACHE_STORAGE_BYTES_V1)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    output
        .try_reserve_exact(next_len - output.len())
        .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn append_count_v1(output: &mut Vec<u8>, count: usize) -> Result<(), ProofCacheErrorV1> {
    let count = u64::try_from(count).map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
    append_bounded_v1(output, &count.to_be_bytes())
}

fn append_canonical_rational_v1(
    output: &mut Vec<u8>,
    value: &BigRational,
) -> Result<(), ProofCacheErrorV1> {
    let canonical = BigRational::new(value.numer().clone(), value.denom().clone());
    let numerator = canonical.numer().to_signed_bytes_be();
    let denominator = canonical.denom().to_signed_bytes_be();
    append_count_v1(output, numerator.len())?;
    append_bounded_v1(output, &numerator)?;
    append_count_v1(output, denominator.len())?;
    append_bounded_v1(output, &denominator)
}

fn hash_count_v1(hasher: &mut Sha256, count: usize) -> Result<(), ProofCacheErrorV1> {
    let count = u64::try_from(count).map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?;
    hasher.update(count.to_be_bytes());
    Ok(())
}

pub(super) fn pair_proof_binding_v1(
    key: &ProofCacheKeyV1,
    conclusion: CachedPairProofConclusionV1,
    work: &ProofCachePairWorkV1,
    dependencies: &PairProofDependenciesV1,
) -> Result<[u8; 32], ProofCacheErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(PAIR_PROOF_BINDING_DOMAIN_V1);
    hasher.update(key.project_instance_id.canonical_bytes());
    hasher.update(key.project_id.canonical_bytes());
    hasher.update(key.revision.to_be_bytes());
    hasher.update(key.geometry_fingerprint);
    hasher.update(key.pose_generation.to_be_bytes());
    hasher.update(key.paper_thickness_bits.to_be_bytes());
    for face in key.faces {
        hasher.update(face.canonical_bytes());
    }
    hasher.update([key.certificate_model.tag()]);
    hasher.update(key.issuer_context);
    hasher.update([match conclusion {
        CachedPairProofConclusionV1::NonBlocking => 0,
        CachedPairProofConclusionV1::Blocking => 1,
    }]);
    for counter in work.additive {
        hash_count_v1(&mut hasher, counter)?;
    }
    for counter in work.maximum {
        hash_count_v1(&mut hasher, counter)?;
    }
    for footprint in &dependencies.footprints {
        hasher.update(footprint.face.canonical_bytes());
        hash_count_v1(&mut hasher, footprint.vertices.len())?;
        for vertex in &footprint.vertices {
            hasher.update(vertex.canonical_bytes());
        }
        hash_count_v1(&mut hasher, footprint.edges.len())?;
        for edge in &footprint.edges {
            hasher.update(edge.canonical_bytes());
        }
    }
    for pose in &dependencies.exact_poses {
        hasher.update(pose.face.canonical_bytes());
        hash_count_v1(&mut hasher, pose.canonical_exact_bytes.len())?;
        hasher.update(&pose.canonical_exact_bytes);
    }
    hash_count_v1(&mut hasher, dependencies.memo_dependencies.len())?;
    for token in &dependencies.memo_dependencies {
        hasher.update(token.issuer);
        hasher.update(token.generation.to_be_bytes());
        hasher.update(token.content_fingerprint);
        hasher.update(token.entry_fingerprint);
    }
    Ok(hasher.finalize().into())
}

pub(super) fn checked_collection_storage_bytes_v1(
    first_count: usize,
    second_count: usize,
    bytes_per_item: usize,
) -> Result<usize, ProofCacheErrorV1> {
    first_count
        .checked_add(second_count)
        .and_then(|count| count.checked_mul(bytes_per_item))
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)
}

pub(super) fn logical_entry_storage_bytes_v1(
    dependencies: &PairProofDependenciesV1,
) -> Result<usize, ProofCacheErrorV1> {
    let id_bytes = CANONICAL_ID_BYTES_V1
        .checked_mul(4)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let fingerprint_bytes = FINGERPRINT_BYTES_V1
        .checked_mul(2)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let scalar_bytes = U64_BYTES_V1
        .checked_mul(3)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let key_bytes = id_bytes
        .checked_add(fingerprint_bytes)
        .and_then(|bytes| bytes.checked_add(scalar_bytes))
        .and_then(|bytes| bytes.checked_add(MODEL_TAG_BYTES_V1))
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let work_counter_count = PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1
        .checked_add(PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let work_bytes = work_counter_count
        .checked_mul(U64_BYTES_V1)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let result_bytes = MODEL_TAG_BYTES_V1
        .checked_add(FINGERPRINT_BYTES_V1)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let mut bytes = key_bytes
        .checked_add(result_bytes)
        .and_then(|value| value.checked_add(work_bytes))
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    for footprint in &dependencies.footprints {
        let counts_bytes = U64_BYTES_V1
            .checked_mul(2)
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        let header_bytes = CANONICAL_ID_BYTES_V1
            .checked_add(counts_bytes)
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        let item_bytes = checked_collection_storage_bytes_v1(
            footprint.vertices.len(),
            footprint.edges.len(),
            CANONICAL_ID_BYTES_V1,
        )?;
        bytes = bytes
            .checked_add(header_bytes)
            .and_then(|value| value.checked_add(item_bytes))
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    }
    for pose in &dependencies.exact_poses {
        let header_bytes = CANONICAL_ID_BYTES_V1
            .checked_add(U64_BYTES_V1)
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
        bytes = bytes
            .checked_add(header_bytes)
            .and_then(|value| value.checked_add(pose.canonical_exact_bytes.len()))
            .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    }
    let memo_token_bytes = FINGERPRINT_BYTES_V1
        .checked_mul(3)
        .and_then(|value| value.checked_add(U64_BYTES_V1))
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    let memo_bytes = dependencies
        .memo_dependencies
        .len()
        .checked_mul(memo_token_bytes)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    bytes = bytes
        .checked_add(U64_BYTES_V1)
        .and_then(|value| value.checked_add(memo_bytes))
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?;
    (bytes <= MAX_PROOF_CACHE_STORAGE_BYTES_V1)
        .then_some(bytes)
        .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)
}
