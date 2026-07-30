//! Safe reader and writer for the single-file `.ori2` container.
//!
//! Container version 1 deliberately rejects multi-disk and ZIP64 archives;
//! its resource limits are well below the thresholds that require either.

use std::io::{Cursor, Read, Write};

use ori_core::{EDITOR_HISTORY_SCHEMA_VERSION_V1, EditorHistoryV1, MAX_EDITOR_HISTORY_ENTRIES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    FormatError, MAX_PROJECT_JSON_BYTES, ProjectDocument, read_project_json, write_project_json,
};

pub const ORI2_CONTAINER_IDENTIFIER: &str = "ORIGAMI2";
pub const CURRENT_ORI2_CONTAINER_VERSION: u32 = 1;
pub const ORI2_MANIFEST_PATH: &str = "manifest.json";
pub const ORI2_PROJECT_PATH: &str = "project.json";
pub const ORI2_EDITOR_HISTORY_PATH: &str = "editor-history.json";
pub const ORI2_LAYER_EVIDENCE_PATH: &str = "layer-evidence.json";
pub const ORI2_FEATURE_INSTRUCTION_TIMELINE_V1: &str = "instruction_timeline_v1";
pub const ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1: &str = "declarative_instruction_steps_v1";
pub const ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1: &str = "numeric_expressions_v1";
pub const ORI2_FEATURE_DETERMINISTIC_GEOMETRY_REFERENCES_V2: &str =
    "deterministic_geometry_references_v2";
pub const ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1: &str = "geometric_constraints_v1";
pub const ORI2_FEATURE_LAYERS_V1: &str = "layers_v1";
pub const ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1: &str = "reference_model_assets_v1";
pub const ORI2_FEATURE_EDITOR_HISTORY_V1: &str = "editor_history_v1";
pub const ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1: &str = "speculative_unproven_fold_v1";
pub const ORI2_FEATURE_LAYER_EVIDENCE_V1: &str = "layer_evidence_v1";
pub const MAX_EDITOR_HISTORY_JSON_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_LAYER_EVIDENCE_JSON_BYTES_V1: usize = 16 * 1024 * 1024;
pub const LAYER_EVIDENCE_SCHEMA_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEvidenceArchiveV1 {
    pub version: u32,
    pub project_instance_id: String,
    pub project_id: String,
    pub revision: u64,
    pub fold_model_fingerprint_sha256: String,
    pub evidence: LayerEvidenceArchiveKindV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerEvidenceArchiveKindV1 {
    Flat {
        canonical_snapshot_json: String,
    },
    NonFlat {
        fixed_face: Option<String>,
        hinge_angles: Vec<LayerEvidenceHingeAngleV1>,
        material_faces: Vec<LayerEvidenceFaceV1>,
        cells: Vec<LayerEvidenceCellV1>,
        pair_orders: Vec<LayerEvidencePairOrderV1>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEvidenceHingeAngleV1 {
    pub edge: String,
    pub angle_degrees: f64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEvidenceFaceV1 {
    pub face_id: String,
    pub face_key_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEvidenceCellV1 {
    pub boundary_xy: Vec<[f64; 2]>,
    pub lower_face: String,
    pub upper_face: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEvidencePairOrderV1 {
    pub lower_face: String,
    pub upper_face: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayerEvidenceArchiveErrorV1 {
    #[error("layer evidence exceeds its byte limit")]
    Oversize,
    #[error("layer evidence JSON is invalid")]
    InvalidJson,
    #[error("layer evidence binding or bounded collection is invalid")]
    InvalidEvidence,
}

pub fn write_layer_evidence_archive_v1(
    value: &LayerEvidenceArchiveV1,
) -> Result<Vec<u8>, LayerEvidenceArchiveErrorV1> {
    validate_layer_evidence_archive_v1(value)?;
    let bytes = serde_json::to_vec(value).map_err(|_| LayerEvidenceArchiveErrorV1::InvalidJson)?;
    if bytes.len() > MAX_LAYER_EVIDENCE_JSON_BYTES_V1 {
        return Err(LayerEvidenceArchiveErrorV1::Oversize);
    }
    Ok(bytes)
}

pub fn read_layer_evidence_archive_v1(
    bytes: &[u8],
) -> Result<LayerEvidenceArchiveV1, LayerEvidenceArchiveErrorV1> {
    if bytes.len() > MAX_LAYER_EVIDENCE_JSON_BYTES_V1 {
        return Err(LayerEvidenceArchiveErrorV1::Oversize);
    }
    let value =
        serde_json::from_slice(bytes).map_err(|_| LayerEvidenceArchiveErrorV1::InvalidJson)?;
    validate_layer_evidence_archive_v1(&value)?;
    Ok(value)
}

fn validate_layer_evidence_archive_v1(
    value: &LayerEvidenceArchiveV1,
) -> Result<(), LayerEvidenceArchiveErrorV1> {
    const MAX_RECORDS: usize = 2_000_000;
    if value.version != LAYER_EVIDENCE_SCHEMA_VERSION_V1
        || value.project_instance_id.is_empty()
        || value.project_id.is_empty()
        || !is_sha256_hex(&value.fold_model_fingerprint_sha256)
    {
        return Err(LayerEvidenceArchiveErrorV1::InvalidEvidence);
    }
    match &value.evidence {
        LayerEvidenceArchiveKindV1::Flat {
            canonical_snapshot_json,
        } => {
            if canonical_snapshot_json.len() > MAX_LAYER_EVIDENCE_JSON_BYTES_V1 {
                return Err(LayerEvidenceArchiveErrorV1::Oversize);
            }
        }
        LayerEvidenceArchiveKindV1::NonFlat {
            hinge_angles,
            material_faces,
            cells,
            pair_orders,
            ..
        } => {
            if material_faces.is_empty()
                || hinge_angles.len() > MAX_RECORDS
                || material_faces.len() > MAX_RECORDS
                || cells.len() > MAX_RECORDS
                || pair_orders.len() > MAX_RECORDS
                || hinge_angles.iter().any(|a| !a.angle_degrees.is_finite())
                || cells.iter().any(|c| {
                    c.boundary_xy.len() < 3
                        || c.boundary_xy.len() > MAX_RECORDS
                        || c.boundary_xy.iter().flatten().any(|v| !v.is_finite())
                })
            {
                return Err(LayerEvidenceArchiveErrorV1::InvalidEvidence);
            }
        }
    }
    Ok(())
}

const DOCUMENT_ONLY_ENTRY_COUNT: usize = 2;
const PROJECT_WITH_HISTORY_ENTRY_COUNT: usize = 3;
const PROJECT_WITH_HISTORY_AND_EVIDENCE_ENTRY_COUNT: usize = 4;
const ORI2_DEFLATE_LEVEL: i64 = 6;
const END_OF_CENTRAL_DIRECTORY_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const END_OF_CENTRAL_DIRECTORY_SIZE: usize = 22;
const MAX_ZIP_COMMENT_SIZE: usize = u16::MAX as usize;

/// Resource limits applied while reading or writing an `.ori2` container.
///
/// The defaults leave ample room for large crease patterns while bounding ZIP
/// bombs, oversized metadata, and archives containing excessive entry counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ori2Limits {
    pub max_archive_size: u64,
    pub max_entry_count: usize,
    pub max_entry_path_length: usize,
    pub max_entry_uncompressed_size: u64,
    pub max_total_uncompressed_size: u64,
    pub max_manifest_size: u64,
    pub max_project_size: u64,
    pub max_editor_history_size: u64,
    pub max_layer_evidence_size: u64,
}

impl Default for Ori2Limits {
    fn default() -> Self {
        Self {
            max_archive_size: 64 * 1024 * 1024,
            max_entry_count: 4_096,
            max_entry_path_length: 1_024,
            max_entry_uncompressed_size: MAX_PROJECT_JSON_BYTES as u64,
            max_total_uncompressed_size: 256 * 1024 * 1024,
            max_manifest_size: 1024 * 1024,
            max_project_size: MAX_PROJECT_JSON_BYTES as u64,
            max_editor_history_size: MAX_EDITOR_HISTORY_JSON_BYTES,
            max_layer_evidence_size: MAX_LAYER_EVIDENCE_JSON_BYTES_V1 as u64,
        }
    }
}

/// All project-local content carried by one `.ori2` archive.
///
/// `ProjectDocument` deliberately remains container-independent version 1.
/// Optional editor history lives in a separate authenticated entry so legacy
/// two-entry archives remain byte-compatible and document-only readers cannot
/// silently discard history.
#[derive(Debug, Clone, PartialEq)]
pub struct Ori2ProjectArchive {
    pub document: ProjectDocument,
    pub editor_history: Option<EditorHistoryV1>,
    /// Untrusted, versioned solver evidence carried without conversion into a
    /// trusted domain model.
    pub layer_evidence: Option<LayerEvidenceArchiveV1>,
}

impl Ori2ProjectArchive {
    #[must_use]
    pub const fn document_only(document: ProjectDocument) -> Self {
        Self {
            document,
            editor_history: None,
            layer_evidence: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ori2Manifest {
    pub container: String,
    pub container_version: u32,
    /// Canonical, duplicate-free feature vector required to read the project.
    #[serde(default)]
    pub required_features: Vec<String>,
    pub project: Ori2ProjectEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_history: Option<Ori2EditorHistoryEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_evidence: Option<Ori2LayerEvidenceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ori2ProjectEntry {
    pub path: String,
    pub format_version: u32,
    pub uncompressed_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ori2EditorHistoryEntry {
    pub path: String,
    pub schema_version: u32,
    pub uncompressed_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ori2LayerEvidenceEntry {
    pub path: String,
    pub schema_version: u32,
    pub uncompressed_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ori2EditorHistoryEnvelope {
    project_sha256: String,
    history: EditorHistoryV1,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigratableEditorHistoryEnvelope {
    project_sha256: serde_json::Value,
    history: MigratableEditorHistory,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigratableEditorHistory {
    schema_version: serde_json::Value,
    project_id: serde_json::Value,
    #[serde(default)]
    history_entry_limit: Option<serde_json::Value>,
    undo_stack: serde_json::Value,
    #[serde(default)]
    redo_stack: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    speculative_unproven_applied_base_v1: Option<serde_json::Value>,
}

/// Migrates the two supported legacy history generations without weakening
/// the strict typed decoder. Unknown fields and unsupported schema versions
/// remain present and are rejected by `EditorHistoryV1`.
pub(crate) fn migrate_editor_history_envelope_json(
    bytes: &[u8],
) -> Result<(serde_json::Value, bool), serde_json::Error> {
    let mut envelope: MigratableEditorHistoryEnvelope = serde_json::from_slice(bytes)?;
    let migrated =
        envelope.history.history_entry_limit.is_none() || envelope.history.redo_stack.is_none();
    envelope
        .history
        .history_entry_limit
        .get_or_insert_with(|| serde_json::json!(MAX_EDITOR_HISTORY_ENTRIES));
    envelope
        .history
        .redo_stack
        .get_or_insert_with(|| serde_json::json!([]));
    serde_json::to_value(envelope).map(|value| (value, migrated))
}

impl Ori2Manifest {
    fn new(
        project_bytes: &[u8],
        project_format_version: u32,
        required_features: Vec<String>,
    ) -> Self {
        Self {
            container: ORI2_CONTAINER_IDENTIFIER.to_owned(),
            container_version: CURRENT_ORI2_CONTAINER_VERSION,
            required_features,
            project: Ori2ProjectEntry {
                path: ORI2_PROJECT_PATH.to_owned(),
                format_version: project_format_version,
                uncompressed_size: project_bytes.len() as u64,
                sha256: sha256_hex(project_bytes),
            },
            editor_history: None,
            layer_evidence: None,
        }
    }
}

pub(crate) fn required_features_for_project_archive_v1(
    document: &ProjectDocument,
    editor_history: Option<&EditorHistoryV1>,
    has_layer_evidence: bool,
) -> Vec<String> {
    let mut required_features = Vec::new();
    if !document.instruction_timeline.steps.is_empty() {
        required_features.push(ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned());
    }
    if document
        .instruction_timeline
        .steps
        .iter()
        .any(|step| step.pose.model == ori_domain::InstructionPoseModel::DeclarativeOnlyV1)
    {
        required_features.push(ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1.to_owned());
    }
    if !document.numeric_expressions.is_empty() {
        required_features.push(ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned());
    }
    if document
        .numeric_expressions
        .requires_deterministic_geometry_references_v2()
    {
        required_features.push(ORI2_FEATURE_DETERMINISTIC_GEOMETRY_REFERENCES_V2.to_owned());
    }
    if !document.geometric_constraints.is_empty() {
        required_features.push(ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned());
    }
    if !document.layers.is_default() {
        required_features.push(ORI2_FEATURE_LAYERS_V1.to_owned());
    }
    if !document.reference_model_assets.is_empty() {
        required_features.push(ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1.to_owned());
    }
    if let Some(history) = editor_history {
        required_features.push(ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned());
        if history.requires_speculative_unproven_fold_feature_v1() {
            required_features.push(ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned());
        }
    }
    if has_layer_evidence {
        required_features.push(ORI2_FEATURE_LAYER_EVIDENCE_V1.to_owned());
    }
    required_features
}

pub(crate) fn is_known_required_feature_v1(feature: &str) -> bool {
    matches!(
        feature,
        ORI2_FEATURE_INSTRUCTION_TIMELINE_V1
            | ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1
            | ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1
            | ORI2_FEATURE_DETERMINISTIC_GEOMETRY_REFERENCES_V2
            | ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1
            | ORI2_FEATURE_LAYERS_V1
            | ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1
            | ORI2_FEATURE_EDITOR_HISTORY_V1
            | ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1
            | ORI2_FEATURE_LAYER_EVIDENCE_V1
    )
}

fn validate_required_features_with_allowlist_v1(
    manifest: &Ori2Manifest,
    is_known: impl Fn(&str) -> bool,
) -> Result<(), FormatError> {
    let mut unsupported_features = manifest
        .required_features
        .iter()
        .filter(|feature| !is_known(feature))
        .cloned()
        .collect::<Vec<_>>();
    unsupported_features.sort_unstable();
    unsupported_features.dedup();
    if unsupported_features.is_empty() {
        Ok(())
    } else {
        Err(FormatError::UnsupportedRequiredFeatures {
            features: unsupported_features,
        })
    }
}

/// Serializes a project into a bounded ZIP-based `.ori2` container.
pub fn write_project_ori2(document: &ProjectDocument) -> Result<Vec<u8>, FormatError> {
    write_project_ori2_with_limits(document, Ori2Limits::default())
}

/// Serializes a project using explicit resource limits.
pub fn write_project_ori2_with_limits(
    document: &ProjectDocument,
    limits: Ori2Limits,
) -> Result<Vec<u8>, FormatError> {
    write_project_archive_parts(document, None, None, limits)
}

/// Serializes a complete project archive, including authenticated Undo/Redo
/// history when the history is not the default empty state.
pub fn write_project_archive_ori2(project: &Ori2ProjectArchive) -> Result<Vec<u8>, FormatError> {
    write_project_archive_ori2_with_limits(project, Ori2Limits::default())
}

/// Serializes a complete project archive using explicit resource limits.
pub fn write_project_archive_ori2_with_limits(
    project: &Ori2ProjectArchive,
    limits: Ori2Limits,
) -> Result<Vec<u8>, FormatError> {
    if project.document.format_version != crate::CURRENT_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: project.document.format_version,
            latest: crate::CURRENT_FORMAT_VERSION,
        });
    }
    if let Some(layer_evidence) = &project.layer_evidence {
        validate_layer_evidence_archive_v1(layer_evidence)
            .map_err(FormatError::InvalidLayerEvidence)?;
    }
    if let Some(history) = &project.editor_history {
        if history.project_id() != project.document.project_id {
            return Err(FormatError::EditorHistoryProjectIdMismatch);
        }
        validate_editor_history_for_document(&project.document, history)?;
    }
    let history = project
        .editor_history
        .as_ref()
        .filter(|history| !history.is_default_empty());
    write_project_archive_parts(
        &project.document,
        history,
        project.layer_evidence.as_ref(),
        limits,
    )
}

fn write_project_archive_parts(
    document: &ProjectDocument,
    editor_history: Option<&EditorHistoryV1>,
    layer_evidence: Option<&LayerEvidenceArchiveV1>,
    limits: Ori2Limits,
) -> Result<Vec<u8>, FormatError> {
    if document.format_version != crate::CURRENT_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: document.format_version,
            latest: crate::CURRENT_FORMAT_VERSION,
        });
    }
    let entry_count = match (editor_history.is_some(), layer_evidence.is_some()) {
        (true, true) => PROJECT_WITH_HISTORY_AND_EVIDENCE_ENTRY_COUNT,
        (true, false) | (false, true) => PROJECT_WITH_HISTORY_ENTRY_COUNT,
        (false, false) => DOCUMENT_ONLY_ENTRY_COUNT,
    };
    ensure_entry_count(entry_count, limits)?;
    ensure_path_length(ORI2_MANIFEST_PATH, limits)?;
    ensure_path_length(ORI2_PROJECT_PATH, limits)?;
    if editor_history.is_some() {
        ensure_path_length(ORI2_EDITOR_HISTORY_PATH, limits)?;
    }
    if layer_evidence.is_some() {
        ensure_path_length(ORI2_LAYER_EVIDENCE_PATH, limits)?;
    }

    let project_bytes = write_project_json(document)?;
    ensure_project_entry_size(project_bytes.len() as u64, limits)?;
    let project_sha256 = sha256_hex(&project_bytes);

    let history_bytes = if let Some(history) = editor_history {
        if history.project_id() != document.project_id {
            return Err(FormatError::EditorHistoryProjectIdMismatch);
        }
        validate_editor_history_for_document(document, history)?;
        let envelope = Ori2EditorHistoryEnvelope {
            project_sha256: project_sha256.clone(),
            history: history.clone(),
        };
        let bytes =
            serde_json::to_vec_pretty(&envelope).map_err(FormatError::InvalidEditorHistoryJson)?;
        ensure_editor_history_entry_size(bytes.len() as u64, limits)?;
        Some(bytes)
    } else {
        None
    };
    if !crate::beginner_generation_document_authority::
        has_authoritative_beginner_generation_history_v1(editor_history, document)
    {
        crate::beginner_generation_document_authority::
            require_current_beginner_generation_document_authority_v1(document)?;
    } else {
        crate::beginner_generation_document_authority::
            reject_mismatched_beginner_generation_document_authority_v1(document)?;
    }
    let layer_evidence_bytes = layer_evidence
        .map(write_layer_evidence_archive_v1)
        .transpose()
        .map_err(FormatError::InvalidLayerEvidence)?;
    if let Some(bytes) = &layer_evidence_bytes {
        ensure_layer_evidence_entry_size(bytes.len() as u64, limits)?;
    }

    let required_features = required_features_for_project_archive_v1(
        document,
        editor_history,
        layer_evidence_bytes.is_some(),
    );
    let mut manifest =
        Ori2Manifest::new(&project_bytes, document.format_version, required_features);
    if let Some(bytes) = &history_bytes {
        manifest.editor_history = Some(Ori2EditorHistoryEntry {
            path: ORI2_EDITOR_HISTORY_PATH.to_owned(),
            schema_version: EDITOR_HISTORY_SCHEMA_VERSION_V1,
            uncompressed_size: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        });
    }
    if let Some(bytes) = &layer_evidence_bytes {
        manifest.layer_evidence = Some(Ori2LayerEvidenceEntry {
            path: ORI2_LAYER_EVIDENCE_PATH.to_owned(),
            schema_version: LAYER_EVIDENCE_SCHEMA_VERSION_V1,
            uncompressed_size: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        });
    }
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(FormatError::InvalidManifestJson)?;
    ensure_entry_size(ORI2_MANIFEST_PATH, manifest_bytes.len() as u64, limits)?;
    ensure_specific_size(
        ORI2_MANIFEST_PATH,
        manifest_bytes.len() as u64,
        limits.max_manifest_size,
    )?;
    let total_size = (manifest_bytes.len() as u64)
        .checked_add(project_bytes.len() as u64)
        .and_then(|size| {
            size.checked_add(
                history_bytes
                    .as_ref()
                    .map_or(0, |history| history.len() as u64),
            )
        })
        .and_then(|size| {
            size.checked_add(
                layer_evidence_bytes
                    .as_ref()
                    .map_or(0, |evidence| evidence.len() as u64),
            )
        })
        .ok_or(FormatError::ExpandedArchiveTooLarge {
            actual: u64::MAX,
            limit: limits.max_total_uncompressed_size,
        })?;
    ensure_total_size(total_size, limits)?;

    let cursor = Cursor::new(Vec::new());
    let mut archive = ZipWriter::new(cursor);
    let options = SimpleFileOptions::DEFAULT
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(ORI2_DEFLATE_LEVEL))
        .last_modified_time(DateTime::DEFAULT)
        .unix_permissions(0o644);

    archive.start_file(ORI2_MANIFEST_PATH, options)?;
    archive.write_all(&manifest_bytes)?;
    archive.start_file(ORI2_PROJECT_PATH, options)?;
    archive.write_all(&project_bytes)?;
    if let Some(history_bytes) = &history_bytes {
        archive.start_file(ORI2_EDITOR_HISTORY_PATH, options)?;
        archive.write_all(history_bytes)?;
    }
    if let Some(layer_evidence_bytes) = &layer_evidence_bytes {
        archive.start_file(ORI2_LAYER_EVIDENCE_PATH, options)?;
        archive.write_all(layer_evidence_bytes)?;
    }

    let bytes = archive.finish()?.into_inner();
    ensure_archive_size(bytes.len() as u64, limits)?;
    Ok(bytes)
}

/// Reads and validates a project from a ZIP-based `.ori2` container.
pub fn read_project_ori2(bytes: &[u8]) -> Result<ProjectDocument, FormatError> {
    read_project_ori2_with_limits(bytes, Ori2Limits::default())
}

/// Reads a document-only project with explicit resource limits.
///
/// This compatibility API rejects archives that contain persisted editor
/// history. Call [`read_project_archive_ori2_with_limits`] when history must be
/// retained; silently dropping it would make a subsequent save destructive.
pub fn read_project_ori2_with_limits(
    bytes: &[u8],
    limits: Ori2Limits,
) -> Result<ProjectDocument, FormatError> {
    let project = read_project_archive_ori2_with_limits(bytes, limits)?;
    if project.editor_history.is_some() {
        return Err(FormatError::EditorHistoryRequiresArchiveApi);
    }
    if project.layer_evidence.is_some() {
        return Err(FormatError::LayerEvidenceRequiresArchiveApi);
    }
    Ok(project.document)
}

/// Reads a complete project archive, including optional persisted Undo/Redo
/// history.
pub fn read_project_archive_ori2(bytes: &[u8]) -> Result<Ori2ProjectArchive, FormatError> {
    read_project_archive_ori2_with_limits(bytes, Ori2Limits::default())
}

/// Reads a complete project archive with explicit resource limits.
///
/// Every entry is inspected before data is expanded. Paths must be portable,
/// relative UTF-8 paths without traversal components. Declared and actually
/// read sizes are independently bounded.
pub fn read_project_archive_ori2_with_limits(
    bytes: &[u8],
    limits: Ori2Limits,
) -> Result<Ori2ProjectArchive, FormatError> {
    ensure_archive_size(bytes.len() as u64, limits)?;
    let declared_entry_count = declared_zip_entry_count(bytes)?;
    ensure_entry_count(declared_entry_count, limits)?;

    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() != declared_entry_count {
        return Err(FormatError::ArchiveEntryCountMismatch {
            declared: declared_entry_count,
            parsed: archive.len(),
        });
    }
    validate_archive_entries(&mut archive, limits)?;

    let manifest_bytes = read_bounded_entry(
        &mut archive,
        ORI2_MANIFEST_PATH,
        limits
            .max_manifest_size
            .min(limits.max_entry_uncompressed_size),
    )?;
    let manifest: Ori2Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(FormatError::InvalidManifestJson)?;
    validate_manifest(&manifest)?;
    let has_history_entry = archive
        .file_names()
        .any(|path| path == ORI2_EDITOR_HISTORY_PATH);
    match (&manifest.editor_history, has_history_entry) {
        (None, true) => return Err(FormatError::UnexpectedEditorHistoryEntry),
        (Some(_), false) => {
            return Err(FormatError::MissingEntry {
                path: ORI2_EDITOR_HISTORY_PATH,
            });
        }
        _ => {}
    }
    let has_layer_evidence_entry = archive
        .file_names()
        .any(|path| path == ORI2_LAYER_EVIDENCE_PATH);
    match (&manifest.layer_evidence, has_layer_evidence_entry) {
        (None, true) => return Err(FormatError::UnexpectedLayerEvidenceEntry),
        (Some(_), false) => {
            return Err(FormatError::MissingEntry {
                path: ORI2_LAYER_EVIDENCE_PATH,
            });
        }
        _ => {}
    }

    ensure_project_entry_size(manifest.project.uncompressed_size, limits)?;
    let archived_project_size = archive.by_name(ORI2_PROJECT_PATH)?.size();
    if manifest.project.uncompressed_size != archived_project_size {
        return Err(FormatError::ProjectSizeMismatch {
            declared: manifest.project.uncompressed_size,
            actual: archived_project_size,
        });
    }
    let project_limit = effective_project_entry_size_limit(limits);
    let project_bytes = read_bounded_entry(&mut archive, ORI2_PROJECT_PATH, project_limit)?;
    let actual_size = project_bytes.len() as u64;
    if manifest.project.uncompressed_size != actual_size {
        return Err(FormatError::ProjectSizeMismatch {
            declared: manifest.project.uncompressed_size,
            actual: actual_size,
        });
    }

    let actual_hash = sha256_hex(&project_bytes);
    if !is_sha256_hex(&manifest.project.sha256)
        || !manifest.project.sha256.eq_ignore_ascii_case(&actual_hash)
    {
        return Err(FormatError::ProjectHashMismatch {
            expected: manifest.project.sha256,
            actual: actual_hash,
        });
    }

    let mut project = read_project_json(&project_bytes)?;
    if manifest.project.format_version != project.format_version {
        return Err(FormatError::ManifestProjectVersionMismatch {
            manifest: manifest.project.format_version,
            project: project.format_version,
        });
    }
    if !project.instruction_timeline.steps.is_empty()
        && !manifest
            .required_features
            .iter()
            .any(|feature| feature == ORI2_FEATURE_INSTRUCTION_TIMELINE_V1)
    {
        return Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_INSTRUCTION_TIMELINE_V1,
        });
    }
    if project
        .instruction_timeline
        .steps
        .iter()
        .any(|step| step.pose.model == ori_domain::InstructionPoseModel::DeclarativeOnlyV1)
        && !manifest
            .required_features
            .iter()
            .any(|feature| feature == ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1)
    {
        return Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1,
        });
    }
    if !project.numeric_expressions.is_empty()
        && !manifest
            .required_features
            .iter()
            .any(|feature| feature == ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1)
    {
        return Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1,
        });
    }
    if !project.geometric_constraints.is_empty()
        && !manifest
            .required_features
            .iter()
            .any(|feature| feature == ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1)
    {
        return Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1,
        });
    }
    if !project.layers.is_default()
        && !manifest
            .required_features
            .iter()
            .any(|feature| feature == ORI2_FEATURE_LAYERS_V1)
    {
        return Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_LAYERS_V1,
        });
    }
    if !project.reference_model_assets.is_empty()
        && !manifest
            .required_features
            .iter()
            .any(|feature| feature == ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1)
    {
        return Err(FormatError::MissingRequiredFeature {
            feature: ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1,
        });
    }

    let editor_history = match &manifest.editor_history {
        Some(descriptor) => Some(read_editor_history_entry(
            &mut archive,
            descriptor,
            &project,
            &actual_hash,
            limits,
        )?),
        None => None,
    };
    if !crate::beginner_generation_document_authority::
        has_authoritative_beginner_generation_history_v1(
        editor_history.as_ref(),
        &project,
    ) {
        crate::beginner_generation_document_authority::
            admit_beginner_generation_document_authority_v1(&mut project)?;
    } else {
        crate::beginner_generation_document_authority::
            reject_mismatched_beginner_generation_document_authority_v1(&project)?;
    }
    let layer_evidence = match &manifest.layer_evidence {
        Some(descriptor) => Some(read_layer_evidence_entry(&mut archive, descriptor, limits)?),
        None => None,
    };
    let expected_features = required_features_for_project_archive_v1(
        &project,
        editor_history.as_ref(),
        layer_evidence.is_some(),
    );
    if manifest.required_features != expected_features {
        return Err(FormatError::RequiredFeaturesMismatch {
            expected: expected_features,
            actual: manifest.required_features,
        });
    }
    Ok(Ori2ProjectArchive {
        document: project,
        editor_history,
        layer_evidence,
    })
}

fn read_layer_evidence_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    descriptor: &Ori2LayerEvidenceEntry,
    limits: Ori2Limits,
) -> Result<LayerEvidenceArchiveV1, FormatError> {
    ensure_layer_evidence_entry_size(descriptor.uncompressed_size, limits)?;
    let archived_size = archive.by_name(ORI2_LAYER_EVIDENCE_PATH)?.size();
    if descriptor.uncompressed_size != archived_size {
        return Err(FormatError::LayerEvidenceSizeMismatch {
            declared: descriptor.uncompressed_size,
            actual: archived_size,
        });
    }
    let bytes = read_bounded_entry(
        archive,
        ORI2_LAYER_EVIDENCE_PATH,
        effective_layer_evidence_entry_size_limit(limits),
    )?;
    if descriptor.uncompressed_size != bytes.len() as u64 {
        return Err(FormatError::LayerEvidenceSizeMismatch {
            declared: descriptor.uncompressed_size,
            actual: bytes.len() as u64,
        });
    }
    let actual_hash = sha256_hex(&bytes);
    if !is_lowercase_sha256_hex(&descriptor.sha256) || descriptor.sha256 != actual_hash {
        return Err(FormatError::LayerEvidenceHashMismatch {
            expected: descriptor.sha256.clone(),
            actual: actual_hash,
        });
    }
    read_layer_evidence_archive_v1(&bytes).map_err(FormatError::InvalidLayerEvidence)
}

fn read_editor_history_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    descriptor: &Ori2EditorHistoryEntry,
    project: &ProjectDocument,
    project_sha256: &str,
    limits: Ori2Limits,
) -> Result<EditorHistoryV1, FormatError> {
    ensure_editor_history_entry_size(descriptor.uncompressed_size, limits)?;
    let archived_size = archive.by_name(ORI2_EDITOR_HISTORY_PATH)?.size();
    if descriptor.uncompressed_size != archived_size {
        return Err(FormatError::EditorHistorySizeMismatch {
            declared: descriptor.uncompressed_size,
            actual: archived_size,
        });
    }
    let history_limit = effective_editor_history_entry_size_limit(limits);
    let history_bytes = read_bounded_entry(archive, ORI2_EDITOR_HISTORY_PATH, history_limit)?;
    let actual_size = history_bytes.len() as u64;
    if descriptor.uncompressed_size != actual_size {
        return Err(FormatError::EditorHistorySizeMismatch {
            declared: descriptor.uncompressed_size,
            actual: actual_size,
        });
    }
    let actual_hash = sha256_hex(&history_bytes);
    if !is_lowercase_sha256_hex(&descriptor.sha256) || descriptor.sha256 != actual_hash {
        return Err(FormatError::EditorHistoryHashMismatch {
            expected: descriptor.sha256.clone(),
            actual: actual_hash,
        });
    }

    let (migrated, _) = migrate_editor_history_envelope_json(&history_bytes)
        .map_err(FormatError::InvalidEditorHistoryJson)?;
    let envelope: Ori2EditorHistoryEnvelope =
        serde_json::from_value(migrated).map_err(FormatError::InvalidEditorHistoryJson)?;
    if !is_lowercase_sha256_hex(&envelope.project_sha256)
        || envelope.project_sha256 != project_sha256
    {
        return Err(FormatError::EditorHistoryProjectHashMismatch);
    }
    if envelope.history.project_id() != project.project_id {
        return Err(FormatError::EditorHistoryProjectIdMismatch);
    }
    validate_editor_history_for_document(project, &envelope.history)?;
    Ok(envelope.history)
}

fn validate_editor_history_for_document(
    document: &ProjectDocument,
    history: &EditorHistoryV1,
) -> Result<(), FormatError> {
    ori_core::EditorState::with_all_document_parts_annotations_underlays_memo_profile_and_history_v1(
        document.crease_pattern.clone(),
        document.paper.clone(),
        document.instruction_timeline.clone(),
        document.geometric_constraints.clone(),
        document.layers.clone(),
        document.element_metadata.clone(),
        document.annotations.clone(),
        document.underlays.clone(),
        document.memo.clone(),
        document.beginner_design_profile.clone(),
        history.clone(),
    )
    .map(|_| ())
    .map_err(FormatError::InvalidEditorHistory)
}

fn validate_archive_entries(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    limits: Ori2Limits,
) -> Result<(), FormatError> {
    ensure_entry_count(archive.len(), limits)?;

    let mut total_size = 0_u64;
    let mut has_manifest = false;
    let mut has_project = false;

    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let path =
            std::str::from_utf8(entry.name_raw()).map_err(|_| FormatError::NonUtf8EntryPath)?;
        validate_entry_path(path, limits)?;

        if entry.encrypted() {
            return Err(FormatError::EncryptedEntry {
                path: path.to_owned(),
            });
        }

        ensure_entry_size(path, entry.size(), limits)?;
        total_size =
            total_size
                .checked_add(entry.size())
                .ok_or(FormatError::ExpandedArchiveTooLarge {
                    actual: u64::MAX,
                    limit: limits.max_total_uncompressed_size,
                })?;
        ensure_total_size(total_size, limits)?;

        if path == ORI2_MANIFEST_PATH {
            if entry.is_dir() {
                return Err(FormatError::RequiredEntryIsDirectory {
                    path: ORI2_MANIFEST_PATH,
                });
            }
            has_manifest = true;
        } else if path == ORI2_PROJECT_PATH {
            if entry.is_dir() {
                return Err(FormatError::RequiredEntryIsDirectory {
                    path: ORI2_PROJECT_PATH,
                });
            }
            has_project = true;
        } else if path == ORI2_EDITOR_HISTORY_PATH && entry.is_dir() {
            return Err(FormatError::RequiredEntryIsDirectory {
                path: ORI2_EDITOR_HISTORY_PATH,
            });
        }
    }

    if !has_manifest {
        return Err(FormatError::MissingEntry {
            path: ORI2_MANIFEST_PATH,
        });
    }
    if !has_project {
        return Err(FormatError::MissingEntry {
            path: ORI2_PROJECT_PATH,
        });
    }
    Ok(())
}

fn declared_zip_entry_count(bytes: &[u8]) -> Result<usize, FormatError> {
    if bytes.len() < END_OF_CENTRAL_DIRECTORY_SIZE {
        return Err(FormatError::InvalidZipFooter);
    }

    let first_candidate = bytes
        .len()
        .saturating_sub(END_OF_CENTRAL_DIRECTORY_SIZE + MAX_ZIP_COMMENT_SIZE);
    let last_candidate = bytes.len() - END_OF_CENTRAL_DIRECTORY_SIZE;
    for offset in (first_candidate..=last_candidate).rev() {
        if bytes[offset..offset + 4] != END_OF_CENTRAL_DIRECTORY_SIGNATURE {
            continue;
        }

        let comment_size = little_endian_u16(bytes, offset + 20) as usize;
        let record_end = offset
            .checked_add(END_OF_CENTRAL_DIRECTORY_SIZE)
            .and_then(|end| end.checked_add(comment_size));
        if record_end != Some(bytes.len()) {
            continue;
        }

        let disk_number = little_endian_u16(bytes, offset + 4);
        let central_directory_disk = little_endian_u16(bytes, offset + 6);
        let entries_on_disk = little_endian_u16(bytes, offset + 8);
        let total_entries = little_endian_u16(bytes, offset + 10);
        if disk_number != 0 || central_directory_disk != 0 || entries_on_disk != total_entries {
            return Err(FormatError::MultiDiskZipNotSupported);
        }

        let central_directory_size = little_endian_u32(bytes, offset + 12);
        let central_directory_offset = little_endian_u32(bytes, offset + 16);
        if total_entries == u16::MAX
            || central_directory_size == u32::MAX
            || central_directory_offset == u32::MAX
        {
            return Err(FormatError::Zip64NotSupported);
        }
        return Ok(total_entries as usize);
    }

    Err(FormatError::InvalidZipFooter)
}

fn little_endian_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn little_endian_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn validate_entry_path(path: &str, limits: Ori2Limits) -> Result<(), FormatError> {
    ensure_path_length(path, limits)?;

    let path_without_directory_slash = path.strip_suffix('/').unwrap_or(path);
    let unsafe_path = path.is_empty()
        || path_without_directory_slash.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains(':')
        || path_without_directory_slash
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..");

    if unsafe_path {
        return Err(FormatError::UnsafeEntryPath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn validate_manifest(manifest: &Ori2Manifest) -> Result<(), FormatError> {
    if manifest.container != ORI2_CONTAINER_IDENTIFIER {
        return Err(FormatError::InvalidContainerIdentifier {
            found: manifest.container.clone(),
        });
    }
    if manifest.container_version != CURRENT_ORI2_CONTAINER_VERSION {
        return Err(FormatError::UnsupportedContainerVersion {
            found: manifest.container_version,
            latest: CURRENT_ORI2_CONTAINER_VERSION,
        });
    }
    validate_required_features_with_allowlist_v1(manifest, is_known_required_feature_v1)?;
    if manifest.project.path != ORI2_PROJECT_PATH {
        return Err(FormatError::InvalidManifestProjectPath {
            found: manifest.project.path.clone(),
        });
    }
    let declares_history_feature = manifest
        .required_features
        .iter()
        .any(|feature| feature == ORI2_FEATURE_EDITOR_HISTORY_V1);
    if declares_history_feature != manifest.editor_history.is_some() {
        return Err(FormatError::EditorHistoryFeatureDescriptorMismatch);
    }
    if let Some(editor_history) = &manifest.editor_history {
        if editor_history.path != ORI2_EDITOR_HISTORY_PATH {
            return Err(FormatError::InvalidManifestEditorHistoryPath {
                found: editor_history.path.clone(),
            });
        }
        if editor_history.schema_version != EDITOR_HISTORY_SCHEMA_VERSION_V1 {
            return Err(FormatError::UnsupportedEditorHistorySchemaVersion {
                found: editor_history.schema_version,
                latest: EDITOR_HISTORY_SCHEMA_VERSION_V1,
            });
        }
    }
    let declares_layer_evidence_feature = manifest
        .required_features
        .iter()
        .any(|feature| feature == ORI2_FEATURE_LAYER_EVIDENCE_V1);
    if declares_layer_evidence_feature != manifest.layer_evidence.is_some() {
        return Err(FormatError::LayerEvidenceFeatureDescriptorMismatch);
    }
    if let Some(layer_evidence) = &manifest.layer_evidence {
        if layer_evidence.path != ORI2_LAYER_EVIDENCE_PATH {
            return Err(FormatError::InvalidManifestLayerEvidencePath {
                found: layer_evidence.path.clone(),
            });
        }
        if layer_evidence.schema_version != LAYER_EVIDENCE_SCHEMA_VERSION_V1 {
            return Err(FormatError::UnsupportedLayerEvidenceSchemaVersion {
                found: layer_evidence.schema_version,
                latest: LAYER_EVIDENCE_SCHEMA_VERSION_V1,
            });
        }
    }
    Ok(())
}

fn read_bounded_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    path: &'static str,
    limit: u64,
) -> Result<Vec<u8>, FormatError> {
    let entry = archive.by_name(path)?;
    if entry.size() > limit {
        return Err(FormatError::EntryTooLarge {
            path: path.to_owned(),
            actual: entry.size(),
            limit,
        });
    }

    let capacity = usize::try_from(entry.size()).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(capacity.min(1024 * 1024));
    let mut bounded = entry.take(limit.saturating_add(1));
    bounded.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(FormatError::EntryTooLarge {
            path: path.to_owned(),
            actual: bytes.len() as u64,
            limit,
        });
    }
    Ok(bytes)
}

fn ensure_archive_size(actual: u64, limits: Ori2Limits) -> Result<(), FormatError> {
    if actual > limits.max_archive_size {
        return Err(FormatError::ContainerTooLarge {
            actual,
            limit: limits.max_archive_size,
        });
    }
    Ok(())
}

fn ensure_entry_count(actual: usize, limits: Ori2Limits) -> Result<(), FormatError> {
    if actual > limits.max_entry_count {
        return Err(FormatError::TooManyEntries {
            actual,
            limit: limits.max_entry_count,
        });
    }
    Ok(())
}

fn ensure_path_length(path: &str, limits: Ori2Limits) -> Result<(), FormatError> {
    if path.len() > limits.max_entry_path_length {
        return Err(FormatError::EntryPathTooLong {
            actual: path.len(),
            limit: limits.max_entry_path_length,
        });
    }
    Ok(())
}

fn ensure_entry_size(path: &str, actual: u64, limits: Ori2Limits) -> Result<(), FormatError> {
    ensure_specific_size(path, actual, limits.max_entry_uncompressed_size)
}

fn effective_project_entry_size_limit(limits: Ori2Limits) -> u64 {
    limits
        .max_project_size
        .min(limits.max_entry_uncompressed_size)
        .min(MAX_PROJECT_JSON_BYTES as u64)
}

fn ensure_project_entry_size(actual: u64, limits: Ori2Limits) -> Result<(), FormatError> {
    ensure_specific_size(
        ORI2_PROJECT_PATH,
        actual,
        effective_project_entry_size_limit(limits),
    )
}

fn effective_editor_history_entry_size_limit(limits: Ori2Limits) -> u64 {
    limits
        .max_editor_history_size
        .min(limits.max_entry_uncompressed_size)
        .min(MAX_EDITOR_HISTORY_JSON_BYTES)
}

fn ensure_editor_history_entry_size(actual: u64, limits: Ori2Limits) -> Result<(), FormatError> {
    ensure_specific_size(
        ORI2_EDITOR_HISTORY_PATH,
        actual,
        effective_editor_history_entry_size_limit(limits),
    )
}

fn effective_layer_evidence_entry_size_limit(limits: Ori2Limits) -> u64 {
    limits
        .max_layer_evidence_size
        .min(limits.max_entry_uncompressed_size)
        .min(MAX_LAYER_EVIDENCE_JSON_BYTES_V1 as u64)
}

fn ensure_layer_evidence_entry_size(actual: u64, limits: Ori2Limits) -> Result<(), FormatError> {
    ensure_specific_size(
        ORI2_LAYER_EVIDENCE_PATH,
        actual,
        effective_layer_evidence_entry_size_limit(limits),
    )
}

fn ensure_specific_size(path: &str, actual: u64, limit: u64) -> Result<(), FormatError> {
    if actual > limit {
        return Err(FormatError::EntryTooLarge {
            path: path.to_owned(),
            actual,
            limit,
        });
    }
    Ok(())
}

fn ensure_total_size(actual: u64, limits: Ori2Limits) -> Result<(), FormatError> {
    if actual > limits.max_total_uncompressed_size {
        return Err(FormatError::ExpandedArchiveTooLarge {
            actual,
            limit: limits.max_total_uncompressed_size,
        });
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_lowercase_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[path = "ori2/tests.rs"]
mod tests;
