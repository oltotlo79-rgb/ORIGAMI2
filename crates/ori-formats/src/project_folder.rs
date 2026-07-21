//! Bounded, deterministic in-memory representation of the expanded project
//! folder format.
//!
//! Filesystem traversal, symlink handling, and atomic directory replacement are
//! deliberately outside this module. Callers must first collect regular-file
//! bytes without following links, then submit those bytes to this admission
//! boundary.

use std::collections::{HashMap, HashSet, hash_map::Entry};

use ori_core::{EDITOR_HISTORY_SCHEMA_VERSION_V1, EditorHistoryV1};
use ori_domain::{EdgeKind, Point2, VertexId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CURRENT_FORMAT_VERSION, FormatError, MAX_EDITOR_HISTORY_JSON_BYTES, MAX_PROJECT_JSON_BYTES,
    ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1, ORI2_FEATURE_EDITOR_HISTORY_V1,
    ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1, ORI2_FEATURE_INSTRUCTION_TIMELINE_V1,
    ORI2_FEATURE_LAYERS_V1, ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1,
    ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1, Ori2ProjectArchive, ProjectDocument, ProjectJsonLimits,
    read_project_json_with_limits, write_project_json,
};

pub const PROJECT_FOLDER_CONTAINER_IDENTIFIER: &str = "ORIGAMI2_EXPANDED_FOLDER";
pub const CURRENT_PROJECT_FOLDER_VERSION: u32 = 1;
pub const PROJECT_FOLDER_MANIFEST_PATH: &str = "manifest.json";
pub const PROJECT_FOLDER_PROJECT_PATH: &str = "project.json";
pub const PROJECT_FOLDER_EDITOR_HISTORY_PATH: &str = "editor-history.json";
pub const PROJECT_FOLDER_PREVIEW_PATH: &str = "preview/crease-pattern.svg";
pub const PROJECT_FOLDER_ROLE_PROJECT: &str = "project";
pub const PROJECT_FOLDER_ROLE_EDITOR_HISTORY: &str = "editor_history";
pub const PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW: &str = "crease_pattern_preview";
pub const PROJECT_FOLDER_PREVIEW_SCHEMA_VERSION: u32 = 1;
pub const MAX_PROJECT_FOLDER_ENTRY_COUNT: usize = 4;
pub const MAX_PROJECT_FOLDER_ENTRY_PATH_BYTES: usize = 256;
pub const MAX_PROJECT_FOLDER_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_PROJECT_FOLDER_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_PROJECT_FOLDER_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

const PROJECT_JSON_CONTENT_TYPE: &str = "application/json";
const SVG_CONTENT_TYPE: &str = "image/svg+xml";
const MAX_PREVIEW_VERTICES: usize = 100_000;
const MAX_PREVIEW_EDGES: usize = 100_000;

/// Caller-tightenable resource limits for an expanded project folder.
///
/// Every field is also capped by a format hard ceiling. Raising a caller value
/// can therefore never weaken the admission boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectFolderLimits {
    pub max_entry_count: usize,
    pub max_entry_path_bytes: usize,
    pub max_entry_bytes: u64,
    pub max_manifest_bytes: u64,
    pub max_project_bytes: u64,
    pub max_editor_history_bytes: u64,
    pub max_preview_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for ProjectFolderLimits {
    fn default() -> Self {
        Self {
            max_entry_count: MAX_PROJECT_FOLDER_ENTRY_COUNT,
            max_entry_path_bytes: MAX_PROJECT_FOLDER_ENTRY_PATH_BYTES,
            max_entry_bytes: MAX_PROJECT_JSON_BYTES as u64,
            max_manifest_bytes: MAX_PROJECT_FOLDER_MANIFEST_BYTES,
            max_project_bytes: MAX_PROJECT_JSON_BYTES as u64,
            max_editor_history_bytes: MAX_EDITOR_HISTORY_JSON_BYTES,
            max_preview_bytes: MAX_PROJECT_FOLDER_PREVIEW_BYTES,
            max_total_bytes: MAX_PROJECT_FOLDER_TOTAL_BYTES,
        }
    }
}

/// One regular-file entry supplied to or emitted by the in-memory boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFolderEntryV1 {
    pub path: String,
    pub bytes: Vec<u8>,
}

impl ProjectFolderEntryV1 {
    #[must_use]
    pub fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
        }
    }
}

/// An admitted project folder and its authenticated project/history payload.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectFolderArtifactV1 {
    entries: Vec<ProjectFolderEntryV1>,
    archive: Ori2ProjectArchive,
}

impl ProjectFolderArtifactV1 {
    #[must_use]
    pub fn entries(&self) -> &[ProjectFolderEntryV1] {
        &self.entries
    }

    #[must_use]
    pub const fn archive(&self) -> &Ori2ProjectArchive {
        &self.archive
    }

    #[must_use]
    pub fn into_archive(self) -> Ori2ProjectArchive {
        self.archive
    }

    #[must_use]
    pub fn preview_svg(&self) -> &[u8] {
        self.entries
            .iter()
            .find(|entry| entry.path == PROJECT_FOLDER_PREVIEW_PATH)
            .map_or(&[], |entry| entry.bytes.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFolderManifestV1 {
    pub container: String,
    pub container_version: u32,
    pub required_features: Vec<String>,
    pub required_roles: Vec<String>,
    pub entries: Vec<ProjectFolderManifestEntryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFolderManifestEntryV1 {
    pub role: String,
    pub path: String,
    pub content_type: String,
    pub schema_version: u32,
    pub uncompressed_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFolderEditorHistoryEnvelopeV1 {
    project_sha256: String,
    history: EditorHistoryV1,
}

#[derive(Debug, Error)]
pub enum ProjectFolderError {
    #[error(transparent)]
    Project(#[from] FormatError),
    #[error("expanded-folder manifest JSON is invalid: {0}")]
    InvalidManifestJson(#[source] serde_json::Error),
    #[error("expanded-folder editor-history JSON is invalid: {0}")]
    InvalidEditorHistoryJson(#[source] serde_json::Error),
    #[error("expanded-folder editor history is invalid for this project: {0}")]
    InvalidEditorHistory(#[source] ori_core::EditorHistoryErrorV1),
    #[error("expanded folder has {actual} entries; the limit is {limit}")]
    TooManyEntries { actual: usize, limit: usize },
    #[error("expanded-folder entry path is {actual} bytes; the limit is {limit} bytes")]
    EntryPathTooLong { actual: usize, limit: usize },
    #[error("expanded-folder entry path is not portable and safe: {path:?}")]
    UnsafeEntryPath { path: String },
    #[error("expanded folder contains duplicate entry path {path:?}")]
    DuplicateEntryPath { path: String },
    #[error("expanded-folder entry paths collide case-insensitively: {first:?} and {second:?}")]
    EntryPathCaseCollision { first: String, second: String },
    #[error("expanded folder is missing the required entry {path}")]
    MissingEntry { path: &'static str },
    #[error(
        "expanded-folder entry order is not canonical (expected {expected:?}, actual {actual:?})"
    )]
    NonCanonicalEntryOrder {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("expanded-folder entry {path:?} is {actual} bytes; the limit is {limit} bytes")]
    EntryTooLarge {
        path: String,
        actual: u64,
        limit: u64,
    },
    #[error("expanded folder totals {actual} bytes; the limit is {limit} bytes")]
    TotalTooLarge { actual: u64, limit: u64 },
    #[error("unexpected expanded-folder container identifier {found:?}")]
    InvalidContainerIdentifier { found: String },
    #[error(
        "expanded-folder version {found} is not supported; latest supported version is {latest}"
    )]
    UnsupportedContainerVersion { found: u32, latest: u32 },
    #[error("expanded folder requires an unknown mandatory role {role:?}")]
    UnknownRequiredRole { role: String },
    #[error("expanded-folder manifest declares an unknown entry role {role:?}")]
    UnknownEntryRole { role: String },
    #[error("expanded folder requires unsupported features: {features:?}")]
    UnsupportedRequiredFeatures { features: Vec<String> },
    #[error(
        "expanded-folder required roles are not canonical (expected {expected:?}, actual {actual:?})"
    )]
    RequiredRolesMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error(
        "expanded-folder required features do not match project content (expected {expected:?}, actual {actual:?})"
    )]
    RequiredFeaturesMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("expanded-folder manifest contains duplicate role {role:?}")]
    DuplicateManifestRole { role: String },
    #[error("expanded-folder manifest contains duplicate path {path:?}")]
    DuplicateManifestPath { path: String },
    #[error("expanded-folder role {role:?} must use path {expected:?}, but declares {actual:?}")]
    InvalidRolePath {
        role: String,
        expected: &'static str,
        actual: String,
    },
    #[error(
        "expanded-folder role {role:?} must use content type {expected:?}, but declares {actual:?}"
    )]
    InvalidContentType {
        role: String,
        expected: &'static str,
        actual: String,
    },
    #[error(
        "expanded-folder role {role:?} schema version {found} is not supported; expected {expected}"
    )]
    UnsupportedRoleSchemaVersion {
        role: String,
        found: u32,
        expected: u32,
    },
    #[error(
        "expanded-folder entry {path:?} declares {declared} bytes, but contains {actual} bytes"
    )]
    EntrySizeMismatch {
        path: String,
        declared: u64,
        actual: u64,
    },
    #[error(
        "expanded-folder entry {path:?} checksum differs (expected {expected}, actual {actual})"
    )]
    EntryHashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("expanded-folder editor history is bound to a different project checksum")]
    EditorHistoryProjectHashMismatch,
    #[error("expanded-folder editor history is bound to a different project ID")]
    EditorHistoryProjectIdMismatch,
    #[error(
        "the exact default empty editor history must be omitted from an expanded-folder V1 artifact"
    )]
    DefaultEditorHistoryMustBeOmitted,
    #[error(
        "expanded-folder preview is not the deterministic read-only projection of project.json"
    )]
    PreviewProjectionMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FolderRole {
    Project,
    EditorHistory,
    Preview,
}

impl FolderRole {
    fn parse(value: &str) -> Result<Self, ProjectFolderError> {
        match value {
            PROJECT_FOLDER_ROLE_PROJECT => Ok(Self::Project),
            PROJECT_FOLDER_ROLE_EDITOR_HISTORY => Ok(Self::EditorHistory),
            PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW => Ok(Self::Preview),
            _ => Err(ProjectFolderError::UnknownEntryRole {
                role: value.to_owned(),
            }),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Project => PROJECT_FOLDER_ROLE_PROJECT,
            Self::EditorHistory => PROJECT_FOLDER_ROLE_EDITOR_HISTORY,
            Self::Preview => PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW,
        }
    }

    const fn path(self) -> &'static str {
        match self {
            Self::Project => PROJECT_FOLDER_PROJECT_PATH,
            Self::EditorHistory => PROJECT_FOLDER_EDITOR_HISTORY_PATH,
            Self::Preview => PROJECT_FOLDER_PREVIEW_PATH,
        }
    }

    const fn content_type(self) -> &'static str {
        match self {
            Self::Project | Self::EditorHistory => PROJECT_JSON_CONTENT_TYPE,
            Self::Preview => SVG_CONTENT_TYPE,
        }
    }

    const fn schema_version(self) -> u32 {
        match self {
            Self::Project => CURRENT_FORMAT_VERSION,
            Self::EditorHistory => EDITOR_HISTORY_SCHEMA_VERSION_V1,
            Self::Preview => PROJECT_FOLDER_PREVIEW_SCHEMA_VERSION,
        }
    }
}

/// Creates a canonical, fully self-verified expanded-folder artifact.
pub fn write_project_folder_v1(
    archive: &Ori2ProjectArchive,
) -> Result<ProjectFolderArtifactV1, ProjectFolderError> {
    write_project_folder_v1_with_limits(archive, ProjectFolderLimits::default())
}

/// Creates a canonical artifact with caller-tightened resource limits.
pub fn write_project_folder_v1_with_limits(
    archive: &Ori2ProjectArchive,
    limits: ProjectFolderLimits,
) -> Result<ProjectFolderArtifactV1, ProjectFolderError> {
    if let Some(history) = &archive.editor_history {
        if history.project_id() != archive.document.project_id {
            return Err(ProjectFolderError::EditorHistoryProjectIdMismatch);
        }
        validate_editor_history_for_document(&archive.document, history)?;
    }
    let history = archive
        .editor_history
        .as_ref()
        .filter(|history| !history.is_default_empty());
    let entry_count = if history.is_some() { 4 } else { 3 };
    ensure_entry_count(entry_count, limits)?;

    for path in [
        PROJECT_FOLDER_MANIFEST_PATH,
        PROJECT_FOLDER_PROJECT_PATH,
        PROJECT_FOLDER_PREVIEW_PATH,
    ] {
        validate_entry_path(path, limits)?;
    }
    if history.is_some() {
        validate_entry_path(PROJECT_FOLDER_EDITOR_HISTORY_PATH, limits)?;
    }

    let project_bytes = write_project_json(&archive.document)?;
    ensure_role_size(FolderRole::Project, project_bytes.len() as u64, limits)?;
    let project_sha256 = sha256_hex(&project_bytes);

    let history_bytes = if let Some(history) = history {
        let envelope = ProjectFolderEditorHistoryEnvelopeV1 {
            project_sha256: project_sha256.clone(),
            history: history.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&envelope)
            .map_err(ProjectFolderError::InvalidEditorHistoryJson)?;
        ensure_role_size(FolderRole::EditorHistory, bytes.len() as u64, limits)?;
        Some(bytes)
    } else {
        None
    };

    let preview_bytes =
        generate_safe_preview_svg(&archive.document, effective_preview_limit(limits))?;
    ensure_role_size(FolderRole::Preview, preview_bytes.len() as u64, limits)?;

    let roles = canonical_roles(history_bytes.is_some());
    let mut descriptors = Vec::with_capacity(roles.len());
    for role in roles.iter().copied() {
        let bytes = match role {
            FolderRole::Project => &project_bytes,
            FolderRole::EditorHistory => history_bytes
                .as_ref()
                .expect("canonical history role requires history bytes"),
            FolderRole::Preview => &preview_bytes,
        };
        descriptors.push(manifest_entry(role, bytes));
    }
    let manifest = ProjectFolderManifestV1 {
        container: PROJECT_FOLDER_CONTAINER_IDENTIFIER.to_owned(),
        container_version: CURRENT_PROJECT_FOLDER_VERSION,
        required_features: required_features(&archive.document, history_bytes.is_some()),
        required_roles: roles.iter().map(|role| role.as_str().to_owned()).collect(),
        entries: descriptors,
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(ProjectFolderError::InvalidManifestJson)?;
    ensure_manifest_size(manifest_bytes.len() as u64, limits)?;

    let mut entries = Vec::with_capacity(entry_count);
    entries.push(ProjectFolderEntryV1::new(
        PROJECT_FOLDER_MANIFEST_PATH,
        manifest_bytes,
    ));
    entries.push(ProjectFolderEntryV1::new(
        PROJECT_FOLDER_PROJECT_PATH,
        project_bytes,
    ));
    if let Some(history_bytes) = history_bytes {
        entries.push(ProjectFolderEntryV1::new(
            PROJECT_FOLDER_EDITOR_HISTORY_PATH,
            history_bytes,
        ));
    }
    entries.push(ProjectFolderEntryV1::new(
        PROJECT_FOLDER_PREVIEW_PATH,
        preview_bytes,
    ));

    // The writer crosses the same admission boundary as untrusted input. This
    // prevents writer-only states from escaping as apparently valid artifacts.
    read_project_folder_v1_with_limits(&entries, limits)
}

/// Admits regular-file bytes collected from an expanded project folder.
pub fn read_project_folder_v1(
    entries: &[ProjectFolderEntryV1],
) -> Result<ProjectFolderArtifactV1, ProjectFolderError> {
    read_project_folder_v1_with_limits(entries, ProjectFolderLimits::default())
}

/// Admits regular-file bytes using caller-tightened resource limits.
pub fn read_project_folder_v1_with_limits(
    entries: &[ProjectFolderEntryV1],
    limits: ProjectFolderLimits,
) -> Result<ProjectFolderArtifactV1, ProjectFolderError> {
    validate_physical_entries(entries, limits)?;

    let manifest_entry = entries
        .iter()
        .find(|entry| entry.path == PROJECT_FOLDER_MANIFEST_PATH)
        .ok_or(ProjectFolderError::MissingEntry {
            path: PROJECT_FOLDER_MANIFEST_PATH,
        })?;
    ensure_manifest_size(manifest_entry.bytes.len() as u64, limits)?;
    let manifest: ProjectFolderManifestV1 = serde_json::from_slice(&manifest_entry.bytes)
        .map_err(ProjectFolderError::InvalidManifestJson)?;
    validate_manifest_envelope(&manifest)?;

    for role in &manifest.required_roles {
        if !is_known_role(role) {
            return Err(ProjectFolderError::UnknownRequiredRole { role: role.clone() });
        }
    }
    let unsupported_features = manifest
        .required_features
        .iter()
        .filter(|feature| !is_known_feature(feature))
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported_features.is_empty() {
        return Err(ProjectFolderError::UnsupportedRequiredFeatures {
            features: unsupported_features,
        });
    }

    let roles = validate_manifest_entries(&manifest, limits)?;
    let has_history = roles.contains(&FolderRole::EditorHistory);
    let expected_roles = canonical_roles(has_history);
    if roles != expected_roles {
        return Err(ProjectFolderError::RequiredRolesMismatch {
            expected: expected_roles
                .iter()
                .map(|role| role.as_str().to_owned())
                .collect(),
            actual: roles.iter().map(|role| role.as_str().to_owned()).collect(),
        });
    }
    let expected_required_roles = expected_roles
        .iter()
        .map(|role| role.as_str().to_owned())
        .collect::<Vec<_>>();
    if manifest.required_roles != expected_required_roles {
        return Err(ProjectFolderError::RequiredRolesMismatch {
            expected: expected_required_roles,
            actual: manifest.required_roles.clone(),
        });
    }

    let expected_entry_paths = std::iter::once(PROJECT_FOLDER_MANIFEST_PATH.to_owned())
        .chain(expected_roles.iter().map(|role| role.path().to_owned()))
        .collect::<Vec<_>>();
    let actual_entry_paths = entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    if actual_entry_paths != expected_entry_paths {
        return Err(ProjectFolderError::NonCanonicalEntryOrder {
            expected: expected_entry_paths,
            actual: actual_entry_paths,
        });
    }

    for descriptor in &manifest.entries {
        let entry = entry_by_path(entries, &descriptor.path)?;
        ensure_descriptor_authentication(descriptor, entry)?;
    }

    let project_entry = entry_by_path(entries, PROJECT_FOLDER_PROJECT_PATH)?;
    let project_limit = effective_project_limit(limits);
    let project = read_project_json_with_limits(
        &project_entry.bytes,
        ProjectJsonLimits {
            max_input_size: project_limit as usize,
        },
    )?;
    let expected_features = required_features(&project, has_history);
    if manifest.required_features != expected_features {
        return Err(ProjectFolderError::RequiredFeaturesMismatch {
            expected: expected_features,
            actual: manifest.required_features.clone(),
        });
    }
    let project_sha256 = sha256_hex(&project_entry.bytes);

    let editor_history = if has_history {
        let history_entry = entry_by_path(entries, PROJECT_FOLDER_EDITOR_HISTORY_PATH)?;
        let envelope: ProjectFolderEditorHistoryEnvelopeV1 =
            serde_json::from_slice(&history_entry.bytes)
                .map_err(ProjectFolderError::InvalidEditorHistoryJson)?;
        if !is_lowercase_sha256_hex(&envelope.project_sha256)
            || envelope.project_sha256 != project_sha256
        {
            return Err(ProjectFolderError::EditorHistoryProjectHashMismatch);
        }
        if envelope.history.project_id() != project.project_id {
            return Err(ProjectFolderError::EditorHistoryProjectIdMismatch);
        }
        validate_editor_history_for_document(&project, &envelope.history)?;
        if envelope.history.is_default_empty() {
            return Err(ProjectFolderError::DefaultEditorHistoryMustBeOmitted);
        }
        Some(envelope.history)
    } else {
        None
    };

    let preview_entry = entry_by_path(entries, PROJECT_FOLDER_PREVIEW_PATH)?;
    let expected_preview = generate_safe_preview_svg(&project, effective_preview_limit(limits))?;
    if preview_entry.bytes != expected_preview {
        return Err(ProjectFolderError::PreviewProjectionMismatch);
    }

    Ok(ProjectFolderArtifactV1 {
        entries: entries.to_vec(),
        archive: Ori2ProjectArchive {
            layer_evidence: None,
            document: project,
            editor_history,
        },
    })
}

fn validate_physical_entries(
    entries: &[ProjectFolderEntryV1],
    limits: ProjectFolderLimits,
) -> Result<(), ProjectFolderError> {
    ensure_entry_count(entries.len(), limits)?;
    let mut exact_paths = HashSet::with_capacity(entries.len());
    let mut folded_paths = HashMap::with_capacity(entries.len());
    let mut total = 0_u64;

    for entry in entries {
        validate_entry_path(&entry.path, limits)?;
        if !exact_paths.insert(entry.path.clone()) {
            return Err(ProjectFolderError::DuplicateEntryPath {
                path: entry.path.clone(),
            });
        }
        let folded = entry.path.to_ascii_lowercase();
        if let Some(first) = folded_paths.insert(folded, entry.path.clone()) {
            return Err(ProjectFolderError::EntryPathCaseCollision {
                first,
                second: entry.path.clone(),
            });
        }
        let size = entry.bytes.len() as u64;
        let entry_limit = effective_entry_limit(limits);
        if size > entry_limit {
            return Err(ProjectFolderError::EntryTooLarge {
                path: entry.path.clone(),
                actual: size,
                limit: entry_limit,
            });
        }
        total = total
            .checked_add(size)
            .ok_or(ProjectFolderError::TotalTooLarge {
                actual: u64::MAX,
                limit: effective_total_limit(limits),
            })?;
        ensure_total_size(total, limits)?;
    }
    Ok(())
}

fn validate_manifest_envelope(
    manifest: &ProjectFolderManifestV1,
) -> Result<(), ProjectFolderError> {
    if manifest.container != PROJECT_FOLDER_CONTAINER_IDENTIFIER {
        return Err(ProjectFolderError::InvalidContainerIdentifier {
            found: manifest.container.clone(),
        });
    }
    if manifest.container_version != CURRENT_PROJECT_FOLDER_VERSION {
        return Err(ProjectFolderError::UnsupportedContainerVersion {
            found: manifest.container_version,
            latest: CURRENT_PROJECT_FOLDER_VERSION,
        });
    }
    Ok(())
}

fn validate_manifest_entries(
    manifest: &ProjectFolderManifestV1,
    limits: ProjectFolderLimits,
) -> Result<Vec<FolderRole>, ProjectFolderError> {
    let mut roles = Vec::with_capacity(manifest.entries.len());
    let mut role_names = HashSet::with_capacity(manifest.entries.len());
    let mut paths = HashSet::with_capacity(manifest.entries.len());
    let mut folded_paths = HashMap::with_capacity(manifest.entries.len());

    for descriptor in &manifest.entries {
        let role = FolderRole::parse(&descriptor.role)?;
        if !role_names.insert(descriptor.role.clone()) {
            return Err(ProjectFolderError::DuplicateManifestRole {
                role: descriptor.role.clone(),
            });
        }
        validate_entry_path(&descriptor.path, limits)?;
        if !paths.insert(descriptor.path.clone()) {
            return Err(ProjectFolderError::DuplicateManifestPath {
                path: descriptor.path.clone(),
            });
        }
        let folded = descriptor.path.to_ascii_lowercase();
        if let Some(first) = folded_paths.insert(folded, descriptor.path.clone()) {
            return Err(ProjectFolderError::EntryPathCaseCollision {
                first,
                second: descriptor.path.clone(),
            });
        }
        if descriptor.path != role.path() {
            return Err(ProjectFolderError::InvalidRolePath {
                role: descriptor.role.clone(),
                expected: role.path(),
                actual: descriptor.path.clone(),
            });
        }
        if descriptor.content_type != role.content_type() {
            return Err(ProjectFolderError::InvalidContentType {
                role: descriptor.role.clone(),
                expected: role.content_type(),
                actual: descriptor.content_type.clone(),
            });
        }
        if descriptor.schema_version != role.schema_version() {
            return Err(ProjectFolderError::UnsupportedRoleSchemaVersion {
                role: descriptor.role.clone(),
                found: descriptor.schema_version,
                expected: role.schema_version(),
            });
        }
        ensure_role_size(role, descriptor.uncompressed_size, limits)?;
        roles.push(role);
    }
    Ok(roles)
}

fn ensure_descriptor_authentication(
    descriptor: &ProjectFolderManifestEntryV1,
    entry: &ProjectFolderEntryV1,
) -> Result<(), ProjectFolderError> {
    let actual_size = entry.bytes.len() as u64;
    if descriptor.uncompressed_size != actual_size {
        return Err(ProjectFolderError::EntrySizeMismatch {
            path: descriptor.path.clone(),
            declared: descriptor.uncompressed_size,
            actual: actual_size,
        });
    }
    let actual_hash = sha256_hex(&entry.bytes);
    if !is_lowercase_sha256_hex(&descriptor.sha256) || descriptor.sha256 != actual_hash {
        return Err(ProjectFolderError::EntryHashMismatch {
            path: descriptor.path.clone(),
            expected: descriptor.sha256.clone(),
            actual: actual_hash,
        });
    }
    Ok(())
}

fn entry_by_path<'a>(
    entries: &'a [ProjectFolderEntryV1],
    path: &str,
) -> Result<&'a ProjectFolderEntryV1, ProjectFolderError> {
    entries
        .iter()
        .find(|entry| entry.path == path)
        .ok_or(match path {
            PROJECT_FOLDER_MANIFEST_PATH => ProjectFolderError::MissingEntry {
                path: PROJECT_FOLDER_MANIFEST_PATH,
            },
            PROJECT_FOLDER_PROJECT_PATH => ProjectFolderError::MissingEntry {
                path: PROJECT_FOLDER_PROJECT_PATH,
            },
            PROJECT_FOLDER_EDITOR_HISTORY_PATH => ProjectFolderError::MissingEntry {
                path: PROJECT_FOLDER_EDITOR_HISTORY_PATH,
            },
            _ => ProjectFolderError::MissingEntry {
                path: PROJECT_FOLDER_PREVIEW_PATH,
            },
        })
}

fn canonical_roles(has_history: bool) -> Vec<FolderRole> {
    let mut roles = vec![FolderRole::Project];
    if has_history {
        roles.push(FolderRole::EditorHistory);
    }
    roles.push(FolderRole::Preview);
    roles
}

fn required_features(document: &ProjectDocument, has_history: bool) -> Vec<String> {
    let mut features = Vec::new();
    if !document.instruction_timeline.steps.is_empty() {
        features.push(ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned());
    }
    if document
        .instruction_timeline
        .steps
        .iter()
        .any(|step| step.pose.model == ori_domain::InstructionPoseModel::DeclarativeOnlyV1)
    {
        features.push(ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1.to_owned());
    }
    if !document.numeric_expressions.is_empty() {
        features.push(ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned());
    }
    if !document.geometric_constraints.is_empty() {
        features.push(ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1.to_owned());
    }
    if !document.layers.is_default() {
        features.push(ORI2_FEATURE_LAYERS_V1.to_owned());
    }
    if !document.reference_model_assets.is_empty() {
        features.push(ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1.to_owned());
    }
    if has_history {
        features.push(ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned());
    }
    features
}

fn is_known_feature(feature: &str) -> bool {
    matches!(
        feature,
        ORI2_FEATURE_INSTRUCTION_TIMELINE_V1
            | ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1
            | ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1
            | ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1
            | ORI2_FEATURE_LAYERS_V1
            | ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1
            | ORI2_FEATURE_EDITOR_HISTORY_V1
    )
}

fn is_known_role(role: &str) -> bool {
    matches!(
        role,
        PROJECT_FOLDER_ROLE_PROJECT
            | PROJECT_FOLDER_ROLE_EDITOR_HISTORY
            | PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW
    )
}

fn manifest_entry(role: FolderRole, bytes: &[u8]) -> ProjectFolderManifestEntryV1 {
    ProjectFolderManifestEntryV1 {
        role: role.as_str().to_owned(),
        path: role.path().to_owned(),
        content_type: role.content_type().to_owned(),
        schema_version: role.schema_version(),
        uncompressed_size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
    }
}

fn validate_editor_history_for_document(
    document: &ProjectDocument,
    history: &EditorHistoryV1,
) -> Result<(), ProjectFolderError> {
    ori_core::EditorState::with_document_parts_layers_and_history_v1(
        document.crease_pattern.clone(),
        document.paper.clone(),
        document.instruction_timeline.clone(),
        document.geometric_constraints.clone(),
        document.layers.clone(),
        history.clone(),
    )
    .map(|_| ())
    .map_err(ProjectFolderError::InvalidEditorHistory)
}

fn validate_entry_path(path: &str, limits: ProjectFolderLimits) -> Result<(), ProjectFolderError> {
    let limit = limits
        .max_entry_path_bytes
        .min(MAX_PROJECT_FOLDER_ENTRY_PATH_BYTES);
    if path.len() > limit {
        return Err(ProjectFolderError::EntryPathTooLong {
            actual: path.len(),
            limit,
        });
    }
    let unsafe_path = path.is_empty()
        || !path.is_ascii()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.bytes().any(|byte| byte.is_ascii_control())
        || path.split('/').any(is_unsafe_path_component)
        || path.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric()
                || byte == b'/'
                || byte == b'.'
                || byte == b'_'
                || byte == b'-')
        });
    if unsafe_path {
        return Err(ProjectFolderError::UnsafeEntryPath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn is_unsafe_path_component(component: &str) -> bool {
    if component.is_empty() || component == "." || component == ".." || component.ends_with('.') {
        return true;
    }
    let stem = component
        .split_once('.')
        .map_or(component, |(stem, _)| stem);
    if stem.eq_ignore_ascii_case("con")
        || stem.eq_ignore_ascii_case("prn")
        || stem.eq_ignore_ascii_case("aux")
        || stem.eq_ignore_ascii_case("nul")
    {
        return true;
    }
    let bytes = stem.as_bytes();
    bytes.len() == 4
        && (stem[..3].eq_ignore_ascii_case("com") || stem[..3].eq_ignore_ascii_case("lpt"))
        && (b'1'..=b'9').contains(&bytes[3])
}

fn ensure_entry_count(
    actual: usize,
    limits: ProjectFolderLimits,
) -> Result<(), ProjectFolderError> {
    let limit = limits.max_entry_count.min(MAX_PROJECT_FOLDER_ENTRY_COUNT);
    if actual > limit {
        return Err(ProjectFolderError::TooManyEntries { actual, limit });
    }
    Ok(())
}

fn ensure_manifest_size(
    actual: u64,
    limits: ProjectFolderLimits,
) -> Result<(), ProjectFolderError> {
    let limit = limits
        .max_manifest_bytes
        .min(effective_entry_limit(limits))
        .min(MAX_PROJECT_FOLDER_MANIFEST_BYTES);
    if actual > limit {
        return Err(ProjectFolderError::EntryTooLarge {
            path: PROJECT_FOLDER_MANIFEST_PATH.to_owned(),
            actual,
            limit,
        });
    }
    Ok(())
}

fn ensure_role_size(
    role: FolderRole,
    actual: u64,
    limits: ProjectFolderLimits,
) -> Result<(), ProjectFolderError> {
    let limit = match role {
        FolderRole::Project => effective_project_limit(limits),
        FolderRole::EditorHistory => effective_history_limit(limits),
        FolderRole::Preview => effective_preview_limit(limits),
    }
    .min(effective_entry_limit(limits));
    if actual > limit {
        return Err(ProjectFolderError::EntryTooLarge {
            path: role.path().to_owned(),
            actual,
            limit,
        });
    }
    Ok(())
}

fn ensure_total_size(actual: u64, limits: ProjectFolderLimits) -> Result<(), ProjectFolderError> {
    let limit = effective_total_limit(limits);
    if actual > limit {
        return Err(ProjectFolderError::TotalTooLarge { actual, limit });
    }
    Ok(())
}

const fn effective_entry_limit(limits: ProjectFolderLimits) -> u64 {
    if limits.max_entry_bytes < MAX_PROJECT_JSON_BYTES as u64 {
        limits.max_entry_bytes
    } else {
        MAX_PROJECT_JSON_BYTES as u64
    }
}

const fn effective_project_limit(limits: ProjectFolderLimits) -> u64 {
    if limits.max_project_bytes < MAX_PROJECT_JSON_BYTES as u64 {
        limits.max_project_bytes
    } else {
        MAX_PROJECT_JSON_BYTES as u64
    }
}

const fn effective_history_limit(limits: ProjectFolderLimits) -> u64 {
    if limits.max_editor_history_bytes < MAX_EDITOR_HISTORY_JSON_BYTES {
        limits.max_editor_history_bytes
    } else {
        MAX_EDITOR_HISTORY_JSON_BYTES
    }
}

const fn effective_preview_limit(limits: ProjectFolderLimits) -> u64 {
    if limits.max_preview_bytes < MAX_PROJECT_FOLDER_PREVIEW_BYTES {
        limits.max_preview_bytes
    } else {
        MAX_PROJECT_FOLDER_PREVIEW_BYTES
    }
}

const fn effective_total_limit(limits: ProjectFolderLimits) -> u64 {
    if limits.max_total_bytes < MAX_PROJECT_FOLDER_TOTAL_BYTES {
        limits.max_total_bytes
    } else {
        MAX_PROJECT_FOLDER_TOTAL_BYTES
    }
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

fn is_lowercase_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Copy)]
enum PreviewFallback {
    NoFiniteGeometry,
    ComplexityLimit,
    NumericRange,
    ByteLimit,
}

impl PreviewFallback {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NoFiniteGeometry => "no_finite_geometry",
            Self::ComplexityLimit => "complexity_limit",
            Self::NumericRange => "numeric_range",
            Self::ByteLimit => "byte_limit",
        }
    }
}

pub(crate) fn generate_safe_preview_svg(
    document: &ProjectDocument,
    byte_limit: u64,
) -> Result<Vec<u8>, ProjectFolderError> {
    let pattern = &document.crease_pattern;
    if pattern.vertices.len() > MAX_PREVIEW_VERTICES || pattern.edges.len() > MAX_PREVIEW_EDGES {
        return bounded_placeholder_preview(PreviewFallback::ComplexityLimit, byte_limit);
    }

    let mut by_id: HashMap<VertexId, Option<Point2>> =
        HashMap::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        let finite = vertex.position.x.is_finite() && vertex.position.y.is_finite();
        match by_id.entry(vertex.id) {
            Entry::Vacant(slot) => {
                slot.insert(finite.then_some(vertex.position));
            }
            Entry::Occupied(mut slot) => {
                slot.insert(None);
            }
        }
    }

    let finite_points = by_id
        .values()
        .filter_map(|point| *point)
        .collect::<Vec<_>>();
    let skipped_vertices = pattern.vertices.len().saturating_sub(finite_points.len());
    if finite_points.is_empty() {
        return bounded_placeholder_preview(PreviewFallback::NoFiniteGeometry, byte_limit);
    }
    let mut min_x = finite_points[0].x;
    let mut max_x = finite_points[0].x;
    let mut min_y = finite_points[0].y;
    let mut max_y = finite_points[0].y;
    for point in finite_points.iter().skip(1) {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    let span_x = max_x - min_x;
    let span_y = max_y - min_y;
    if !span_x.is_finite() || !span_y.is_finite() || span_x < 0.0 || span_y < 0.0 {
        return bounded_placeholder_preview(PreviewFallback::NumericRange, byte_limit);
    }
    let width = span_x.max(1.0);
    let height = span_y.max(1.0);

    let mut rendered_edges = Vec::with_capacity(pattern.edges.len());
    let mut skipped_edges = 0_usize;
    for edge in &pattern.edges {
        let start = by_id.get(&edge.start).and_then(|point| *point);
        let end = by_id.get(&edge.end).and_then(|point| *point);
        let Some((start, end)) = start.zip(end) else {
            skipped_edges += 1;
            continue;
        };
        if start.x == end.x && start.y == end.y {
            skipped_edges += 1;
            continue;
        }
        let coordinates = [
            start.x - min_x,
            start.y - min_y,
            end.x - min_x,
            end.y - min_y,
        ];
        if coordinates.iter().any(|value| !value.is_finite()) {
            skipped_edges += 1;
            continue;
        }
        rendered_edges.push((edge.kind, coordinates));
    }

    let status = if skipped_vertices == 0
        && skipped_edges == 0
        && !pattern.edges.is_empty()
        && rendered_edges.len() == pattern.edges.len()
    {
        "complete"
    } else {
        "partial"
    };
    let capacity = 512_usize
        .saturating_add(rendered_edges.len().saturating_mul(180))
        .saturating_add(finite_points.len().saturating_mul(90));
    let mut svg = String::with_capacity(capacity);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" viewBox=\"0 0 ");
    svg.push_str(&canonical_number(width));
    svg.push(' ');
    svg.push_str(&canonical_number(height));
    svg.push_str("\" role=\"img\" aria-label=\"Read-only crease-pattern preview\"");
    svg.push_str(" data-origami-preview=\"read-only\" data-preview-status=\"");
    svg.push_str(status);
    svg.push_str("\" data-skipped-vertices=\"");
    svg.push_str(&skipped_vertices.to_string());
    svg.push_str("\" data-skipped-edges=\"");
    svg.push_str(&skipped_edges.to_string());
    svg.push_str("\">\n");
    svg.push_str("  <title>Read-only crease-pattern preview</title>\n");
    svg.push_str("  <desc>Preview status: ");
    svg.push_str(status);
    svg.push_str(". Skipped vertices: ");
    svg.push_str(&skipped_vertices.to_string());
    svg.push_str(". Skipped edges: ");
    svg.push_str(&skipped_edges.to_string());
    svg.push_str(".</desc>\n");
    svg.push_str("  <rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n");
    svg.push_str("  <g fill=\"none\" stroke-width=\"1\" vector-effect=\"non-scaling-stroke\">\n");
    for (kind, [x1, y1, x2, y2]) in rendered_edges {
        let (semantic, stroke, dash, line_cap) = match kind {
            EdgeKind::Boundary => ("boundary", "#111111", None, "butt"),
            EdgeKind::Mountain => ("mountain", "#d32f2f", Some("6 2 1 2"), "butt"),
            EdgeKind::Valley => ("valley", "#1976d2", Some("3 1.5"), "butt"),
            EdgeKind::Auxiliary => ("auxiliary", "#757575", Some("0.5 1.5"), "round"),
            EdgeKind::Cut => ("cut", "#000000", Some("8 2 1 2 1 2"), "butt"),
        };
        svg.push_str("    <line class=\"");
        svg.push_str(semantic);
        svg.push_str("\" x1=\"");
        svg.push_str(&canonical_number(x1));
        svg.push_str("\" y1=\"");
        svg.push_str(&canonical_number(y1));
        svg.push_str("\" x2=\"");
        svg.push_str(&canonical_number(x2));
        svg.push_str("\" y2=\"");
        svg.push_str(&canonical_number(y2));
        svg.push_str("\" stroke=\"");
        svg.push_str(stroke);
        svg.push('"');
        if let Some(dash) = dash {
            svg.push_str(" stroke-dasharray=\"");
            svg.push_str(dash);
            svg.push('"');
        }
        svg.push_str(" stroke-linecap=\"");
        svg.push_str(line_cap);
        svg.push_str("\" data-origami-kind=\"");
        svg.push_str(semantic);
        svg.push_str("\"/>\n");
    }
    svg.push_str("  </g>\n");
    svg.push_str("  <g fill=\"#212121\">\n");
    for vertex in &pattern.vertices {
        let Some(Some(point)) = by_id.get(&vertex.id) else {
            continue;
        };
        let x = point.x - min_x;
        let y = point.y - min_y;
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        svg.push_str("    <circle cx=\"");
        svg.push_str(&canonical_number(x));
        svg.push_str("\" cy=\"");
        svg.push_str(&canonical_number(y));
        svg.push_str("\" r=\"0.75\" vector-effect=\"non-scaling-stroke\"/>\n");
    }
    svg.push_str("  </g>\n</svg>\n");
    let bytes = svg.into_bytes();
    if bytes.len() as u64 > byte_limit {
        bounded_placeholder_preview(PreviewFallback::ByteLimit, byte_limit)
    } else {
        Ok(bytes)
    }
}

fn bounded_placeholder_preview(
    reason: PreviewFallback,
    byte_limit: u64,
) -> Result<Vec<u8>, ProjectFolderError> {
    let svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" viewBox=\"0 0 100 100\" role=\"img\" aria-label=\"Read-only crease-pattern preview\" data-origami-preview=\"read-only\" data-preview-status=\"placeholder\" data-preview-reason=\"{}\">\n\
  <rect x=\"0\" y=\"0\" width=\"100\" height=\"100\" fill=\"#ffffff\" stroke=\"#757575\"/>\n\
  <path d=\"M20 50H80M50 20V80\" fill=\"none\" stroke=\"#bdbdbd\" stroke-width=\"2\"/>\n\
</svg>\n",
        reason.as_str()
    )
    .into_bytes();
    if svg.len() as u64 > byte_limit {
        return Err(ProjectFolderError::EntryTooLarge {
            path: PROJECT_FOLDER_PREVIEW_PATH.to_owned(),
            actual: svg.len() as u64,
            limit: byte_limit,
        });
    }
    Ok(svg)
}

fn canonical_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::{
        AssetId, CreasePattern, Edge, EdgeId, InstructionPose, InstructionPoseModel,
        InstructionStep, InstructionStepId, ProjectId, Vertex,
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
        let first = VertexId::new();
        let second = VertexId::new();
        let third = VertexId::new();
        ProjectDocument::new(
            "expanded folder round trip",
            CreasePattern {
                vertices: vec![
                    Vertex {
                        id: first,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: second,
                        position: Point2::new(200.0, 0.0),
                    },
                    Vertex {
                        id: third,
                        position: Point2::new(100.0, 200.0),
                    },
                ],
                edges: vec![
                    Edge {
                        id: EdgeId::new(),
                        start: first,
                        end: second,
                        kind: EdgeKind::Boundary,
                    },
                    Edge {
                        id: EdgeId::new(),
                        start: first,
                        end: third,
                        kind: EdgeKind::Mountain,
                    },
                ],
            },
        )
    }

    fn non_default_empty_history(project_id: ProjectId) -> EditorHistoryV1 {
        serde_json::from_value(serde_json::json!({
            "schema_version": EDITOR_HISTORY_SCHEMA_VERSION_V1,
            "project_id": project_id,
            "history_entry_limit": 7,
            "undo_stack": [],
            "redo_stack": [],
        }))
        .expect("valid non-default history")
    }

    fn add_declarative_instruction(document: &mut ProjectDocument) {
        document.instruction_timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: "説明専用".to_owned(),
            description: "3D姿勢を変更しません。".to_owned(),
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

    fn manifest(entries: &[ProjectFolderEntryV1]) -> ProjectFolderManifestV1 {
        serde_json::from_slice(
            &entries
                .iter()
                .find(|entry| entry.path == PROJECT_FOLDER_MANIFEST_PATH)
                .expect("manifest")
                .bytes,
        )
        .expect("manifest JSON")
    }

    fn replace_manifest(entries: &mut [ProjectFolderEntryV1], manifest: &ProjectFolderManifestV1) {
        entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_MANIFEST_PATH)
            .expect("manifest")
            .bytes = serde_json::to_vec_pretty(manifest).expect("manifest JSON");
    }

    fn descriptor_mut<'a>(
        manifest: &'a mut ProjectFolderManifestV1,
        role: &str,
    ) -> &'a mut ProjectFolderManifestEntryV1 {
        manifest
            .entries
            .iter_mut()
            .find(|entry| entry.role == role)
            .expect("descriptor")
    }

    #[test]
    fn writer_is_deterministic_and_emits_canonical_order() {
        let archive = Ori2ProjectArchive::document_only(sample_document());
        let first = write_project_folder_v1(&archive).expect("first write");
        let second = write_project_folder_v1(&archive).expect("second write");

        assert_eq!(first, second);
        assert_eq!(
            first
                .entries()
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                PROJECT_FOLDER_MANIFEST_PATH,
                PROJECT_FOLDER_PROJECT_PATH,
                PROJECT_FOLDER_PREVIEW_PATH,
            ]
        );
        assert_eq!(first.archive(), &archive);
        assert!(
            std::str::from_utf8(first.preview_svg())
                .expect("preview UTF-8")
                .contains("data-origami-preview=\"read-only\"")
        );
        let preview = std::str::from_utf8(first.preview_svg()).expect("preview UTF-8");
        let mountain = preview
            .lines()
            .find(|line| line.contains("data-origami-kind=\"mountain\""))
            .expect("mountain line");
        assert!(mountain.contains("y1=\"0\""));
        assert!(mountain.contains("y2=\"200\""));
    }

    #[test]
    fn declarative_instruction_folder_round_trip_requires_both_timeline_features() {
        let mut document = sample_document();
        add_declarative_instruction(&mut document);
        let original =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(document.clone()))
                .expect("write declarative project folder")
                .entries()
                .to_vec();
        assert_eq!(
            manifest(&original).required_features,
            vec![
                ORI2_FEATURE_INSTRUCTION_TIMELINE_V1,
                ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1,
            ]
        );
        let restored = read_project_folder_v1(&original).expect("read declarative project folder");
        assert_eq!(
            restored.archive.document.instruction_timeline,
            document.instruction_timeline
        );

        let mut missing_feature = original.clone();
        let mut missing_manifest = manifest(&missing_feature);
        missing_manifest.required_features.pop();
        replace_manifest(&mut missing_feature, &missing_manifest);
        assert!(matches!(
            read_project_folder_v1(&missing_feature),
            Err(ProjectFolderError::RequiredFeaturesMismatch { .. })
        ));
    }

    #[test]
    fn optional_history_is_authenticated_and_round_trips() {
        let document = sample_document();
        let history = non_default_empty_history(document.project_id);
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            document,
            editor_history: Some(history),
        };
        let written = write_project_folder_v1(&archive).expect("write with history");
        assert_eq!(
            written
                .entries()
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                PROJECT_FOLDER_MANIFEST_PATH,
                PROJECT_FOLDER_PROJECT_PATH,
                PROJECT_FOLDER_EDITOR_HISTORY_PATH,
                PROJECT_FOLDER_PREVIEW_PATH,
            ]
        );
        assert_eq!(written.archive(), &archive);
    }

    #[test]
    fn writer_omits_the_exact_default_empty_history() {
        let document = sample_document();
        let default_history: EditorHistoryV1 = serde_json::from_value(serde_json::json!({
            "schema_version": EDITOR_HISTORY_SCHEMA_VERSION_V1,
            "project_id": document.project_id,
            "history_entry_limit": 128,
            "undo_stack": [],
            "redo_stack": [],
        }))
        .expect("valid default history");
        let written = write_project_folder_v1(&Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(default_history),
            document,
        })
        .expect("write");

        assert!(written.archive().editor_history.is_none());
        assert_eq!(written.entries().len(), 3);
        assert!(
            written
                .entries()
                .iter()
                .all(|entry| entry.path != PROJECT_FOLDER_EDITOR_HISTORY_PATH)
        );
    }

    #[test]
    fn unfinished_empty_project_uses_safe_placeholder_instead_of_blocking_save() {
        let archive = Ori2ProjectArchive::document_only(ProjectDocument::new(
            "unfinished",
            CreasePattern::empty(),
        ));
        let written =
            write_project_folder_v1(&archive).expect("unfinished project remains savable");
        let preview = std::str::from_utf8(written.preview_svg()).expect("preview UTF-8");

        assert!(preview.contains("data-preview-status=\"placeholder\""));
        assert!(preview.contains("data-preview-reason=\"no_finite_geometry\""));
        assert!(!preview.contains("<script"));
        assert!(!preview.contains("href="));
    }

    #[test]
    fn isolated_vertices_use_partial_preview_instead_of_strict_export_validation() {
        let document = ProjectDocument::new(
            "isolated vertex",
            CreasePattern {
                vertices: vec![Vertex {
                    id: VertexId::new(),
                    position: Point2::new(1.0, 2.0),
                }],
                edges: Vec::new(),
            },
        );
        let written = write_project_folder_v1(&Ori2ProjectArchive::document_only(document))
            .expect("isolated vertex remains savable");
        let preview = std::str::from_utf8(written.preview_svg()).expect("preview UTF-8");
        assert!(preview.contains("data-preview-status=\"partial\""));
    }

    #[test]
    fn preview_uses_the_shared_five_edge_styles_without_color_dependency() {
        let origin = VertexId::new();
        let targets = std::array::from_fn::<_, 5, _>(|_| VertexId::new());
        let kinds = [
            EdgeKind::Boundary,
            EdgeKind::Mountain,
            EdgeKind::Valley,
            EdgeKind::Auxiliary,
            EdgeKind::Cut,
        ];
        let document = ProjectDocument::new(
            "all edge styles",
            CreasePattern {
                vertices: std::iter::once(Vertex {
                    id: origin,
                    position: Point2::new(0.0, 0.0),
                })
                .chain(targets.iter().enumerate().map(|(index, id)| Vertex {
                    id: *id,
                    position: Point2::new(10.0, (index + 1) as f64 * 10.0),
                }))
                .collect(),
                edges: kinds
                    .iter()
                    .zip(targets)
                    .map(|(kind, target)| Edge {
                        id: EdgeId::new(),
                        start: origin,
                        end: target,
                        kind: *kind,
                    })
                    .collect(),
            },
        );
        let written = write_project_folder_v1(&Ori2ProjectArchive::document_only(document))
            .expect("all styles");
        let preview = std::str::from_utf8(written.preview_svg()).expect("preview UTF-8");
        let expected = [
            ("boundary", None, "butt"),
            ("mountain", Some("6 2 1 2"), "butt"),
            ("valley", Some("3 1.5"), "butt"),
            ("auxiliary", Some("0.5 1.5"), "round"),
            ("cut", Some("8 2 1 2 1 2"), "butt"),
        ];

        for (semantic, dash, line_cap) in expected {
            let line = preview
                .lines()
                .find(|line| line.contains(&format!("data-origami-kind=\"{semantic}\"")))
                .expect("semantic edge line");
            assert_eq!(
                line.contains("stroke-dasharray="),
                dash.is_some(),
                "{semantic} dash presence"
            );
            if let Some(dash) = dash {
                assert!(
                    line.contains(&format!("stroke-dasharray=\"{dash}\"")),
                    "{semantic} dash"
                );
            }
            assert!(
                line.contains(&format!("stroke-linecap=\"{line_cap}\"")),
                "{semantic} line cap"
            );
        }
    }

    #[test]
    fn traversal_paths_are_rejected_before_manifest_parsing() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        entries[2].path = "preview/../crease-pattern.svg".to_owned();

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::UnsafeEntryPath { .. })
        ));
    }

    #[test]
    fn every_nonportable_path_form_is_rejected_before_manifest_parsing() {
        let original =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let unsafe_paths = [
            "",
            "/absolute.json",
            "\\\\server\\share.json",
            "C:/drive.json",
            "preview/name:stream.svg",
            "preview\\crease-pattern.svg",
            "preview//crease-pattern.svg",
            "preview/./crease-pattern.svg",
            "preview/../crease-pattern.svg",
            "preview/crease-pattern.svg/",
            "preview/name.",
            "preview/CON.svg",
            "preview/lpt9.txt",
            "preview/\u{0000}crease-pattern.svg",
            "preview/折り紙.svg",
            "preview/crease pattern.svg",
            "preview/*.svg",
        ];

        for path in unsafe_paths {
            let mut entries = original.clone();
            entries[2].path = path.to_owned();
            assert!(
                matches!(
                    read_project_folder_v1(&entries),
                    Err(ProjectFolderError::UnsafeEntryPath { .. })
                ),
                "unsafe path was accepted: {path:?}"
            );
        }
    }

    #[test]
    fn exact_duplicate_paths_are_rejected() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        entries.push(entries[1].clone());

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::DuplicateEntryPath { .. })
        ));
    }

    #[test]
    fn case_colliding_paths_are_rejected() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        entries.push(ProjectFolderEntryV1::new("Manifest.json", b"{}".to_vec()));

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::EntryPathCaseCollision { .. })
        ));
    }

    #[test]
    fn entry_count_is_hard_bounded() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        entries.push(ProjectFolderEntryV1::new("extra-a.json", b"{}".to_vec()));
        entries.push(ProjectFolderEntryV1::new("extra-b.json", b"{}".to_vec()));

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::TooManyEntries {
                actual: 5,
                limit: MAX_PROJECT_FOLDER_ENTRY_COUNT
            })
        ));
    }

    #[test]
    fn unknown_mandatory_role_is_rejected_explicitly() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let mut manifest = manifest(&entries);
        manifest
            .required_roles
            .push("future_required_role".to_owned());
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::UnknownRequiredRole { role })
                if role == "future_required_role"
        ));
    }

    #[test]
    fn unknown_required_feature_is_rejected_explicitly() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let mut manifest = manifest(&entries);
        manifest
            .required_features
            .push("future_project_feature".to_owned());
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::UnsupportedRequiredFeatures { features })
                if features == vec!["future_project_feature"]
        ));
    }

    #[test]
    fn manifest_and_history_envelopes_reject_unknown_fields() {
        let mut document_only_entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let manifest_entry = document_only_entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_MANIFEST_PATH)
            .expect("manifest");
        let mut manifest_value: serde_json::Value =
            serde_json::from_slice(&manifest_entry.bytes).expect("manifest JSON");
        manifest_value["unknown_field"] = serde_json::Value::Bool(true);
        manifest_entry.bytes = serde_json::to_vec_pretty(&manifest_value).expect("manifest JSON");
        assert!(matches!(
            read_project_folder_v1(&document_only_entries),
            Err(ProjectFolderError::InvalidManifestJson(_))
        ));

        let document = sample_document();
        let mut history_entries = write_project_folder_v1(&Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        })
        .expect("write")
        .entries()
        .to_vec();
        let history_entry = history_entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_EDITOR_HISTORY_PATH)
            .expect("history");
        let mut history_value: serde_json::Value =
            serde_json::from_slice(&history_entry.bytes).expect("history JSON");
        history_value["unknown_field"] = serde_json::Value::Bool(true);
        history_entry.bytes = serde_json::to_vec_pretty(&history_value).expect("history JSON");
        let hash = sha256_hex(&history_entry.bytes);
        let size = history_entry.bytes.len() as u64;
        let mut history_manifest = manifest(&history_entries);
        let descriptor = descriptor_mut(&mut history_manifest, PROJECT_FOLDER_ROLE_EDITOR_HISTORY);
        descriptor.sha256 = hash;
        descriptor.uncompressed_size = size;
        replace_manifest(&mut history_entries, &history_manifest);
        assert!(matches!(
            read_project_folder_v1(&history_entries),
            Err(ProjectFolderError::InvalidEditorHistoryJson(_))
        ));
    }

    #[test]
    fn unsafe_manifest_descriptor_path_is_rejected() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let mut manifest = manifest(&entries);
        descriptor_mut(&mut manifest, PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW).path =
            "preview/../outside.svg".to_owned();
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::UnsafeEntryPath { .. })
        ));
    }

    #[test]
    fn manifest_duplicate_role_path_and_case_collision_are_distinguished() {
        let original =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();

        let mut duplicate_role = original.clone();
        let mut duplicate_role_manifest = manifest(&duplicate_role);
        duplicate_role_manifest
            .entries
            .push(duplicate_role_manifest.entries[0].clone());
        replace_manifest(&mut duplicate_role, &duplicate_role_manifest);
        assert!(matches!(
            read_project_folder_v1(&duplicate_role),
            Err(ProjectFolderError::DuplicateManifestRole { .. })
        ));

        let mut duplicate_path = original.clone();
        let mut duplicate_path_manifest = manifest(&duplicate_path);
        descriptor_mut(
            &mut duplicate_path_manifest,
            PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW,
        )
        .path = PROJECT_FOLDER_PROJECT_PATH.to_owned();
        replace_manifest(&mut duplicate_path, &duplicate_path_manifest);
        assert!(matches!(
            read_project_folder_v1(&duplicate_path),
            Err(ProjectFolderError::DuplicateManifestPath { .. })
        ));

        let mut case_collision = original;
        let mut case_collision_manifest = manifest(&case_collision);
        descriptor_mut(
            &mut case_collision_manifest,
            PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW,
        )
        .path = "Project.json".to_owned();
        replace_manifest(&mut case_collision, &case_collision_manifest);
        assert!(matches!(
            read_project_folder_v1(&case_collision),
            Err(ProjectFolderError::EntryPathCaseCollision { .. })
        ));
    }

    #[test]
    fn role_path_content_type_and_schema_are_fixed() {
        let original =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();

        let mut wrong_path = original.clone();
        let mut wrong_path_manifest = manifest(&wrong_path);
        descriptor_mut(
            &mut wrong_path_manifest,
            PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW,
        )
        .path = "preview/other.svg".to_owned();
        replace_manifest(&mut wrong_path, &wrong_path_manifest);
        assert!(matches!(
            read_project_folder_v1(&wrong_path),
            Err(ProjectFolderError::InvalidRolePath { .. })
        ));

        let mut wrong_content_type = original.clone();
        let mut wrong_content_type_manifest = manifest(&wrong_content_type);
        descriptor_mut(
            &mut wrong_content_type_manifest,
            PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW,
        )
        .content_type = "text/html".to_owned();
        replace_manifest(&mut wrong_content_type, &wrong_content_type_manifest);
        assert!(matches!(
            read_project_folder_v1(&wrong_content_type),
            Err(ProjectFolderError::InvalidContentType { .. })
        ));

        let mut wrong_schema = original;
        let mut wrong_schema_manifest = manifest(&wrong_schema);
        descriptor_mut(
            &mut wrong_schema_manifest,
            PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW,
        )
        .schema_version += 1;
        replace_manifest(&mut wrong_schema, &wrong_schema_manifest);
        assert!(matches!(
            read_project_folder_v1(&wrong_schema),
            Err(ProjectFolderError::UnsupportedRoleSchemaVersion { .. })
        ));
    }

    #[test]
    fn required_roles_reject_missing_duplicate_and_reordered_values() {
        let document = sample_document();
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let original = write_project_folder_v1(&archive)
            .expect("write")
            .entries()
            .to_vec();

        let mut mutations = Vec::new();
        let mut missing = manifest(&original);
        missing.required_roles.pop();
        mutations.push(missing);
        let mut duplicate = manifest(&original);
        duplicate
            .required_roles
            .push(PROJECT_FOLDER_ROLE_PROJECT.to_owned());
        mutations.push(duplicate);
        let mut reordered = manifest(&original);
        reordered.required_roles.swap(0, 1);
        mutations.push(reordered);

        for mutated_manifest in mutations {
            let mut entries = original.clone();
            replace_manifest(&mut entries, &mutated_manifest);
            assert!(matches!(
                read_project_folder_v1(&entries),
                Err(ProjectFolderError::RequiredRolesMismatch { .. })
            ));
        }
    }

    #[test]
    fn required_features_reject_missing_duplicate_reordered_and_excess_known_values() {
        let mut document = sample_document();
        document.numeric_expressions.rectangular_paper_creation = Some(
            crate::RectangularPaperCreationExpressions::new("200", "200", 200.0, 200.0),
        );
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let original = write_project_folder_v1(&archive)
            .expect("write")
            .entries()
            .to_vec();
        assert_eq!(
            manifest(&original).required_features,
            vec![
                ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1,
                ORI2_FEATURE_EDITOR_HISTORY_V1
            ]
        );

        let mut mutations = Vec::new();
        let mut missing = manifest(&original);
        missing.required_features.pop();
        mutations.push(missing);
        let mut duplicate = manifest(&original);
        duplicate
            .required_features
            .push(ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned());
        mutations.push(duplicate);
        let mut reordered = manifest(&original);
        reordered.required_features.swap(0, 1);
        mutations.push(reordered);
        let mut excess = manifest(&original);
        excess
            .required_features
            .insert(1, ORI2_FEATURE_LAYERS_V1.to_owned());
        mutations.push(excess);

        for mutated_manifest in mutations {
            let mut entries = original.clone();
            replace_manifest(&mut entries, &mutated_manifest);
            assert!(matches!(
                read_project_folder_v1(&entries),
                Err(ProjectFolderError::RequiredFeaturesMismatch { .. })
            ));
        }
    }

    #[test]
    fn project_size_and_hash_are_independently_authenticated() {
        let original =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();

        let mut wrong_size = original.clone();
        let mut size_manifest = manifest(&wrong_size);
        descriptor_mut(&mut size_manifest, PROJECT_FOLDER_ROLE_PROJECT).uncompressed_size += 1;
        replace_manifest(&mut wrong_size, &size_manifest);
        assert!(matches!(
            read_project_folder_v1(&wrong_size),
            Err(ProjectFolderError::EntrySizeMismatch { path, .. })
                if path == PROJECT_FOLDER_PROJECT_PATH
        ));

        let mut wrong_hash = original;
        let mut hash_manifest = manifest(&wrong_hash);
        descriptor_mut(&mut hash_manifest, PROJECT_FOLDER_ROLE_PROJECT).sha256 = "0".repeat(64);
        replace_manifest(&mut wrong_hash, &hash_manifest);
        assert!(matches!(
            read_project_folder_v1(&wrong_hash),
            Err(ProjectFolderError::EntryHashMismatch { path, .. })
                if path == PROJECT_FOLDER_PROJECT_PATH
        ));
    }

    #[test]
    fn every_payload_role_has_independent_size_and_hash_authentication() {
        let document = sample_document();
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let original = write_project_folder_v1(&archive)
            .expect("write")
            .entries()
            .to_vec();
        let roles = [
            PROJECT_FOLDER_ROLE_PROJECT,
            PROJECT_FOLDER_ROLE_EDITOR_HISTORY,
            PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW,
        ];

        for role in roles {
            let mut wrong_size = original.clone();
            let mut size_manifest = manifest(&wrong_size);
            let path = descriptor_mut(&mut size_manifest, role).path.clone();
            descriptor_mut(&mut size_manifest, role).uncompressed_size += 1;
            replace_manifest(&mut wrong_size, &size_manifest);
            assert!(
                matches!(
                    read_project_folder_v1(&wrong_size),
                    Err(ProjectFolderError::EntrySizeMismatch {
                        path: error_path,
                        ..
                    }) if error_path == path
                ),
                "size authentication for {role}"
            );

            let mut wrong_hash = original.clone();
            let mut hash_manifest = manifest(&wrong_hash);
            let path = descriptor_mut(&mut hash_manifest, role).path.clone();
            descriptor_mut(&mut hash_manifest, role).sha256 = "0".repeat(64);
            replace_manifest(&mut wrong_hash, &hash_manifest);
            assert!(
                matches!(
                    read_project_folder_v1(&wrong_hash),
                    Err(ProjectFolderError::EntryHashMismatch {
                        path: error_path,
                        ..
                    }) if error_path == path
                ),
                "hash authentication for {role}"
            );
        }
    }

    #[test]
    fn noncanonical_physical_order_is_rejected() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        entries.swap(1, 2);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::NonCanonicalEntryOrder { .. })
        ));
    }

    #[test]
    fn unknown_extra_physical_and_manifest_entries_are_rejected() {
        let original =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();

        let mut extra_physical = original.clone();
        extra_physical.push(ProjectFolderEntryV1::new("extra.json", b"{}".to_vec()));
        assert!(matches!(
            read_project_folder_v1(&extra_physical),
            Err(ProjectFolderError::NonCanonicalEntryOrder { .. })
        ));

        let mut extra_descriptor = original;
        let mut extra_descriptor_manifest = manifest(&extra_descriptor);
        extra_descriptor_manifest
            .entries
            .push(ProjectFolderManifestEntryV1 {
                role: "future_optional_role".to_owned(),
                path: "future.json".to_owned(),
                content_type: PROJECT_JSON_CONTENT_TYPE.to_owned(),
                schema_version: 1,
                uncompressed_size: 2,
                sha256: sha256_hex(b"{}"),
            });
        replace_manifest(&mut extra_descriptor, &extra_descriptor_manifest);
        assert!(matches!(
            read_project_folder_v1(&extra_descriptor),
            Err(ProjectFolderError::UnknownEntryRole { role })
                if role == "future_optional_role"
        ));
    }

    #[test]
    fn noncanonical_manifest_descriptor_order_is_rejected() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let mut manifest = manifest(&entries);
        manifest.entries.swap(0, 1);
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::RequiredRolesMismatch { .. })
        ));
    }

    #[test]
    fn container_version_is_fixed() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let mut manifest = manifest(&entries);
        manifest.container_version += 1;
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::UnsupportedContainerVersion {
                found: 2,
                latest: CURRENT_PROJECT_FOLDER_VERSION
            })
        ));
    }

    #[test]
    fn forged_preview_is_rejected_even_when_manifest_hash_is_updated() {
        let mut entries =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write")
                .entries()
                .to_vec();
        let preview = entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_PREVIEW_PATH)
            .expect("preview");
        preview.bytes = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script/></svg>".to_vec();
        let forged_hash = sha256_hex(&preview.bytes);
        let forged_size = preview.bytes.len() as u64;
        let mut manifest = manifest(&entries);
        let descriptor = descriptor_mut(&mut manifest, PROJECT_FOLDER_ROLE_CREASE_PATTERN_PREVIEW);
        descriptor.sha256 = forged_hash;
        descriptor.uncompressed_size = forged_size;
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::PreviewProjectionMismatch)
        ));
    }

    #[test]
    fn history_cannot_be_rebound_by_updating_only_its_descriptor_hash() {
        let document = sample_document();
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let mut entries = write_project_folder_v1(&archive)
            .expect("write")
            .entries()
            .to_vec();
        let history = entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_EDITOR_HISTORY_PATH)
            .expect("history");
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&history.bytes).expect("history JSON");
        envelope["project_sha256"] = serde_json::Value::String("0".repeat(64));
        history.bytes = serde_json::to_vec_pretty(&envelope).expect("history JSON");
        let forged_hash = sha256_hex(&history.bytes);
        let forged_size = history.bytes.len() as u64;
        let mut manifest = manifest(&entries);
        let descriptor = descriptor_mut(&mut manifest, PROJECT_FOLDER_ROLE_EDITOR_HISTORY);
        descriptor.sha256 = forged_hash;
        descriptor.uncompressed_size = forged_size;
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::EditorHistoryProjectHashMismatch)
        ));
    }

    #[test]
    fn history_project_id_cannot_be_rebound_even_with_updated_descriptor_hash() {
        let document = sample_document();
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let mut entries = write_project_folder_v1(&archive)
            .expect("write")
            .entries()
            .to_vec();
        let history = entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_EDITOR_HISTORY_PATH)
            .expect("history");
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&history.bytes).expect("history JSON");
        envelope["history"]["project_id"] =
            serde_json::to_value(ProjectId::new()).expect("project ID JSON");
        history.bytes = serde_json::to_vec_pretty(&envelope).expect("history JSON");
        let hash = sha256_hex(&history.bytes);
        let size = history.bytes.len() as u64;
        let mut manifest = manifest(&entries);
        let descriptor = descriptor_mut(&mut manifest, PROJECT_FOLDER_ROLE_EDITOR_HISTORY);
        descriptor.sha256 = hash;
        descriptor.uncompressed_size = size;
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::EditorHistoryProjectIdMismatch)
        ));
    }

    #[test]
    fn exact_default_empty_history_is_rejected_as_noncanonical() {
        let document = sample_document();
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let mut entries = write_project_folder_v1(&archive)
            .expect("write")
            .entries()
            .to_vec();
        let history = entries
            .iter_mut()
            .find(|entry| entry.path == PROJECT_FOLDER_EDITOR_HISTORY_PATH)
            .expect("history");
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&history.bytes).expect("history JSON");
        envelope["history"]["history_entry_limit"] = serde_json::Value::from(128);
        history.bytes = serde_json::to_vec_pretty(&envelope).expect("history JSON");
        let hash = sha256_hex(&history.bytes);
        let size = history.bytes.len() as u64;
        let mut manifest = manifest(&entries);
        let descriptor = descriptor_mut(&mut manifest, PROJECT_FOLDER_ROLE_EDITOR_HISTORY);
        descriptor.sha256 = hash;
        descriptor.uncompressed_size = size;
        replace_manifest(&mut entries, &manifest);

        assert!(matches!(
            read_project_folder_v1(&entries),
            Err(ProjectFolderError::DefaultEditorHistoryMustBeOmitted)
        ));
    }

    #[test]
    fn read_write_read_is_byte_stable() {
        let document = sample_document();
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let first = write_project_folder_v1(&archive).expect("first write");
        let first_read = read_project_folder_v1(first.entries()).expect("first read");
        let second = write_project_folder_v1(first_read.archive()).expect("second write");
        let second_read = read_project_folder_v1(second.entries()).expect("second read");

        assert_eq!(first.entries(), second.entries());
        assert_eq!(first_read.archive(), second_read.archive());
    }

    #[test]
    fn manifest_caller_limit_accepts_exact_and_rejects_one_short() {
        let archive = Ori2ProjectArchive::document_only(sample_document());
        let written = write_project_folder_v1(&archive).expect("write");
        let manifest_size = written
            .entries()
            .iter()
            .find(|entry| entry.path == PROJECT_FOLDER_MANIFEST_PATH)
            .expect("manifest")
            .bytes
            .len() as u64;
        let exact = ProjectFolderLimits {
            max_manifest_bytes: manifest_size,
            ..ProjectFolderLimits::default()
        };
        read_project_folder_v1_with_limits(written.entries(), exact).expect("exact manifest limit");

        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_manifest_bytes: manifest_size - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::EntryTooLarge { path, .. })
                if path == PROJECT_FOLDER_MANIFEST_PATH
        ));
    }

    #[test]
    fn every_payload_role_caller_limit_accepts_exact_and_rejects_one_short() {
        let document = sample_document();
        let archive = Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        };
        let written = write_project_folder_v1(&archive).expect("write");
        let size = |path: &str| {
            written
                .entries()
                .iter()
                .find(|entry| entry.path == path)
                .expect("role entry")
                .bytes
                .len() as u64
        };

        let project_size = size(PROJECT_FOLDER_PROJECT_PATH);
        read_project_folder_v1_with_limits(
            written.entries(),
            ProjectFolderLimits {
                max_project_bytes: project_size,
                ..ProjectFolderLimits::default()
            },
        )
        .expect("exact project limit");
        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_project_bytes: project_size - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::EntryTooLarge { path, .. })
                if path == PROJECT_FOLDER_PROJECT_PATH
        ));

        let history_size = size(PROJECT_FOLDER_EDITOR_HISTORY_PATH);
        read_project_folder_v1_with_limits(
            written.entries(),
            ProjectFolderLimits {
                max_editor_history_bytes: history_size,
                ..ProjectFolderLimits::default()
            },
        )
        .expect("exact history limit");
        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_editor_history_bytes: history_size - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::EntryTooLarge { path, .. })
                if path == PROJECT_FOLDER_EDITOR_HISTORY_PATH
        ));

        let preview_size = size(PROJECT_FOLDER_PREVIEW_PATH);
        read_project_folder_v1_with_limits(
            written.entries(),
            ProjectFolderLimits {
                max_preview_bytes: preview_size,
                ..ProjectFolderLimits::default()
            },
        )
        .expect("exact preview limit");
        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_preview_bytes: preview_size - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::EntryTooLarge { path, .. })
                if path == PROJECT_FOLDER_PREVIEW_PATH
        ));
    }

    #[test]
    fn generic_entry_caller_limit_accepts_exact_and_rejects_one_short() {
        let written =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write");
        let largest = written
            .entries()
            .iter()
            .max_by_key(|entry| entry.bytes.len())
            .expect("largest entry");
        let largest_size = largest.bytes.len() as u64;
        let largest_path = largest.path.clone();
        read_project_folder_v1_with_limits(
            written.entries(),
            ProjectFolderLimits {
                max_entry_bytes: largest_size,
                ..ProjectFolderLimits::default()
            },
        )
        .expect("exact generic entry limit");
        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_entry_bytes: largest_size - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::EntryTooLarge { path, .. }) if path == largest_path
        ));
    }

    #[test]
    fn preview_byte_pressure_falls_back_without_blocking_normal_save() {
        let placeholder =
            bounded_placeholder_preview(PreviewFallback::ByteLimit, u64::MAX).expect("placeholder");
        let limits = ProjectFolderLimits {
            max_preview_bytes: placeholder.len() as u64,
            ..ProjectFolderLimits::default()
        };
        let written = write_project_folder_v1_with_limits(
            &Ori2ProjectArchive::document_only(sample_document()),
            limits,
        )
        .expect("placeholder fallback");
        let preview = std::str::from_utf8(written.preview_svg()).expect("preview UTF-8");
        assert!(preview.contains("data-preview-status=\"placeholder\""));
        assert!(preview.contains("data-preview-reason=\"byte_limit\""));

        assert!(matches!(
            write_project_folder_v1_with_limits(
                &Ori2ProjectArchive::document_only(sample_document()),
                ProjectFolderLimits {
                    max_preview_bytes: placeholder.len() as u64 - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::EntryTooLarge { path, .. })
                if path == PROJECT_FOLDER_PREVIEW_PATH
        ));
    }

    #[test]
    fn total_caller_limit_accepts_exact_and_rejects_one_short() {
        let written =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write");
        let total_size = written
            .entries()
            .iter()
            .map(|entry| entry.bytes.len() as u64)
            .sum::<u64>();
        read_project_folder_v1_with_limits(
            written.entries(),
            ProjectFolderLimits {
                max_total_bytes: total_size,
                ..ProjectFolderLimits::default()
            },
        )
        .expect("exact total limit");
        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_total_bytes: total_size - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::TotalTooLarge { .. })
        ));
    }

    #[test]
    fn path_caller_limit_accepts_exact_and_rejects_one_short() {
        let written =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write");
        let longest_path = written
            .entries()
            .iter()
            .map(|entry| entry.path.len())
            .max()
            .expect("entry path");
        read_project_folder_v1_with_limits(
            written.entries(),
            ProjectFolderLimits {
                max_entry_path_bytes: longest_path,
                ..ProjectFolderLimits::default()
            },
        )
        .expect("exact path limit");
        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_entry_path_bytes: longest_path - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::EntryPathTooLong { .. })
        ));
    }

    #[test]
    fn entry_count_caller_limit_accepts_exact_and_rejects_one_short() {
        let document = sample_document();
        let written = write_project_folder_v1(&Ori2ProjectArchive {
            layer_evidence: None,
            editor_history: Some(non_default_empty_history(document.project_id)),
            document,
        })
        .expect("write");
        read_project_folder_v1_with_limits(
            written.entries(),
            ProjectFolderLimits {
                max_entry_count: written.entries().len(),
                ..ProjectFolderLimits::default()
            },
        )
        .expect("exact entry-count limit");
        assert!(matches!(
            read_project_folder_v1_with_limits(
                written.entries(),
                ProjectFolderLimits {
                    max_entry_count: written.entries().len() - 1,
                    ..ProjectFolderLimits::default()
                }
            ),
            Err(ProjectFolderError::TooManyEntries { .. })
        ));
    }

    #[test]
    fn caller_values_above_hard_ceilings_do_not_change_valid_output() {
        let written =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(sample_document()))
                .expect("write");
        let relaxed = ProjectFolderLimits {
            max_entry_count: usize::MAX,
            max_entry_path_bytes: usize::MAX,
            max_entry_bytes: u64::MAX,
            max_manifest_bytes: u64::MAX,
            max_project_bytes: u64::MAX,
            max_editor_history_bytes: u64::MAX,
            max_preview_bytes: u64::MAX,
            max_total_bytes: u64::MAX,
        };
        let read =
            read_project_folder_v1_with_limits(written.entries(), relaxed).expect("bounded read");
        assert_eq!(read, written);
    }

    #[test]
    fn reference_model_assets_require_the_expanded_folder_feature() {
        let mut document = sample_document();
        document
            .reference_model_assets
            .push(crate::ProjectReferenceModelAssetV1 {
                id: AssetId::new(),
                bytes: minimal_reference_glb(),
            });
        let original =
            write_project_folder_v1(&Ori2ProjectArchive::document_only(document.clone()))
                .expect("write reference model folder");
        assert_eq!(
            manifest(original.entries()).required_features,
            vec![ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1.to_owned()]
        );
        assert_eq!(
            read_project_folder_v1(original.entries())
                .expect("read reference model folder")
                .archive()
                .document
                .reference_model_assets,
            document.reference_model_assets
        );

        let mut missing = original.entries().to_vec();
        let mut missing_manifest = manifest(&missing);
        missing_manifest.required_features.clear();
        replace_manifest(&mut missing, &missing_manifest);
        assert!(matches!(
            read_project_folder_v1(&missing),
            Err(ProjectFolderError::RequiredFeaturesMismatch { .. })
        ));
    }
}
