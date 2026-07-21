use std::io::Cursor;

use ori_domain::{
    AssetId, BeginnerRecognitionProposalV1, ProjectId, UnderlayId, analyze_marker_png_rgba_v1,
    analyze_silhouette_png_rgba_v1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    AppState, ProjectSnapshot, ProjectState, ensure_expected_project, execute_command, lock_project,
};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecognizeBeginnerTargetRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    underlay_id: UnderlayId,
    asset_id: AssetId,
}

#[tauri::command]
pub(crate) fn recognize_beginner_silhouette(
    state: State<'_, AppState>,
    request: RecognizeBeginnerTargetRequest,
) -> Result<BeginnerRecognitionProposalV1, String> {
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?
    };
    let source_sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let (width, height, rgba) = decode_general_image(&bytes)?;
    let proposal = analyze_silhouette_png_rgba_v1(
        request.underlay_id,
        request.asset_id,
        source_sha256,
        width,
        height,
        &rgba,
    )
    .map_err(|error| match error {
        ori_domain::BeginnerRecognitionErrorV1::AmbiguousSilhouette => {
            "recognition_ambiguous_silhouette".to_owned()
        }
        ori_domain::BeginnerRecognitionErrorV1::UnsupportedSilhouette => {
            "recognition_unsupported_silhouette".to_owned()
        }
        ori_domain::BeginnerRecognitionErrorV1::InvalidDimensions
        | ori_domain::BeginnerRecognitionErrorV1::PixelLimit
        | ori_domain::BeginnerRecognitionErrorV1::InvalidRgbaLength
        | ori_domain::BeginnerRecognitionErrorV1::ComponentLimit
        | ori_domain::BeginnerRecognitionErrorV1::PartLimit
        | ori_domain::BeginnerRecognitionErrorV1::SkeletonLimit => {
            "recognition_resource_limit".to_owned()
        }
        ori_domain::BeginnerRecognitionErrorV1::EmptyShape
        | ori_domain::BeginnerRecognitionErrorV1::UnsupportedMarker => {
            "recognition_unsupported_silhouette".to_owned()
        }
    })?;
    {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        let live_bytes = project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.as_slice())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?;
        let live_hash: [u8; 32] = Sha256::digest(live_bytes).into();
        if live_hash != source_sha256 {
            return Err("recognition_asset_changed".to_owned());
        }
    }
    Ok(proposal)
}

#[derive(Debug, Serialize)]
pub(crate) struct BeginnerOutlineCandidatesResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    underlay_id: UnderlayId,
    asset_id: AssetId,
    source_sha256: [u8; 32],
    candidates: Vec<ori_domain::BeginnerOutlineCandidateV1>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BeginnerPartSuggestionV1 {
    candidate_id: u8,
    suggested_kind: ori_domain::BeginnerTargetPartKindV1,
    confidence_reason: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct BeginnerPartSuggestionsResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    underlay_id: UnderlayId,
    asset_id: AssetId,
    selected_outline_id: u8,
    suggestions: Vec<BeginnerPartSuggestionV1>,
}

#[tauri::command]
pub(crate) fn recognize_beginner_outline_candidates(
    state: State<'_, AppState>,
    request: RecognizeBeginnerTargetRequest,
) -> Result<BeginnerOutlineCandidatesResponse, String> {
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?
    };
    let source_hash: [u8; 32] = Sha256::digest(&bytes).into();
    let (width, height, rgba) = decode_general_image(&bytes)?;
    let candidates = ori_domain::analyze_outline_candidates_rgba_v1(width, height, &rgba)
        .map_err(|_| "recognition_resource_limit".to_owned())?;
    let project = lock_project(&state)?;
    ensure_recognition_binding(&project, request)?;
    let live = project
        .texture_assets
        .iter()
        .find(|asset| asset.id == request.asset_id)
        .ok_or_else(|| "recognition_asset_unavailable".to_owned())?;
    if <[u8; 32]>::from(Sha256::digest(&live.bytes)) != source_hash {
        return Err("recognition_asset_changed".to_owned());
    }
    Ok(BeginnerOutlineCandidatesResponse {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        underlay_id: request.underlay_id,
        asset_id: request.asset_id,
        source_sha256: source_hash,
        candidates,
    })
}

#[tauri::command]
pub(crate) fn recognize_beginner_part_suggestions(
    state: State<'_, AppState>,
    request: ApplyBeginnerOutlineCandidateRequest,
) -> Result<BeginnerPartSuggestionsResponse, String> {
    let binding = RecognizeBeginnerTargetRequest {
        expected_project_instance_id: request.expected_project_instance_id,
        expected_project_id: request.expected_project_id,
        expected_revision: request.expected_revision,
        underlay_id: request.underlay_id,
        asset_id: request.asset_id,
    };
    let (bytes, target_category, target_parts) = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, binding)?;
        let bytes = project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?;
        (
            bytes,
            project
                .editor
                .beginner_design_profile()
                .generation_constraints
                .target_category,
            project
                .editor
                .beginner_design_profile()
                .generation_constraints
                .target_parts
                .clone(),
        )
    };
    let (width, height, rgba) = decode_general_image(&bytes)?;
    let candidates = ori_domain::analyze_outline_candidates_rgba_v1(width, height, &rgba)
        .map_err(|_| "recognition_resource_limit".to_owned())?;
    if candidates.get(usize::from(request.candidate.id)) != Some(&request.candidate) {
        return Err("outline_candidate_stale".to_owned());
    }
    let others = candidates
        .iter()
        .filter(|candidate| candidate.id != request.candidate.id)
        .take(7)
        .collect::<Vec<_>>();
    if others.is_empty() {
        return Err("part_suggestion_ambiguous".to_owned());
    }
    let mut suggestions = vec![BeginnerPartSuggestionV1 {
        candidate_id: request.candidate.id,
        suggested_kind: ori_domain::BeginnerTargetPartKindV1::Torso,
        confidence_reason: "selected_primary_outline",
    }];
    let axis_twice =
        i64::from(request.candidate.bounds.min_x) + i64::from(request.candidate.bounds.max_x);
    let mut bilateral = std::collections::HashSet::new();
    for (index, left) in others.iter().enumerate() {
        for right in others.iter().skip(index + 1) {
            let left_center_twice = i64::from(left.bounds.min_x) + i64::from(left.bounds.max_x);
            let right_center_twice = i64::from(right.bounds.min_x) + i64::from(right.bounds.max_x);
            let same_height = left.bounds.min_y.abs_diff(right.bounds.min_y) <= 1
                && left.bounds.max_y.abs_diff(right.bounds.max_y) <= 1;
            let area_close = left.area_pixels.abs_diff(right.area_pixels)
                <= left.area_pixels.max(right.area_pixels) / 10 + 1;
            if (left_center_twice + right_center_twice - axis_twice * 2).abs() <= 2
                && same_height
                && area_close
            {
                bilateral.insert(left.id);
                bilateral.insert(right.id);
            }
        }
    }
    let requested_bilateral_kind = if target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Wing && part.count == 2)
    {
        ori_domain::BeginnerTargetPartKindV1::Wing
    } else if target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Fin && part.count == 2)
    {
        ori_domain::BeginnerTargetPartKindV1::Fin
    } else if target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Ear && part.count == 2)
    {
        ori_domain::BeginnerTargetPartKindV1::Ear
    } else if target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Horn && part.count == 2)
    {
        ori_domain::BeginnerTargetPartKindV1::Horn
    } else if target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Antenna && part.count == 2)
    {
        ori_domain::BeginnerTargetPartKindV1::Antenna
    } else if target_parts
        .iter()
        .any(|part| part.kind == ori_domain::BeginnerTargetPartKindV1::Leg && part.count == 2)
    {
        ori_domain::BeginnerTargetPartKindV1::Leg
    } else if target_category == Some(ori_domain::BeginnerTargetCategoryV1::Insect) {
        ori_domain::BeginnerTargetPartKindV1::Wing
    } else {
        ori_domain::BeginnerTargetPartKindV1::Leg
    };
    for (index, candidate) in others.into_iter().enumerate() {
        suggestions.push(BeginnerPartSuggestionV1 {
            candidate_id: candidate.id,
            suggested_kind: if bilateral.contains(&candidate.id) {
                requested_bilateral_kind
            } else if index == 0
                && candidate.area_pixels.saturating_mul(4) >= request.candidate.area_pixels
            {
                ori_domain::BeginnerTargetPartKindV1::Head
            } else {
                if target_category == Some(ori_domain::BeginnerTargetCategoryV1::Insect) {
                    ori_domain::BeginnerTargetPartKindV1::Wing
                } else {
                    ori_domain::BeginnerTargetPartKindV1::Leg
                }
            },
            confidence_reason: if bilateral.contains(&candidate.id) {
                "bilateral_secondary_pair"
            } else if index == 0 {
                "largest_secondary_outline"
            } else {
                "small_secondary_outline"
            },
        });
    }
    let project = lock_project(&state)?;
    ensure_recognition_binding(&project, binding)?;
    Ok(BeginnerPartSuggestionsResponse {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        underlay_id: request.underlay_id,
        asset_id: request.asset_id,
        selected_outline_id: request.candidate.id,
        suggestions,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BeginnerPartAssignmentV1 {
    candidate_id: u8,
    kind: ori_domain::BeginnerTargetPartKindV1,
    #[serde(default)]
    source_candidate_ids: Vec<u8>,
    #[serde(default)]
    split_fragment: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyBeginnerPartAssignmentsRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    underlay_id: UnderlayId,
    asset_id: AssetId,
    source_sha256: [u8; 32],
    selected_outline: ori_domain::BeginnerOutlineCandidateV1,
    assignments: Vec<BeginnerPartAssignmentV1>,
    confirmed: bool,
}

fn candidate_pair_is_symmetric(
    axis_twice: i64,
    left: &ori_domain::BeginnerRecognitionBoundsV1,
    right: &ori_domain::BeginnerRecognitionBoundsV1,
) -> bool {
    let left_x = i64::from(left.min_x) + i64::from(left.max_x);
    let right_x = i64::from(right.min_x) + i64::from(right.max_x);
    let left_y = i64::from(left.min_y) + i64::from(left.max_y);
    let right_y = i64::from(right.min_y) + i64::from(right.max_y);
    left_x + right_x == axis_twice * 2
        && left_y == right_y
        && left.max_x - left.min_x == right.max_x - right.min_x
        && left.max_y - left.min_y == right.max_y - right.min_y
}

#[tauri::command]
pub(crate) fn apply_beginner_part_assignments(
    state: State<'_, AppState>,
    request: ApplyBeginnerPartAssignmentsRequest,
) -> Result<ProjectSnapshot, String> {
    if !request.confirmed || request.assignments.is_empty() || request.assignments.len() > 10 {
        return Err("part_assignment_confirmation_required".to_owned());
    }
    let binding = RecognizeBeginnerTargetRequest {
        expected_project_instance_id: request.expected_project_instance_id,
        expected_project_id: request.expected_project_id,
        expected_revision: request.expected_revision,
        underlay_id: request.underlay_id,
        asset_id: request.asset_id,
    };
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, binding)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?
    };
    let source_hash: [u8; 32] = Sha256::digest(&bytes).into();
    if request.source_sha256 != source_hash {
        return Err("part_assignment_edit_digest_tampered".to_owned());
    }
    let (width, height, rgba) = decode_general_image(&bytes)?;
    let candidates = ori_domain::analyze_outline_candidates_rgba_v1(width, height, &rgba)
        .map_err(|_| "recognition_resource_limit".to_owned())?;
    if candidates.get(usize::from(request.selected_outline.id)) != Some(&request.selected_outline) {
        return Err("part_assignment_stale".to_owned());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut edited_sources = std::collections::BTreeSet::new();
    if request.assignments.iter().any(|assignment| {
        let sources_valid = match assignment.source_candidate_ids.as_slice() {
            [] => assignment.split_fragment.is_none(),
            [source] => {
                *source == assignment.candidate_id
                    && assignment
                        .split_fragment
                        .is_some_and(|fragment| fragment < 2)
            }
            [first, second] => {
                first < second
                    && *first == assignment.candidate_id
                    && assignment.split_fragment.is_none()
            }
            _ => false,
        };
        let source_ids = if assignment.source_candidate_ids.is_empty() {
            vec![assignment.candidate_id]
        } else {
            assignment.source_candidate_ids.clone()
        };
        let key = (assignment.candidate_id, assignment.split_fragment);
        !sources_valid
            || !seen.insert(key)
            || source_ids
                .iter()
                .any(|source| !candidates.iter().any(|candidate| candidate.id == *source))
            || (!assignment.source_candidate_ids.is_empty()
                && !edited_sources.insert((source_ids, assignment.split_fragment)))
    }) || !request
        .assignments
        .iter()
        .any(|assignment| assignment.kind == ori_domain::BeginnerTargetPartKindV1::Torso)
    {
        return Err("part_assignment_invalid".to_owned());
    }
    for source in request.assignments.iter().filter_map(|assignment| {
        (assignment.source_candidate_ids.len() == 1).then_some(assignment.candidate_id)
    }) {
        let fragments = request
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.candidate_id == source
                    && assignment.source_candidate_ids.as_slice() == [source]
            })
            .collect::<Vec<_>>();
        if fragments.len() != 2 || fragments[0].kind == fragments[1].kind {
            return Err("part_assignment_split_semantics_unconfirmed".to_owned());
        }
    }
    let leg_candidate_ids = request
        .assignments
        .iter()
        .filter(|assignment| assignment.kind == ori_domain::BeginnerTargetPartKindV1::Leg)
        .map(|assignment| assignment.candidate_id)
        .collect::<Vec<_>>();
    let wing_candidate_ids = request
        .assignments
        .iter()
        .filter(|assignment| assignment.kind == ori_domain::BeginnerTargetPartKindV1::Wing)
        .map(|assignment| assignment.candidate_id)
        .collect::<Vec<_>>();
    let tail_candidate_ids = request
        .assignments
        .iter()
        .filter(|assignment| assignment.kind == ori_domain::BeginnerTargetPartKindV1::Tail)
        .map(|assignment| assignment.candidate_id)
        .collect::<Vec<_>>();
    let horn_candidate_ids = request
        .assignments
        .iter()
        .filter(|assignment| assignment.kind == ori_domain::BeginnerTargetPartKindV1::Horn)
        .map(|assignment| assignment.candidate_id)
        .collect::<Vec<_>>();
    let antenna_candidate_ids = request
        .assignments
        .iter()
        .filter(|assignment| assignment.kind == ori_domain::BeginnerTargetPartKindV1::Antenna)
        .map(|assignment| assignment.candidate_id)
        .collect::<Vec<_>>();
    let ear_candidate_ids = request
        .assignments
        .iter()
        .filter(|assignment| assignment.kind == ori_domain::BeginnerTargetPartKindV1::Ear)
        .map(|assignment| assignment.candidate_id)
        .collect::<Vec<_>>();
    let mut counts = [0_u8; 9];
    for assignment in &request.assignments {
        let index = match assignment.kind {
            ori_domain::BeginnerTargetPartKindV1::Torso => 0,
            ori_domain::BeginnerTargetPartKindV1::Head => 1,
            ori_domain::BeginnerTargetPartKindV1::Leg => 2,
            ori_domain::BeginnerTargetPartKindV1::Wing => 3,
            ori_domain::BeginnerTargetPartKindV1::Tail => 4,
            ori_domain::BeginnerTargetPartKindV1::Horn => 5,
            ori_domain::BeginnerTargetPartKindV1::Antenna => 6,
            ori_domain::BeginnerTargetPartKindV1::Ear => 7,
            ori_domain::BeginnerTargetPartKindV1::Fin => 8,
        };
        counts[index] += 1;
    }
    let target_parts = [
        ori_domain::BeginnerTargetPartKindV1::Torso,
        ori_domain::BeginnerTargetPartKindV1::Head,
        ori_domain::BeginnerTargetPartKindV1::Leg,
        ori_domain::BeginnerTargetPartKindV1::Wing,
        ori_domain::BeginnerTargetPartKindV1::Tail,
        ori_domain::BeginnerTargetPartKindV1::Horn,
        ori_domain::BeginnerTargetPartKindV1::Antenna,
        ori_domain::BeginnerTargetPartKindV1::Ear,
        ori_domain::BeginnerTargetPartKindV1::Fin,
    ]
    .into_iter()
    .zip(counts)
    .filter(|(_, count)| *count > 0)
    .flat_map(|(kind, count)| {
        // Nonstandard repeated semantics remain individually addressable.
        // This lets exact image candidate IDs map one-to-one to generic
        // topology features instead of inventing bilateral authority.
        let records = if matches!(count, 3 | 5 | 7 | 8) {
            usize::from(count)
        } else {
            1
        };
        (0..records).map(move |_| ori_domain::BeginnerTargetPartRecordV1 {
            kind,
            count: if records == 1 { count } else { 1 },
        })
    })
    .collect();
    let mut project = lock_project(&state)?;
    ensure_recognition_binding(&project, binding)?;
    let live = project
        .texture_assets
        .iter()
        .find(|asset| asset.id == request.asset_id)
        .ok_or_else(|| "recognition_asset_unavailable".to_owned())?;
    if <[u8; 32]>::from(Sha256::digest(&live.bytes)) != source_hash {
        return Err("part_assignment_stale".to_owned());
    }
    let mut profile = project.editor.beginner_design_profile().clone();
    profile.generation_constraints.target_parts = target_parts;
    if wing_candidate_ids.len() == 2
        && antenna_candidate_ids.len() == 2
        && profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Insect)
    {
        let axis_twice = i64::from(request.selected_outline.bounds.min_x)
            + i64::from(request.selected_outline.bounds.max_x);
        let derive_pair = |ids: &[u8],
                           id: u16,
                           vertical: bool|
         -> Result<ori_domain::BeginnerProtrusionTargetV1, String> {
            let mut pair = ids
                .iter()
                .filter_map(|candidate_id| {
                    candidates
                        .iter()
                        .find(|candidate| candidate.id == *candidate_id)
                })
                .collect::<Vec<_>>();
            pair.sort_by_key(|candidate| candidate.bounds.min_x);
            if pair.len() != 2 {
                return Err("part_assignment_wing_antenna_binding_invalid".to_owned());
            }
            if !candidate_pair_is_symmetric(axis_twice, &pair[0].bounds, &pair[1].bounds) {
                return Err("part_assignment_wing_antenna_binding_invalid".to_owned());
            }
            let left_x = i64::from(pair[0].bounds.min_x) + i64::from(pair[0].bounds.max_x);
            let right_x = i64::from(pair[1].bounds.min_x) + i64::from(pair[1].bounds.max_x);
            let left_y = i64::from(pair[0].bounds.min_y) + i64::from(pair[0].bounds.max_y);
            let right_y = i64::from(pair[1].bounds.min_y) + i64::from(pair[1].bounds.max_y);
            Ok(ori_domain::BeginnerProtrusionTargetV1 {
                id,
                count: 2,
                length_tenths_mm: u32::try_from(
                    (right_x - left_x).unsigned_abs().saturating_mul(5).max(1),
                )
                .map_err(|_| "part_assignment_wing_antenna_binding_invalid")?,
                thickness_tenths_mm: 10,
                root_width_tenths_mm: None,
                tip_width_tenths_mm: None,
                local_outline_tenths_mm: None,
                position_tenths_mm: [
                    i32::try_from(axis_twice.saturating_mul(5))
                        .map_err(|_| "part_assignment_wing_antenna_binding_invalid")?,
                    i32::try_from((left_y + right_y).saturating_mul(5) / 2)
                        .map_err(|_| "part_assignment_wing_antenna_binding_invalid")?,
                    0,
                ],
                direction_milli: if vertical {
                    [0, -1000, 0]
                } else {
                    [1000, 0, 0]
                },
                symmetry: ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                curvature_degrees: 0,
                joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
                motion_degrees: [0, 0],
                side: ori_domain::BeginnerProtrusionSideV1::Either,
                priority: 50,
            })
        };
        profile.generation_constraints.protrusions = vec![
            derive_pair(&wing_candidate_ids, 1, false)?,
            derive_pair(&antenna_candidate_ids, 2, true)?,
        ];
        if ori_domain::insect_wing_antenna_bindings_v1(&profile.generation_constraints).is_none() {
            return Err("part_assignment_wing_antenna_binding_invalid".to_owned());
        }
    }
    let vertical_candidate_ids = if horn_candidate_ids.len() == 1
        && profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
    {
        Some(&horn_candidate_ids)
    } else if antenna_candidate_ids.len() == 1
        && profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Insect)
    {
        Some(&antenna_candidate_ids)
    } else {
        None
    };
    if let Some(vertical_candidate_ids) = vertical_candidate_ids {
        let horn = candidates
            .iter()
            .find(|candidate| candidate.id == vertical_candidate_ids[0])
            .ok_or_else(|| "part_assignment_horn_binding_invalid".to_owned())?;
        let torso_center_y_twice = i64::from(request.selected_outline.bounds.min_y)
            + i64::from(request.selected_outline.bounds.max_y);
        let horn_center_y_twice = i64::from(horn.bounds.min_y) + i64::from(horn.bounds.max_y);
        if horn_center_y_twice == torso_center_y_twice {
            return Err("part_assignment_horn_binding_invalid".to_owned());
        }
        let torso_height_tenths = request
            .selected_outline
            .bounds
            .max_y
            .saturating_sub(request.selected_outline.bounds.min_y)
            .saturating_add(1)
            .saturating_mul(10);
        let inferred_length = u32::try_from(
            (horn_center_y_twice - torso_center_y_twice)
                .unsigned_abs()
                .saturating_mul(5)
                .max(1),
        )
        .map_err(|_| "part_assignment_horn_binding_invalid")?;
        let minimum_length = torso_height_tenths
            .saturating_mul(2)
            .checked_div(100)
            .unwrap_or(1)
            .max(1);
        let maximum_length = torso_height_tenths
            .saturating_mul(45)
            .checked_div(100)
            .unwrap_or(1)
            .max(minimum_length);
        let axis_twice = i64::from(request.selected_outline.bounds.min_x)
            + i64::from(request.selected_outline.bounds.max_x);
        profile.generation_constraints.protrusions = vec![ori_domain::BeginnerProtrusionTargetV1 {
            id: 1,
            count: 1,
            length_tenths_mm: inferred_length.clamp(minimum_length, maximum_length),
            thickness_tenths_mm: u16::try_from(
                (horn.bounds.max_x - horn.bounds.min_x + 1)
                    .saturating_mul(10)
                    .min(10_000),
            )
            .map_err(|_| "part_assignment_horn_binding_invalid")?,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: None,
            position_tenths_mm: [
                i32::try_from(axis_twice.saturating_mul(5))
                    .map_err(|_| "part_assignment_horn_binding_invalid")?,
                i32::try_from(torso_center_y_twice.saturating_mul(5))
                    .map_err(|_| "part_assignment_horn_binding_invalid")?,
                0,
            ],
            direction_milli: [
                0,
                if horn_center_y_twice < torso_center_y_twice {
                    -1000
                } else {
                    1000
                },
                0,
            ],
            symmetry: ori_domain::BeginnerProtrusionSymmetryV1::None,
            curvature_degrees: 0,
            joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: ori_domain::BeginnerProtrusionSideV1::Either,
            priority: 50,
        }];
        if horn_candidate_ids.len() == 1 && ear_candidate_ids.len() == 2 {
            let mut ears = ear_candidate_ids
                .iter()
                .filter_map(|id| candidates.iter().find(|candidate| candidate.id == *id))
                .collect::<Vec<_>>();
            ears.sort_by_key(|candidate| candidate.bounds.min_x);
            let left_center = i64::from(ears[0].bounds.min_x) + i64::from(ears[0].bounds.max_x);
            let right_center = i64::from(ears[1].bounds.min_x) + i64::from(ears[1].bounds.max_x);
            let left_y = i64::from(ears[0].bounds.min_y) + i64::from(ears[0].bounds.max_y);
            let right_y = i64::from(ears[1].bounds.min_y) + i64::from(ears[1].bounds.max_y);
            if (left_center + right_center - axis_twice * 2).abs() > 2
                || (left_y - right_y).abs() > 2
            {
                return Err("part_assignment_horn_ear_binding_invalid".to_owned());
            }
            let mut ear_target = profile.generation_constraints.protrusions[0].clone();
            ear_target.id = 2;
            ear_target.count = 2;
            ear_target.length_tenths_mm = u32::try_from(
                (right_center - left_center)
                    .unsigned_abs()
                    .saturating_mul(5)
                    .max(1),
            )
            .map_err(|_| "part_assignment_horn_ear_binding_invalid")?;
            ear_target.position_tenths_mm[1] =
                i32::try_from((left_y + right_y).saturating_mul(5) / 2)
                    .map_err(|_| "part_assignment_horn_ear_binding_invalid")?;
            ear_target.direction_milli = [1000, 0, 0];
            ear_target.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
            profile.generation_constraints.protrusions.push(ear_target);
            if ori_domain::animal_horn_ear_bindings_v1(&profile.generation_constraints).is_none() {
                return Err("part_assignment_horn_ear_binding_invalid".to_owned());
            }
        }
    }
    let recognized_horn = (horn_candidate_ids.len() == 1)
        .then(|| profile.generation_constraints.protrusions.first().cloned())
        .flatten();
    if tail_candidate_ids.len() == 1
        && profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
    {
        let tail = candidates
            .iter()
            .find(|candidate| candidate.id == tail_candidate_ids[0])
            .ok_or_else(|| "part_assignment_tail_binding_invalid".to_owned())?;
        let axis_twice = i64::from(request.selected_outline.bounds.min_x)
            + i64::from(request.selected_outline.bounds.max_x);
        let center_x_twice = i64::from(tail.bounds.min_x) + i64::from(tail.bounds.max_x);
        let center_y_twice = i64::from(tail.bounds.min_y) + i64::from(tail.bounds.max_y);
        if center_x_twice == axis_twice {
            return Err("part_assignment_tail_binding_invalid".to_owned());
        }
        let direction = if center_x_twice < axis_twice {
            -1000
        } else {
            1000
        };
        let torso_width_tenths = request
            .selected_outline
            .bounds
            .max_x
            .saturating_sub(request.selected_outline.bounds.min_x)
            .saturating_add(1)
            .saturating_mul(10);
        let inferred_length = u32::try_from(
            (center_x_twice - axis_twice)
                .unsigned_abs()
                .saturating_mul(5)
                .max(1),
        )
        .map_err(|_| "part_assignment_tail_binding_invalid")?;
        let minimum_length = torso_width_tenths
            .saturating_mul(2)
            .checked_div(100)
            .unwrap_or(1)
            .max(1);
        let maximum_length = torso_width_tenths
            .saturating_mul(45)
            .checked_div(100)
            .unwrap_or(1)
            .max(minimum_length);
        let torso_min_y_twice = i64::from(request.selected_outline.bounds.min_y).saturating_mul(2);
        let torso_max_y_twice = i64::from(request.selected_outline.bounds.max_y).saturating_mul(2);
        profile.generation_constraints.protrusions = vec![ori_domain::BeginnerProtrusionTargetV1 {
            id: 1,
            count: 1,
            length_tenths_mm: inferred_length.clamp(minimum_length, maximum_length),
            thickness_tenths_mm: u16::try_from(
                (tail.bounds.max_y - tail.bounds.min_y + 1)
                    .saturating_mul(10)
                    .min(10_000),
            )
            .map_err(|_| "part_assignment_tail_binding_invalid")?,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: None,
            position_tenths_mm: [
                i32::try_from(axis_twice.saturating_mul(5))
                    .map_err(|_| "part_assignment_tail_binding_invalid")?,
                i32::try_from(
                    center_y_twice
                        .clamp(torso_min_y_twice, torso_max_y_twice)
                        .saturating_mul(5),
                )
                .map_err(|_| "part_assignment_tail_binding_invalid")?,
                0,
            ],
            direction_milli: [direction, 0, 0],
            symmetry: ori_domain::BeginnerProtrusionSymmetryV1::None,
            curvature_degrees: 0,
            joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: ori_domain::BeginnerProtrusionSideV1::Either,
            priority: 50,
        }];
        if ear_candidate_ids.len() == 2 {
            let mut ears = ear_candidate_ids
                .iter()
                .filter_map(|id| candidates.iter().find(|candidate| candidate.id == *id))
                .collect::<Vec<_>>();
            ears.sort_by_key(|candidate| candidate.bounds.min_x);
            let left_center = i64::from(ears[0].bounds.min_x) + i64::from(ears[0].bounds.max_x);
            let right_center = i64::from(ears[1].bounds.min_x) + i64::from(ears[1].bounds.max_x);
            let left_y = i64::from(ears[0].bounds.min_y) + i64::from(ears[0].bounds.max_y);
            let right_y = i64::from(ears[1].bounds.min_y) + i64::from(ears[1].bounds.max_y);
            if ears.len() != 2
                || (left_center + right_center - axis_twice * 2).abs() > 2
                || (left_y - right_y).abs() > 2
            {
                return Err("part_assignment_tail_ear_binding_invalid".to_owned());
            }
            let mut ear_target = profile.generation_constraints.protrusions[0].clone();
            ear_target.id = 2;
            ear_target.count = 2;
            ear_target.length_tenths_mm = u32::try_from(
                (right_center - left_center)
                    .unsigned_abs()
                    .saturating_mul(5)
                    .max(1),
            )
            .map_err(|_| "part_assignment_tail_ear_binding_invalid")?;
            ear_target.position_tenths_mm[1] =
                i32::try_from((left_y + right_y).saturating_mul(5) / 2)
                    .map_err(|_| "part_assignment_tail_ear_binding_invalid")?;
            ear_target.direction_milli = [1000, 0, 0];
            ear_target.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
            profile.generation_constraints.protrusions.push(ear_target);
            if ori_domain::animal_tail_ear_bindings_v1(&profile.generation_constraints).is_none() {
                return Err("part_assignment_tail_ear_binding_invalid".to_owned());
            }
        }
        if let Some(mut horn_target) = recognized_horn {
            if ear_candidate_ids.is_empty() {
                horn_target.id = 2;
                profile.generation_constraints.protrusions.push(horn_target);
                if ori_domain::animal_horn_tail_bindings_v1(&profile.generation_constraints)
                    .is_none()
                {
                    return Err("part_assignment_horn_tail_binding_invalid".to_owned());
                }
            } else if ear_candidate_ids.len() == 2 {
                horn_target.id = 3;
                profile.generation_constraints.protrusions.push(horn_target);
                if ori_domain::animal_horn_tail_ear_bindings_v1(&profile.generation_constraints)
                    .is_none()
                {
                    return Err("part_assignment_horn_tail_ear_binding_invalid".to_owned());
                }
            }
        }
    }
    if leg_candidate_ids.len() == 4
        && horn_candidate_ids.len() == 1
        && tail_candidate_ids.len() == 1
        && ear_candidate_ids.len() == 2
        && matches!(wing_candidate_ids.len(), 0 | 2)
        && profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Animal)
    {
        let axis_twice = i64::from(request.selected_outline.bounds.min_x)
            + i64::from(request.selected_outline.bounds.max_x);
        let mut legs = leg_candidate_ids
            .iter()
            .filter_map(|id| candidates.iter().find(|candidate| candidate.id == *id))
            .collect::<Vec<_>>();
        legs.sort_by_key(|candidate| {
            (
                candidate.bounds.min_y + candidate.bounds.max_y,
                candidate.bounds.min_x,
            )
        });
        if legs.len() != 4
            || legs.chunks_exact(2).any(|pair| {
                !candidate_pair_is_symmetric(axis_twice, &pair[0].bounds, &pair[1].bounds)
            })
        {
            return Err("part_assignment_complete_animal_binding_invalid".to_owned());
        }
        let mut leg_target = profile
            .generation_constraints
            .protrusions
            .first()
            .cloned()
            .ok_or_else(|| "part_assignment_complete_animal_binding_invalid".to_owned())?;
        leg_target.id = 4;
        leg_target.count = 4;
        leg_target.direction_milli = [0, 1000, 0];
        leg_target.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
        profile.generation_constraints.protrusions.push(leg_target);
        if wing_candidate_ids.len() == 2 {
            let wings = wing_candidate_ids
                .iter()
                .filter_map(|id| candidates.iter().find(|candidate| candidate.id == *id))
                .collect::<Vec<_>>();
            if wings.len() != 2
                || !candidate_pair_is_symmetric(axis_twice, &wings[0].bounds, &wings[1].bounds)
            {
                return Err("part_assignment_complete_animal_binding_invalid".to_owned());
            }
            let mut wing_target = profile.generation_constraints.protrusions[2].clone();
            wing_target.id = 5;
            wing_target.count = 2;
            wing_target.priority = 60;
            wing_target.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
            profile.generation_constraints.protrusions.push(wing_target);
        }
        let ordered_ids = if let Some(winged) =
            ori_domain::animal_complete_winged_bindings_v1(&profile.generation_constraints)
        {
            vec![
                winged.animal.horn_protrusion_id,
                winged.animal.tail_protrusion_id,
                winged.animal.ear_pair_protrusion_id,
                winged.animal.leg_protrusion_id,
                winged.wing_pair_protrusion_id,
            ]
        } else {
            let complete = ori_domain::animal_complete_bindings_v1(&profile.generation_constraints)
                .ok_or_else(|| "part_assignment_complete_animal_binding_invalid".to_owned())?;
            vec![
                complete.horn_protrusion_id,
                complete.tail_protrusion_id,
                complete.ear_pair_protrusion_id,
                complete.leg_protrusion_id,
            ]
        };
        let canonical = ordered_ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                let mut target = profile
                    .generation_constraints
                    .protrusions
                    .iter()
                    .find(|target| target.id == id)
                    .cloned()
                    .ok_or_else(|| "part_assignment_complete_animal_binding_invalid".to_owned())?;
                target.id = index as u16 + 1;
                Ok(target)
            })
            .collect::<Result<Vec<_>, String>>()?;
        profile.generation_constraints.protrusions = canonical;
        if ori_domain::animal_complete_bindings_v1(&profile.generation_constraints).is_none()
            && ori_domain::animal_complete_winged_bindings_v1(&profile.generation_constraints)
                .is_none()
        {
            return Err("part_assignment_complete_animal_binding_invalid".to_owned());
        }
    }
    let recognized_wing_antenna = (wing_candidate_ids.len() == 2
        && antenna_candidate_ids.len() == 2)
        .then(|| profile.generation_constraints.protrusions.clone());
    if leg_candidate_ids.len() == 6
        && profile.generation_constraints.target_category
            == Some(ori_domain::BeginnerTargetCategoryV1::Insect)
    {
        let axis_twice = i64::from(request.selected_outline.bounds.min_x)
            + i64::from(request.selected_outline.bounds.max_x);
        let mut legs = leg_candidate_ids
            .iter()
            .filter_map(|id| candidates.iter().find(|candidate| candidate.id == *id))
            .collect::<Vec<_>>();
        legs.sort_by_key(|candidate| {
            (
                candidate.bounds.min_y + candidate.bounds.max_y,
                candidate.bounds.min_x,
            )
        });
        let mut protrusions = Vec::with_capacity(3);
        for (pair_index, pair) in legs.chunks_exact(2).enumerate() {
            let left_center_twice =
                i64::from(pair[0].bounds.min_x) + i64::from(pair[0].bounds.max_x);
            let right_center_twice =
                i64::from(pair[1].bounds.min_x) + i64::from(pair[1].bounds.max_x);
            let left_y_twice = i64::from(pair[0].bounds.min_y) + i64::from(pair[0].bounds.max_y);
            let right_y_twice = i64::from(pair[1].bounds.min_y) + i64::from(pair[1].bounds.max_y);
            if !candidate_pair_is_symmetric(axis_twice, &pair[0].bounds, &pair[1].bounds) {
                return Err("part_assignment_six_leg_binding_invalid".to_owned());
            }
            let length_tenths_mm = u32::try_from(
                (right_center_twice - left_center_twice)
                    .unsigned_abs()
                    .saturating_mul(5)
                    .max(1),
            )
            .map_err(|_| "part_assignment_six_leg_binding_invalid")?;
            let thickness_pixels = (pair[0].bounds.max_y - pair[0].bounds.min_y + 1)
                .min(pair[1].bounds.max_y - pair[1].bounds.min_y + 1);
            protrusions.push(ori_domain::BeginnerProtrusionTargetV1 {
                id: pair_index as u16 + 1,
                count: 2,
                length_tenths_mm,
                thickness_tenths_mm: u16::try_from(thickness_pixels.saturating_mul(10).min(10_000))
                    .map_err(|_| "part_assignment_six_leg_binding_invalid")?,
                root_width_tenths_mm: None,
                tip_width_tenths_mm: None,
                local_outline_tenths_mm: None,
                position_tenths_mm: [
                    i32::try_from(axis_twice.saturating_mul(5))
                        .map_err(|_| "part_assignment_six_leg_binding_invalid")?,
                    i32::try_from((left_y_twice + right_y_twice).saturating_mul(5) / 2)
                        .map_err(|_| "part_assignment_six_leg_binding_invalid")?,
                    0,
                ],
                direction_milli: [1000, 0, 0],
                symmetry: ori_domain::BeginnerProtrusionSymmetryV1::Bilateral,
                curvature_degrees: 0,
                joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
                motion_degrees: [0, 0],
                side: ori_domain::BeginnerProtrusionSideV1::Either,
                priority: 50,
            });
        }
        profile.generation_constraints.protrusions = protrusions;
        if let Some(mut pairs) = recognized_wing_antenna {
            if pairs.len() == 2 {
                pairs[0].id = 4;
                pairs[0].priority = 60;
                pairs[1].id = 5;
                pairs[1].priority = 60;
                profile.generation_constraints.protrusions.extend(pairs);
                if ori_domain::insect_complete_bindings_v1(&profile.generation_constraints)
                    .is_none()
                {
                    return Err("part_assignment_complete_insect_binding_invalid".to_owned());
                }
            }
        }
        if ori_domain::insect_three_pair_bindings_v1(&profile.generation_constraints).is_none() {
            return Err("part_assignment_six_leg_binding_invalid".to_owned());
        }
    }
    let feature_parts = profile
        .generation_constraints
        .target_parts
        .iter()
        .filter(|part| {
            !matches!(
                part.kind,
                ori_domain::BeginnerTargetPartKindV1::Head
                    | ori_domain::BeginnerTargetPartKindV1::Torso
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let specialized = ori_domain::animal_complete_bindings_v1(&profile.generation_constraints)
        .is_some()
        || ori_domain::animal_complete_winged_bindings_v1(&profile.generation_constraints)
            .is_some()
        || ori_domain::insect_complete_bindings_v1(&profile.generation_constraints).is_some()
        || ori_domain::animal_horn_tail_ear_bindings_v1(&profile.generation_constraints).is_some()
        || ori_domain::animal_horn_tail_bindings_v1(&profile.generation_constraints).is_some()
        || ori_domain::animal_horn_ear_bindings_v1(&profile.generation_constraints).is_some()
        || ori_domain::animal_tail_ear_bindings_v1(&profile.generation_constraints).is_some()
        || ori_domain::insect_wing_antenna_bindings_v1(&profile.generation_constraints).is_some();
    if !specialized && (2..=8).contains(&feature_parts.len()) {
        let axis_twice = i64::from(request.selected_outline.bounds.min_x)
            + i64::from(request.selected_outline.bounds.max_x);
        let mut generic = Vec::with_capacity(feature_parts.len());
        for (index, part) in feature_parts.iter().enumerate() {
            if !matches!(part.count, 1 | 2 | 4) {
                return Err("part_assignment_generic_binding_invalid".to_owned());
            }
            let mut members = request
                .assignments
                .iter()
                .filter(|assignment| assignment.kind == part.kind)
                .filter_map(|assignment| {
                    candidates
                        .iter()
                        .find(|candidate| candidate.id == assignment.candidate_id)
                })
                .collect::<Vec<_>>();
            members.sort_by_key(|candidate| {
                (candidate.bounds.min_y, candidate.bounds.min_x, candidate.id)
            });
            let repeated_single_rank = feature_parts[..index]
                .iter()
                .filter(|previous| previous.kind == part.kind && previous.count == 1)
                .count();
            if part.count == 1 && members.len() > 1 {
                members = members
                    .get(repeated_single_rank)
                    .copied()
                    .into_iter()
                    .collect();
            }
            if members.len() != usize::from(part.count)
                || part.count > 1
                    && members.chunks_exact(2).any(|pair| {
                        !candidate_pair_is_symmetric(axis_twice, &pair[0].bounds, &pair[1].bounds)
                    })
            {
                return Err("part_assignment_generic_binding_invalid".to_owned());
            }
            let min_x = members
                .iter()
                .map(|candidate| candidate.bounds.min_x)
                .min()
                .unwrap();
            let max_x = members
                .iter()
                .map(|candidate| candidate.bounds.max_x)
                .max()
                .unwrap();
            let min_y = members
                .iter()
                .map(|candidate| candidate.bounds.min_y)
                .min()
                .unwrap();
            let max_y = members
                .iter()
                .map(|candidate| candidate.bounds.max_y)
                .max()
                .unwrap();
            let vertical = matches!(
                part.kind,
                ori_domain::BeginnerTargetPartKindV1::Leg
                    | ori_domain::BeginnerTargetPartKindV1::Horn
                    | ori_domain::BeginnerTargetPartKindV1::Antenna
            );
            let half_width = i32::try_from(max_x.saturating_sub(min_x).saturating_add(1))
                .unwrap_or(2_000)
                .saturating_mul(5)
                .clamp(1, 10_000);
            let half_height = i32::try_from(max_y.saturating_sub(min_y).saturating_add(1))
                .unwrap_or(2_000)
                .saturating_mul(5)
                .clamp(1, 10_000);
            generic.push(ori_domain::BeginnerProtrusionTargetV1 {
                id: index as u16 + 1,
                count: part.count,
                length_tenths_mm: (if vertical {
                    max_y.saturating_sub(min_y).saturating_add(1)
                } else {
                    max_x.saturating_sub(min_x).saturating_add(1)
                })
                .saturating_mul(10)
                .clamp(1, 1_000_000),
                thickness_tenths_mm: u16::try_from(
                    max_y
                        .saturating_sub(min_y)
                        .saturating_add(1)
                        .saturating_mul(10),
                )
                .unwrap_or(10_000)
                .clamp(1, 10_000),
                root_width_tenths_mm: None,
                tip_width_tenths_mm: None,
                local_outline_tenths_mm: Some(vec![
                    [-half_width, -half_height],
                    [half_width, -half_height],
                    [half_width, half_height],
                    [-half_width, half_height],
                ]),
                position_tenths_mm: [
                    i32::try_from(axis_twice.saturating_mul(5))
                        .map_err(|_| "part_assignment_generic_binding_invalid")?,
                    i32::try_from(min_y.saturating_add(max_y).saturating_mul(5))
                        .map_err(|_| "part_assignment_generic_binding_invalid")?,
                    0,
                ],
                direction_milli: if vertical { [0, 1000, 0] } else { [1000, 0, 0] },
                symmetry: if part.count == 1 {
                    ori_domain::BeginnerProtrusionSymmetryV1::None
                } else {
                    ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
                },
                curvature_degrees: 0,
                joint: ori_domain::BeginnerProtrusionJointV1::Fixed,
                motion_degrees: [0, 0],
                side: ori_domain::BeginnerProtrusionSideV1::Either,
                priority: 50_u8.saturating_add(index as u8 * 5),
            });
        }
        profile.generation_constraints.protrusions = generic;
    }
    execute_command(
        &mut project,
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
        ori_core::Command::UpdateBeginnerDesignProfile { profile },
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyBeginnerOutlineCandidateRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    underlay_id: UnderlayId,
    asset_id: AssetId,
    candidate: ori_domain::BeginnerOutlineCandidateV1,
    confirmed: bool,
}

#[tauri::command]
pub(crate) fn apply_beginner_outline_candidate(
    state: State<'_, AppState>,
    request: ApplyBeginnerOutlineCandidateRequest,
) -> Result<ProjectSnapshot, String> {
    if !request.confirmed {
        return Err("outline_candidate_confirmation_required".to_owned());
    }
    let binding = RecognizeBeginnerTargetRequest {
        expected_project_instance_id: request.expected_project_instance_id,
        expected_project_id: request.expected_project_id,
        expected_revision: request.expected_revision,
        underlay_id: request.underlay_id,
        asset_id: request.asset_id,
    };
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, binding)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?
    };
    let (width, height, rgba) = decode_general_image(&bytes)?;
    let candidates = ori_domain::analyze_outline_candidates_rgba_v1(width, height, &rgba)
        .map_err(|_| "recognition_resource_limit".to_owned())?;
    if candidates.get(usize::from(request.candidate.id)) != Some(&request.candidate) {
        return Err("outline_candidate_stale".to_owned());
    }
    let mut project = lock_project(&state)?;
    ensure_recognition_binding(&project, binding)?;
    let live = project
        .texture_assets
        .iter()
        .find(|asset| asset.id == request.asset_id)
        .ok_or_else(|| "recognition_asset_unavailable".to_owned())?;
    if <[u8; 32]>::from(Sha256::digest(&live.bytes)) != <[u8; 32]>::from(Sha256::digest(&bytes)) {
        return Err("recognition_asset_changed".to_owned());
    }
    let bounds = request.candidate.bounds;
    let mut profile = project.editor.beginner_design_profile().clone();
    let center_x = i32::try_from((bounds.min_x + bounds.max_x) / 2)
        .map_err(|_| "outline_candidate_stale")?
        * 10;
    let center_y = i32::try_from((bounds.min_y + bounds.max_y) / 2)
        .map_err(|_| "outline_candidate_stale")?
        * 10;
    let thickness_tenths_mm = u16::try_from(
        (bounds.max_y - bounds.min_y + 1)
            .saturating_mul(10)
            .min(10_000),
    )
    .map_err(|_| "outline_candidate_stale")?;
    profile.generation_constraints.skeleton_segments = vec![
        ori_domain::BeginnerSkeletonSegmentV1 {
            id: 0,
            start: ori_domain::BeginnerSkeletonPointV1 {
                x_tenths_mm: i32::try_from(bounds.min_x).map_err(|_| "outline_candidate_stale")?
                    * 10,
                y_tenths_mm: center_y,
            },
            end: ori_domain::BeginnerSkeletonPointV1 {
                x_tenths_mm: i32::try_from(bounds.max_x).map_err(|_| "outline_candidate_stale")?
                    * 10,
                y_tenths_mm: center_y,
            },
            thickness_tenths_mm,
        },
        ori_domain::BeginnerSkeletonSegmentV1 {
            id: 1,
            start: ori_domain::BeginnerSkeletonPointV1 {
                x_tenths_mm: center_x,
                y_tenths_mm: i32::try_from(bounds.min_y).map_err(|_| "outline_candidate_stale")?
                    * 10,
            },
            end: ori_domain::BeginnerSkeletonPointV1 {
                x_tenths_mm: center_x,
                y_tenths_mm: i32::try_from(bounds.max_y).map_err(|_| "outline_candidate_stale")?
                    * 10,
            },
            thickness_tenths_mm,
        },
    ];
    execute_command(
        &mut project,
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
        ori_core::Command::UpdateBeginnerDesignProfile { profile },
    )
}

#[tauri::command]
pub(crate) fn recognize_beginner_target(
    state: State<'_, AppState>,
    request: RecognizeBeginnerTargetRequest,
) -> Result<BeginnerRecognitionProposalV1, String> {
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "target recognition asset is unavailable".to_owned())?
    };
    let source_sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let (width, height, rgba) = decode_marker_png(&bytes)?;
    let proposal = analyze_marker_png_rgba_v1(
        request.underlay_id,
        request.asset_id,
        source_sha256,
        width,
        height,
        &rgba,
    )
    .map_err(|error| format!("marker PNG recognition failed: {error:?}"))?;
    {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        let live_bytes = project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.as_slice())
            .ok_or_else(|| "target recognition asset is unavailable".to_owned())?;
        let live_hash: [u8; 32] = Sha256::digest(live_bytes).into();
        if live_hash != source_sha256 {
            return Err("target recognition asset changed during analysis".to_owned());
        }
    }
    Ok(proposal)
}

fn ensure_recognition_binding(
    project: &ProjectState,
    request: RecognizeBeginnerTargetRequest,
) -> Result<(), String> {
    ensure_expected_project(
        project,
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
    )?;
    project
        .editor
        .underlays()
        .underlays
        .iter()
        .any(|underlay| underlay.id == request.underlay_id && underlay.asset == request.asset_id)
        .then_some(())
        .ok_or_else(|| "target recognition underlay binding changed".to_owned())
}

fn decode_marker_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|_| "target recognition requires a valid CRC-checked PNG".to_owned())?;
    let info = reader.info();
    let pixels = usize::try_from(info.width)
        .ok()
        .and_then(|width| {
            usize::try_from(info.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| "target recognition dimensions overflow".to_owned())?;
    if info.width > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || info.height > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || pixels > ori_domain::MAX_BEGINNER_RECOGNITION_PIXELS_V1
    {
        return Err("target recognition image exceeds the supported dimensions".to_owned());
    }
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| "target recognition output size is unavailable".to_owned())?;
    if output_size != pixels.saturating_mul(4) {
        return Err("marker_png_v1 requires 8-bit RGBA pixels".to_owned());
    }
    let mut output = vec![0_u8; output_size];
    let frame = reader
        .next_frame(&mut output)
        .map_err(|_| "target recognition PNG decode failed".to_owned())?;
    if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
        return Err("marker_png_v1 requires 8-bit RGBA pixels".to_owned());
    }
    output.truncate(frame.buffer_size());
    Ok((frame.width, frame.height, output))
}

fn decode_general_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| "recognition_requires_valid_png".to_owned())?;
    let info = reader.info();
    let pixels = usize::try_from(info.width)
        .ok()
        .and_then(|width| usize::try_from(info.height).ok()?.checked_mul(width))
        .ok_or_else(|| "recognition_resource_limit".to_owned())?;
    if info.width > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || info.height > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || pixels > ori_domain::MAX_BEGINNER_RECOGNITION_PIXELS_V1
    {
        return Err("recognition_resource_limit".to_owned());
    }
    let mut decoded = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| "recognition_resource_limit".to_owned())?
    ];
    let frame = reader
        .next_frame(&mut decoded)
        .map_err(|_| "recognition_requires_valid_png".to_owned())?;
    decoded.truncate(frame.buffer_size());
    let mut rgba = Vec::with_capacity(
        pixels
            .checked_mul(4)
            .ok_or_else(|| "recognition_resource_limit".to_owned())?,
    );
    match frame.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(&decoded),
        png::ColorType::Rgb => decoded.chunks_exact(3).for_each(|pixel| {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }),
        png::ColorType::Grayscale => decoded
            .iter()
            .for_each(|value| rgba.extend_from_slice(&[*value, *value, *value, 255])),
        png::ColorType::GrayscaleAlpha => decoded.chunks_exact(2).for_each(|pixel| {
            rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
        }),
        png::ColorType::Indexed => return Err("recognition_requires_valid_png".to_owned()),
    }
    if rgba.len() != pixels * 4 {
        return Err("recognition_resource_limit".to_owned());
    }
    Ok((frame.width, frame.height, rgba))
}

fn decode_general_image(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        decode_general_png(bytes)
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        decode_general_jpeg(bytes)
    } else {
        Err("recognition_requires_png_or_jpeg".to_owned())
    }
}

fn decode_general_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder
        .read_info()
        .map_err(|_| "recognition_requires_valid_jpeg".to_owned())?;
    let info = decoder
        .info()
        .ok_or_else(|| "recognition_requires_valid_jpeg".to_owned())?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    let pixels = usize::from(info.width)
        .checked_mul(usize::from(info.height))
        .ok_or_else(|| "recognition_resource_limit".to_owned())?;
    if width > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || height > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || pixels > ori_domain::MAX_BEGINNER_RECOGNITION_PIXELS_V1
    {
        return Err("recognition_resource_limit".to_owned());
    }
    let decoded = decoder
        .decode()
        .map_err(|_| "recognition_requires_valid_jpeg".to_owned())?;
    let mut rgba = Vec::with_capacity(
        pixels
            .checked_mul(4)
            .ok_or_else(|| "recognition_resource_limit".to_owned())?,
    );
    match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => decoded
            .iter()
            .for_each(|value| rgba.extend_from_slice(&[*value, *value, *value, 255])),
        jpeg_decoder::PixelFormat::RGB24 => decoded.chunks_exact(3).for_each(|pixel| {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }),
        _ => return Err("recognition_unsupported_jpeg_color".to_owned()),
    }
    if rgba.len() != pixels * 4 {
        return Err("recognition_resource_limit".to_owned());
    }
    Ok((width, height, rgba))
}

#[cfg(test)]
mod tests {
    use super::{
        candidate_pair_is_symmetric, decode_general_image, decode_general_png, decode_marker_png,
    };

    #[test]
    fn complete_insect_image_pairs_require_both_equal_mirrored_sides() {
        let left = ori_domain::BeginnerRecognitionBoundsV1 {
            min_x: 1,
            min_y: 4,
            max_x: 3,
            max_y: 8,
        };
        let right = ori_domain::BeginnerRecognitionBoundsV1 {
            min_x: 7,
            min_y: 4,
            max_x: 9,
            max_y: 8,
        };
        assert!(candidate_pair_is_symmetric(10, &left, &right));
        let mut asymmetric_width = right.clone();
        asymmetric_width.max_x = 10;
        assert!(!candidate_pair_is_symmetric(10, &left, &asymmetric_width));
        let mut asymmetric_height = right.clone();
        asymmetric_height.max_y = 9;
        assert!(!candidate_pair_is_symmetric(10, &left, &asymmetric_height));
        let mut missing_mirror = right;
        missing_mirror.min_x = 6;
        missing_mirror.max_x = 8;
        assert!(!candidate_pair_is_symmetric(10, &left, &missing_mirror));
    }

    fn encode(color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("header");
            writer.write_image_data(pixels).expect("pixels");
        }
        bytes
    }

    #[test]
    fn decodes_exact_rgba_png() {
        let pixels = [255, 0, 0, 255, 0, 0, 0, 0];
        let decoded = decode_marker_png(&encode(png::ColorType::Rgba, &pixels)).expect("decode");
        assert_eq!(decoded, (2, 1, pixels.to_vec()));
    }

    #[test]
    fn rejects_non_rgba_and_corrupt_png() {
        let rgb = encode(png::ColorType::Rgb, &[255, 0, 0, 0, 0, 0]);
        assert!(decode_marker_png(&rgb).is_err());

        let mut corrupt = encode(png::ColorType::Rgba, &[255, 0, 0, 255, 0, 0, 0, 0]);
        let index = corrupt.len() / 2;
        corrupt[index] ^= 0xff;
        assert!(decode_marker_png(&corrupt).is_err());
    }

    #[test]
    fn general_silhouette_decoder_deterministically_expands_rgb_and_grayscale() {
        assert_eq!(
            decode_general_png(&encode(png::ColorType::Rgb, &[10, 20, 30, 40, 50, 60]))
                .expect("RGB"),
            (2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255])
        );
        assert_eq!(
            decode_general_png(&encode(png::ColorType::Grayscale, &[12, 34])).expect("grayscale"),
            (2, 1, vec![12, 12, 12, 255, 34, 34, 34, 255])
        );

        let mut palette_png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut palette_png, 2, 1);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(vec![10, 20, 30, 40, 50, 60]);
            let mut writer = encoder.write_header().expect("palette header");
            writer.write_image_data(&[0, 1]).expect("palette pixels");
        }
        assert_eq!(
            decode_general_png(&palette_png).expect("palette"),
            (2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255])
        );
    }

    #[test]
    fn general_decoder_rejects_unknown_and_corrupt_jpeg_envelopes() {
        assert!(decode_general_image(b"not an image").is_err());
        assert!(decode_general_image(&[0xff, 0xd8, 0xff, 0xd9]).is_err());
    }
}
