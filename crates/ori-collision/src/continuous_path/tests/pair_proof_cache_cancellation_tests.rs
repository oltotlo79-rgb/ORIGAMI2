use ori_domain::ProjectId;

use super::super::pair_proof_cache;
use super::*;

#[test]
fn model4_partial_cold_cancellation_never_publishes_or_advances_progress() {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let model = branched_triangle_model(6, false);
    let (moving, initial) = zero_tree_pose(&model);
    let requested = positive_tree_max_angle_degrees_v1(model.hinges().len()).unwrap();
    let moving_set = moving.iter().copied().collect::<HashSet<_>>();
    let endpoint =
        solve_collective_pose(&model, &initial, &moving_set, requested).expect("endpoint");
    let exact_pair = model
        .face_ids()
        .iter()
        .enumerate()
        .find_map(|(index, first)| {
            model.face_ids().iter().skip(index + 1).find_map(|second| {
                let adjacent = model.hinges().iter().any(|hinge| {
                    (hinge.left_face() == *first && hinge.right_face() == *second)
                        || (hinge.left_face() == *second && hinge.right_face() == *first)
                });
                (!adjacent && !faces_share_material_vertex_v1(&model, *first, *second))
                    .then_some((*first, *second))
            })
        })
        .expect("fixture has a non-adjacent pair without a shared material vertex");
    let exact_pairs = [exact_pair];
    let bound = model.bind_pose(&endpoint).expect("bound model-4 endpoint");
    let expected_pairs = model.face_ids().len() * (model.face_ids().len() - 1) / 2;
    let runtime =
        crate::PersistentPairProofCacheRuntimeV1::new(crate::ProofCacheLimitsV1::default())
            .expect("runtime");
    let capture = runtime
        .capture_v1(
            crate::ProofCacheRuntimeBindingV1::new(
                ProjectId::new(),
                ProjectId::new(),
                1,
                [0x71; 32],
                1,
                1.0,
            )
            .expect("binding"),
        )
        .expect("capture");
    let cancelled = AtomicBool::new(false);
    let cache = pair_proof_cache::PositiveEndpointPairCacheUseV1 {
        runtime: &runtime,
        capture: &capture,
        issuer_context: [0x72; 32],
        control: crate::ProofCacheOperationControlV1::new(
            Some(&cancelled),
            Instant::now() + Duration::from_secs(30),
        ),
    };
    let completed_cold_pairs = Cell::new(0);
    let result = pair_proof_cache::prove_positive_endpoint_pairs_with_cache_after_cold_hook_v1(
        bound,
        1.0,
        &exact_pairs,
        expected_pairs,
        &cache,
        |completed| {
            assert_eq!(completed, 1);
            completed_cold_pairs.set(completed);
            cancelled.store(true, Ordering::Release);
        },
    );
    assert_eq!(
        completed_cold_pairs.get(),
        1,
        "cancellation must occur after a real cold pair kernel"
    );
    assert_eq!(result, Err(StackedFoldPathDiagnosticErrorV1::Cancelled));
    let progress = runtime.progress_v1().expect("progress");
    assert_eq!(progress.proven_pairs, 0);
    assert_eq!(progress.persistent_cached_pairs, 0);
    assert_eq!(progress.cold_proofs, 0);
}
