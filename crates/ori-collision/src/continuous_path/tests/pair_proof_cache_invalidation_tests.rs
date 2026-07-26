use std::collections::HashMap;

use ori_domain::{FaceId, ProjectId};

use super::*;

#[test]
fn stable_identity_fifteen_face_fixture_reproves_fourteen_of_one_hundred_five_pairs() {
    use crate::proof_cache::{
        ExactFacePoseCacheWitnessV1, FaceDependencyFootprintV1, PairProofCacheCandidateV1,
        PairProofDependenciesV1,
    };

    let source_model = fourteen_hinge_triangle_model_with_leaf_offset(0.0);
    let target_model = fourteen_hinge_triangle_model_with_leaf_offset(0.125);
    assert_eq!(source_model.face_ids(), target_model.face_ids());
    assert_eq!(source_model.face_ids().len(), 15);
    let expected_pairs = 15 * 14 / 2;
    assert_eq!(expected_pairs, 105);
    let leaf_vertex: ori_domain::VertexId = fixed_id("8b10", 2);
    let leaf_faces = source_model
        .face_ids()
        .iter()
        .copied()
        .filter(|face| {
            source_model
                .face_boundary(*face)
                .expect("source boundary")
                .vertices()
                .contains(&leaf_vertex)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        leaf_faces.len(),
        1,
        "the legal stable-ID fixture edits one unique leaf"
    );
    let leaf_face = leaf_faces[0];
    let (_, source_pose) = zero_tree_pose(&source_model);
    let (_, target_pose) = zero_tree_pose(&target_model);
    assert_ne!(
        source_pose
            .vertex_position(leaf_vertex)
            .expect("source leaf")
            .x()
            .to_bits(),
        target_pose
            .vertex_position(leaf_vertex)
            .expect("target leaf")
            .x()
            .to_bits()
    );

    let snapshots = |model: &MaterialTreeKinematicsModel, target: bool| {
        let mut footprints = HashMap::new();
        let mut exact_poses = HashMap::new();
        for face in model.face_ids() {
            let boundary = model.face_boundary(*face).expect("complete boundary");
            footprints.insert(
                *face,
                FaceDependencyFootprintV1::from_complete_face_v1(
                    *face,
                    boundary.vertices().to_vec(),
                    boundary.edges().to_vec(),
                )
                .expect("legal face footprint"),
            );
            let mut bytes = face.canonical_bytes().to_vec();
            bytes.push(u8::from(target && *face == leaf_face));
            exact_poses.insert(
                *face,
                ExactFacePoseCacheWitnessV1::from_test_canonical_exact_bytes_v1(*face, bytes)
                    .expect("compact exact test witness"),
            );
        }
        (footprints, exact_poses)
    };
    let (source_footprints, source_exact_poses) = snapshots(&source_model, false);
    let (target_footprints, target_exact_poses) = snapshots(&target_model, true);
    let runtime =
        crate::PersistentPairProofCacheRuntimeV1::new(crate::ProofCacheLimitsV1::default())
            .expect("runtime");
    let project_instance_id = ProjectId::new();
    let project_id = ProjectId::new();
    let source_capture = runtime
        .capture_v1(
            crate::ProofCacheRuntimeBindingV1::new(
                project_instance_id,
                project_id,
                1,
                [0x81; 32],
                1,
                0.1,
            )
            .expect("source binding"),
        )
        .expect("source capture");
    let issuer_context = [0x82; 32];
    let mut pair_additive = [0; crate::PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1];
    pair_additive[0] = 1;
    let pair_work = crate::ProofCachePairWorkV1::from_exact_pair_counters_v1(
        pair_additive,
        [0; crate::PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
    );
    let candidate_for =
        |key: crate::ProofCacheKeyV1,
         footprints: &HashMap<FaceId, FaceDependencyFootprintV1>,
         exact_poses: &HashMap<FaceId, ExactFacePoseCacheWitnessV1>| {
            let faces = key.faces();
            let dependencies = PairProofDependenciesV1::new_v1(
                &key,
                [footprints[&faces[0]].clone(), footprints[&faces[1]].clone()],
                [
                    exact_poses[&faces[0]].clone(),
                    exact_poses[&faces[1]].clone(),
                ],
                Vec::new(),
            )
            .expect("pair dependencies");
            PairProofCacheCandidateV1::new_v1(
                key,
                crate::CachedPairProofConclusionV1::NonBlocking,
                pair_work.clone(),
                dependencies,
            )
            .expect("sealed pair candidate")
        };

    let faces = source_model.face_ids();
    let mut source_candidates = Vec::with_capacity(expected_pairs);
    for (index, first) in faces.iter().enumerate() {
        for second in faces.iter().skip(index + 1) {
            let key = crate::ProofCacheKeyV1::new(
                source_capture.key_input_v1([*first, *second], issuer_context),
            )
            .expect("source key");
            source_candidates.push(candidate_for(key, &source_footprints, &source_exact_poses));
        }
    }
    let mut total_additive = [0; crate::PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1];
    total_additive[0] = expected_pairs;
    let total_work = crate::ProofCachePairWorkV1::from_exact_pair_counters_v1(
        total_additive,
        [0; crate::PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
    );
    let source_report = runtime
        .publish_two_hinge_positive_v1(
            &source_capture,
            issuer_context,
            source_candidates,
            expected_pairs,
            expected_pairs,
            0,
            &total_work,
            crate::ProofCacheOperationControlV1::new(
                None,
                std::time::Instant::now() + std::time::Duration::from_secs(30),
            ),
        )
        .expect("source publication");
    assert_eq!(source_report.admitted_entries, expected_pairs);

    runtime
        .begin_complete_edit_v1(
            1,
            2,
            vec![leaf_vertex],
            Vec::new(),
            Vec::new(),
            crate::ProofCacheOperationControlV1::new(
                None,
                std::time::Instant::now() + std::time::Duration::from_secs(30),
            ),
        )
        .expect("complete leaf edit");
    runtime
        .advance_pose_authority_v1(2)
        .expect("target pose transition");
    let target_capture = runtime
        .capture_v1(
            crate::ProofCacheRuntimeBindingV1::new(
                project_instance_id,
                project_id,
                2,
                [0x83; 32],
                2,
                0.1,
            )
            .expect("target binding"),
        )
        .expect("target capture");
    let mut target_keys = Vec::with_capacity(expected_pairs);
    let mut cold_candidates = Vec::new();
    for (index, first) in faces.iter().enumerate() {
        for second in faces.iter().skip(index + 1) {
            let key = crate::ProofCacheKeyV1::new(
                target_capture.key_input_v1([*first, *second], issuer_context),
            )
            .expect("target key");
            if *first == leaf_face || *second == leaf_face {
                cold_candidates.push(candidate_for(
                    key.clone(),
                    &target_footprints,
                    &target_exact_poses,
                ));
            }
            target_keys.push(key);
        }
    }
    let work_limits = crate::ProofCachePairWorkLimitsV1::new(
        [usize::MAX; crate::PROOF_CACHE_ADDITIVE_WORK_COUNTERS_V1],
        [usize::MAX; crate::PROOF_CACHE_MAXIMUM_WORK_COUNTERS_V1],
    );
    let lookup = runtime
        .lookup_two_hinge_positive_v1(
            &target_capture,
            issuer_context,
            target_footprints.values().cloned().collect(),
            target_exact_poses.values().cloned().collect(),
            &target_keys,
            &work_limits,
            crate::ProofCacheOperationControlV1::new(
                None,
                std::time::Instant::now() + std::time::Duration::from_secs(30),
            ),
        )
        .expect("bounded differential lookup");
    let actual_cold_reproofs = lookup.missing_entries();
    assert_eq!(lookup.hits().len(), 91);
    assert_eq!(actual_cold_reproofs, 14);
    assert_eq!(cold_candidates.len(), actual_cold_reproofs);
    assert!(
        actual_cold_reproofs * 100 < expected_pairs * 20,
        "actual reproof ratio must stay below 20%: {actual_cold_reproofs}/{expected_pairs}"
    );
    runtime
        .publish_two_hinge_positive_v1(
            &target_capture,
            issuer_context,
            cold_candidates,
            expected_pairs,
            expected_pairs,
            lookup.hits().len(),
            &total_work,
            crate::ProofCacheOperationControlV1::new(
                None,
                std::time::Instant::now() + std::time::Duration::from_secs(30),
            ),
        )
        .expect("publish actual cold reproof set");
    let progress = runtime.progress_v1().expect("progress");
    assert_eq!(progress.cold_proofs, 14);
    assert_eq!(progress.cache_hits, 91);
    assert_eq!(progress.persistent_cached_pairs, 105);
}
