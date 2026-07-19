//! Safe reader and writer for the single-file `.ori2` container.
//!
//! Container version 1 deliberately rejects multi-disk and ZIP64 archives;
//! its resource limits are well below the thresholds that require either.

use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{FormatError, ProjectDocument, read_project_json, write_project_json};

pub const ORI2_CONTAINER_IDENTIFIER: &str = "ORIGAMI2";
pub const CURRENT_ORI2_CONTAINER_VERSION: u32 = 1;
pub const ORI2_MANIFEST_PATH: &str = "manifest.json";
pub const ORI2_PROJECT_PATH: &str = "project.json";
pub const ORI2_FEATURE_INSTRUCTION_TIMELINE_V1: &str = "instruction_timeline_v1";
pub const ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1: &str = "numeric_expressions_v1";

const REQUIRED_ENTRY_COUNT: usize = 2;
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
}

impl Default for Ori2Limits {
    fn default() -> Self {
        Self {
            max_archive_size: 64 * 1024 * 1024,
            max_entry_count: 4_096,
            max_entry_path_length: 1_024,
            max_entry_uncompressed_size: 128 * 1024 * 1024,
            max_total_uncompressed_size: 256 * 1024 * 1024,
            max_manifest_size: 1024 * 1024,
            max_project_size: 128 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ori2Manifest {
    pub container: String,
    pub container_version: u32,
    #[serde(default)]
    pub required_features: Vec<String>,
    pub project: Ori2ProjectEntry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ori2ProjectEntry {
    pub path: String,
    pub format_version: u32,
    pub uncompressed_size: u64,
    pub sha256: String,
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
        }
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
    if document.format_version != crate::CURRENT_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: document.format_version,
            latest: crate::CURRENT_FORMAT_VERSION,
        });
    }
    ensure_entry_count(REQUIRED_ENTRY_COUNT, limits)?;
    ensure_path_length(ORI2_MANIFEST_PATH, limits)?;
    ensure_path_length(ORI2_PROJECT_PATH, limits)?;

    let project_bytes = write_project_json(document)?;
    ensure_entry_size(ORI2_PROJECT_PATH, project_bytes.len() as u64, limits)?;
    ensure_specific_size(
        ORI2_PROJECT_PATH,
        project_bytes.len() as u64,
        limits.max_project_size,
    )?;

    let mut required_features = Vec::new();
    if !document.instruction_timeline.steps.is_empty() {
        required_features.push(ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned());
    }
    if !document.numeric_expressions.is_empty() {
        required_features.push(ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1.to_owned());
    }
    let manifest = Ori2Manifest::new(&project_bytes, document.format_version, required_features);
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

    let bytes = archive.finish()?.into_inner();
    ensure_archive_size(bytes.len() as u64, limits)?;
    Ok(bytes)
}

/// Reads and validates a project from a ZIP-based `.ori2` container.
pub fn read_project_ori2(bytes: &[u8]) -> Result<ProjectDocument, FormatError> {
    read_project_ori2_with_limits(bytes, Ori2Limits::default())
}

/// Reads a project with explicit resource limits.
///
/// Every entry is inspected before data is expanded. Paths must be portable,
/// relative UTF-8 paths without traversal components. Declared and actually
/// read sizes are independently bounded.
pub fn read_project_ori2_with_limits(
    bytes: &[u8],
    limits: Ori2Limits,
) -> Result<ProjectDocument, FormatError> {
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

    ensure_specific_size(
        ORI2_PROJECT_PATH,
        manifest.project.uncompressed_size,
        limits.max_project_size,
    )?;
    let archived_project_size = archive.by_name(ORI2_PROJECT_PATH)?.size();
    if manifest.project.uncompressed_size != archived_project_size {
        return Err(FormatError::ProjectSizeMismatch {
            declared: manifest.project.uncompressed_size,
            actual: archived_project_size,
        });
    }
    let project_limit = limits
        .max_project_size
        .min(limits.max_entry_uncompressed_size);
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

    let project = read_project_json(&project_bytes)?;
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
    Ok(project)
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
    let unsupported_features = manifest
        .required_features
        .iter()
        .filter(|feature| {
            !matches!(
                feature.as_str(),
                ORI2_FEATURE_INSTRUCTION_TIMELINE_V1 | ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported_features.is_empty() {
        return Err(FormatError::UnsupportedRequiredFeatures {
            features: unsupported_features,
        });
    }
    if manifest.project.path != ORI2_PROJECT_PATH {
        return Err(FormatError::InvalidManifestProjectPath {
            found: manifest.project.path.clone(),
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::{
        AssetId, CreasePattern, Edge, EdgeId, EdgeKind, FaceId, InstructionHingeAngle,
        InstructionPose, InstructionPoseModel, InstructionStep, InstructionStepId, Paper,
        PaperAppearance, Point2, RgbaColor, Vertex, VertexId,
    };

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

    fn raw_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (path, bytes) in entries {
            writer.start_file(*path, options).expect("start test entry");
            writer.write_all(bytes).expect("write test entry");
        }
        writer.finish().expect("finish test ZIP").into_inner()
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
        assert_eq!(archive.len(), REQUIRED_ENTRY_COUNT);
        assert!(archive.by_name(ORI2_MANIFEST_PATH).is_ok());
        assert!(archive.by_name(ORI2_PROJECT_PATH).is_ok());
    }

    #[test]
    fn writer_is_byte_deterministic_and_fixes_zip_metadata() {
        assert_eq!(
            ORI2_DEFLATE_LEVEL, 6,
            "container v1 fixes the compression level"
        );
        let mut original = sample_document();
        add_sample_instruction(&mut original);

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
    fn rejects_unknown_required_feature_but_accepts_known_timeline_feature() {
        let project = write_project_json(&sample_document()).expect("project JSON");
        let known_manifest = manifest_for_features(
            &project,
            vec![ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned()],
        );
        let known_bytes = raw_zip(&[
            (ORI2_MANIFEST_PATH, &known_manifest),
            (ORI2_PROJECT_PATH, &project),
        ]);
        read_project_ori2(&known_bytes).expect("known required feature");

        let unknown_manifest = manifest_for_features(
            &project,
            vec![
                ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned(),
                "future_fold_solver_v9".to_owned(),
            ],
        );
        let unknown_bytes = raw_zip(&[
            (ORI2_MANIFEST_PATH, &unknown_manifest),
            (ORI2_PROJECT_PATH, &project),
        ]);
        let error =
            read_project_ori2(&unknown_bytes).expect_err("unknown required feature must fail");
        assert!(matches!(
            error,
            FormatError::UnsupportedRequiredFeatures { features }
                if features == vec!["future_fold_solver_v9".to_owned()]
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

        let error =
            read_project_ori2(&bytes).expect_err("timeline feature declaration is required");
        assert!(matches!(
            error,
            FormatError::MissingRequiredFeature {
                feature: ORI2_FEATURE_INSTRUCTION_TIMELINE_V1
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

        let bytes = write_project_ori2(&original).expect("write .ori2 with paper");
        let restored = read_project_ori2(&bytes).expect("read .ori2 with paper");

        assert_eq!(restored.paper, original.paper);
        assert_eq!(
            restored.paper.length_display_unit,
            original.paper.length_display_unit
        );
        assert_eq!(restored.paper.front.texture_asset, Some(front_texture));
        assert_eq!(restored.paper.back.texture_asset, Some(back_texture));
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
}
