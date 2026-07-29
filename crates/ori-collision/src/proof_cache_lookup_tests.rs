use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use num_bigint::BigInt;
use num_rational::BigRational;
use ori_domain::FaceId;

use super::{super::*, support::*};

fn key_with_thickness(thickness: f64) -> ProofCacheKeyV1 {
    let source = base_key();
    ProofCacheKeyV1::new(ProofCacheKeyInputV1 {
        project_instance_id: source.project_instance_id,
        project_id: source.project_id,
        revision: source.revision,
        geometry_fingerprint: source.geometry_fingerprint,
        pose_generation: source.pose_generation,
        paper_thickness_mm: thickness,
        faces: source.faces,
        certificate_model: source.certificate_model,
        issuer_context: source.issuer_context,
    })
    .expect("valid thickness variant")
}

#[test]
fn proof_cache_hit_requires_all_key_components() {
    let fixture = base_fixture();
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    assert!(
        cache
            .lookup_v1(&fixture.key, &generous_work_limits())
            .expect("lookup")
            .is_some()
    );

    let mut variants = Vec::new();
    let mut key = fixture.key.clone();
    key.project_instance_id = project(3);
    variants.push(key);
    let mut key = fixture.key.clone();
    key.project_id = project(4);
    variants.push(key);
    let mut key = fixture.key.clone();
    key.revision += 1;
    variants.push(key);
    let mut key = fixture.key.clone();
    key.geometry_fingerprint[0] ^= 1;
    variants.push(key);
    let mut key = fixture.key.clone();
    key.pose_generation += 1;
    variants.push(key);
    let mut key = fixture.key.clone();
    key.paper_thickness_bits = f64::from_bits(THICKNESS.to_bits() + 1).to_bits();
    variants.push(key);
    let mut key = fixture.key.clone();
    key.faces[1] = face(3);
    key.faces.sort_unstable_by_key(FaceId::canonical_bytes);
    variants.push(key);
    let mut key = fixture.key.clone();
    key.certificate_model = ProofCacheCertificateModelV1::TreeIntervalZeroThickness;
    variants.push(key);
    let mut key = fixture.key.clone();
    key.issuer_context[0] ^= 1;
    variants.push(key);

    assert_eq!(variants.len(), 9);
    for variant in variants {
        assert!(
            cache
                .lookup_v1(&variant, &generous_work_limits())
                .expect("bounded lookup")
                .is_none()
        );
    }
}

#[test]
fn proof_cache_miss_on_pose_generation_change_only() {
    let fixture = base_fixture();
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let mut changed = fixture.key.clone();
    changed.pose_generation += 1;
    assert!(
        cache
            .lookup_v1(&changed, &generous_work_limits())
            .expect("lookup")
            .is_none()
    );
}

#[test]
fn proof_cache_miss_on_thickness_one_ulp_drift() {
    let fixture = fixture_for(key_with_thickness(THICKNESS), 0);
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let drifted = key_with_thickness(f64::from_bits(THICKNESS.to_bits() + 1));
    assert_eq!(
        drifted.paper_thickness_bits(),
        fixture.key.paper_thickness_bits() + 1
    );
    assert!(
        cache
            .lookup_v1(&drifted, &generous_work_limits())
            .expect("lookup")
            .is_none()
    );
}

#[test]
fn proof_cache_rejects_signed_zero_thickness_conflation() {
    let positive = key_with_thickness(0.0);
    let negative = key_with_thickness(-0.0);
    assert_ne!(
        positive.paper_thickness_bits(),
        negative.paper_thickness_bits()
    );
    let fixture = fixture_for(positive, 0);
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    assert!(
        cache
            .lookup_v1(&negative, &generous_work_limits())
            .expect("lookup")
            .is_none()
    );
}

#[test]
fn proof_cache_aba_same_angle_different_generation_is_miss() {
    let fixture = base_fixture();
    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let mut aba = fixture.key.clone();
    aba.pose_generation = aba
        .pose_generation
        .checked_add(2)
        .expect("small generation");
    assert_eq!(aba.geometry_fingerprint, fixture.key.geometry_fingerprint);
    assert!(
        cache
            .lookup_v1(&aba, &generous_work_limits())
            .expect("lookup")
            .is_none()
    );
}

#[test]
fn proof_cache_capacity_exhaustion_reverts_entries_to_unproven_explicitly() {
    let first = base_fixture();
    let mut second_key = base_key();
    second_key.revision += 1;
    second_key.pose_generation += 1;
    second_key.geometry_fingerprint[0] ^= 1;
    let second = fixture_for(second_key, 1);
    let mut cache = PersistentPairProofCacheV1::new(cache_limits(
        1,
        MAX_PROOF_CACHE_STORAGE_BYTES_V1,
        MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
    ))
    .expect("bounded cache");
    let report = cache
        .publish_batch_v1(
            vec![second.candidate.clone(), first.candidate.clone()],
            operation_control(),
        )
        .expect("capacity is reported, not hidden");
    assert_eq!(report.admitted_entries, 1);
    assert_eq!(report.unproven_due_to_capacity, 1);
    assert_eq!(report.total_entries, 1);
    assert_eq!(cache.entry_count(), 1);

    let admitted_key = [&first.key, &second.key]
        .into_iter()
        .min()
        .expect("two keys");
    let rejected_key = [&first.key, &second.key]
        .into_iter()
        .max()
        .expect("two keys");
    assert!(
        cache
            .lookup_v1(admitted_key, &generous_work_limits())
            .expect("lookup")
            .is_some()
    );
    assert!(
        cache
            .lookup_v1(rejected_key, &generous_work_limits())
            .expect("lookup")
            .is_none()
    );
}

#[test]
fn proof_cache_conflicting_batch_rolls_back_atomically() {
    let existing = base_fixture();
    let mut earlier_key = existing.key.clone();
    earlier_key.project_instance_id = project(0);
    let earlier = fixture_for(earlier_key, 4);
    assert!(earlier.key < existing.key);
    let conflicting = PairProofCacheCandidateV1::new_v1(
        existing.key.clone(),
        CachedPairProofConclusionV1::Blocking,
        existing.candidate.work.clone(),
        existing.candidate.dependencies.clone(),
    )
    .expect("valid contradictory sealed observation");
    let mut cache = default_cache();
    publish_fixture(&mut cache, &existing);

    assert_eq!(
        cache.publish_batch_v1(
            vec![conflicting, earlier.candidate.clone()],
            operation_control(),
        ),
        Err(ProofCacheErrorV1::ConflictingEvidence)
    );
    assert_eq!(cache.entry_count(), 1);
    assert!(
        cache
            .lookup_v1(&existing.key, &generous_work_limits())
            .expect("existing lookup")
            .is_some()
    );
    assert!(
        cache
            .lookup_v1(&earlier.key, &generous_work_limits())
            .expect("planned lookup")
            .is_none()
    );
}

#[test]
fn replacement_publication_panic_never_exposes_a_partial_batch() {
    let existing = base_fixture();
    let mut first_key = existing.key.clone();
    first_key.revision += 1;
    first_key.pose_generation += 1;
    first_key.geometry_fingerprint[0] ^= 1;
    let first = fixture_for(first_key, 5);
    let mut second_key = existing.key.clone();
    second_key.revision += 2;
    second_key.pose_generation += 2;
    second_key.geometry_fingerprint[0] ^= 2;
    let second = fixture_for(second_key, 6);
    let mut cache = default_cache();
    publish_fixture(&mut cache, &existing);
    let storage_before = cache.logical_storage_bytes();

    arm_publish_replacement_panic_for_test_v1(1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = cache.publish_batch_v1(
                vec![first.candidate.clone(), second.candidate.clone()],
                operation_control(),
            );
        }))
        .is_err(),
        "the deterministic fault must interrupt replacement construction"
    );

    assert_eq!(cache.entry_count(), 1);
    assert_eq!(cache.logical_storage_bytes(), storage_before);
    assert!(
        cache
            .lookup_v1(&existing.key, &generous_work_limits())
            .expect("existing lookup after unwind")
            .is_some()
    );
    for key in [&first.key, &second.key] {
        assert!(
            cache
                .lookup_v1(key, &generous_work_limits())
                .expect("planned lookup after unwind")
                .is_none()
        );
    }
}

#[test]
fn proof_cache_result_identical_to_cold_run_bit_exact() {
    let fixture = base_fixture();
    let cold_result = fixture.candidate.result.clone();
    let cold_work = fixture.candidate.work.clone();
    let faces = fixture.key.faces();
    let reversed = ProofCacheKeyV1::new(ProofCacheKeyInputV1 {
        project_instance_id: fixture.key.project_instance_id,
        project_id: fixture.key.project_id,
        revision: fixture.key.revision,
        geometry_fingerprint: fixture.key.geometry_fingerprint,
        pose_generation: fixture.key.pose_generation,
        paper_thickness_mm: f64::from_bits(fixture.key.paper_thickness_bits),
        faces: [faces[1], faces[0]],
        certificate_model: fixture.key.certificate_model,
        issuer_context: fixture.key.issuer_context,
    })
    .expect("reverse pair canonicalizes");
    assert_eq!(reversed, fixture.key);

    let mut cache = default_cache();
    publish_fixture(&mut cache, &fixture);
    let hit = cache
        .lookup_v1(&reversed, &generous_work_limits())
        .expect("lookup")
        .expect("exact key hit");
    assert_eq!(hit.result(), &cold_result);
    assert_eq!(hit.accounted_work(), &cold_work);
    assert_eq!(hit.result().binding(), cold_result.binding());
    assert_eq!(hit.result().conclusion(), cold_result.conclusion());

    let batch = cache
        .lookup_canonical_batch_v1(
            &[reversed, fixture.key.clone()],
            &generous_work_limits(),
            operation_control(),
        )
        .expect("canonical bounded batch");
    assert_eq!(batch.hits().len(), 1);
    assert_eq!(batch.missing_entries(), 0);
    assert_eq!(batch.total_accounted_work(), &cold_work);
}

#[test]
fn proof_cache_storage_and_work_one_short_fail_closed() {
    let fixture = base_fixture();
    let exact_storage = fixture.candidate.logical_storage_bytes;
    let mut one_short = PersistentPairProofCacheV1::new(cache_limits(
        1,
        exact_storage - 1,
        MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
    ))
    .expect("valid one-short storage limit");
    let rejected = one_short
        .publish_batch_v1(vec![fixture.candidate.clone()], operation_control())
        .expect("capacity outcome");
    assert_eq!(rejected.admitted_entries, 0);
    assert_eq!(rejected.unproven_due_to_capacity, 1);

    let mut exact = PersistentPairProofCacheV1::new(cache_limits(
        1,
        exact_storage,
        MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
    ))
    .expect("valid exact storage limit");
    let admitted = exact
        .publish_batch_v1(vec![fixture.candidate.clone()], operation_control())
        .expect("exact storage boundary");
    assert_eq!(admitted.admitted_entries, 1);

    let exact_limits = ProofCachePairWorkLimitsV1::new(
        *fixture.candidate.work.additive_counters(),
        *fixture.candidate.work.maximum_counters(),
    );
    assert!(
        exact
            .lookup_v1(&fixture.key, &exact_limits)
            .expect("exact work boundary")
            .is_some()
    );
    let mut additive = *fixture.candidate.work.additive_counters();
    additive[0] -= 1;
    let one_short_work =
        ProofCachePairWorkLimitsV1::new(additive, *fixture.candidate.work.maximum_counters());
    assert_eq!(
        exact.lookup_v1(&fixture.key, &one_short_work),
        Err(ProofCacheErrorV1::ResourceLimitExceeded)
    );
}

#[test]
fn proof_cache_checked_storage_arithmetic_rejects_overflow() {
    assert_eq!(
        checked_collection_storage_bytes_v1(usize::MAX, 1, CANONICAL_ID_BYTES_V1),
        Err(ProofCacheErrorV1::ResourceLimitExceeded)
    );
    assert_eq!(
        checked_collection_storage_bytes_v1(usize::MAX / 104 + 1, 0, 104),
        Err(ProofCacheErrorV1::ResourceLimitExceeded)
    );
    assert!(matches!(
        preflight_canonical_operation_v1(
            MAX_PROOF_CACHE_ENTRIES_V1 + 1,
            MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
        ),
        Err(ProofCacheErrorV1::ResourceLimitExceeded)
    ));
}

#[test]
fn proof_cache_operations_are_cancelled_or_deadlined_before_mutation() {
    let fixture = base_fixture();
    let cancellation = AtomicBool::new(true);
    assert_eq!(
        AppliedEditImpactSetV1::from_complete_aggregate_v1(
            SOURCE_REVISION,
            TARGET_REVISION,
            vec![vertex(0xF1)],
            Vec::new(),
            Vec::new(),
            &ProofCacheOperationControlV1::new(
                Some(&cancellation),
                Instant::now() + Duration::from_secs(30),
            ),
        ),
        Err(ProofCacheErrorV1::Cancelled)
    );
    let mut cache = default_cache();
    assert_eq!(
        cache.publish_batch_v1(
            vec![fixture.candidate.clone()],
            ProofCacheOperationControlV1::new(
                Some(&cancellation),
                Instant::now() + Duration::from_secs(30),
            ),
        ),
        Err(ProofCacheErrorV1::Cancelled)
    );
    assert_eq!(cache.entry_count(), 0);
    assert_eq!(
        cache.publish_batch_v1(
            vec![fixture.candidate.clone()],
            ProofCacheOperationControlV1::new(None, Instant::now()),
        ),
        Err(ProofCacheErrorV1::DeadlineExceeded)
    );
    assert_eq!(cache.entry_count(), 0);
}

fn rational(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new(BigInt::from(numerator), BigInt::from(denominator))
}

fn raw_rational(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new_raw(BigInt::from(numerator), BigInt::from(denominator))
}

#[test]
fn proof_cache_exact_pose_encoding_canonicalizes_rational_payload() {
    let identity = [
        [rational(1, 1), rational(0, 1), rational(0, 1)],
        [rational(0, 1), rational(1, 1), rational(0, 1)],
        [rational(0, 1), rational(0, 1), rational(1, 1)],
    ];
    let scaled_identity = [
        [raw_rational(2, 2), raw_rational(0, 2), raw_rational(0, 3)],
        [raw_rational(0, 4), raw_rational(3, 3), raw_rational(0, 5)],
        [raw_rational(0, 6), raw_rational(0, 7), raw_rational(4, 4)],
    ];
    let translation = [rational(1, 2), rational(-1, 3), rational(0, 1)];
    let scaled_translation = [raw_rational(2, 4), raw_rational(-2, 6), raw_rational(0, 9)];
    let boundary = vec![
        (vertex(1), [rational(0, 1), rational(0, 1), rational(0, 1)]),
        (vertex(2), [rational(1, 1), rational(0, 1), rational(0, 1)]),
        (vertex(3), [rational(0, 1), rational(1, 1), rational(0, 1)]),
    ];
    let scaled_boundary = vec![
        (
            vertex(1),
            [raw_rational(0, 2), raw_rational(0, 3), raw_rational(0, 4)],
        ),
        (
            vertex(2),
            [raw_rational(2, 2), raw_rational(0, 5), raw_rational(0, 6)],
        ),
        (
            vertex(3),
            [raw_rational(0, 7), raw_rational(3, 3), raw_rational(0, 8)],
        ),
    ];
    let canonical =
        ExactFacePoseCacheWitnessV1::from_exact_components_v1(ExactFacePoseComponentsV1 {
            face: face(1),
            rotation: &identity,
            translation: &translation,
            boundary: &boundary,
        })
        .expect("canonical exact face pose");
    let structurally_scaled =
        ExactFacePoseCacheWitnessV1::from_exact_components_v1(ExactFacePoseComponentsV1 {
            face: face(1),
            rotation: &scaled_identity,
            translation: &scaled_translation,
            boundary: &scaled_boundary,
        })
        .expect("equivalent exact face pose");
    assert_eq!(canonical, structurally_scaled);
}
