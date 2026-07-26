use ori_core::{
    AppliedPoseLimitsV1, SpeculativeApproximateBlockingObservationV1,
    SpeculativeUnprovenFoldBindingV1, create_rectangular_sheet, prepare_applied_pose_v1,
};
use ori_domain::{
    DEFAULT_PROJECT_LAYER_ID, Edge, EdgeId, EdgeKind, FaceId, InstructionHingeAngle,
    InstructionPose, InstructionPoseModel, InstructionStep, InstructionStepId, InstructionTimeline,
    InstructionVisual, MIN_INSTRUCTION_DURATION_MS, ProjectId, ProjectLayerDocumentV1,
};

use super::*;

pub(super) fn marked_archive(applied_base: bool) -> Ori2ProjectArchive {
    let sheet = create_rectangular_sheet(80.0, 60.0, false).expect("rectangular sheet");
    let (source_pattern, mut paper) = sheet.into_parts();
    paper.thickness_mm = 0.1;
    let mut editor = EditorState::with_paper(source_pattern.clone(), paper.clone());
    if applied_base {
        editor
            .set_history_entry_limit(1)
            .expect("one-entry history");
    }

    let mut target_pattern = source_pattern;
    let hinge = EdgeId::new();
    target_pattern.edges.push(Edge {
        id: hinge,
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let timeline = InstructionTimeline {
        steps: vec![InstructionStep {
            id: InstructionStepId::new(),
            title: "Speculative stacked fold".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: ori_core::fold_model_fingerprint_v1(
                    &target_pattern,
                    &paper,
                ),
                fixed_face: Some(FaceId::new()),
                hinge_angles: vec![InstructionHingeAngle {
                    edge: hinge,
                    angle_degrees: 90.0,
                }],
            },
        }],
    };
    let mut faces = [FaceId::new(), FaceId::new()];
    faces.sort_by_key(FaceId::canonical_bytes);
    let pose_hinge = EdgeId::new();
    let applied_pose = prepare_applied_pose_v1(
        &faces,
        &[pose_hinge],
        Some(faces[0]),
        &[(pose_hinge, 90.0)],
        AppliedPoseLimitsV1::default(),
    )
    .expect("applied pose");
    let project_id = ProjectId::new();
    let binding = SpeculativeUnprovenFoldBindingV1::new(
        ProjectId::new(),
        project_id,
        editor.revision(),
        editor.fold_model_fingerprint_v1(),
        7,
        ProjectId::new(),
        editor.paper().thickness_mm,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("speculative binding");
    editor
        .execute_stacked_fold_document_with_unproven_mark_v1(
            editor.revision(),
            target_pattern,
            paper,
            timeline,
            ProjectLayerDocumentV1::default(),
            applied_pose,
            binding,
        )
        .expect("marked stacked-fold apply");
    if applied_base {
        editor
            .execute(
                editor.revision(),
                Command::RenameLayer {
                    layer: DEFAULT_PROJECT_LAYER_ID,
                    name: "Post-speculative edit".to_owned(),
                },
            )
            .expect("trim marked entry into applied-base ledger");
    }

    let history = editor
        .export_history_v1(project_id)
        .expect("marked history");
    assert!(history.requires_speculative_unproven_fold_feature_v1());
    let mut document =
        ProjectDocument::new("Speculative feature fixture", editor.pattern().clone());
    document.project_id = project_id;
    document.paper = editor.paper().clone();
    document.instruction_timeline = editor.instruction_timeline().clone();
    document.geometric_constraints = editor.geometric_constraints().clone();
    document.layers = editor.project_layers().clone();
    Ori2ProjectArchive {
        document,
        editor_history: Some(history),
        layer_evidence: None,
    }
}

pub(super) fn rewrite_ori2_manifest(
    bytes: &[u8],
    mutate: impl FnOnce(&mut Ori2Manifest),
) -> Vec<u8> {
    let mut entries = archive_entries(bytes);
    let manifest = entries
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest");
    let mut value: Ori2Manifest = serde_json::from_slice(&manifest.1).expect("manifest JSON");
    mutate(&mut value);
    manifest.1 = serde_json::to_vec_pretty(&value).expect("mutated manifest");
    raw_zip_owned(&entries)
}

pub(super) fn ori2_without_history_entry(bytes: &[u8]) -> Vec<u8> {
    let mut entries = archive_entries(bytes);
    entries.retain(|(path, _)| path != ORI2_EDITOR_HISTORY_PATH);
    raw_zip_owned(&entries)
}

pub(super) fn ori2_without_history_mark(bytes: &[u8]) -> Vec<u8> {
    let mut entries = archive_entries(bytes);
    let history = entries
        .iter()
        .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
        .expect("history");
    let mut value: serde_json::Value = serde_json::from_slice(&history.1).expect("history JSON");
    value["history"]["undo_stack"][0]
        .as_object_mut()
        .expect("undo entry")
        .remove("speculative_unproven_fold_v1");
    let history_bytes = serde_json::to_vec_pretty(&value).expect("tampered history");
    reseal_history_entry(&mut entries, history_bytes);
    raw_zip_owned(&entries)
}

pub(super) fn folder_manifest(
    entries: &[crate::ProjectFolderEntryV1],
) -> crate::ProjectFolderManifestV1 {
    let bytes = &entries
        .iter()
        .find(|entry| entry.path == crate::PROJECT_FOLDER_MANIFEST_PATH)
        .expect("folder manifest")
        .bytes;
    serde_json::from_slice(bytes).expect("folder manifest JSON")
}

pub(super) fn rewrite_folder_manifest(
    entries: &[crate::ProjectFolderEntryV1],
    mutate: impl FnOnce(&mut crate::ProjectFolderManifestV1),
) -> Vec<crate::ProjectFolderEntryV1> {
    let mut entries = entries.to_vec();
    let manifest_entry = entries
        .iter_mut()
        .find(|entry| entry.path == crate::PROJECT_FOLDER_MANIFEST_PATH)
        .expect("folder manifest");
    let mut manifest: crate::ProjectFolderManifestV1 =
        serde_json::from_slice(&manifest_entry.bytes).expect("folder manifest JSON");
    mutate(&mut manifest);
    manifest_entry.bytes = serde_json::to_vec_pretty(&manifest).expect("mutated folder manifest");
    entries
}

pub(super) fn folder_without_history_mark(
    entries: &[crate::ProjectFolderEntryV1],
) -> Vec<crate::ProjectFolderEntryV1> {
    let mut entries = entries.to_vec();
    let history_entry = entries
        .iter_mut()
        .find(|entry| entry.path == crate::PROJECT_FOLDER_EDITOR_HISTORY_PATH)
        .expect("folder history");
    let mut value: serde_json::Value =
        serde_json::from_slice(&history_entry.bytes).expect("folder history JSON");
    value["history"]["undo_stack"][0]
        .as_object_mut()
        .expect("undo entry")
        .remove("speculative_unproven_fold_v1");
    history_entry.bytes = serde_json::to_vec_pretty(&value).expect("tampered folder history");
    let size = history_entry.bytes.len() as u64;
    let hash = sha256_hex(&history_entry.bytes);
    let manifest_entry = entries
        .iter_mut()
        .find(|entry| entry.path == crate::PROJECT_FOLDER_MANIFEST_PATH)
        .expect("folder manifest");
    let mut manifest: crate::ProjectFolderManifestV1 =
        serde_json::from_slice(&manifest_entry.bytes).expect("folder manifest JSON");
    let descriptor = manifest
        .entries
        .iter_mut()
        .find(|entry| entry.role == crate::PROJECT_FOLDER_ROLE_EDITOR_HISTORY)
        .expect("history descriptor");
    descriptor.uncompressed_size = size;
    descriptor.sha256 = hash;
    manifest_entry.bytes = serde_json::to_vec_pretty(&manifest).expect("resealed folder manifest");
    entries
}
