use super::*;

#[path = "tests/speculative_unproven_feature_test_support.rs"]
mod speculative_unproven_feature_test_support;
#[path = "tests/speculative_unproven_feature_tests.rs"]
mod speculative_unproven_feature_tests;

fn layer_evidence_fixture() -> LayerEvidenceArchiveV1 {
    LayerEvidenceArchiveV1 {
        version: 1,
        project_instance_id: "instance".into(),
        project_id: "project".into(),
        revision: 7,
        fold_model_fingerprint_sha256: "ab".repeat(32),
        evidence: LayerEvidenceArchiveKindV1::NonFlat {
            fixed_face: None,
            hinge_angles: vec![],
            material_faces: vec![LayerEvidenceFaceV1 {
                face_id: "face".into(),
                face_key_sha256: "cd".repeat(32),
            }],
            cells: vec![],
            pair_orders: vec![],
        },
    }
}

#[test]
fn strict_layer_evidence_codec_is_canonical_and_fail_closed() {
    let value = layer_evidence_fixture();
    let first = write_layer_evidence_archive_v1(&value).unwrap();
    let restored = read_layer_evidence_archive_v1(&first).unwrap();
    assert_eq!(restored, value);
    assert_eq!(write_layer_evidence_archive_v1(&restored).unwrap(), first);

    let mut unknown = serde_json::to_value(&value).unwrap();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("forged".into(), serde_json::json!(true));
    assert_eq!(
        read_layer_evidence_archive_v1(&serde_json::to_vec(&unknown).unwrap()),
        Err(LayerEvidenceArchiveErrorV1::InvalidJson)
    );

    let mut stale = value.clone();
    stale.fold_model_fingerprint_sha256 = "not-a-hash".into();
    assert_eq!(
        write_layer_evidence_archive_v1(&stale),
        Err(LayerEvidenceArchiveErrorV1::InvalidEvidence)
    );
    assert_eq!(
        read_layer_evidence_archive_v1(&vec![b' '; MAX_LAYER_EVIDENCE_JSON_BYTES_V1 + 1]),
        Err(LayerEvidenceArchiveErrorV1::Oversize)
    );
}
use ori_core::{Command, EditorState};
use ori_domain::{
    AssetId, ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, EdgeLayerAssignmentV1, FaceId,
    GeometricConstraintKindV1, GeometricConstraintRecordV1, InstructionHingeAngle, InstructionPose,
    InstructionPoseModel, InstructionStep, InstructionStepId, LayerContentKindV1, LayerId,
    LayerRecordV1, Paper, PaperAppearance, Point2, RgbaColor, Vertex, VertexId,
};

fn minimal_reference_glb() -> Vec<u8> {
    let json = br#"{"asset":{"version":"2.0"}}"#;
    let padded_len = (json.len() + 3) & !3;
    let total_len = 12 + 8 + padded_len;
    let mut bytes = Vec::with_capacity(total_len);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&(total_len as u32).to_le_bytes());
    bytes.extend_from_slice(&(padded_len as u32).to_le_bytes());
    bytes.extend_from_slice(&0x4E4F_534A_u32.to_le_bytes());
    bytes.extend_from_slice(json);
    bytes.resize(total_len, b' ');
    bytes
}

fn sample_document() -> ProjectDocument {
    let start = VertexId::new();
    let end = VertexId::new();
    ProjectDocument::new(
        "ORI2 round trip",
        CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(8.0, 3.0),
                },
            ],
            edges: vec![Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Valley,
            }],
        },
    )
}

fn project_id_from_wire(value: &str) -> ori_domain::ProjectId {
    serde_json::from_str(&format!("\"{value}\"")).expect("project ID fixture")
}

fn empty_editor_history(
    project_id: ori_domain::ProjectId,
    history_entry_limit: u32,
) -> EditorHistoryV1 {
    serde_json::from_value(serde_json::json!({
        "schema_version": EDITOR_HISTORY_SCHEMA_VERSION_V1,
        "project_id": project_id,
        "history_entry_limit": history_entry_limit,
        "undo_stack": [],
        "redo_stack": [],
    }))
    .expect("valid empty editor-history fixture")
}

fn add_all_geometric_constraint_kinds(document: &mut ProjectDocument) {
    let vertex_ids = std::array::from_fn::<_, 4, _>(|_| VertexId::new());
    let edge_ids = std::array::from_fn::<_, 6, _>(|_| EdgeId::new());
    document.crease_pattern = CreasePattern {
        vertices: vertex_ids
            .iter()
            .enumerate()
            .map(|(index, id)| Vertex {
                id: *id,
                position: Point2::new(index as f64, (index * index) as f64),
            })
            .collect(),
        edges: edge_ids
            .iter()
            .enumerate()
            .map(|(index, id)| Edge {
                id: *id,
                start: vertex_ids[index % vertex_ids.len()],
                end: vertex_ids[(index + 1) % vertex_ids.len()],
                kind: EdgeKind::Valley,
            })
            .collect(),
    };
    document.geometric_constraints.constraints = [
        GeometricConstraintKindV1::FixedLength {
            edge: edge_ids[0],
            length_mm: 10.5,
        },
        GeometricConstraintKindV1::FixedAngle {
            vertex: vertex_ids[0],
            first_edge: edge_ids[0],
            second_edge: edge_ids[1],
            angle_degrees: 45.0,
        },
        GeometricConstraintKindV1::Horizontal { edge: edge_ids[2] },
        GeometricConstraintKindV1::Vertical { edge: edge_ids[3] },
        GeometricConstraintKindV1::EqualLength {
            first_edge: edge_ids[0],
            second_edge: edge_ids[1],
        },
        GeometricConstraintKindV1::Parallel {
            first_edge: edge_ids[2],
            second_edge: edge_ids[3],
        },
        GeometricConstraintKindV1::PointOnLine {
            vertex: vertex_ids[1],
            line_edge: edge_ids[4],
        },
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: vertex_ids[0],
            second_vertex: vertex_ids[1],
            axis_edge: edge_ids[5],
        },
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: vertex_ids[0],
            source_vertex: vertex_ids[1],
            target_vertex: vertex_ids[2],
            angle_degrees: 120.0,
        },
        GeometricConstraintKindV1::AngleBisector {
            vertex: vertex_ids[3],
            first_edge: edge_ids[0],
            second_edge: edge_ids[1],
            bisector_edge: edge_ids[2],
        },
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge: edge_ids[4],
            denominator_edge: edge_ids[5],
            ratio: 2.0,
        },
    ]
    .into_iter()
    .map(|constraint| GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint,
    })
    .collect();
}

fn raw_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (path, bytes) in entries {
        writer.start_file(*path, options).expect("start test entry");
        writer.write_all(bytes).expect("write test entry");
    }
    writer.finish().expect("finish test ZIP").into_inner()
}

fn archive_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open test archive");
    (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).expect("open test entry");
            let name = entry.name().to_owned();
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .expect("read test entry contents");
            (name, contents)
        })
        .collect()
}

fn raw_zip_owned(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let borrowed = entries
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    raw_zip(&borrowed)
}

fn reseal_history_entry(entries: &mut [(String, Vec<u8>)], history_bytes: Vec<u8>) {
    let manifest_index = entries
        .iter()
        .position(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest fixture");
    let history_index = entries
        .iter()
        .position(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
        .expect("history fixture");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(&entries[manifest_index].1).expect("parse fixture manifest");
    let descriptor = manifest
        .editor_history
        .as_mut()
        .expect("fixture history descriptor");
    descriptor.uncompressed_size = history_bytes.len() as u64;
    descriptor.sha256 = sha256_hex(&history_bytes);
    entries[manifest_index].1 =
        serde_json::to_vec_pretty(&manifest).expect("reseal fixture manifest");
    entries[history_index].1 = history_bytes;
}

fn history_archive_fixture() -> (ProjectDocument, Vec<u8>) {
    let document = sample_document();
    let project = Ori2ProjectArchive {
        layer_evidence: None,
        editor_history: Some(empty_editor_history(document.project_id, 17)),
        document: document.clone(),
    };
    let bytes = write_project_archive_ori2(&project).expect("write history fixture");
    (document, bytes)
}

fn layer_evidence_archive_fixture(with_history: bool) -> Vec<u8> {
    let document = sample_document();
    let archive = Ori2ProjectArchive {
        editor_history: with_history.then(|| empty_editor_history(document.project_id, 17)),
        layer_evidence: Some(layer_evidence_fixture()),
        document,
    };
    write_project_archive_ori2(&archive).expect("write layer-evidence fixture")
}

fn version_matrix_archive_fixture() -> Ori2ProjectArchive {
    let sheet =
        ori_core::create_rectangular_sheet(80.0, 60.0, false).expect("valid version matrix sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut document = ProjectDocument::new("ORI2 version matrix", pattern.clone());
    document.paper = paper.clone();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_provenance = Some(ori_domain::BeginnerGenerationProvenanceV1 {
        schema_version: 1,
        topology_authority_sha256: [0x31; 32],
        fold_path_certificate_sha256: Some([0x62; 32]),
        document_authority_sha256: None,
        confidence_score: 93,
        confidence_reasons: vec!["bounded_native_fold_path_v2".to_owned()],
        explicit_override: false,
        source_asset_fingerprint: "asset:version-matrix".to_owned(),
        semantic_landmark_provenance: None,
        generic_tree: None,
        reference_consensus: None,
        reference_consensus_summary: None,
    });
    let mut editor = EditorState::with_paper(pattern, paper);
    editor.set_history_entry_limit(17).unwrap();
    let mut instruction_timeline = editor.instruction_timeline().clone();
    instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "Version matrix authority".to_owned(),
        description: "Authenticated provenance migration fixture".to_owned(),
        caution: String::new(),
        duration_ms: 1_000,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::DeclarativeOnlyV1,
            source_model_fingerprint: editor.fold_model_fingerprint_v1(),
            fixed_face: None,
            hinge_angles: Vec::new(),
        },
    });
    editor
        .execute(
            0,
            Command::ApplyBeginnerGeneratedDocument {
                pattern: editor.pattern().clone(),
                paper: editor.paper().clone(),
                instruction_timeline,
                project_layers: editor.project_layers().clone(),
                beginner_design_profile: Box::new(profile),
            },
        )
        .expect("mint typed provenance through an authenticated history edge");
    document.crease_pattern = editor.pattern().clone();
    document.paper = editor.paper().clone();
    document.instruction_timeline = editor.instruction_timeline().clone();
    document.layers = editor.project_layers().clone();
    document.beginner_design_profile = editor.beginner_design_profile().clone();
    document.texture_assets.push(crate::ProjectTextureAssetV1 {
        id: AssetId::new(),
        media_type: crate::ProjectTextureMediaTypeV1::Png,
        bytes: b"\x89PNG\r\n\x1a\nversion-matrix".to_vec(),
    });
    document
        .reference_model_assets
        .push(crate::ProjectReferenceModelAssetV1 {
            id: AssetId::new(),
            bytes: minimal_reference_glb(),
        });
    let history = editor.export_history_v1(document.project_id).unwrap();
    Ori2ProjectArchive {
        document,
        editor_history: Some(history),
        layer_evidence: Some(layer_evidence_fixture()),
    }
}

fn manifest_for(project_bytes: &[u8]) -> Vec<u8> {
    manifest_for_features(project_bytes, Vec::new())
}

fn manifest_for_features(project_bytes: &[u8], required_features: Vec<String>) -> Vec<u8> {
    serde_json::to_vec(&Ori2Manifest::new(
        project_bytes,
        crate::CURRENT_FORMAT_VERSION,
        required_features,
    ))
    .expect("serialize manifest")
}

fn add_sample_instruction(document: &mut ProjectDocument) {
    let edge = document.crease_pattern.edges[0].id;
    document.instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "半分に折る".to_owned(),
        description: "辺を正確に重ねます。".to_owned(),
        caution: String::new(),
        duration_ms: 1_500,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: "0123456789abcdef".repeat(4),
            fixed_face: Some(FaceId::new()),
            hinge_angles: vec![InstructionHingeAngle {
                edge,
                angle_degrees: 180.0,
            }],
        },
    });
}

fn add_sample_declarative_instruction(document: &mut ProjectDocument) {
    document.instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "中割り折り（説明）".to_owned(),
        description: "説明テンプレートとして追加します。".to_owned(),
        caution: "物理操作は自動実行しません。".to_owned(),
        duration_ms: 1_500,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::DeclarativeOnlyV1,
            source_model_fingerprint: "0123456789abcdef".repeat(4),
            fixed_face: None,
            hinge_angles: Vec::new(),
        },
    });
}

fn manifest_from_archive(bytes: &[u8]) -> Ori2Manifest {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open generated ZIP");
    let mut entry = archive
        .by_name(ORI2_MANIFEST_PATH)
        .expect("generated manifest");
    let mut manifest_bytes = Vec::new();
    entry
        .read_to_end(&mut manifest_bytes)
        .expect("read generated manifest");
    serde_json::from_slice(&manifest_bytes).expect("parse generated manifest")
}

fn replace_zip_entry_name(bytes: &mut [u8], old: &[u8], new: &[u8]) -> usize {
    assert_eq!(old.len(), new.len(), "ZIP names must have equal lengths");
    assert!(!old.is_empty(), "ZIP names must not be empty");
    if old.len() > bytes.len() {
        return 0;
    }
    let mut replacements = 0;
    let mut index = 0;
    while index <= bytes.len() - old.len() {
        if bytes[index..].starts_with(old) {
            bytes[index..index + old.len()].copy_from_slice(new);
            replacements += 1;
            index += old.len();
        } else {
            index += 1;
        }
    }
    replacements
}

#[test]
fn ori2_round_trip_preserves_project() {
    let original = sample_document();
    let bytes = write_project_ori2(&original).expect("write .ori2");
    let restored = read_project_ori2(&bytes).expect("read .ori2");
    assert_eq!(restored, original);

    let mut archive = ZipArchive::new(Cursor::new(&bytes)).expect("open generated ZIP");
    assert_eq!(archive.len(), DOCUMENT_ONLY_ENTRY_COUNT);
    assert!(archive.by_name(ORI2_MANIFEST_PATH).is_ok());
    assert!(archive.by_name(ORI2_PROJECT_PATH).is_ok());
}

#[test]
fn ori2_round_trip_preserves_typed_fold_path_provenance() {
    let mut document = sample_document();
    document.beginner_design_profile.generation_provenance =
        Some(ori_domain::BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256: [0x21; 32],
            fold_path_certificate_sha256: Some([0x42; 32]),
            document_authority_sha256: None,
            confidence_score: 88,
            confidence_reasons: vec!["bounded_native_fold_path_v2".to_owned()],
            explicit_override: false,
            source_asset_fingerprint: "asset:typed-provenance".to_owned(),
            semantic_landmark_provenance: None,
            generic_tree: None,
            reference_consensus: None,
            reference_consensus_summary: None,
        });
    ori_core::bind_beginner_generation_document_authority_v1(
        &document.crease_pattern,
        &document.paper,
        &mut document.beginner_design_profile,
    )
    .expect("bind typed provenance to final document");
    let archive = Ori2ProjectArchive::document_only(document);
    let bytes = write_project_archive_ori2(&archive).expect("write typed provenance");
    let reopened = read_project_archive_ori2(&bytes).expect("read typed provenance");
    assert_eq!(reopened, archive);
    assert_eq!(
        reopened
            .document
            .beginner_design_profile
            .generation_provenance
            .as_ref()
            .and_then(|value| value.fold_path_certificate_sha256),
        Some([0x42; 32])
    );
    let mut legacy_document = archive.document.clone();
    legacy_document
        .beginner_design_profile
        .generation_provenance
        .as_mut()
        .unwrap()
        .document_authority_sha256 = None;
    let mut unsupported = legacy_document.clone();
    unsupported.format_version = crate::CURRENT_FORMAT_VERSION + 1;
    assert!(matches!(
        write_project_ori2(&unsupported),
        Err(FormatError::UnsupportedVersion { .. })
    ));
    let legacy_project = serde_json::to_vec_pretty(&legacy_document).unwrap();
    let legacy_manifest = manifest_for(&legacy_project);
    let legacy_bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &legacy_manifest),
        (ORI2_PROJECT_PATH, &legacy_project),
    ]);
    let legacy = read_project_archive_ori2(&legacy_bytes).expect("read legacy archive");
    assert!(
        legacy
            .document
            .beginner_design_profile
            .generation_provenance
            .is_none()
    );

    let mut stale = archive.document.clone();
    stale.crease_pattern.vertices[0].position.x += 0.001;
    assert!(matches!(
        write_project_ori2(&stale),
        Err(FormatError::InvalidBeginnerDesignProfile)
    ));
    let stale_project = serde_json::to_vec_pretty(&stale).unwrap();
    let stale_manifest = manifest_for(&stale_project);
    let stale_bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &stale_manifest),
        (ORI2_PROJECT_PATH, &stale_project),
    ]);
    assert!(matches!(
        read_project_ori2(&stale_bytes),
        Err(FormatError::InvalidBeginnerDesignProfile)
    ));
}

#[test]
fn default_empty_editor_history_preserves_the_legacy_two_entry_archive() {
    let document = sample_document();
    let legacy = write_project_ori2(&document).expect("write legacy document-only archive");
    let project = Ori2ProjectArchive {
        layer_evidence: None,
        document: document.clone(),
        editor_history: Some(empty_editor_history(
            document.project_id,
            ori_core::MAX_EDITOR_HISTORY_ENTRIES as u32,
        )),
    };

    let with_default_history =
        write_project_archive_ori2(&project).expect("write default-empty history");
    assert_eq!(
        with_default_history, legacy,
        "the default empty history must not change canonical legacy bytes"
    );

    let restored =
        read_project_archive_ori2(&with_default_history).expect("read legacy-compatible archive");
    assert_eq!(restored.document, document);
    assert_eq!(restored.editor_history, None);
    assert_eq!(restored.layer_evidence, None);
    let archive = ZipArchive::new(Cursor::new(&with_default_history)).expect("open generated ZIP");
    assert_eq!(archive.len(), DOCUMENT_ONLY_ENTRY_COUNT);
}

#[test]
fn layer_evidence_round_trips_as_an_authenticated_third_entry() {
    let document = sample_document();
    let evidence = layer_evidence_fixture();
    let project = Ori2ProjectArchive {
        document: document.clone(),
        editor_history: None,
        layer_evidence: Some(evidence.clone()),
    };

    let bytes = write_project_archive_ori2(&project).expect("write layer evidence");
    let manifest = manifest_from_archive(&bytes);
    assert_eq!(
        manifest.required_features.last().map(String::as_str),
        Some(ORI2_FEATURE_LAYER_EVIDENCE_V1)
    );
    assert!(manifest.layer_evidence.is_some());
    let archive = ZipArchive::new(Cursor::new(&bytes)).expect("open generated ZIP");
    assert_eq!(archive.len(), PROJECT_WITH_HISTORY_ENTRY_COUNT);

    let restored = read_project_archive_ori2(&bytes).expect("read canonical archive");
    assert_eq!(restored.document, document);
    assert_eq!(restored.editor_history, None);
    assert_eq!(restored.layer_evidence, Some(evidence));
    assert!(matches!(
        read_project_ori2(&bytes),
        Err(FormatError::LayerEvidenceRequiresArchiveApi)
    ));
}

#[test]
fn history_and_layer_evidence_use_the_authenticated_fourth_entry() {
    let bytes = layer_evidence_archive_fixture(true);
    let archive = read_project_archive_ori2(&bytes).expect("read complete archive");
    assert!(archive.editor_history.is_some());
    assert_eq!(archive.layer_evidence, Some(layer_evidence_fixture()));
    assert_eq!(
        archive_entries(&bytes).len(),
        PROJECT_WITH_HISTORY_AND_EVIDENCE_ENTRY_COUNT
    );
}

#[test]
fn layer_evidence_obeys_its_dedicated_entry_size_limit() {
    let document = sample_document();
    let project = Ori2ProjectArchive {
        document,
        editor_history: None,
        layer_evidence: Some(layer_evidence_fixture()),
    };
    let limits = Ori2Limits {
        max_layer_evidence_size: 1,
        ..Ori2Limits::default()
    };
    assert!(matches!(
        write_project_archive_ori2_with_limits(&project, limits),
        Err(FormatError::EntryTooLarge { path, .. })
            if path == ORI2_LAYER_EVIDENCE_PATH
    ));

    let bytes = write_project_archive_ori2(&project).expect("write evidence fixture");
    assert!(matches!(
        read_project_archive_ori2_with_limits(&bytes, limits),
        Err(FormatError::EntryTooLarge { path, .. })
            if path == ORI2_LAYER_EVIDENCE_PATH
    ));
}

#[test]
fn writer_rejects_invalid_in_memory_layer_evidence() {
    let mut evidence = layer_evidence_fixture();
    evidence.fold_model_fingerprint_sha256 = "not-a-hash".into();
    let project = Ori2ProjectArchive {
        document: sample_document(),
        editor_history: None,
        layer_evidence: Some(evidence),
    };

    assert!(matches!(
        write_project_archive_ori2(&project),
        Err(FormatError::InvalidLayerEvidence(
            LayerEvidenceArchiveErrorV1::InvalidEvidence
        ))
    ));
}

#[test]
fn custom_history_limit_round_trips_in_an_authenticated_third_entry() {
    let document = sample_document();
    let history = empty_editor_history(document.project_id, 17);
    assert!(!history.is_default_empty());
    let project = Ori2ProjectArchive {
        layer_evidence: None,
        document: document.clone(),
        editor_history: Some(history.clone()),
    };

    let first = write_project_archive_ori2(&project).expect("write history archive");
    let second = write_project_archive_ori2(&project).expect("rewrite history archive");
    assert_eq!(
        second, first,
        "history-bearing archives must be deterministic"
    );

    let manifest = manifest_from_archive(&first);
    assert_eq!(
        manifest.required_features,
        vec![ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned()]
    );
    let descriptor = manifest
        .editor_history
        .expect("history descriptor must be present");
    assert_eq!(descriptor.path, ORI2_EDITOR_HISTORY_PATH);
    assert_eq!(descriptor.schema_version, EDITOR_HISTORY_SCHEMA_VERSION_V1);
    assert!(is_lowercase_sha256_hex(&descriptor.sha256));

    let mut archive = ZipArchive::new(Cursor::new(&first)).expect("open generated ZIP");
    assert_eq!(archive.len(), PROJECT_WITH_HISTORY_ENTRY_COUNT);
    assert!(archive.by_name(ORI2_EDITOR_HISTORY_PATH).is_ok());

    let restored = read_project_archive_ori2(&first).expect("read history archive");
    assert_eq!(restored.document, document);
    assert_eq!(restored.editor_history, Some(history));
    assert!(matches!(
        read_project_ori2(&first),
        Err(FormatError::EditorHistoryRequiresArchiveApi)
    ));
}

#[test]
fn nonempty_undo_and_redo_history_round_trip_and_remain_operational() {
    let mut document = sample_document();
    let mut editor = EditorState::with_document_parts_and_constraints(
        document.crease_pattern.clone(),
        document.paper.clone(),
        document.instruction_timeline.clone(),
        document.geometric_constraints.clone(),
    );
    editor
        .set_history_entry_limit(17)
        .expect("set history limit");
    let vertex = editor.pattern().vertices[0].id;
    editor
        .execute(
            editor.revision(),
            Command::MoveVertex {
                id: vertex,
                position: Point2::new(1.0, 2.0),
            },
        )
        .expect("first edit");
    editor
        .execute(
            editor.revision(),
            Command::MoveVertex {
                id: vertex,
                position: Point2::new(3.0, 4.0),
            },
        )
        .expect("second edit");
    editor.undo(editor.revision()).expect("create redo history");
    assert!(editor.can_undo());
    assert!(editor.can_redo());

    document.crease_pattern = editor.pattern().clone();
    document.paper = editor.paper().clone();
    document.instruction_timeline = editor.instruction_timeline().clone();
    document.geometric_constraints = editor.geometric_constraints().clone();
    let history = editor
        .export_history_v1(document.project_id)
        .expect("export editor history");
    let bytes = write_project_archive_ori2(&Ori2ProjectArchive {
        layer_evidence: None,
        document: document.clone(),
        editor_history: Some(history.clone()),
    })
    .expect("write nonempty history");

    let restored_archive = read_project_archive_ori2(&bytes).expect("read nonempty history");
    let restored_history = restored_archive
        .editor_history
        .clone()
        .expect("restored history");
    assert_eq!(restored_history, history);
    let mut restored = EditorState::with_document_parts_and_history_v1(
        restored_archive.document.crease_pattern.clone(),
        restored_archive.document.paper.clone(),
        restored_archive.document.instruction_timeline.clone(),
        restored_archive.document.geometric_constraints.clone(),
        restored_history,
    )
    .expect("restore operational editor history");
    assert_eq!(restored.revision(), 0);
    assert_eq!(restored.history_entry_limit(), 17);
    assert!(restored.can_undo());
    assert!(restored.can_redo());
    assert_eq!(
        restored
            .export_history_v1(document.project_id)
            .expect("re-export restored history"),
        history
    );

    restored.undo(0).expect("first post-load undo");
    assert_eq!(restored.revision(), 1);
}

#[test]
fn writer_rejects_editor_history_bound_to_another_project() {
    let document = sample_document();
    let history = empty_editor_history(ori_domain::ProjectId::new(), 17);
    let project = Ori2ProjectArchive {
        layer_evidence: None,
        document,
        editor_history: Some(history),
    };

    assert!(matches!(
        write_project_archive_ori2(&project),
        Err(FormatError::EditorHistoryProjectIdMismatch)
    ));
}

#[test]
fn history_feature_and_manifest_descriptor_must_be_declared_together() {
    let (_, bytes) = history_archive_fixture();

    let mut missing_feature = archive_entries(&bytes);
    let (_, manifest_bytes) = missing_feature
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest fixture");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(manifest_bytes).expect("parse manifest");
    manifest
        .required_features
        .retain(|feature| feature != ORI2_FEATURE_EDITOR_HISTORY_V1);
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("serialize manifest");
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&missing_feature)),
        Err(FormatError::EditorHistoryFeatureDescriptorMismatch)
    ));

    let mut missing_descriptor = archive_entries(&bytes);
    let (_, manifest_bytes) = missing_descriptor
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest fixture");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(manifest_bytes).expect("parse manifest");
    manifest.editor_history = None;
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("serialize manifest");
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&missing_descriptor)),
        Err(FormatError::EditorHistoryFeatureDescriptorMismatch)
    ));
}

#[test]
fn history_entry_presence_must_match_the_authenticated_manifest() {
    let (document, bytes) = history_archive_fixture();
    let mut missing = archive_entries(&bytes);
    missing.retain(|(path, _)| path != ORI2_EDITOR_HISTORY_PATH);
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&missing)),
        Err(FormatError::MissingEntry {
            path: ORI2_EDITOR_HISTORY_PATH
        })
    ));

    let document_only = write_project_ori2(&document).expect("write document-only fixture");
    let mut orphan = archive_entries(&document_only);
    orphan.push((
        ORI2_EDITOR_HISTORY_PATH.to_owned(),
        br#"{"untrusted":true}"#.to_vec(),
    ));
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&orphan)),
        Err(FormatError::UnexpectedEditorHistoryEntry)
    ));
}

#[test]
fn layer_evidence_feature_descriptor_and_entry_presence_are_coherent() {
    let bytes = layer_evidence_archive_fixture(false);

    let mut missing_feature = archive_entries(&bytes);
    let manifest_bytes = &mut missing_feature
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest")
        .1;
    let mut manifest: Ori2Manifest = serde_json::from_slice(manifest_bytes).unwrap();
    manifest
        .required_features
        .retain(|feature| feature != ORI2_FEATURE_LAYER_EVIDENCE_V1);
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&missing_feature)),
        Err(FormatError::LayerEvidenceFeatureDescriptorMismatch)
    ));

    let mut missing_descriptor = archive_entries(&bytes);
    let manifest_bytes = &mut missing_descriptor
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .unwrap()
        .1;
    let mut manifest: Ori2Manifest = serde_json::from_slice(manifest_bytes).unwrap();
    manifest.layer_evidence = None;
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&missing_descriptor)),
        Err(FormatError::LayerEvidenceFeatureDescriptorMismatch)
    ));

    let mut missing_entry = archive_entries(&bytes);
    missing_entry.retain(|(path, _)| path != ORI2_LAYER_EVIDENCE_PATH);
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&missing_entry)),
        Err(FormatError::MissingEntry {
            path: ORI2_LAYER_EVIDENCE_PATH
        })
    ));

    let document_only = write_project_ori2(&sample_document()).unwrap();
    let mut surplus = archive_entries(&document_only);
    surplus.push((ORI2_LAYER_EVIDENCE_PATH.to_owned(), b"{}".to_vec()));
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&surplus)),
        Err(FormatError::UnexpectedLayerEvidenceEntry)
    ));
}

#[test]
fn layer_evidence_descriptor_size_hash_and_payload_are_authenticated() {
    let bytes = layer_evidence_archive_fixture(false);

    for (mutation, expected) in [
        ("path", "path"),
        ("schema", "schema"),
        ("size", "size"),
        ("hash", "hash"),
    ] {
        let mut entries = archive_entries(&bytes);
        let manifest_bytes = &mut entries
            .iter_mut()
            .find(|(path, _)| path == ORI2_MANIFEST_PATH)
            .unwrap()
            .1;
        let mut manifest: Ori2Manifest = serde_json::from_slice(manifest_bytes).unwrap();
        let descriptor = manifest.layer_evidence.as_mut().unwrap();
        match mutation {
            "path" => descriptor.path = "../layer-evidence.json".to_owned(),
            "schema" => descriptor.schema_version += 1,
            "size" => descriptor.uncompressed_size += 1,
            "hash" => descriptor.sha256 = "0".repeat(64),
            _ => unreachable!(),
        }
        *manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let error = read_project_archive_ori2(&raw_zip_owned(&entries)).unwrap_err();
        assert!(
            matches!(
                (expected, error),
                ("path", FormatError::InvalidManifestLayerEvidencePath { .. })
                    | (
                        "schema",
                        FormatError::UnsupportedLayerEvidenceSchemaVersion { .. }
                    )
                    | ("size", FormatError::LayerEvidenceSizeMismatch { .. })
                    | ("hash", FormatError::LayerEvidenceHashMismatch { .. })
            ),
            "unexpected {expected} error"
        );
    }

    let mut tampered = archive_entries(&bytes);
    tampered
        .iter_mut()
        .find(|(path, _)| path == ORI2_LAYER_EVIDENCE_PATH)
        .unwrap()
        .1[0] ^= 1;
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&tampered)),
        Err(FormatError::LayerEvidenceHashMismatch { .. })
    ));
}

#[test]
fn single_file_and_expanded_folder_preserve_layer_evidence_equivalently() {
    let document = sample_document();
    let archive = Ori2ProjectArchive {
        document,
        editor_history: None,
        layer_evidence: Some(layer_evidence_fixture()),
    };
    let single = write_project_archive_ori2(&archive).unwrap();
    let from_single = read_project_archive_ori2(&single).unwrap();
    let folder = crate::write_project_folder_v1(&from_single).unwrap();
    assert_eq!(folder.archive(), &archive);
    let single_again = write_project_archive_ori2(folder.archive()).unwrap();
    assert_eq!(read_project_archive_ori2(&single_again).unwrap(), archive);
}

#[test]
fn rejects_invalid_history_descriptor_path_schema_size_and_hash() {
    let (_, bytes) = history_archive_fixture();

    let mut bad_path = archive_entries(&bytes);
    let (_, manifest_bytes) = bad_path
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest fixture");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(manifest_bytes).expect("parse manifest");
    manifest
        .editor_history
        .as_mut()
        .expect("history descriptor")
        .path = "../editor-history.json".to_owned();
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("serialize manifest");
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&bad_path)),
        Err(FormatError::InvalidManifestEditorHistoryPath { .. })
    ));

    let mut bad_schema = archive_entries(&bytes);
    let (_, manifest_bytes) = bad_schema
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest fixture");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(manifest_bytes).expect("parse manifest");
    manifest
        .editor_history
        .as_mut()
        .expect("history descriptor")
        .schema_version += 1;
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("serialize manifest");
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&bad_schema)),
        Err(FormatError::UnsupportedEditorHistorySchemaVersion { .. })
    ));

    let mut bad_size = archive_entries(&bytes);
    let (_, manifest_bytes) = bad_size
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest fixture");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(manifest_bytes).expect("parse manifest");
    manifest
        .editor_history
        .as_mut()
        .expect("history descriptor")
        .uncompressed_size += 1;
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("serialize manifest");
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&bad_size)),
        Err(FormatError::EditorHistorySizeMismatch { .. })
    ));

    let mut bad_hash = archive_entries(&bytes);
    let (_, manifest_bytes) = bad_hash
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest fixture");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(manifest_bytes).expect("parse manifest");
    manifest
        .editor_history
        .as_mut()
        .expect("history descriptor")
        .sha256 = "0".repeat(64);
    *manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("serialize manifest");
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&bad_hash)),
        Err(FormatError::EditorHistoryHashMismatch { .. })
    ));
}

#[test]
fn rejects_history_rebound_to_different_project_bytes_or_identity() {
    let (_, bytes) = history_archive_fixture();

    let mut wrong_project_hash = archive_entries(&bytes);
    let (_, history_bytes) = wrong_project_hash
        .iter()
        .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
        .expect("history fixture");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(history_bytes).expect("parse history envelope");
    envelope["project_sha256"] = serde_json::Value::String("0".repeat(64));
    let history_bytes = serde_json::to_vec_pretty(&envelope).expect("serialize history envelope");
    reseal_history_entry(&mut wrong_project_hash, history_bytes);
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&wrong_project_hash)),
        Err(FormatError::EditorHistoryProjectHashMismatch)
    ));

    let mut wrong_project_id = archive_entries(&bytes);
    let (_, history_bytes) = wrong_project_id
        .iter()
        .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
        .expect("history fixture");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(history_bytes).expect("parse history envelope");
    envelope["history"]["project_id"] =
        serde_json::to_value(ori_domain::ProjectId::new()).expect("serialize project ID");
    let history_bytes = serde_json::to_vec_pretty(&envelope).expect("serialize history envelope");
    reseal_history_entry(&mut wrong_project_id, history_bytes);
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&wrong_project_id)),
        Err(FormatError::EditorHistoryProjectIdMismatch)
    ));
}

#[test]
fn history_envelope_is_strict_and_history_size_has_a_hard_ceiling() {
    let (_, bytes) = history_archive_fixture();
    let mut unknown_field = archive_entries(&bytes);
    let (_, history_bytes) = unknown_field
        .iter()
        .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
        .expect("history fixture");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(history_bytes).expect("parse history envelope");
    envelope
        .as_object_mut()
        .expect("history envelope object")
        .insert("future_field".to_owned(), serde_json::json!(true));
    let history_bytes = serde_json::to_vec_pretty(&envelope).expect("serialize history envelope");
    reseal_history_entry(&mut unknown_field, history_bytes);
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&unknown_field)),
        Err(FormatError::InvalidEditorHistoryJson(_))
    ));

    let mut unsupported_history_schema = archive_entries(&bytes);
    let (_, history_bytes) = unsupported_history_schema
        .iter()
        .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
        .expect("history fixture");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(history_bytes).expect("parse history envelope");
    envelope["history"]["schema_version"] = serde_json::json!(EDITOR_HISTORY_SCHEMA_VERSION_V1 + 1);
    let history_bytes = serde_json::to_vec_pretty(&envelope).expect("serialize history envelope");
    reseal_history_entry(&mut unsupported_history_schema, history_bytes);
    assert!(matches!(
        read_project_archive_ori2(&raw_zip_owned(&unsupported_history_schema)),
        Err(FormatError::InvalidEditorHistory(
            ori_core::EditorHistoryErrorV1::UnsupportedSchemaVersion
        ))
    ));

    let relaxed = Ori2Limits {
        max_entry_uncompressed_size: MAX_EDITOR_HISTORY_JSON_BYTES * 2,
        max_editor_history_size: MAX_EDITOR_HISTORY_JSON_BYTES * 2,
        ..Ori2Limits::default()
    };
    assert_eq!(
        effective_editor_history_entry_size_limit(relaxed),
        MAX_EDITOR_HISTORY_JSON_BYTES
    );
    ensure_editor_history_entry_size(MAX_EDITOR_HISTORY_JSON_BYTES, relaxed)
        .expect("equality with the history hard ceiling must succeed");
    assert!(matches!(
        ensure_editor_history_entry_size(MAX_EDITOR_HISTORY_JSON_BYTES + 1, relaxed),
        Err(FormatError::EntryTooLarge {
            path,
            actual,
            limit
        }) if path == ORI2_EDITOR_HISTORY_PATH
            && actual == MAX_EDITOR_HISTORY_JSON_BYTES + 1
            && limit == MAX_EDITOR_HISTORY_JSON_BYTES
    ));
}

#[test]
fn migrates_two_legacy_history_generations_and_resaves_canonically() {
    let original = version_matrix_archive_fixture();
    let bytes = write_project_archive_ori2(&original).expect("write version matrix fixture");

    for generation in 0..=2 {
        let mut entries = archive_entries(&bytes);
        let (_, history_bytes) = entries
            .iter()
            .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
            .expect("history fixture");
        let mut envelope: serde_json::Value =
            serde_json::from_slice(history_bytes).expect("parse history envelope");
        let history = envelope["history"].as_object_mut().expect("history object");
        if generation >= 1 {
            history.remove("redo_stack");
        }
        if generation >= 2 {
            history.remove("history_entry_limit");
        }
        reseal_history_entry(
            &mut entries,
            serde_json::to_vec_pretty(&envelope).expect("legacy history fixture"),
        );

        let migrated = read_project_archive_ori2(&raw_zip_owned(&entries))
            .expect("read legacy history generation");
        let migrated_history = migrated.editor_history.as_ref().expect("migrated history");
        assert_eq!(migrated.document, original.document);
        assert_eq!(
            migrated.document.texture_assets,
            original.document.texture_assets
        );
        assert_eq!(
            migrated.document.reference_model_assets,
            original.document.reference_model_assets
        );
        assert_eq!(migrated.layer_evidence, original.layer_evidence);
        assert_eq!(
            migrated
                .document
                .beginner_design_profile
                .generation_provenance
                .as_ref()
                .and_then(|value| value.fold_path_certificate_sha256),
            Some([0x62; 32]),
            "generation {generation} must preserve typed fold-path provenance"
        );
        assert_eq!(migrated_history.undo_len(), 1);
        assert_eq!(migrated_history.redo_len(), 0);
        assert_eq!(
            migrated_history.history_entry_limit(),
            if generation <= 1 {
                17
            } else {
                MAX_EDITOR_HISTORY_ENTRIES as u32
            }
        );

        let canonical = write_project_archive_ori2(&migrated).expect("canonical resave");
        let reread = read_project_archive_ori2(&canonical).expect("read canonical resave");
        assert_eq!(reread.document, migrated.document);
        assert_eq!(reread, migrated);
        assert_eq!(
            write_project_archive_ori2(&reread).expect("second canonical resave"),
            canonical,
            "generation {generation} resave must be byte-idempotent"
        );
    }
}

#[test]
fn ori2_version_policy_rejects_future_and_downgrade_writes() {
    for unsupported in [0, crate::CURRENT_FORMAT_VERSION + 1] {
        let mut archive = version_matrix_archive_fixture();
        archive.document.format_version = unsupported;
        assert!(matches!(
            write_project_archive_ori2(&archive),
            Err(FormatError::UnsupportedVersion { found, latest })
                if found == unsupported && latest == crate::CURRENT_FORMAT_VERSION
        ));
    }
}

#[test]
fn deterministic_history_tamper_corpus_rejects_192_payloads_without_state_change() {
    let (document, bytes) = history_archive_fixture();
    let valid_control = read_project_archive_ori2(&bytes).expect("valid control");
    assert!(valid_control.document.current_pose.is_none());
    assert!(
        valid_control
            .document
            .beginner_design_profile
            .generation_provenance
            .is_none()
    );
    assert!(
        valid_control
            .document
            .beginner_design_profile
            .reference_consensus_v1
            .is_none()
    );

    for generation in 0..3 {
        let mut generation_entries = archive_entries(&bytes);
        if generation > 0 {
            let history_bytes = generation_entries
                .iter()
                .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
                .expect("history fixture")
                .1
                .clone();
            let mut envelope: serde_json::Value =
                serde_json::from_slice(&history_bytes).expect("history envelope");
            envelope["history"]
                .as_object_mut()
                .expect("history object")
                .remove("redo_stack");
            if generation == 2 {
                envelope["history"]
                    .as_object_mut()
                    .expect("history object")
                    .remove("history_entry_limit");
            }
            reseal_history_entry(
                &mut generation_entries,
                serde_json::to_vec_pretty(&envelope).expect("legacy generation"),
            );
            let control = read_project_archive_ori2(&raw_zip_owned(&generation_entries))
                .expect("valid legacy control");
            assert_eq!(control.document, document);
        }

        for corpus_index in 0..64 {
            let mut tampered = generation_entries.clone();
            let history_bytes = tampered
                .iter()
                .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
                .expect("history fixture")
                .1
                .clone();
            let mut envelope: serde_json::Value =
                serde_json::from_slice(&history_bytes).expect("history envelope");
            let resealed = match corpus_index % 8 {
                0 => vec![0xff, 0xfe, corpus_index as u8],
                1 => format!("{{\"truncated\":{corpus_index}").into_bytes(),
                2 => {
                    envelope.as_object_mut().expect("envelope object").insert(
                        format!("unknown_required_{corpus_index}"),
                        serde_json::json!(true),
                    );
                    serde_json::to_vec(&envelope).expect("unknown envelope")
                }
                3 => {
                    envelope["history"]
                        .as_object_mut()
                        .expect("history object")
                        .insert(
                            format!("unknown_required_{corpus_index}"),
                            serde_json::json!(true),
                        );
                    serde_json::to_vec(&envelope).expect("unknown history")
                }
                4 => {
                    envelope["history"]["schema_version"] = serde_json::json!(
                        EDITOR_HISTORY_SCHEMA_VERSION_V1 + 1 + corpus_index as u32
                    );
                    serde_json::to_vec(&envelope).expect("future history")
                }
                5 => {
                    envelope["project_sha256"] =
                        serde_json::Value::String(format!("{:064x}", corpus_index + 1));
                    serde_json::to_vec(&envelope).expect("hash rebound")
                }
                6 => {
                    envelope["history"]
                        .as_object_mut()
                        .expect("history object")
                        .remove("undo_stack");
                    serde_json::to_vec(&envelope).expect("partial metadata group")
                }
                _ => {
                    let encoded = serde_json::to_string(&envelope).expect("history JSON");
                    encoded
                        .replacen(
                            "\"schema_version\":1",
                            "\"schema_version\":1,\"schema_version\":1",
                            1,
                        )
                        .into_bytes()
                }
            };
            reseal_history_entry(&mut tampered, resealed);
            assert!(
                read_project_archive_ori2(&raw_zip_owned(&tampered)).is_err(),
                "generation {generation}, corpus {corpus_index} must reject"
            );
            assert_eq!(
                read_project_archive_ori2(&bytes).expect("control remains readable"),
                valid_control,
                "rejection must not mutate the source project or recovery slot"
            );
        }
    }
}

#[test]
fn ori2_reader_and_writer_reject_the_nil_project_id_with_a_typed_error() {
    let mut document = sample_document();
    document.project_id = project_id_from_wire("00000000-0000-0000-0000-000000000000");

    assert!(matches!(
        write_project_ori2(&document),
        Err(FormatError::NilProjectId)
    ));

    let project = serde_json::to_vec(&document).expect("serialize nil-ID archive fixture");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);
    assert!(matches!(
        read_project_ori2(&bytes),
        Err(FormatError::NilProjectId)
    ));
}

#[test]
fn ori2_round_trip_accepts_non_nil_uuid_versions_and_variants() {
    for wire in [
        "10000000-0000-0000-0000-000000000001",
        "10000000-0000-1000-8000-000000000001",
        "10000000-0000-f000-c000-000000000001",
        "10000000-0000-7000-e000-000000000001",
    ] {
        let mut document = sample_document();
        document.project_id = project_id_from_wire(wire);

        let bytes = write_project_ori2(&document).expect("write non-nil project ID");
        let restored = read_project_ori2(&bytes).expect("read non-nil project ID");
        assert_eq!(restored.project_id, document.project_id, "{wire}");
    }
}

#[test]
fn writer_is_byte_deterministic_and_fixes_zip_metadata() {
    assert_eq!(
        ORI2_DEFLATE_LEVEL, 6,
        "container v1 fixes the compression level"
    );
    let mut original = sample_document();
    add_sample_instruction(&mut original);
    add_all_geometric_constraint_kinds(&mut original);

    let first = write_project_ori2(&original).expect("first .ori2 write");
    for attempt in 2..=3 {
        let actual = write_project_ori2(&original).expect("repeated .ori2 write");
        assert_eq!(
            actual, first,
            "write attempt {attempt} must produce identical archive bytes"
        );
    }

    let restored = read_project_ori2(&first).expect("read deterministic .ori2");
    let rewritten = write_project_ori2(&restored).expect("rewrite restored .ori2");
    assert_eq!(
        rewritten, first,
        "read then write must preserve the canonical archive bytes"
    );

    let mut archive = ZipArchive::new(Cursor::new(&first)).expect("open generated ZIP");
    let metadata = (0..archive.len())
        .map(|index| {
            let entry = archive.by_index(index).expect("generated entry");
            (
                entry.name().to_owned(),
                entry.compression(),
                entry.last_modified(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        metadata,
        [
            (
                ORI2_MANIFEST_PATH.to_owned(),
                CompressionMethod::Deflated,
                Some(DateTime::DEFAULT),
            ),
            (
                ORI2_PROJECT_PATH.to_owned(),
                CompressionMethod::Deflated,
                Some(DateTime::DEFAULT),
            ),
        ],
        "entry order, compression, and DOS timestamp are part of the .ori2 byte contract"
    );
}

#[test]
fn geometric_constraint_manifest_wire_and_digest_are_exact_v1_goldens() {
    const EXPECTED_MANIFEST: &str = r#"{
  "container": "ORIGAMI2",
  "container_version": 1,
  "required_features": [
    "geometric_constraints_v1"
  ],
  "project": {
    "path": "project.json",
    "format_version": 1,
    "uncompressed_size": 20,
    "sha256": "d88bf399e67c0574c03d47dd19ec99ebe1641083faa6688893cd902eb6051a3f"
  }
}"#;
    const EXPECTED_MANIFEST_SHA256: &str =
        "c9f787cb3ae0fc17d5d45857a2c0db9b773e5076880da75f0cad9e717014a7dd";
    let project = br#"{"format_version":1}"#;

    assert_eq!(
        ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1,
        "geometric_constraints_v1"
    );
    assert_eq!(project.len(), 20);
    let manifest = Ori2Manifest::new(
        project,
        crate::CURRENT_FORMAT_VERSION,
        vec![ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned()],
    );
    let wire = serde_json::to_string_pretty(&manifest).expect("serialize exact manifest");
    assert_eq!(wire, EXPECTED_MANIFEST);
    assert_eq!(sha256_hex(wire.as_bytes()), EXPECTED_MANIFEST_SHA256);
    assert_eq!(
        manifest.project.sha256,
        "d88bf399e67c0574c03d47dd19ec99ebe1641083faa6688893cd902eb6051a3f"
    );
    assert_eq!(
        Ori2Limits::default().max_project_size,
        MAX_PROJECT_JSON_BYTES as u64,
        "direct JSON and default .ori2 readers share the hard project ceiling"
    );
}

#[test]
fn ori2_round_trip_preserves_instructions_and_declares_required_feature() {
    let mut original = sample_document();
    add_sample_instruction(&mut original);

    let bytes = write_project_ori2(&original).expect("write instructions");
    let manifest = manifest_from_archive(&bytes);
    assert_eq!(
        manifest.required_features,
        vec![ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned()]
    );

    let restored = read_project_ori2(&bytes).expect("read instructions");
    assert_eq!(restored.instruction_timeline, original.instruction_timeline);
}

#[test]
fn ori2_round_trip_preserves_declarative_instructions_and_declares_both_features() {
    let mut original = sample_document();
    add_sample_declarative_instruction(&mut original);

    let bytes = write_project_ori2(&original).expect("write declarative instructions");
    let manifest = manifest_from_archive(&bytes);
    assert_eq!(
        manifest.required_features,
        vec![
            ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned(),
            ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1.to_owned(),
        ]
    );

    let restored = read_project_ori2(&bytes).expect("read declarative instructions");
    assert_eq!(restored.instruction_timeline, original.instruction_timeline);
    assert_eq!(
        restored.instruction_timeline.steps[0].pose.model,
        InstructionPoseModel::DeclarativeOnlyV1
    );
}

#[test]
fn ori2_preserves_numeric_expressions_and_declares_the_required_feature() {
    let mut original = sample_document();
    original.numeric_expressions.rectangular_paper_creation =
        Some(crate::RectangularPaperCreationExpressions::new(
            "200 * sqrt(2)",
            "400 / 3",
            282.842_712_474_619,
            133.333_333_333_333_34,
        ));

    let bytes = write_project_ori2(&original).expect("write expressions");
    let manifest = manifest_from_archive(&bytes);
    assert_eq!(
        manifest.required_features,
        vec![ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned()]
    );
    let restored = read_project_ori2(&bytes).expect("read expressions");
    assert_eq!(restored.numeric_expressions, original.numeric_expressions);
}

#[test]
fn deterministic_geometry_reference_declares_its_required_feature_and_model() {
    let mut original = sample_document();
    let edge = serde_json::to_value(original.crease_pattern.edges[0].id)
        .expect("edge ID")
        .as_str()
        .expect("wire edge ID")
        .to_owned();
    let vertex = original.crease_pattern.vertices[0].clone();
    let binding =
        crate::VertexCoordinateExpressions::new_with_geometry_reference_model_support_for_test(
            vertex.id,
            format!("e.{edge}.length"),
            vertex.position.y.to_string(),
            vertex.position.x,
            vertex.position.y,
            true,
        );
    original
        .numeric_expressions
        .vertex_coordinates
        .push(binding.clone());

    let bytes = write_project_ori2(&original).expect("write deterministic geometry reference");
    assert_eq!(
        manifest_from_archive(&bytes).required_features,
        vec![
            ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned(),
            ORI2_FEATURE_DETERMINISTIC_GEOMETRY_REFERENCES_V2.to_owned(),
        ]
    );
    assert_eq!(
        read_project_ori2(&bytes)
            .expect("read deterministic geometry reference")
            .numeric_expressions,
        original.numeric_expressions
    );

    let mut history_only = original;
    history_only.numeric_expressions.vertex_coordinates.clear();
    history_only
        .numeric_expressions
        .vertex_undo_stack
        .push(Some(crate::VertexCoordinateExpressionTransition {
            changes: vec![crate::VertexCoordinateExpressionChange {
                vertex: binding.vertex,
                before: None,
                after: Some(binding),
            }],
        }));
    let history_bytes =
        write_project_ori2(&history_only).expect("write deterministic geometry reference history");
    assert_eq!(
        manifest_from_archive(&history_bytes).required_features,
        vec![
            ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned(),
            ORI2_FEATURE_DETERMINISTIC_GEOMETRY_REFERENCES_V2.to_owned(),
        ]
    );
}

#[test]
fn ori2_preserves_all_constraint_kinds_and_declares_the_required_feature() {
    let mut original = sample_document();
    add_all_geometric_constraint_kinds(&mut original);

    let bytes = write_project_ori2(&original).expect("write constraints");
    let manifest = manifest_from_archive(&bytes);
    assert_eq!(
        manifest.required_features,
        vec![ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned()]
    );

    let restored = read_project_ori2(&bytes).expect("read constraints");
    assert_eq!(restored, original);
    assert_eq!(restored.geometric_constraints.constraints.len(), 11);
}

#[test]
fn writer_uses_fixed_required_feature_order_for_combined_project() {
    let mut document = sample_document();
    add_sample_instruction(&mut document);
    document.numeric_expressions.rectangular_paper_creation = Some(
        crate::RectangularPaperCreationExpressions::new("400", "400", 400.0, 400.0),
    );
    add_all_geometric_constraint_kinds(&mut document);
    let layer = LayerRecordV1 {
        id: LayerId::new(),
        name: "Details".to_owned(),
        content_kind: LayerContentKindV1::CreasePattern,
        visible: true,
        locked: false,
        opacity: 1.0,
    };
    document.layers.layers.push(layer);

    let bytes = write_project_ori2(&document).expect("write combined project");
    assert_eq!(
        manifest_from_archive(&bytes).required_features,
        vec![
            ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned(),
            ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned(),
            ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned(),
            ORI2_FEATURE_LAYERS_V1.to_owned(),
        ],
        "writer feature order is part of deterministic container v1"
    );
}

#[test]
fn ori2_without_instructions_does_not_require_timeline_feature() {
    let bytes = write_project_ori2(&sample_document()).expect("write project");
    let manifest = manifest_from_archive(&bytes);
    assert!(manifest.required_features.is_empty());
}

#[test]
fn reads_legacy_ori2_without_instruction_timeline_field() {
    let document = sample_document();
    let mut value = serde_json::to_value(&document).expect("serialize document");
    value
        .as_object_mut()
        .expect("project object")
        .remove("instruction_timeline");
    let project = serde_json::to_vec(&value).expect("serialize legacy project");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let restored = read_project_ori2(&bytes).expect("read legacy .ori2");
    assert!(restored.instruction_timeline.steps.is_empty());
}

#[test]
fn reads_legacy_ori2_without_numeric_expressions() {
    let document = sample_document();
    let mut value = serde_json::to_value(&document).expect("serialize document");
    value
        .as_object_mut()
        .expect("project object")
        .remove("numeric_expressions");
    let project = serde_json::to_vec(&value).expect("serialize legacy project");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let restored = read_project_ori2(&bytes).expect("read legacy .ori2");
    assert!(restored.numeric_expressions.is_empty());
}

#[test]
fn reads_legacy_ori2_without_geometric_constraints() {
    let document = sample_document();
    let mut value = serde_json::to_value(&document).expect("serialize document");
    value
        .as_object_mut()
        .expect("project object")
        .remove("geometric_constraints");
    let project = serde_json::to_vec(&value).expect("serialize legacy project");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let restored = read_project_ori2(&bytes).expect("read legacy .ori2");
    assert!(restored.geometric_constraints.is_empty());
    let rewritten = write_project_ori2(&restored).expect("rewrite legacy .ori2");
    assert!(
        !manifest_from_archive(&rewritten)
            .required_features
            .iter()
            .any(|feature| feature == ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1)
    );
}

#[test]
fn manifest_defaults_legacy_features_but_rejects_unknown_envelope_fields() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    let base = serde_json::to_value(Ori2Manifest::new(
        &project,
        crate::CURRENT_FORMAT_VERSION,
        vec![],
    ))
    .expect("serialize manifest value");

    let mut legacy = base.clone();
    legacy
        .as_object_mut()
        .expect("manifest object")
        .remove("required_features");
    let legacy_manifest = serde_json::to_vec(&legacy).expect("serialize legacy manifest");
    let legacy_archive = raw_zip(&[
        (ORI2_MANIFEST_PATH, &legacy_manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);
    read_project_ori2(&legacy_archive).expect("legacy missing feature set defaults to empty");

    let mut unknown_manifest = base.clone();
    unknown_manifest
        .as_object_mut()
        .expect("manifest object")
        .insert("future_container_field".to_owned(), serde_json::json!(true));
    let unknown_manifest =
        serde_json::to_vec(&unknown_manifest).expect("serialize unknown manifest field");
    let unknown_manifest_archive = raw_zip(&[
        (ORI2_MANIFEST_PATH, &unknown_manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);
    assert!(matches!(
        read_project_ori2(&unknown_manifest_archive),
        Err(FormatError::InvalidManifestJson(_))
    ));

    let mut unknown_project_entry = base;
    unknown_project_entry["project"]
        .as_object_mut()
        .expect("project entry object")
        .insert("future_project_field".to_owned(), serde_json::json!(true));
    let unknown_project_entry =
        serde_json::to_vec(&unknown_project_entry).expect("serialize unknown project field");
    let unknown_project_archive = raw_zip(&[
        (ORI2_MANIFEST_PATH, &unknown_project_entry),
        (ORI2_PROJECT_PATH, &project),
    ]);
    assert!(matches!(
        read_project_ori2(&unknown_project_archive),
        Err(FormatError::InvalidManifestJson(_))
    ));
}

#[test]
fn reads_legacy_ori2_without_length_display_unit_as_millimetres() {
    let document = sample_document();
    let mut value = serde_json::to_value(&document).expect("serialize document");
    value["paper"]
        .as_object_mut()
        .expect("paper object")
        .remove("length_display_unit");
    let project = serde_json::to_vec(&value).expect("serialize legacy project");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let restored = read_project_ori2(&bytes).expect("read legacy .ori2");
    assert_eq!(
        restored.paper.length_display_unit,
        ori_domain::LengthDisplayUnit::Millimeter
    );
}

#[test]
fn required_features_are_canonical_and_unknown_errors_are_canonical() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    for features in [
        vec![ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned()],
        vec![
            ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned(),
            ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned(),
        ],
    ] {
        let manifest = manifest_for_features(&project, features);
        let bytes = raw_zip(&[
            (ORI2_MANIFEST_PATH, &manifest),
            (ORI2_PROJECT_PATH, &project),
        ]);
        assert!(matches!(
            read_project_ori2(&bytes),
            Err(FormatError::RequiredFeaturesMismatch { .. })
        ));
    }

    let unknown_manifest = manifest_for_features(
        &project,
        vec![
            "future_z_solver_v9".to_owned(),
            ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned(),
            "future_a_solver_v2".to_owned(),
            "future_z_solver_v9".to_owned(),
            "future_a_solver_v2".to_owned(),
        ],
    );
    let unknown_bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &unknown_manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);
    let error = read_project_ori2(&unknown_bytes).expect_err("unknown required feature must fail");
    assert!(matches!(
        error,
        FormatError::UnsupportedRequiredFeatures { features }
            if features
                == vec![
                    "future_a_solver_v2".to_owned(),
                    "future_z_solver_v9".to_owned(),
                ]
    ));
}

#[test]
fn rejects_instruction_content_without_required_manifest_feature() {
    let mut document = sample_document();
    add_sample_instruction(&mut document);
    let project = write_project_json(&document).expect("project JSON");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes).expect_err("timeline feature declaration is required");
    assert!(matches!(
        error,
        FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_INSTRUCTION_TIMELINE_V1
        }
    ));
}

#[test]
fn rejects_declarative_instruction_without_its_dedicated_required_feature() {
    let mut document = sample_document();
    add_sample_declarative_instruction(&mut document);
    let project = write_project_json(&document).expect("project JSON");
    let manifest = manifest_for_features(
        &project,
        vec![ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned()],
    );
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes)
        .expect_err("declarative instruction feature declaration is required");
    assert!(matches!(
        error,
        FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1
        }
    ));
}

#[test]
fn rejects_numeric_expression_content_without_required_manifest_feature() {
    let mut document = sample_document();
    document.numeric_expressions.rectangular_paper_creation = Some(
        crate::RectangularPaperCreationExpressions::new("400", "400", 400.0, 400.0),
    );
    let project = write_project_json(&document).expect("project JSON");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes).expect_err("numeric-expression feature is required");
    assert!(matches!(
        error,
        FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1
        }
    ));
}

#[test]
fn rejects_geometric_constraint_content_without_required_manifest_feature() {
    let mut document = sample_document();
    add_all_geometric_constraint_kinds(&mut document);
    let project = write_project_json(&document).expect("project JSON");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes).expect_err("geometric-constraint feature is required");
    assert!(matches!(
        error,
        FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1
        }
    ));
}

#[test]
fn ori2_reader_and_writer_reject_invalid_constraint_metadata_and_references() {
    let mut invalid_metadata = sample_document();
    add_all_geometric_constraint_kinds(&mut invalid_metadata);
    invalid_metadata.geometric_constraints.schema_version += 1;
    assert!(matches!(
        write_project_ori2(&invalid_metadata),
        Err(FormatError::InvalidGeometricConstraints(
            ori_domain::GeometricConstraintDocumentValidationErrorV1::UnsupportedSchemaVersion { .. }
        ))
    ));
    let project =
        serde_json::to_vec(&invalid_metadata).expect("serialize invalid metadata fixture");
    let manifest = manifest_for_features(
        &project,
        vec![ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned()],
    );
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);
    assert!(matches!(
        read_project_ori2(&bytes),
        Err(FormatError::InvalidGeometricConstraints(
            ori_domain::GeometricConstraintDocumentValidationErrorV1::UnsupportedSchemaVersion { .. }
        ))
    ));

    let mut dangling = sample_document();
    add_all_geometric_constraint_kinds(&mut dangling);
    let missing = EdgeId::new();
    let GeometricConstraintKindV1::FixedLength { edge, .. } =
        &mut dangling.geometric_constraints.constraints[0].constraint
    else {
        panic!("first fixture is fixed length");
    };
    *edge = missing;
    assert!(matches!(
        write_project_ori2(&dangling),
        Err(FormatError::MissingConstraintEdge { edge, .. }) if edge == missing
    ));
    let project = serde_json::to_vec(&dangling).expect("serialize dangling fixture");
    let manifest = manifest_for_features(
        &project,
        vec![ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned()],
    );
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);
    assert!(matches!(
        read_project_ori2(&bytes),
        Err(FormatError::MissingConstraintEdge { edge, .. }) if edge == missing
    ));
}

#[test]
fn rejects_invalid_instruction_timeline_inside_ori2() {
    let mut document = sample_document();
    add_sample_instruction(&mut document);
    document.instruction_timeline.steps[0].duration_ms = 0;
    let project =
        serde_json::to_vec(&document).expect("serialize invalid project directly for test");
    let manifest = manifest_for_features(
        &project,
        vec![ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned()],
    );
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes).expect_err("invalid timeline must fail");
    assert!(matches!(
        error,
        FormatError::InvalidInstructionTimeline(
            ori_domain::InstructionTimelineValidationError::DurationOutOfRange {
                step_index: 0,
                ..
            }
        )
    ));
}

#[test]
fn ori2_round_trip_preserves_complete_paper_definition() {
    let mut original = sample_document();
    let front_texture = AssetId::new();
    let back_texture = AssetId::new();
    original.paper = Paper {
        boundary_vertices: original
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect(),
        thickness_mm: 0.075,
        length_display_unit: ori_domain::LengthDisplayUnit::PaperEdgeRatio {
            reference_edge: original.crease_pattern.edges[0].id,
        },
        cutting_allowed: true,
        front: PaperAppearance {
            color: RgbaColor {
                red: 210,
                green: 45,
                blue: 80,
                alpha: 250,
            },
            texture_asset: Some(front_texture),
        },
        back: PaperAppearance {
            color: RgbaColor {
                red: 250,
                green: 248,
                blue: 235,
                alpha: 255,
            },
            texture_asset: Some(back_texture),
        },
    };
    original.texture_assets = vec![
        crate::ProjectTextureAssetV1 {
            id: front_texture,
            media_type: crate::ProjectTextureMediaTypeV1::Png,
            bytes: b"\x89PNG\r\n\x1a\nfront".to_vec(),
        },
        crate::ProjectTextureAssetV1 {
            id: back_texture,
            media_type: crate::ProjectTextureMediaTypeV1::Jpeg,
            bytes: vec![0xff, 0xd8, b'b', b'a', b'c', b'k', 0xff, 0xd9],
        },
    ];

    let bytes = write_project_ori2(&original).expect("write .ori2 with paper");
    let restored = read_project_ori2(&bytes).expect("read .ori2 with paper");

    assert_eq!(restored.paper, original.paper);
    assert_eq!(
        restored.paper.length_display_unit,
        original.paper.length_display_unit
    );
    assert_eq!(restored.paper.front.texture_asset, Some(front_texture));
    assert_eq!(restored.paper.back.texture_asset, Some(back_texture));
    assert_eq!(restored.texture_assets, original.texture_assets);
}

#[test]
fn rejects_bytes_that_are_not_a_zip_archive() {
    let error = read_project_ori2(b"not a ZIP archive").expect_err("invalid ZIP must fail");
    assert!(matches!(error, FormatError::InvalidZipFooter));
}

#[test]
fn rejects_missing_required_entry() {
    let bytes = raw_zip(&[(ORI2_MANIFEST_PATH, b"{}")]);
    let error = read_project_ori2(&bytes).expect_err("missing project must fail");
    assert!(matches!(
        error,
        FormatError::MissingEntry {
            path: ORI2_PROJECT_PATH
        }
    ));
}

#[test]
fn rejects_duplicate_entry_names() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    let manifest = manifest_for(&project);
    let mut bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
        ("duplicate.js", &project),
    ]);
    assert_eq!(
        replace_zip_entry_name(&mut bytes, b"duplicate.js", b"project.json"),
        2,
        "local and central ZIP names should be replaced"
    );
    let error = read_project_ori2(&bytes).expect_err("duplicate path must fail");
    assert!(matches!(
        error,
        FormatError::ArchiveEntryCountMismatch {
            declared: 3,
            parsed: 2
        }
    ));
}

#[test]
fn rejects_path_traversal_even_in_an_unknown_entry() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    let manifest = manifest_for(&project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
        ("assets/../../escape.txt", b"escape"),
    ]);
    let error = read_project_ori2(&bytes).expect_err("traversal path must fail");
    assert!(matches!(
        error,
        FormatError::UnsafeEntryPath { path } if path == "assets/../../escape.txt"
    ));
}

#[test]
fn rejects_archive_larger_than_configured_limit() {
    let bytes = write_project_ori2(&sample_document()).expect("write .ori2");
    let limits = Ori2Limits {
        max_archive_size: bytes.len() as u64 - 1,
        ..Ori2Limits::default()
    };
    let error =
        read_project_ori2_with_limits(&bytes, limits).expect_err("oversized archive must fail");
    assert!(matches!(error, FormatError::ContainerTooLarge { .. }));
}

#[test]
fn writer_rejects_project_larger_than_configured_limit() {
    let limits = Ori2Limits {
        max_project_size: 1,
        ..Ori2Limits::default()
    };
    let error = write_project_ori2_with_limits(&sample_document(), limits)
        .expect_err("writer must enforce project limit");
    assert!(matches!(
        error,
        FormatError::EntryTooLarge { path, .. } if path == ORI2_PROJECT_PATH
    ));
}

#[test]
fn relaxed_ori2_limits_cannot_exceed_the_project_json_hard_ceiling() {
    let relaxed = Ori2Limits {
        max_archive_size: 256 * 1024 * 1024,
        max_entry_uncompressed_size: 256 * 1024 * 1024,
        max_total_uncompressed_size: 512 * 1024 * 1024,
        max_project_size: 256 * 1024 * 1024,
        ..Ori2Limits::default()
    };

    let bytes = write_project_ori2_with_limits(&sample_document(), relaxed)
        .expect("a normal project remains writable with relaxed caller limits");
    read_project_ori2_with_limits(&bytes, relaxed)
        .expect("a writer artifact remains readable with the same limits");

    assert_eq!(
        effective_project_entry_size_limit(relaxed),
        MAX_PROJECT_JSON_BYTES as u64
    );
    ensure_project_entry_size(MAX_PROJECT_JSON_BYTES as u64, relaxed)
        .expect("equality with the hard ceiling must succeed");
    assert!(matches!(
        ensure_project_entry_size(MAX_PROJECT_JSON_BYTES as u64 + 1, relaxed),
        Err(FormatError::EntryTooLarge {
            path,
            actual,
            limit
        }) if path == ORI2_PROJECT_PATH
            && actual == MAX_PROJECT_JSON_BYTES as u64 + 1
            && limit == MAX_PROJECT_JSON_BYTES as u64
    ));
}

#[test]
fn rejects_uncompressed_entry_larger_than_configured_limit() {
    let bytes = write_project_ori2(&sample_document()).expect("write .ori2");
    let limits = Ori2Limits {
        max_entry_uncompressed_size: 8,
        ..Ori2Limits::default()
    };
    let error = read_project_ori2_with_limits(&bytes, limits)
        .expect_err("oversized expanded entry must fail");
    assert!(matches!(error, FormatError::EntryTooLarge { .. }));
}

#[test]
fn rejects_total_uncompressed_size_larger_than_configured_limit() {
    let bytes = write_project_ori2(&sample_document()).expect("write .ori2");
    let limits = Ori2Limits {
        max_total_uncompressed_size: 1,
        ..Ori2Limits::default()
    };
    let error = read_project_ori2_with_limits(&bytes, limits)
        .expect_err("oversized expanded archive must fail");
    assert!(matches!(error, FormatError::ExpandedArchiveTooLarge { .. }));
}

#[test]
fn rejects_project_whose_checksum_does_not_match_manifest() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    let mut manifest = Ori2Manifest::new(&project, crate::CURRENT_FORMAT_VERSION, Vec::new());
    manifest.project.sha256 = "0".repeat(64);
    let manifest = serde_json::to_vec(&manifest).expect("manifest JSON");
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes).expect_err("bad checksum must fail");
    assert!(matches!(error, FormatError::ProjectHashMismatch { .. }));
}

#[test]
fn rejects_project_whose_size_does_not_match_manifest() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    let mut manifest = Ori2Manifest::new(&project, crate::CURRENT_FORMAT_VERSION, Vec::new());
    manifest.project.uncompressed_size += 1;
    let manifest = serde_json::to_vec(&manifest).expect("manifest JSON");
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes).expect_err("bad project size must fail");
    assert!(matches!(error, FormatError::ProjectSizeMismatch { .. }));
}

#[test]
fn rejects_malformed_manifest_json() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, b"{not-json"),
        (ORI2_PROJECT_PATH, &project),
    ]);
    let error = read_project_ori2(&bytes).expect_err("bad manifest must fail");
    assert!(matches!(error, FormatError::InvalidManifestJson(_)));
}

#[test]
fn rejects_manifest_project_path_traversal() {
    let project = write_project_json(&sample_document()).expect("project JSON");
    let mut manifest = Ori2Manifest::new(&project, crate::CURRENT_FORMAT_VERSION, Vec::new());
    manifest.project.path = "../project.json".to_owned();
    let manifest = serde_json::to_vec(&manifest).expect("manifest JSON");
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, &project),
    ]);

    let error = read_project_ori2(&bytes).expect_err("manifest traversal must fail");
    assert!(matches!(
        error,
        FormatError::InvalidManifestProjectPath { .. }
    ));
}

#[test]
fn rejects_corrupted_project_json_after_integrity_checks() {
    let project = b"{not-json";
    let manifest = manifest_for(project);
    let bytes = raw_zip(&[
        (ORI2_MANIFEST_PATH, &manifest),
        (ORI2_PROJECT_PATH, project),
    ]);
    let error = read_project_ori2(&bytes).expect_err("bad project JSON must fail");
    assert!(matches!(error, FormatError::InvalidJson(_)));
}

#[test]
fn authored_layers_round_trip_with_required_feature_and_default_stays_legacy() {
    let default_bytes = write_project_ori2(&sample_document()).expect("write default project");
    assert!(
        !manifest_from_archive(&default_bytes)
            .required_features
            .contains(&ORI2_FEATURE_LAYERS_V1.to_owned())
    );

    let mut document = sample_document();
    let edge = document.crease_pattern.edges[0].id;
    let layer = LayerRecordV1 {
        id: LayerId::new(),
        name: "Details".to_owned(),
        content_kind: LayerContentKindV1::CreasePattern,
        visible: false,
        locked: true,
        opacity: 0.35,
    };
    document.layers.layers.push(layer.clone());
    document
        .layers
        .edge_assignments
        .push(EdgeLayerAssignmentV1 {
            edge,
            layer: layer.id,
        });

    let bytes = write_project_ori2(&document).expect("write layered project");
    assert_eq!(
        manifest_from_archive(&bytes).required_features,
        vec![ORI2_FEATURE_LAYERS_V1.to_owned()]
    );
    assert_eq!(
        read_project_ori2(&bytes).expect("read layered project"),
        document
    );
}

#[test]
fn ori2_accepts_legacy_layer_records_without_presentation_fields() {
    let mut document = sample_document();
    let legacy_layer = LayerRecordV1 {
        id: LayerId::new(),
        name: "Legacy details".to_owned(),
        content_kind: LayerContentKindV1::CreasePattern,
        visible: true,
        locked: false,
        opacity: 1.0,
    };
    document.layers.layers.push(legacy_layer.clone());

    let bytes = write_project_ori2(&document).expect("write legacy-shaped layer archive");
    let entries = archive_entries(&bytes);
    let project_bytes = &entries
        .iter()
        .find(|(path, _)| path == ORI2_PROJECT_PATH)
        .expect("project JSON entry")
        .1;
    let project: serde_json::Value = serde_json::from_slice(project_bytes).expect("project JSON");
    let legacy_layer_id = serde_json::to_value(legacy_layer.id).expect("serialized layer ID");
    let serialized_layer = project["layers"]["layers"]
        .as_array()
        .expect("layer records")
        .iter()
        .find(|value| value["id"] == legacy_layer_id)
        .expect("legacy layer record");
    assert!(serialized_layer.get("visible").is_none());
    assert!(serialized_layer.get("locked").is_none());
    assert!(serialized_layer.get("opacity").is_none());

    let restored = read_project_ori2(&bytes).expect("read legacy layer archive");
    let restored_layer = restored
        .layers
        .layers
        .iter()
        .find(|record| record.id == legacy_layer.id)
        .expect("restored legacy layer");
    assert!(restored_layer.visible);
    assert!(!restored_layer.locked);
    assert_eq!(restored_layer.opacity, 1.0);
}

#[test]
fn layered_project_without_required_manifest_feature_is_rejected() {
    let mut document = sample_document();
    document.layers.layers.push(LayerRecordV1 {
        id: LayerId::new(),
        name: "Notes".to_owned(),
        content_kind: LayerContentKindV1::Annotation,
        visible: true,
        locked: false,
        opacity: 1.0,
    });
    let bytes = write_project_ori2(&document).expect("write layered project");
    let mut entries = archive_entries(&bytes);
    let manifest_entry = entries
        .iter_mut()
        .find(|(path, _)| path == ORI2_MANIFEST_PATH)
        .expect("manifest entry");
    let mut manifest: Ori2Manifest =
        serde_json::from_slice(&manifest_entry.1).expect("manifest JSON");
    manifest
        .required_features
        .retain(|feature| feature != ORI2_FEATURE_LAYERS_V1);
    manifest_entry.1 = serde_json::to_vec_pretty(&manifest).expect("modified manifest");

    assert!(matches!(
        read_project_ori2(&raw_zip_owned(&entries)),
        Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_LAYERS_V1
        })
    ));
}

#[test]
fn authenticated_archive_restores_operational_layer_history() {
    let mut document = sample_document();
    let edge = document.crease_pattern.edges[0].id;
    let layer = LayerRecordV1 {
        id: LayerId::new(),
        name: "Details".to_owned(),
        content_kind: LayerContentKindV1::CreasePattern,
        visible: true,
        locked: false,
        opacity: 1.0,
    };
    let mut editor = EditorState::with_document_parts_constraints_and_layers(
        document.crease_pattern.clone(),
        document.paper.clone(),
        document.instruction_timeline.clone(),
        document.geometric_constraints.clone(),
        document.layers.clone(),
    );
    editor
        .execute(
            0,
            Command::CreateLayer {
                layer: layer.clone(),
                target_index: 1,
            },
        )
        .expect("create layer");
    editor
        .execute(
            1,
            Command::AssignEdgeToLayer {
                edge,
                layer: layer.id,
            },
        )
        .expect("assign edge");
    editor
        .execute(
            2,
            Command::UpdateLayerPresentation {
                layer: layer.id,
                visible: false,
                locked: true,
                opacity: 0.35,
            },
        )
        .expect("persist layer presentation");
    document.layers = editor.project_layers().clone();
    let archive = Ori2ProjectArchive {
        layer_evidence: None,
        editor_history: Some(
            editor
                .export_history_v1(document.project_id)
                .expect("export layer history"),
        ),
        document: document.clone(),
    };

    let bytes = write_project_archive_ori2(&archive).expect("write layer-history archive");
    let manifest = manifest_from_archive(&bytes);
    assert_eq!(
        manifest.required_features,
        vec![
            ORI2_FEATURE_LAYERS_V1.to_owned(),
            ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned(),
        ]
    );
    let restored = read_project_archive_ori2(&bytes).expect("read layer-history archive");
    let mut reopened = EditorState::with_document_parts_layers_and_history_v1(
        restored.document.crease_pattern.clone(),
        restored.document.paper.clone(),
        restored.document.instruction_timeline.clone(),
        restored.document.geometric_constraints.clone(),
        restored.document.layers.clone(),
        restored.editor_history.expect("authenticated history"),
    )
    .expect("restore operational layer history");
    assert_eq!(reopened.project_layers().layer_for_edge(edge), layer.id);
    assert_eq!(
        reopened
            .project_layers()
            .layers
            .iter()
            .find(|record| record.id == layer.id)
            .map(|record| (record.visible, record.locked, record.opacity)),
        Some((false, true, 0.35))
    );
    reopened.undo(0).expect("undo reopened presentation");
    assert_eq!(
        reopened
            .project_layers()
            .layers
            .iter()
            .find(|record| record.id == layer.id)
            .map(|record| (record.visible, record.locked, record.opacity)),
        Some((true, false, 1.0))
    );
    reopened.undo(1).expect("undo reopened assignment");
    assert_eq!(
        reopened.project_layers().layer_for_edge(edge),
        ori_domain::DEFAULT_PROJECT_LAYER_ID
    );
    reopened.undo(2).expect("undo reopened layer creation");
    assert_eq!(
        reopened.project_layers(),
        &ori_domain::ProjectLayerDocumentV1::default()
    );
}

#[test]
fn reference_model_assets_require_their_manifest_feature() {
    let mut document = sample_document();
    document
        .reference_model_assets
        .push(crate::ProjectReferenceModelAssetV1 {
            id: AssetId::new(),
            bytes: minimal_reference_glb(),
        });
    let archive = write_project_ori2(&document).expect("write reference model");
    let manifest = manifest_from_archive(&archive);
    assert!(
        manifest
            .required_features
            .contains(&ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1.to_owned())
    );
    assert_eq!(
        read_project_ori2(&archive)
            .expect("read reference model")
            .reference_model_assets,
        document.reference_model_assets
    );

    let project = write_project_json(&document).expect("project fixture");
    let missing_manifest = manifest_for(&project);
    let missing = raw_zip(&[
        (ORI2_MANIFEST_PATH, missing_manifest.as_slice()),
        (ORI2_PROJECT_PATH, project.as_slice()),
    ]);
    assert!(matches!(
        read_project_ori2(&missing),
        Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1
        })
    ));
}
