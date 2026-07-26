use std::time::{Duration, Instant};

use ori_domain::{EdgeId, FaceId, ProjectId, VertexId};

use super::super::*;

pub(crate) const SOURCE_REVISION: u64 = 41;
pub(crate) const TARGET_REVISION: u64 = 42;
pub(crate) const SOURCE_POSE_GENERATION: u64 = 17;
pub(crate) const TARGET_POSE_GENERATION: u64 = 18;
pub(crate) const THICKNESS: f64 = 0.125;
pub(crate) const SOURCE_GEOMETRY: [u8; 32] = [0x31; 32];
pub(crate) const TARGET_GEOMETRY: [u8; 32] = [0x32; 32];
pub(crate) const ISSUER_CONTEXT: [u8; 32] = [0x51; 32];

#[derive(Clone)]
pub(crate) struct Fixture {
    pub(crate) key: ProofCacheKeyV1,
    pub(crate) candidate: PairProofCacheCandidateV1,
}

pub(crate) fn project(seed: u8) -> ProjectId {
    let mut bytes = [0xA5; 16];
    bytes[15] = seed;
    ProjectId::schema_namespace(bytes)
}

pub(crate) fn face(seed: u8) -> FaceId {
    FaceId::derive_v5(project(0x70), &[b'f', seed])
}

pub(crate) fn vertex(seed: u8) -> VertexId {
    VertexId::derive_v5(project(0x71), &[b'v', seed])
}

pub(crate) fn edge(seed: u8) -> EdgeId {
    EdgeId::derive_v5(project(0x72), &[b'e', seed])
}

pub(crate) fn base_key() -> ProofCacheKeyV1 {
    ProofCacheKeyV1::new(ProofCacheKeyInputV1 {
        project_instance_id: project(1),
        project_id: project(2),
        revision: SOURCE_REVISION,
        geometry_fingerprint: SOURCE_GEOMETRY,
        pose_generation: SOURCE_POSE_GENERATION,
        paper_thickness_mm: THICKNESS,
        faces: [face(1), face(2)],
        certificate_model: ProofCacheCertificateModelV1::SingleHingeZeroThickness,
        issuer_context: ISSUER_CONTEXT,
    })
    .expect("valid base proof-cache key")
}

pub(crate) fn memo_token(seed: u8, generation: u64) -> ProofMemoDependencyTokenV1 {
    ProofMemoDependencyTokenV1::new_v1(
        [seed; 32],
        generation,
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
    )
    .expect("valid memo token")
}

pub(crate) fn work(seed: usize) -> ProofCachePairWorkV1 {
    let mut additive = [0; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1];
    let mut maximum = [0; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1];
    additive[0] = 7 + seed;
    additive[7] = 11 + seed;
    maximum[0] = 5 + seed;
    maximum[4] = 13 + seed;
    ProofCachePairWorkV1::from_exact_pair_counters_v1(additive, maximum)
}

pub(crate) fn fixture_for(key: ProofCacheKeyV1, seed: u8) -> Fixture {
    let faces = key.faces();
    let footprints = faces.map(|face_id| {
        let prefix = face_id.canonical_bytes()[15].wrapping_add(seed);
        FaceDependencyFootprintV1::from_complete_face_v1(
            face_id,
            vec![
                vertex(prefix),
                vertex(prefix.wrapping_add(1)),
                vertex(prefix.wrapping_add(2)),
            ],
            vec![
                edge(prefix),
                edge(prefix.wrapping_add(1)),
                edge(prefix.wrapping_add(2)),
            ],
        )
        .expect("complete non-empty footprint")
    });
    let exact_poses = faces.map(|face_id| {
        let mut bytes = vec![seed];
        bytes.extend_from_slice(&face_id.canonical_bytes());
        ExactFacePoseCacheWitnessV1::from_test_canonical_exact_bytes_v1(face_id, bytes)
            .expect("bounded test-only exact pose")
    });
    let dependencies =
        PairProofDependenciesV1::new_v1(&key, footprints, exact_poses, vec![memo_token(0x61, 9)])
            .expect("key-bound dependencies");
    let pair_work = work(usize::from(seed));
    let candidate = PairProofCacheCandidateV1::new_v1(
        key.clone(),
        CachedPairProofConclusionV1::NonBlocking,
        pair_work,
        dependencies,
    )
    .expect("sealed cache candidate");
    Fixture { key, candidate }
}

pub(crate) fn base_fixture() -> Fixture {
    fixture_for(base_key(), 0)
}

pub(crate) fn cache_limits(
    max_entries: usize,
    max_storage_bytes: usize,
    max_invalidation_work: usize,
) -> ProofCacheLimitsV1 {
    ProofCacheLimitsV1 {
        max_entries,
        max_storage_bytes,
        max_invalidation_work,
    }
}

pub(crate) fn default_cache() -> PersistentPairProofCacheV1 {
    PersistentPairProofCacheV1::new(ProofCacheLimitsV1::default())
        .expect("valid default cache limits")
}

pub(crate) fn generous_work_limits() -> ProofCachePairWorkLimitsV1 {
    ProofCachePairWorkLimitsV1::new(
        [usize::MAX; PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
        [usize::MAX; PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
    )
}

pub(crate) fn operation_control() -> ProofCacheOperationControlV1<'static> {
    ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(30))
}

pub(crate) fn publish_fixture(
    cache: &mut PersistentPairProofCacheV1,
    fixture: &Fixture,
) -> ProofCachePublishReportV1 {
    cache
        .publish_batch_v1(vec![fixture.candidate.clone()], operation_control())
        .expect("fixture publication succeeds")
}

pub(crate) fn disjoint_impact() -> AppliedEditImpactSetV1 {
    AppliedEditImpactSetV1::from_complete_aggregate_v1(
        SOURCE_REVISION,
        TARGET_REVISION,
        vec![VertexId::derive_v5(project(0xE1), b"disjoint-vertex")],
        vec![EdgeId::derive_v5(project(0xE2), b"disjoint-edge")],
        vec![FaceId::derive_v5(project(0xE3), b"disjoint-face")],
        &operation_control(),
    )
    .expect("complete disjoint impact")
}

pub(crate) fn rebind_request(
    fixture: &Fixture,
    impact: AppliedEditImpactSetV1,
    healthy_memos: Vec<ProofMemoDependencyTokenV1>,
) -> ProofCacheRebindRequestV1 {
    let context = ProofCacheRebindContextV1::new(
        fixture.key.project_instance_id,
        fixture.key.project_id,
        TARGET_REVISION,
        TARGET_GEOMETRY,
        TARGET_POSE_GENERATION,
        f64::from_bits(fixture.key.paper_thickness_bits),
        fixture.key.issuer_context,
    )
    .expect("valid monotonic rebind context");
    ProofCacheRebindRequestV1::from_complete_revision_snapshot_v1(
        context,
        impact,
        fixture.candidate.dependencies.footprints.to_vec(),
        fixture.candidate.dependencies.exact_poses.to_vec(),
        healthy_memos,
    )
    .expect("complete current dependency snapshot")
}

pub(crate) fn rebound_key(source: &ProofCacheKeyV1) -> ProofCacheKeyV1 {
    let mut rebound = source.clone();
    rebound.revision = TARGET_REVISION;
    rebound.geometry_fingerprint = TARGET_GEOMETRY;
    rebound.pose_generation = TARGET_POSE_GENERATION;
    rebound
}
