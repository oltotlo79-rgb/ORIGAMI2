//! Safe reader and writer for the single-file `.ori2` container.
//!
//! Container version 1 deliberately rejects multi-disk and ZIP64 archives;
//! its resource limits are well below the thresholds that require either.

use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{FormatError, ProjectDocument, read_project_json, write_project_json};

pub const ORI2_CONTAINER_IDENTIFIER: &str = "ORIGAMI2";
pub const CURRENT_ORI2_CONTAINER_VERSION: u32 = 1;
pub const ORI2_MANIFEST_PATH: &str = "manifest.json";
pub const ORI2_PROJECT_PATH: &str = "project.json";

const REQUIRED_ENTRY_COUNT: usize = 2;
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
    fn new(project_bytes: &[u8], project_format_version: u32) -> Self {
        Self {
            container: ORI2_CONTAINER_IDENTIFIER.to_owned(),
            container_version: CURRENT_ORI2_CONTAINER_VERSION,
            required_features: Vec::new(),
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

    let manifest = Ori2Manifest::new(&project_bytes, document.format_version);
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
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
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
    if !manifest.required_features.is_empty() {
        return Err(FormatError::UnsupportedRequiredFeatures {
            features: manifest.required_features.clone(),
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
    use ori_domain::{
        AssetId, CreasePattern, Edge, EdgeId, EdgeKind, Paper, PaperAppearance, Point2, RgbaColor,
        Vertex, VertexId,
    };

    use super::*;

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
        serde_json::to_vec(&Ori2Manifest::new(
            project_bytes,
            crate::CURRENT_FORMAT_VERSION,
        ))
        .expect("serialize manifest")
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
        let mut limits = Ori2Limits::default();
        limits.max_archive_size = bytes.len() as u64 - 1;
        let error =
            read_project_ori2_with_limits(&bytes, limits).expect_err("oversized archive must fail");
        assert!(matches!(error, FormatError::ContainerTooLarge { .. }));
    }

    #[test]
    fn writer_rejects_project_larger_than_configured_limit() {
        let mut limits = Ori2Limits::default();
        limits.max_project_size = 1;
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
        let mut limits = Ori2Limits::default();
        limits.max_entry_uncompressed_size = 8;
        let error = read_project_ori2_with_limits(&bytes, limits)
            .expect_err("oversized expanded entry must fail");
        assert!(matches!(error, FormatError::EntryTooLarge { .. }));
    }

    #[test]
    fn rejects_total_uncompressed_size_larger_than_configured_limit() {
        let bytes = write_project_ori2(&sample_document()).expect("write .ori2");
        let mut limits = Ori2Limits::default();
        limits.max_total_uncompressed_size = 1;
        let error = read_project_ori2_with_limits(&bytes, limits)
            .expect_err("oversized expanded archive must fail");
        assert!(matches!(error, FormatError::ExpandedArchiveTooLarge { .. }));
    }

    #[test]
    fn rejects_project_whose_checksum_does_not_match_manifest() {
        let project = write_project_json(&sample_document()).expect("project JSON");
        let mut manifest = Ori2Manifest::new(&project, crate::CURRENT_FORMAT_VERSION);
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
        let mut manifest = Ori2Manifest::new(&project, crate::CURRENT_FORMAT_VERSION);
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
        let mut manifest = Ori2Manifest::new(&project, crate::CURRENT_FORMAT_VERSION);
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
