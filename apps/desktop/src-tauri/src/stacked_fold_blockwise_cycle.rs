//! Fail-closed blockwise current-cycle fallback certification.
//!
//! Command orchestration remains in the parent module. This module owns the
//! deterministic two-block decomposition, continuous-layer transport, and
//! private transaction-premise installation path.

use std::sync::atomic::Ordering;

use ori_collision::{
    GeneralCellTransportInputV1, GeneralCellTransportLimitsV1,
    certify_canonical_positive_thickness_cycle_schedule_path_v1,
    certify_general_multi_face_cell_transport_v1,
};
use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalEdgeBlockLimitsV1, CycleBasisLimitsV1, DyadicIntervalClosureLimitsV1,
    GeneratedMultiHingePathCandidateV1,
};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use super::{
    CANCELLED_MESSAGE, CYCLE_NONCLOSING_MESSAGE, CYCLE_PATH_RESOURCE_MESSAGE,
    CYCLE_PATH_UNCERTIFIED_MESSAGE, CYCLE_PATH_UNSUPPORTED_MESSAGE, CurrentAppliedPoseCapability,
    CurrentCyclePosePreviewResponseV1, CurrentLayerOrderCapability, LayerOrderPairDtoV1,
    STACKED_FOLD_READ_GENERATION, emit_current_cycle_progress_v1,
    production_cycle_schedule_limits_v1,
};

pub(super) fn certified_blockwise_layer_pairs_v1(
    sources: &[Box<ori_foldability::LayerOrderSnapshot>],
    certified_pair_count: usize,
) -> Result<Vec<(FaceId, FaceId)>, String> {
    let mut pairs = sources
        .iter()
        .flat_map(|source| source.face_pair_orders.iter())
        .map(|pair| (pair.lower_face.face_id, pair.upper_face.face_id))
        .collect::<Vec<_>>();
    pairs.sort_unstable_by_key(|(lower, upper)| (lower.canonical_bytes(), upper.canonical_bytes()));
    pairs.dedup();
    if pairs.len() != certified_pair_count {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    }
    Ok(pairs)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_blockwise_current_cycle_fallback_v1(
    app: Option<&AppHandle>,
    transaction_state: &super::stacked_fold_transaction::StackedFoldTransactionState,
    project: &super::ProjectState,
    pose_capability: CurrentAppliedPoseCapability,
    layer_capability: Option<CurrentLayerOrderCapability>,
    generated: &GeneratedMultiHingePathCandidateV1,
    requested: &ori_kinematics::CanonicalHingeAngles,
    thickness: f64,
    generation: u64,
    progress_request_id: Option<&str>,
    source_revision: u64,
) -> Result<CurrentCyclePosePreviewResponseV1, String> {
    let layer_capability = layer_capability.ok_or_else(|| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
    if !thickness.is_finite() || thickness <= 0.0 {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    }
    let (geometry, audit, pose) = pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(audit, CanonicalEdgeBlockLimitsV1::default())
        .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?;
    let [first, second] = decomposition.blocks() else {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    };
    let [articulation] = decomposition.articulation_faces() else {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    };
    if *articulation != pose.fixed_face() {
        return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
    }
    let prepare = |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1| {
        let schedule = generated
            .schedule()
            .restrict_to_edge_block_v1(geometry, audit, block.geometry(), block.audit())
            .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
        let closure = block
            .geometry()
            .prove_simultaneous_cycle_basis_schedule_closure_v1(
                block.audit(),
                *articulation,
                &schedule,
                ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                CycleBasisLimitsV1::default(),
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 16,
                    max_leaves: 65_536,
                    max_work: 1_048_576,
                    schedule_limits: production_cycle_schedule_limits_v1(),
                },
            )
            .map_err(|_| CYCLE_NONCLOSING_MESSAGE.to_owned())?
            .closure()
            .clone();
        let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
            block.geometry(),
            block.audit(),
            *articulation,
            &schedule,
            &closure,
            thickness,
            32,
        )
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
        Ok::<_, String>((schedule, closure, positive))
    };
    let (first_schedule, first_closure, first_positive) = prepare(first)?;
    let (second_schedule, second_closure, second_positive) = prepare(second)?;
    let block_faces = [first, second].map(|block| {
        block
            .geometry()
            .face_ids()
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
    });
    let source_snapshot = layer_capability.snapshot();
    let mut cell_owners = std::collections::HashMap::new();
    for cell in &source_snapshot.overlap_cells {
        if cell.covering_faces.is_empty() || cell.bottom_to_top_faces.is_empty() {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        }
        let owners = block_faces
            .iter()
            .enumerate()
            .filter_map(|(index, faces)| {
                (cell
                    .covering_faces
                    .iter()
                    .all(|face| faces.contains(&face.face_id))
                    && cell
                        .bottom_to_top_faces
                        .iter()
                        .all(|face| faces.contains(face)))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        };
        if cell_owners.insert(cell.cell_key, *owner).is_some() {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        }
    }
    for pair in &source_snapshot.face_pair_orders {
        let owners = block_faces
            .iter()
            .enumerate()
            .filter_map(|(index, faces)| {
                (faces.contains(&pair.lower_face.face_id)
                    && faces.contains(&pair.upper_face.face_id)
                    && !pair.supporting_cells.is_empty()
                    && pair
                        .supporting_cells
                        .iter()
                        .all(|cell| cell_owners.get(cell) == Some(&index)))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches!(owners.as_slice(), [_]) {
            return Err(CYCLE_NONCLOSING_MESSAGE.to_owned());
        }
    }
    let restrict_snapshot = |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1| {
        let faces = block.geometry().face_ids();
        let mut snapshot = layer_capability.snapshot().clone();
        snapshot
            .material_faces
            .retain(|face| faces.contains(&face.face_id));
        snapshot
            .folded_faces
            .retain(|face| faces.contains(&face.face.face_id));
        snapshot
            .global_bottom_to_top
            .as_mut()
            .iter_mut()
            .for_each(|order| {
                order.retain(|face| faces.contains(&face.face_id));
            });
        snapshot.reference_face = snapshot
            .reference_face
            .filter(|face| faces.contains(&face.face_id));
        snapshot.overlap_cells.retain(|cell| {
            !cell.covering_faces.is_empty()
                && cell
                    .covering_faces
                    .iter()
                    .all(|face| faces.contains(&face.face_id))
                && cell
                    .bottom_to_top_faces
                    .iter()
                    .all(|face| faces.contains(face))
        });
        let cells = snapshot
            .overlap_cells
            .iter()
            .map(|cell| cell.cell_key)
            .collect::<std::collections::HashSet<_>>();
        snapshot.face_pair_orders.retain_mut(|pair| {
            pair.supporting_cells.retain(|cell| cells.contains(cell));
            faces.contains(&pair.lower_face.face_id)
                && faces.contains(&pair.upper_face.face_id)
                && !pair.supporting_cells.is_empty()
        });
        snapshot.proof_summary = None;
        Box::new(snapshot)
    };
    let sources = [restrict_snapshot(first), restrict_snapshot(second)];
    let transport =
        |block: &ori_kinematics::CanonicalMaterialEdgeBlockV1,
         source: &ori_foldability::LayerOrderSnapshot,
         schedule: &ori_kinematics::CanonicalCycleScheduleV1,
         closure: &ori_kinematics::DyadicMaterialHingeIntervalClosureCertificateV1,
         positive: &ori_collision::PositiveThicknessContinuousCertificateV1| {
            let transitions = closure
                .leaves()
                .len()
                .checked_add(1)
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            let layer_records = source
                .overlap_cells
                .iter()
                .try_fold(0usize, |sum, cell| {
                    sum.checked_add(cell.bottom_to_top_faces.len())
                })
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            let boundary_samples = source
                .overlap_cells
                .iter()
                .try_fold(0usize, |sum, cell| {
                    cell.exact_boundary
                        .len()
                        .checked_mul(cell.bottom_to_top_faces.len())
                        .and_then(|work| sum.checked_add(work))
                })
                .and_then(|work| work.checked_mul(transitions))
                .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
            certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
                geometry: block.geometry(),
                audit: block.audit(),
                source,
                schedule,
                closure,
                positive_continuous: positive,
                paper_thickness_mm: thickness,
                tolerance: ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                limits: GeneralCellTransportLimitsV1 {
                    max_transitions: transitions,
                    max_cells: source.overlap_cells.len(),
                    max_layer_records: layer_records,
                    max_boundary_samples: boundary_samples,
                },
            })
            .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
        };
    let first_layer = transport(
        first,
        &sources[0],
        &first_schedule,
        &first_closure,
        &first_positive,
    )?;
    let second_layer = transport(
        second,
        &sources[1],
        &second_schedule,
        &second_closure,
        &second_positive,
    )?;
    let issuer_context = ori_foldability::fold_model_fingerprint_v1(
        project.editor.pattern(),
        project.editor.paper(),
    )
    .0;
    let parent = ori_collision::issue_blockwise_closure_authority_v1(
        [
            ori_collision::BlockwiseClosureInputV1 {
                geometry: first.geometry(),
                audit: first.audit(),
                schedule: &first_schedule,
                closure: &first_closure,
            },
            ori_collision::BlockwiseClosureInputV1 {
                geometry: second.geometry(),
                audit: second.audit(),
                schedule: &second_schedule,
                closure: &second_closure,
            },
        ],
        *articulation,
        thickness,
        issuer_context,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"blockwise-current-cycle-articulation-layer-v1");
    fingerprint.update(articulation.canonical_bytes());
    fingerprint.update(issuer_context);
    let articulation_layer_fingerprint: [u8; 32] = fingerprint.finalize().into();
    let authority = ori_collision::issue_blockwise_positive_layer_authority_v1(
        parent,
        [
            ori_collision::BlockwisePositiveLayerInputV1 {
                source: &sources[0],
                positive: first_positive,
                layer: first_layer,
            },
            ori_collision::BlockwisePositiveLayerInputV1 {
                source: &sources[1],
                positive: second_positive,
                layer: second_layer,
            },
        ],
        *articulation,
        thickness,
        issuer_context,
        articulation_layer_fingerprint,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let transition_count = authority.transition_count_v1();
    let pair_count = authority.pair_order_count_v1();
    let target_hash = authority
        .target_order_hash_v1()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let layer_pairs = certified_blockwise_layer_pairs_v1(&sources, pair_count)?;
    let response_pairs = layer_pairs
        .iter()
        .map(|(lower_face, upper_face)| LayerOrderPairDtoV1 {
            lower_face: *lower_face,
            upper_face: *upper_face,
        })
        .collect::<Vec<_>>();
    if STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) != generation {
        return Err(CANCELLED_MESSAGE.to_owned());
    }
    emit_current_cycle_progress_v1(app, progress_request_id, 1, 1);
    let closure_leaf_count = first_closure.leaves().len() + second_closure.leaves().len();
    let closure_max_depth = first_closure
        .leaves()
        .iter()
        .chain(second_closure.leaves())
        .map(|(depth, _, _)| *depth)
        .max()
        .unwrap_or(0);
    let checked_hinge_count = first.geometry().hinges().len() + second.geometry().hinges().len();
    let total_hinge_count = geometry.hinges().len();
    let token = super::stacked_fold_transaction::install_pending_blockwise_current_cycle_pose_v1(
        transaction_state,
        super::stacked_fold_transaction::PendingBlockwiseCurrentCyclePremisesV1 {
            expected_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            expected_source_fingerprint: issuer_context,
            expected_pose_generation: pose_capability.generation(),
            expected_layer_generation: layer_capability.generation(),
            geometry: geometry.clone(),
            fixed_face: *articulation,
            authority,
            sources,
            articulation: *articulation,
            thickness,
            issuer_context,
            articulation_layer_fingerprint,
            layer_order_pairs: layer_pairs,
            target_angles: requested
                .as_slice()
                .iter()
                .map(|angle| (angle.edge(), angle.angle_degrees()))
                .collect(),
        },
        pose_capability,
        layer_capability,
    )?;
    Ok(CurrentCyclePosePreviewResponseV1 {
        version: 1,
        transaction_token: token,
        source_revision,
        target_revision: source_revision + 1,
        closure_leaf_count,
        closure_max_depth,
        checked_hinge_count,
        total_hinge_count,
        continuous_path_certified: true,
        continuous_layer_transport_model_id: Some(
            ori_collision::BLOCKWISE_POSITIVE_LAYER_MODEL_ID_V1,
        ),
        continuous_layer_transition_count: transition_count,
        continuous_layer_pair_order_count: pair_count,
        continuous_layer_target_order_sha256: Some(target_hash),
        target_layer_order: response_pairs.clone(),
        source_layer_order: response_pairs,
        authorizes_project_mutation: false,
    })
}
