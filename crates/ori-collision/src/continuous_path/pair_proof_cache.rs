//! Production pair-proof cache orchestration for continuous certificate model 4.
//!
//! The other seven continuous certificate models deliberately remain on their
//! established uncached paths. Exact pose preparation and witness encoding
//! happen before lookup; only cache misses run cold prism proofs, outside the
//! runtime mutex, before the atomic publication operation.

use std::collections::HashSet;

use ori_domain::{EdgeId, FaceId};
use ori_kinematics::{HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose};

use crate::cayley::{
    PositiveThicknessPrismPairDispositionV1, positive_thickness_exact_pair_cache_work_limits_v1,
    prepare_positive_thickness_exact_pair_cache_session_v1,
};
use crate::proof_cache::{
    CachedPairProofConclusionV1, PairProofCacheCandidateV1, PairProofDependenciesV1,
    PersistentPairProofCacheRuntimeV1, ProofCacheErrorV1, ProofCacheKeyV1,
    ProofCacheOperationControlV1, ProofCacheRuntimeCaptureV1, ProofCacheRuntimeErrorV1,
};

use super::{
    STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2,
    StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1,
    StackedFoldPathDiagnosticLimitsV1, collective_path_absolute_angles_v1,
    diagnose_collective_hinge_path_from_pose_with_optional_cache_v1,
};

pub(super) struct PositiveEndpointPairCacheUseV1<'a> {
    pub(super) runtime: &'a PersistentPairProofCacheRuntimeV1,
    pub(super) capture: &'a ProofCacheRuntimeCaptureV1,
    pub(super) issuer_context: [u8; 32],
    pub(super) control: ProofCacheOperationControlV1<'a>,
}

/// Production model-4 entrypoint.
///
/// `capture` must be obtained while the desktop owns project then pose
/// authority. The epoch and every binding component are rechecked atomically
/// at publication after the cache-lock-free cold proof.
#[allow(clippy::too_many_arguments)]
pub fn diagnose_collective_hinge_path_with_pair_cache_v1<'a>(
    model: &MaterialTreeKinematicsModel,
    initial_pose: &MaterialTreePose,
    moving_hinges: &[EdgeId],
    requested_angle_degrees: f64,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    runtime: &'a PersistentPairProofCacheRuntimeV1,
    capture: &'a ProofCacheRuntimeCaptureV1,
    control: ProofCacheOperationControlV1<'a>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    if limits.static_collision != crate::StaticCollisionLimits::default() {
        // The exact cache session has a fixed, independently audited work
        // envelope. Until every caller-supplied static limit can be translated
        // into and rebound to that envelope, a non-default limit must keep the
        // established uncached semantics. In particular, a warm proof may
        // never widen a caller's tighter budget.
        return super::diagnose_collective_hinge_path_v1(
            model,
            initial_pose,
            moving_hinges,
            requested_angle_degrees,
            paper_thickness_mm,
            limits,
        );
    }
    let (source_absolute, target_absolute) =
        collective_path_absolute_angles_v1(initial_pose, moving_hinges, requested_angle_degrees)?;
    let issuer_context = positive_two_hinge_cache_issuer_context_v2(
        initial_pose,
        source_absolute,
        target_absolute.as_slice(),
    );
    let cache = PositiveEndpointPairCacheUseV1 {
        runtime,
        capture,
        issuer_context,
        control,
    };
    diagnose_collective_hinge_path_from_pose_with_optional_cache_v1(
        model,
        initial_pose,
        source_absolute,
        target_absolute.as_slice(),
        paper_thickness_mm,
        limits,
        Some(&cache),
    )
}

fn positive_two_hinge_cache_issuer_context_v2(
    initial_pose: &MaterialTreePose,
    source_absolute: &[HingeAngle],
    target_absolute: &[HingeAngle],
) -> [u8; 32] {
    positive_two_hinge_cache_issuer_context_from_parts_v2(
        initial_pose.fixed_face(),
        source_absolute,
        target_absolute,
    )
}

fn positive_two_hinge_cache_issuer_context_from_parts_v2(
    fixed_face: Option<FaceId>,
    source_absolute: &[HingeAngle],
    target_absolute: &[HingeAngle],
) -> [u8; 32] {
    use sha2::Digest as _;

    let mut hash = sha2::Sha256::new();
    hash.update(b"ori-continuous-model4-pair-cache-issuer-v2");
    hash.update(
        (STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2.len() as u64)
            .to_be_bytes(),
    );
    hash.update(
        STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2.as_bytes(),
    );
    hash.update((ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.len() as u64).to_be_bytes());
    hash.update(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes());
    match fixed_face {
        Some(face) => {
            hash.update([1]);
            hash.update(face.canonical_bytes());
        }
        None => hash.update([0]),
    }
    for angles in [source_absolute, target_absolute] {
        hash.update((angles.len() as u64).to_be_bytes());
        for angle in angles {
            hash.update(angle.edge().canonical_bytes());
            hash.update(angle.angle_degrees().to_bits().to_be_bytes());
        }
    }
    hash.finalize().into()
}

pub(super) fn prove_positive_endpoint_pairs_with_cache_v1(
    bound: ori_kinematics::BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    exact_pairs: &[(FaceId, FaceId)],
    expected_pairs: usize,
    cache: &PositiveEndpointPairCacheUseV1<'_>,
) -> Result<bool, StackedFoldPathDiagnosticErrorV1> {
    prove_positive_endpoint_pairs_with_cache_inner_v1(
        bound,
        paper_thickness_mm,
        exact_pairs,
        expected_pairs,
        cache,
        |_| {},
    )
}

#[cfg(test)]
pub(super) fn prove_positive_endpoint_pairs_with_cache_after_cold_hook_v1(
    bound: ori_kinematics::BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    exact_pairs: &[(FaceId, FaceId)],
    expected_pairs: usize,
    cache: &PositiveEndpointPairCacheUseV1<'_>,
    after_cold_pair: impl FnMut(usize),
) -> Result<bool, StackedFoldPathDiagnosticErrorV1> {
    prove_positive_endpoint_pairs_with_cache_inner_v1(
        bound,
        paper_thickness_mm,
        exact_pairs,
        expected_pairs,
        cache,
        after_cold_pair,
    )
}

fn prove_positive_endpoint_pairs_with_cache_inner_v1(
    bound: ori_kinematics::BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    exact_pairs: &[(FaceId, FaceId)],
    expected_pairs: usize,
    cache: &PositiveEndpointPairCacheUseV1<'_>,
    mut after_cold_pair: impl FnMut(usize),
) -> Result<bool, StackedFoldPathDiagnosticErrorV1> {
    if cache.capture.paper_thickness_bits() != paper_thickness_mm.to_bits()
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
    {
        return Err(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable);
    }
    if exact_pairs.is_empty() {
        if cache.issuer_context == [0; 32] {
            return Err(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable);
        }
        cache
            .control
            .check_v1()
            .map_err(map_pair_cache_evidence_error_v1)?;
        // The endpoint/topology theorem already discharged every pair. No
        // cache key, witness, lookup result, or publication is consumed here,
        // so preparing a complete exact face snapshot would add no authority.
        return Ok(true);
    }

    let session = prepare_positive_thickness_exact_pair_cache_session_v1(bound, paper_thickness_mm)
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(exact_pairs.len())
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable)?;
    for (first, second) in exact_pairs {
        keys.push(
            ProofCacheKeyV1::new(
                cache
                    .capture
                    .key_input_v1([*first, *second], cache.issuer_context),
            )
            .map_err(map_pair_cache_evidence_error_v1)?,
        );
    }
    let work_limits = positive_thickness_exact_pair_cache_work_limits_v1(exact_pairs.len())
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable)?;
    let current_footprints = session
        .complete_face_snapshots_v1()
        .iter()
        .map(|snapshot| snapshot.footprint.clone())
        .collect::<Vec<_>>();
    let current_exact_poses = session
        .complete_face_snapshots_v1()
        .iter()
        .map(|snapshot| snapshot.exact_pose.clone())
        .collect::<Vec<_>>();
    let lookup = cache
        .runtime
        .lookup_two_hinge_positive_v1(
            cache.capture,
            cache.issuer_context,
            current_footprints,
            current_exact_poses,
            &keys,
            &work_limits,
            cache.control,
        )
        .map_err(map_pair_cache_runtime_error_v1)?;
    let mut hit_pairs = HashSet::new();
    hit_pairs
        .try_reserve(lookup.hits().len())
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable)?;
    for hit in lookup.hits() {
        if hit.result().conclusion() != CachedPairProofConclusionV1::NonBlocking
            || !hit_pairs.insert(hit.key().faces())
        {
            return Err(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable);
        }
    }
    if hit_pairs.len() + lookup.missing_entries() != keys.len() {
        return Err(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable);
    }

    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(lookup.missing_entries())
        .map_err(|_| StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable)?;
    let mut accounted_work = lookup.total_accounted_work().clone();
    let mut completed_cold_pairs = 0usize;
    for ((first, second), key) in exact_pairs.iter().zip(&keys) {
        if hit_pairs.contains(&key.faces()) {
            continue;
        }
        cache
            .control
            .check_v1()
            .map_err(map_pair_cache_evidence_error_v1)?;
        // The exact pair kernel has a fixed, independently bounded pair-level
        // granularity. Cooperative cancellation is checked on both sides.
        let observation = session
            .analyze_pair_v1(*first, *second)
            .map_err(|_| StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable)?;
        completed_cold_pairs = completed_cold_pairs
            .checked_add(1)
            .ok_or(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable)?;
        after_cold_pair(completed_cold_pairs);
        cache
            .control
            .check_v1()
            .map_err(map_pair_cache_evidence_error_v1)?;
        if observation.diagnostic.first_face != key.faces()[0]
            || observation.diagnostic.second_face != key.faces()[1]
            || observation.diagnostic.disposition
                != PositiveThicknessPrismPairDispositionV1::Separated
        {
            return Ok(false);
        }
        let [first_dependency, second_dependency] = observation.dependencies;
        accounted_work = accounted_work
            .checked_merge(&observation.work, &work_limits)
            .map_err(map_pair_cache_evidence_error_v1)?;
        let dependencies = PairProofDependenciesV1::new_v1(
            key,
            [first_dependency.footprint, second_dependency.footprint],
            [first_dependency.exact_pose, second_dependency.exact_pose],
            Vec::new(),
        )
        .map_err(map_pair_cache_evidence_error_v1)?;
        candidates.push(
            PairProofCacheCandidateV1::new_v1(
                key.clone(),
                CachedPairProofConclusionV1::NonBlocking,
                observation.work,
                dependencies,
            )
            .map_err(map_pair_cache_evidence_error_v1)?,
        );
    }
    if accounted_work
        .additive_counters()
        .iter()
        .zip(work_limits.additive_counters())
        .any(|(actual, maximum)| actual > maximum)
        || accounted_work
            .maximum_counters()
            .iter()
            .zip(work_limits.maximum_counters())
            .any(|(actual, maximum)| actual > maximum)
    {
        return Err(StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable);
    }
    cache
        .control
        .check_v1()
        .map_err(map_pair_cache_evidence_error_v1)?;
    cache
        .runtime
        .publish_two_hinge_positive_v1(
            cache.capture,
            cache.issuer_context,
            candidates,
            expected_pairs,
            expected_pairs,
            hit_pairs.len(),
            &accounted_work,
            cache.control,
        )
        .map_err(map_pair_cache_runtime_error_v1)?;
    Ok(true)
}

const fn map_pair_cache_evidence_error_v1(
    error: ProofCacheErrorV1,
) -> StackedFoldPathDiagnosticErrorV1 {
    match error {
        ProofCacheErrorV1::Cancelled => StackedFoldPathDiagnosticErrorV1::Cancelled,
        _ => StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable,
    }
}

const fn map_pair_cache_runtime_error_v1(
    error: ProofCacheRuntimeErrorV1,
) -> StackedFoldPathDiagnosticErrorV1 {
    match error {
        ProofCacheRuntimeErrorV1::StaleProof | ProofCacheRuntimeErrorV1::InvalidationPending => {
            StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult
        }
        ProofCacheRuntimeErrorV1::Cache(ProofCacheErrorV1::Cancelled) => {
            StackedFoldPathDiagnosticErrorV1::Cancelled
        }
        _ => StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::EdgeId;

    use super::*;

    fn fixed_id<T: serde::de::DeserializeOwned>(prefix: &str, index: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{index:012x}\"")).unwrap()
    }

    #[test]
    fn model4_v2_pair_cache_issuer_has_a_fixed_cross_runtime_golden() {
        let first: EdgeId = fixed_id("ea00", 1);
        let second: EdgeId = fixed_id("ea00", 2);
        let source = [
            HingeAngle::new(first, 0.0).unwrap(),
            HingeAngle::new(second, 10.0).unwrap(),
        ];
        let target = [
            HingeAngle::new(first, 30.0).unwrap(),
            HingeAngle::new(second, 45.0).unwrap(),
        ];

        assert_eq!(
            positive_two_hinge_cache_issuer_context_from_parts_v2(
                Some(fixed_id("fa00", 1)),
                &source,
                &target,
            ),
            [
                0xaa, 0x15, 0x35, 0x45, 0x70, 0xed, 0x40, 0xec, 0xd1, 0x1a, 0x6c, 0xf7, 0x86, 0xda,
                0x73, 0x5c, 0x65, 0x7f, 0xa7, 0xae, 0x29, 0xe2, 0xfb, 0x00, 0x5e, 0x32, 0x1e, 0xe6,
                0x2a, 0x87, 0x28, 0x8a,
            ],
            "the issuer binds its V2 model ID and deterministic transcendental model"
        );
    }
}
