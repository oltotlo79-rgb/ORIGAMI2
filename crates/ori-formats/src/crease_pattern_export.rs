//! Deterministic, bounded crease-pattern export.
//!
//! The exporters in this module deliberately emit only static 2D interchange
//! or print subsets. They do not encode a folded pose or executable content.

mod dxf;
mod pdf;

use std::{collections::HashMap, fmt::Write as _};

use ori_domain::{
    BeginnerGenerationProvenanceV1, CreasePattern, EdgeKind, Paper, Point2, VertexId,
    validate_beginner_generation_provenance_v1,
};
use ori_geometry::{
    PaperValidationIssue, ValidationIssue, validate_crease_pattern, validate_paper,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default maximum size of one exported crease-pattern file.
pub const MAX_CREASE_PATTERN_EXPORT_BYTES: usize = 16 * 1024 * 1024;
/// Default maximum number of vertices in one exported crease pattern.
pub const MAX_CREASE_PATTERN_EXPORT_VERTICES: usize = 10_000;
/// Default maximum number of edges in one exported crease pattern.
pub const MAX_CREASE_PATTERN_EXPORT_EDGES: usize = 10_000;
/// Default maximum number of paper-boundary vertices.
pub const MAX_CREASE_PATTERN_EXPORT_BOUNDARY_VERTICES: usize = 1_414;
/// Default maximum number of broad-phase edge-intersection candidates.
pub const MAX_CREASE_PATTERN_EXPORT_INTERSECTION_CANDIDATES: usize = 1_000_000;
/// Default maximum title length, counted as Unicode scalar values.
pub const MAX_CREASE_PATTERN_EXPORT_TITLE_CHARS: usize = 512;

/// Static crease-pattern interchange format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CreasePatternExportFormat {
    /// FOLD specification 1.2 JSON.
    #[serde(rename = "fold")]
    Fold12,
    /// Static SVG 1.1 using only straight line elements.
    #[serde(rename = "svg")]
    Svg,
    /// One-page, full-scale vector PDF 1.7.
    #[serde(rename = "pdf")]
    Pdf17,
    /// UTF-8 text-form AutoCAD 2007 DXF.
    #[serde(rename = "dxf")]
    Dxf2007Ascii,
}

impl CreasePatternExportFormat {
    /// MIME type suitable for a save dialog or HTTP response.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Fold12 => "application/json",
            Self::Svg => "image/svg+xml",
            Self::Pdf17 => "application/pdf",
            Self::Dxf2007Ascii => "image/vnd.dxf",
        }
    }

    /// Conventional file extension without a leading dot.
    #[must_use]
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Fold12 => "fold",
            Self::Svg => "svg",
            Self::Pdf17 => "pdf",
            Self::Dxf2007Ascii => "dxf",
        }
    }
}

/// Resource limits applied before and after crease-pattern serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreasePatternExportLimits {
    pub max_output_bytes: usize,
    pub max_vertices: usize,
    pub max_edges: usize,
    pub max_boundary_vertices: usize,
    pub max_intersection_candidates: usize,
    pub max_title_chars: usize,
}

impl Default for CreasePatternExportLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: MAX_CREASE_PATTERN_EXPORT_BYTES,
            max_vertices: MAX_CREASE_PATTERN_EXPORT_VERTICES,
            max_edges: MAX_CREASE_PATTERN_EXPORT_EDGES,
            max_boundary_vertices: MAX_CREASE_PATTERN_EXPORT_BOUNDARY_VERTICES,
            max_intersection_candidates: MAX_CREASE_PATTERN_EXPORT_INTERSECTION_CANDIDATES,
            max_title_chars: MAX_CREASE_PATTERN_EXPORT_TITLE_CHARS,
        }
    }
}

/// Fully serialized crease-pattern file and save metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreasePatternExportArtifact {
    pub format: CreasePatternExportFormat,
    pub media_type: &'static str,
    pub file_extension: &'static str,
    pub bytes: Vec<u8>,
    pub vertex_count: usize,
    pub edge_count: usize,
    pub has_cuts: bool,
}

/// Endpoint named by an invalid edge reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreasePatternExportEndpoint {
    Start,
    End,
}

/// Failure while validating or serializing a crease pattern.
#[derive(Debug, Error)]
pub enum CreasePatternExportError {
    #[error("export title is {actual} characters; the limit is {maximum}")]
    TitleTooLong { actual: usize, maximum: usize },
    #[error(
        "export title contains XML 1.0 character U+{code_point:04X} at character {character_index}"
    )]
    InvalidXmlCharacter {
        character_index: usize,
        code_point: u32,
    },
    #[error("crease pattern has {actual} vertices; the limit is {maximum}")]
    TooManyVertices { actual: usize, maximum: usize },
    #[error("crease pattern has {actual} edges; the limit is {maximum}")]
    TooManyEdges { actual: usize, maximum: usize },
    #[error("paper boundary has {actual} vertices; the limit is {maximum}")]
    TooManyBoundaryVertices { actual: usize, maximum: usize },
    #[error("vertex records {first_index} and {duplicate_index} use the same vertex identifier")]
    DuplicateVertexId {
        first_index: usize,
        duplicate_index: usize,
    },
    #[error("vertex {vertex_index} has a non-finite coordinate")]
    NonFiniteVertex { vertex_index: usize },
    #[error("vertices {first_index} and {duplicate_index} occupy the same coordinate")]
    DuplicateVertexPosition {
        first_index: usize,
        duplicate_index: usize,
    },
    #[error("edge records {first_index} and {duplicate_index} use the same edge identifier")]
    DuplicateEdgeId {
        first_index: usize,
        duplicate_index: usize,
    },
    #[error("edge {edge_index} {endpoint:?} endpoint references a missing vertex")]
    MissingEdgeEndpoint {
        edge_index: usize,
        endpoint: CreasePatternExportEndpoint,
    },
    #[error("edge {edge_index} has equal endpoints")]
    DegenerateEdge { edge_index: usize },
    #[error("edge {edge_index} span is not representable as finite binary64")]
    EdgeSpanNotRepresentable { edge_index: usize },
    #[error("edge records {first_index} and {duplicate_index} describe the same segment")]
    DuplicateEdge {
        first_index: usize,
        duplicate_index: usize,
    },
    #[error("edge {edge_index} is a cut, but cutting is disabled for the paper")]
    CuttingDisabled { edge_index: usize },
    #[error("crease-pattern validation exceeds {maximum} broad-phase intersection candidates")]
    TooManyIntersectionCandidates { maximum: usize },
    #[error("crease pattern is not exportable: {issue:?}")]
    InvalidCreasePattern { issue: ValidationIssue },
    #[error("paper boundary is not exportable: {issue:?}")]
    InvalidPaper { issue: PaperValidationIssue },
    #[error("paper boundary bounds cannot be represented by a finite positive SVG viewBox")]
    ViewBoxNotRepresentable,
    #[error("SVG export cannot preserve unreferenced vertex {vertex_index}")]
    UnreferencedSvgVertex { vertex_index: usize },
    #[error("PDF export cannot preserve unreferenced vertex {vertex_index}")]
    UnreferencedPdfVertex { vertex_index: usize },
    #[error("DXF export cannot preserve unreferenced vertex {vertex_index}")]
    UnreferencedDxfVertex { vertex_index: usize },
    #[error("drawing bounds cannot be represented by a finite positive range")]
    DrawingBoundsNotRepresentable,
    #[error("PDF page dimensions exceed the PDF 1.7 compatibility limit")]
    PdfPageTooLarge,
    #[error("a PDF numeric token exceeds the {maximum}-character compatibility limit")]
    PdfNumberTooLong { maximum: usize },
    #[error("PDF structure could not be represented within fixed resource limits")]
    PdfStructureNotRepresentable,
    #[error(
        "DXF title contains unsupported control character U+{code_point:04X} at character {character_index}"
    )]
    InvalidDxfTitleCharacter {
        character_index: usize,
        code_point: u32,
    },
    #[error("DXF structure could not be represented within fixed resource limits")]
    DxfStructureNotRepresentable,
    #[error("FOLD JSON serialization failed: {0}")]
    FoldSerialization(#[source] serde_json::Error),
    #[error("export is {actual} bytes; the limit is {maximum} bytes")]
    OutputTooLarge { actual: usize, maximum: usize },
    #[error("generation provenance metadata occurs more than once")]
    DuplicateGenerationProvenance,
    #[error("generation provenance metadata is malformed")]
    MalformedGenerationProvenance,
    #[error("generation provenance metadata violates the V1 contract")]
    InvalidGenerationProvenance,
}

/// Reads the optional typed provenance from a bounded crease-pattern export.
///
/// Legacy files without the metadata return `None`; ambiguous, malformed, or
/// contract-invalid metadata is rejected instead of being silently ignored.
pub fn read_crease_pattern_generation_provenance(
    format: CreasePatternExportFormat,
    bytes: &[u8],
) -> Result<Option<BeginnerGenerationProvenanceV1>, CreasePatternExportError> {
    if bytes.len() > MAX_CREASE_PATTERN_EXPORT_BYTES {
        return Err(CreasePatternExportError::OutputTooLarge {
            actual: bytes.len(),
            maximum: MAX_CREASE_PATTERN_EXPORT_BYTES,
        });
    }
    let encoded = match format {
        CreasePatternExportFormat::Fold12 => {
            const KEY: &str = "\"origami2_generation_provenance\"";
            let text = std::str::from_utf8(bytes)
                .map_err(|_| CreasePatternExportError::MalformedGenerationProvenance)?;
            match text.match_indices(KEY).count() {
                0 => return Ok(None),
                1 => {}
                _ => return Err(CreasePatternExportError::DuplicateGenerationProvenance),
            }
            let value: serde_json::Value = serde_json::from_slice(bytes)
                .map_err(|_| CreasePatternExportError::MalformedGenerationProvenance)?;
            let metadata = value
                .as_object()
                .and_then(|object| object.get("origami2_generation_provenance"))
                .ok_or(CreasePatternExportError::MalformedGenerationProvenance)?;
            let provenance = serde_json::from_value(metadata.clone())
                .map_err(|_| CreasePatternExportError::MalformedGenerationProvenance)?;
            return validate_read_provenance(provenance).map(Some);
        }
        CreasePatternExportFormat::Svg => extract_single_marker(
            bytes,
            b"<metadata id=\"origami2-generation-provenance\">",
            b"</metadata>",
        )?,
        CreasePatternExportFormat::Pdf17 => {
            extract_single_line_value(bytes, b"% ORIGAMI2_GENERATION_PROVENANCE ")?
        }
        CreasePatternExportFormat::Dxf2007Ascii => {
            extract_single_line_value(bytes, b"ORIGAMI2_GENERATION_PROVENANCE ")?
        }
    };
    let Some(encoded) = encoded else {
        return Ok(None);
    };
    let json = decode_lower_hex(encoded)?;
    let provenance = serde_json::from_slice(&json)
        .map_err(|_| CreasePatternExportError::MalformedGenerationProvenance)?;
    validate_read_provenance(provenance).map(Some)
}

fn validate_read_provenance(
    provenance: BeginnerGenerationProvenanceV1,
) -> Result<BeginnerGenerationProvenanceV1, CreasePatternExportError> {
    validate_beginner_generation_provenance_v1(&provenance)
        .then_some(provenance)
        .ok_or(CreasePatternExportError::InvalidGenerationProvenance)
}

fn extract_single_marker<'a>(
    bytes: &'a [u8],
    prefix: &[u8],
    suffix: &[u8],
) -> Result<Option<&'a [u8]>, CreasePatternExportError> {
    let starts = bytes
        .windows(prefix.len())
        .filter(|window| *window == prefix)
        .count();
    if starts == 0 {
        return Ok(None);
    }
    if starts != 1 {
        return Err(CreasePatternExportError::DuplicateGenerationProvenance);
    }
    let start = bytes
        .windows(prefix.len())
        .position(|window| window == prefix)
        .unwrap()
        + prefix.len();
    let rest = &bytes[start..];
    let end = rest
        .windows(suffix.len())
        .position(|window| window == suffix)
        .ok_or(CreasePatternExportError::MalformedGenerationProvenance)?;
    Ok(Some(&rest[..end]))
}

fn extract_single_line_value<'a>(
    bytes: &'a [u8],
    prefix: &[u8],
) -> Result<Option<&'a [u8]>, CreasePatternExportError> {
    let starts: Vec<_> = bytes
        .windows(prefix.len())
        .enumerate()
        .filter_map(|(index, window)| (window == prefix).then_some(index))
        .collect();
    match starts.as_slice() {
        [] => Ok(None),
        [start] => {
            let value = &bytes[start + prefix.len()..];
            let end = value
                .iter()
                .position(|byte| *byte == b'\r' || *byte == b'\n')
                .unwrap_or(value.len());
            Ok(Some(&value[..end]))
        }
        _ => Err(CreasePatternExportError::DuplicateGenerationProvenance),
    }
}

fn decode_lower_hex(encoded: &[u8]) -> Result<Vec<u8>, CreasePatternExportError> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(2) {
        return Err(CreasePatternExportError::MalformedGenerationProvenance);
    }
    encoded
        .chunks_exact(2)
        .map(|pair| {
            let nibble = |byte| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            };
            Ok(
                (nibble(pair[0]).ok_or(CreasePatternExportError::MalformedGenerationProvenance)?
                    << 4)
                    | nibble(pair[1])
                        .ok_or(CreasePatternExportError::MalformedGenerationProvenance)?,
            )
        })
        .collect()
}

/// Exports a crease pattern using conservative desktop resource limits.
pub fn export_crease_pattern(
    format: CreasePatternExportFormat,
    title: &str,
    crease_pattern: &CreasePattern,
    paper: &Paper,
) -> Result<CreasePatternExportArtifact, CreasePatternExportError> {
    export_crease_pattern_with_limits(
        format,
        title,
        crease_pattern,
        paper,
        CreasePatternExportLimits::default(),
    )
}

/// Exports a crease pattern and embeds the typed generation provenance when present.
pub fn export_crease_pattern_with_provenance(
    format: CreasePatternExportFormat,
    title: &str,
    crease_pattern: &CreasePattern,
    paper: &Paper,
    provenance: Option<&BeginnerGenerationProvenanceV1>,
) -> Result<CreasePatternExportArtifact, CreasePatternExportError> {
    let mut artifact = export_crease_pattern(format, title, crease_pattern, paper)?;
    let Some(provenance) = provenance else {
        return Ok(artifact);
    };
    if !validate_beginner_generation_provenance_v1(provenance) {
        return Err(CreasePatternExportError::InvalidGenerationProvenance);
    }
    let json =
        serde_json::to_vec(provenance).map_err(CreasePatternExportError::FoldSerialization)?;
    let hex = json
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    artifact.bytes = match format {
        CreasePatternExportFormat::Fold12 => {
            let mut value: serde_json::Value = serde_json::from_slice(&artifact.bytes)
                .map_err(CreasePatternExportError::FoldSerialization)?;
            value.as_object_mut().expect("FOLD root object").insert(
                "origami2_generation_provenance".to_owned(),
                serde_json::to_value(provenance)
                    .map_err(CreasePatternExportError::FoldSerialization)?,
            );
            serde_json::to_vec(&value).map_err(CreasePatternExportError::FoldSerialization)?
        }
        CreasePatternExportFormat::Svg => {
            let text = String::from_utf8(artifact.bytes).expect("generated SVG is UTF-8");
            text.replacen(
                "</svg>",
                &format!("<metadata id=\"origami2-generation-provenance\">{hex}</metadata></svg>"),
                1,
            )
            .into_bytes()
        }
        CreasePatternExportFormat::Pdf17 => {
            artifact.bytes.extend_from_slice(
                format!("\n% ORIGAMI2_GENERATION_PROVENANCE {hex}\n").as_bytes(),
            );
            artifact.bytes
        }
        CreasePatternExportFormat::Dxf2007Ascii => {
            artifact.bytes.extend_from_slice(
                format!("999\r\nORIGAMI2_GENERATION_PROVENANCE {hex}\r\n").as_bytes(),
            );
            artifact.bytes
        }
    };
    if artifact.bytes.len() > MAX_CREASE_PATTERN_EXPORT_BYTES {
        return Err(CreasePatternExportError::OutputTooLarge {
            actual: artifact.bytes.len(),
            maximum: MAX_CREASE_PATTERN_EXPORT_BYTES,
        });
    }
    Ok(artifact)
}

/// Exports a crease pattern using caller-supplied resource limits.
pub fn export_crease_pattern_with_limits(
    format: CreasePatternExportFormat,
    title: &str,
    crease_pattern: &CreasePattern,
    paper: &Paper,
    limits: CreasePatternExportLimits,
) -> Result<CreasePatternExportArtifact, CreasePatternExportError> {
    let title_length = title.chars().count();
    if title_length > limits.max_title_chars {
        return Err(CreasePatternExportError::TitleTooLong {
            actual: title_length,
            maximum: limits.max_title_chars,
        });
    }
    if format == CreasePatternExportFormat::Svg {
        validate_xml_text(title)?;
    }

    let validated = validate_export_input(crease_pattern, paper, limits)?;
    for (vertex_index, referenced) in validated.referenced_vertices.iter().enumerate() {
        if !referenced {
            match format {
                CreasePatternExportFormat::Fold12 => {}
                CreasePatternExportFormat::Svg => {
                    return Err(CreasePatternExportError::UnreferencedSvgVertex { vertex_index });
                }
                CreasePatternExportFormat::Pdf17 => {
                    return Err(CreasePatternExportError::UnreferencedPdfVertex { vertex_index });
                }
                CreasePatternExportFormat::Dxf2007Ascii => {
                    return Err(CreasePatternExportError::UnreferencedDxfVertex { vertex_index });
                }
            }
        }
    }

    let bytes = match format {
        CreasePatternExportFormat::Fold12 => {
            serialize_fold12(title, crease_pattern, &validated.vertex_indices)?
        }
        CreasePatternExportFormat::Svg => serialize_svg(
            title,
            crease_pattern,
            &validated.vertex_indices,
            paper_bounds(paper, crease_pattern, &validated.vertex_indices)?,
        )
        .into_bytes(),
        CreasePatternExportFormat::Pdf17 => {
            pdf::serialize_pdf17(title, crease_pattern, &validated.vertex_indices)?
        }
        CreasePatternExportFormat::Dxf2007Ascii => {
            dxf::serialize_dxf2007_ascii(title, crease_pattern, paper, &validated.vertex_indices)?
        }
    };
    if bytes.len() > limits.max_output_bytes {
        return Err(CreasePatternExportError::OutputTooLarge {
            actual: bytes.len(),
            maximum: limits.max_output_bytes,
        });
    }

    Ok(CreasePatternExportArtifact {
        format,
        media_type: format.media_type(),
        file_extension: format.file_extension(),
        bytes,
        vertex_count: crease_pattern.vertices.len(),
        edge_count: crease_pattern.edges.len(),
        has_cuts: validated.has_cuts,
    })
}

struct ValidatedExport {
    vertex_indices: HashMap<VertexId, usize>,
    referenced_vertices: Vec<bool>,
    has_cuts: bool,
}

#[derive(Debug, Clone, Copy)]
struct PaperBounds {
    min_x: f64,
    min_y: f64,
    width: f64,
    height: f64,
}

fn validate_export_input(
    crease_pattern: &CreasePattern,
    paper: &Paper,
    limits: CreasePatternExportLimits,
) -> Result<ValidatedExport, CreasePatternExportError> {
    if crease_pattern.vertices.len() > limits.max_vertices {
        return Err(CreasePatternExportError::TooManyVertices {
            actual: crease_pattern.vertices.len(),
            maximum: limits.max_vertices,
        });
    }
    if crease_pattern.edges.len() > limits.max_edges {
        return Err(CreasePatternExportError::TooManyEdges {
            actual: crease_pattern.edges.len(),
            maximum: limits.max_edges,
        });
    }
    if paper.boundary_vertices.len() > limits.max_boundary_vertices {
        return Err(CreasePatternExportError::TooManyBoundaryVertices {
            actual: paper.boundary_vertices.len(),
            maximum: limits.max_boundary_vertices,
        });
    }

    let mut vertex_indices = HashMap::with_capacity(crease_pattern.vertices.len());
    let mut position_indices = HashMap::with_capacity(crease_pattern.vertices.len());
    for (vertex_index, vertex) in crease_pattern.vertices.iter().enumerate() {
        if let Some(first_index) = vertex_indices.insert(vertex.id, vertex_index) {
            return Err(CreasePatternExportError::DuplicateVertexId {
                first_index,
                duplicate_index: vertex_index,
            });
        }
        if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
            return Err(CreasePatternExportError::NonFiniteVertex { vertex_index });
        }
        let position_key = point_key(vertex.position);
        if let Some(first_index) = position_indices.insert(position_key, vertex_index) {
            return Err(CreasePatternExportError::DuplicateVertexPosition {
                first_index,
                duplicate_index: vertex_index,
            });
        }
    }

    let mut edge_ids = HashMap::with_capacity(crease_pattern.edges.len());
    let mut segment_indices = HashMap::with_capacity(crease_pattern.edges.len());
    let mut referenced_vertices = vec![false; crease_pattern.vertices.len()];
    let mut bounds = Vec::with_capacity(crease_pattern.edges.len());
    let mut has_cuts = false;
    for (edge_index, edge) in crease_pattern.edges.iter().enumerate() {
        if let Some(first_index) = edge_ids.insert(edge.id, edge_index) {
            return Err(CreasePatternExportError::DuplicateEdgeId {
                first_index,
                duplicate_index: edge_index,
            });
        }
        let start_index = vertex_indices.get(&edge.start).copied().ok_or(
            CreasePatternExportError::MissingEdgeEndpoint {
                edge_index,
                endpoint: CreasePatternExportEndpoint::Start,
            },
        )?;
        let end_index = vertex_indices.get(&edge.end).copied().ok_or(
            CreasePatternExportError::MissingEdgeEndpoint {
                edge_index,
                endpoint: CreasePatternExportEndpoint::End,
            },
        )?;
        if start_index == end_index {
            return Err(CreasePatternExportError::DegenerateEdge { edge_index });
        }
        let start = crease_pattern.vertices[start_index].position;
        let end = crease_pattern.vertices[end_index].position;
        if !(end.x - start.x).is_finite() || !(end.y - start.y).is_finite() {
            return Err(CreasePatternExportError::EdgeSpanNotRepresentable { edge_index });
        }
        let segment_key = if start_index < end_index {
            (start_index, end_index)
        } else {
            (end_index, start_index)
        };
        if let Some(first_index) = segment_indices.insert(segment_key, edge_index) {
            return Err(CreasePatternExportError::DuplicateEdge {
                first_index,
                duplicate_index: edge_index,
            });
        }

        referenced_vertices[start_index] = true;
        referenced_vertices[end_index] = true;
        bounds.push(IntersectionCandidateBounds::new(edge_index, start, end));
        if edge.kind == EdgeKind::Cut {
            if !paper.cutting_allowed {
                return Err(CreasePatternExportError::CuttingDisabled { edge_index });
            }
            has_cuts = true;
        }
    }

    validate_intersection_candidate_limit(&mut bounds, limits.max_intersection_candidates)?;
    if let Some(issue) = validate_crease_pattern(crease_pattern)
        .into_issues()
        .into_iter()
        .next()
    {
        return Err(CreasePatternExportError::InvalidCreasePattern { issue });
    }
    if let Some(issue) = validate_paper(paper, crease_pattern)
        .into_issues()
        .into_iter()
        .find(|issue| {
            !matches!(
                issue,
                PaperValidationIssue::NonFiniteThickness { .. }
                    | PaperValidationIssue::NegativeThickness { .. }
            )
        })
    {
        return Err(CreasePatternExportError::InvalidPaper { issue });
    }

    Ok(ValidatedExport {
        vertex_indices,
        referenced_vertices,
        has_cuts,
    })
}

#[derive(Debug, Clone, Copy)]
struct IntersectionCandidateBounds {
    edge_index: usize,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl IntersectionCandidateBounds {
    fn new(edge_index: usize, start: Point2, end: Point2) -> Self {
        Self {
            edge_index,
            min_x: start.x.min(end.x),
            max_x: start.x.max(end.x),
            min_y: start.y.min(end.y),
            max_y: start.y.max(end.y),
        }
    }
}

fn validate_intersection_candidate_limit(
    bounds: &mut [IntersectionCandidateBounds],
    maximum: usize,
) -> Result<(), CreasePatternExportError> {
    bounds.sort_unstable_by(|left, right| {
        left.min_x
            .total_cmp(&right.min_x)
            .then_with(|| left.edge_index.cmp(&right.edge_index))
    });
    let mut candidates = 0_usize;
    for (position, left) in bounds.iter().copied().enumerate() {
        for right in bounds.iter().copied().skip(position + 1) {
            if right.min_x > left.max_x {
                break;
            }
            if left.min_y > right.max_y || right.min_y > left.max_y {
                continue;
            }
            candidates = candidates.saturating_add(1);
            if candidates > maximum {
                return Err(CreasePatternExportError::TooManyIntersectionCandidates { maximum });
            }
        }
    }
    Ok(())
}

fn paper_bounds(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
) -> Result<PaperBounds, CreasePatternExportError> {
    let mut positions = paper
        .boundary_vertices
        .iter()
        .map(|vertex_id| crease_pattern.vertices[vertex_indices[vertex_id]].position);
    let first = positions
        .next()
        .expect("validated paper boundary has at least three vertices");
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
    for position in positions {
        min_x = min_x.min(position.x);
        max_x = max_x.max(position.x);
        min_y = min_y.min(position.y);
        max_y = max_y.max(position.y);
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(CreasePatternExportError::ViewBoxNotRepresentable);
    }
    Ok(PaperBounds {
        min_x: canonical_zero(min_x),
        min_y: canonical_zero(min_y),
        width,
        height,
    })
}

#[derive(Serialize)]
struct Fold12Document<'a> {
    file_spec: f64,
    file_creator: &'static str,
    file_title: &'a str,
    file_classes: [&'static str; 1],
    frame_classes: [&'static str; 1],
    frame_attributes: Vec<&'static str>,
    frame_unit: &'static str,
    vertices_coords: Vec<[f64; 2]>,
    edges_vertices: Vec<[usize; 2]>,
    edges_assignment: Vec<&'static str>,
}

fn serialize_fold12(
    title: &str,
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
) -> Result<Vec<u8>, CreasePatternExportError> {
    let document = Fold12Document {
        file_spec: 1.2,
        file_creator: "ORIGAMI2",
        file_title: title,
        file_classes: ["singleModel"],
        frame_classes: ["creasePattern"],
        frame_attributes: if crease_pattern
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Cut)
        {
            vec!["2D", "cuts"]
        } else {
            vec!["2D"]
        },
        frame_unit: "mm",
        vertices_coords: crease_pattern
            .vertices
            .iter()
            .map(|vertex| {
                [
                    canonical_zero(vertex.position.x),
                    canonical_zero(vertex.position.y),
                ]
            })
            .collect(),
        edges_vertices: crease_pattern
            .edges
            .iter()
            .map(|edge| [vertex_indices[&edge.start], vertex_indices[&edge.end]])
            .collect(),
        edges_assignment: crease_pattern
            .edges
            .iter()
            .map(|edge| fold_assignment(edge.kind))
            .collect(),
    };
    serde_json::to_vec(&document).map_err(CreasePatternExportError::FoldSerialization)
}

const fn fold_assignment(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Boundary => "B",
        EdgeKind::Mountain => "M",
        EdgeKind::Valley => "V",
        EdgeKind::Auxiliary => "F",
        EdgeKind::Cut => "C",
    }
}

fn serialize_svg(
    title: &str,
    crease_pattern: &CreasePattern,
    vertex_indices: &HashMap<VertexId, usize>,
    bounds: PaperBounds,
) -> String {
    let mut output = String::with_capacity(
        256_usize.saturating_add(crease_pattern.edges.len().saturating_mul(160)),
    );
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        output,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" viewBox=\"{} {} {} {}\" width=\"{}mm\" height=\"{}mm\" fill=\"none\">",
        bounds.min_x, bounds.min_y, bounds.width, bounds.height, bounds.width, bounds.height
    )
    .expect("writing to a String cannot fail");
    output.push_str("  <title>");
    push_escaped_xml_text(&mut output, title);
    output.push_str("</title>\n");
    for edge in &crease_pattern.edges {
        let start = crease_pattern.vertices[vertex_indices[&edge.start]].position;
        let end = crease_pattern.vertices[vertex_indices[&edge.end]].position;
        let style = svg_edge_style(edge.kind);
        writeln!(
            output,
            "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"{} stroke-linecap=\"{}\" data-origami-kind=\"{}\"/>",
            canonical_zero(start.x),
            canonical_zero(start.y),
            canonical_zero(end.x),
            canonical_zero(end.y),
            style.stroke,
            style.dash_attribute,
            style.line_cap,
            style.semantic
        )
        .expect("writing to a String cannot fail");
    }
    output.push_str("</svg>\n");
    output
}

#[derive(Debug, Clone, Copy)]
struct SvgEdgeStyle {
    stroke: &'static str,
    dash_attribute: &'static str,
    line_cap: &'static str,
    semantic: &'static str,
}

const fn svg_edge_style(kind: EdgeKind) -> SvgEdgeStyle {
    match kind {
        EdgeKind::Boundary => SvgEdgeStyle {
            stroke: "#111111",
            dash_attribute: "",
            line_cap: "butt",
            semantic: "boundary",
        },
        EdgeKind::Mountain => SvgEdgeStyle {
            stroke: "#d32f2f",
            dash_attribute: " stroke-dasharray=\"6 2 1 2\"",
            line_cap: "butt",
            semantic: "mountain",
        },
        EdgeKind::Valley => SvgEdgeStyle {
            stroke: "#1976d2",
            dash_attribute: " stroke-dasharray=\"3 1.5\"",
            line_cap: "butt",
            semantic: "valley",
        },
        EdgeKind::Auxiliary => SvgEdgeStyle {
            stroke: "#757575",
            dash_attribute: " stroke-dasharray=\"0.5 1.5\"",
            line_cap: "round",
            semantic: "auxiliary",
        },
        EdgeKind::Cut => SvgEdgeStyle {
            stroke: "#000000",
            dash_attribute: " stroke-dasharray=\"8 2 1 2 1 2\"",
            line_cap: "butt",
            semantic: "cut",
        },
    }
}

fn validate_xml_text(text: &str) -> Result<(), CreasePatternExportError> {
    for (character_index, character) in text.chars().enumerate() {
        let code_point = u32::from(character);
        let valid = matches!(code_point, 0x9 | 0xA | 0xD)
            || (0x20..=0xD7FF).contains(&code_point)
            || (0xE000..=0xFFFD).contains(&code_point)
            || (0x10000..=0x10FFFF).contains(&code_point);
        if !valid {
            return Err(CreasePatternExportError::InvalidXmlCharacter {
                character_index,
                code_point,
            });
        }
    }
    Ok(())
}

fn push_escaped_xml_text(output: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            '\r' => output.push_str("&#13;"),
            _ => output.push(character),
        }
    }
}

fn point_key(point: Point2) -> (u64, u64) {
    (
        canonical_zero(point.x).to_bits(),
        canonical_zero(point.y).to_bits(),
    )
}

const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ori_domain::{Edge, EdgeId, LengthDisplayUnit, Vertex, VertexId};

    use super::*;
    use crate::{
        FoldAssignmentMapping, FoldAssignmentTarget, FoldConversionOptions, SvgConversionOptions,
        SvgGroupMapping, SvgGroupTarget, read_fold_preview, read_svg_preview,
    };

    type CoordinateKey = (u64, u64);
    type EdgeSignature = (u8, CoordinateKey, CoordinateKey);

    fn sample_pattern() -> (CreasePattern, Paper) {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(20.0, 0.0),
            Point2::new(20.0, 20.0),
            Point2::new(0.0, 20.0),
            Point2::new(10.0, 10.0),
        ];
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let kinds = [
            (0, 1, EdgeKind::Boundary),
            (1, 2, EdgeKind::Boundary),
            (2, 3, EdgeKind::Boundary),
            (3, 0, EdgeKind::Boundary),
            (4, 0, EdgeKind::Mountain),
            (4, 1, EdgeKind::Valley),
            (4, 2, EdgeKind::Auxiliary),
            (4, 3, EdgeKind::Cut),
        ];
        let edges = kinds
            .into_iter()
            .map(|(start, end, kind)| Edge {
                id: EdgeId::new(),
                start: vertices[start].id,
                end: vertices[end].id,
                kind,
            })
            .collect();
        let paper = Paper {
            boundary_vertices: vertices[..4].iter().map(|vertex| vertex.id).collect(),
            cutting_allowed: true,
            ..Paper::default()
        };
        (CreasePattern { vertices, edges }, paper)
    }

    fn edge_signatures(pattern: &CreasePattern) -> BTreeSet<EdgeSignature> {
        let positions = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, point_key(vertex.position)))
            .collect::<HashMap<_, _>>();
        pattern
            .edges
            .iter()
            .map(|edge| {
                let start = positions[&edge.start];
                let end = positions[&edge.end];
                let (start, end) = if start < end {
                    (start, end)
                } else {
                    (end, start)
                };
                let kind = match edge.kind {
                    EdgeKind::Boundary => 0,
                    EdgeKind::Mountain => 1,
                    EdgeKind::Valley => 2,
                    EdgeKind::Auxiliary => 3,
                    EdgeKind::Cut => 4,
                };
                (kind, start, end)
            })
            .collect()
    }

    fn fold_options() -> FoldConversionOptions {
        FoldConversionOptions {
            assignment_mapping: FoldAssignmentMapping {
                boundary: Some(FoldAssignmentTarget::ImportAs {
                    edge_kind: EdgeKind::Boundary,
                }),
                mountain: Some(FoldAssignmentTarget::ImportAs {
                    edge_kind: EdgeKind::Mountain,
                }),
                valley: Some(FoldAssignmentTarget::ImportAs {
                    edge_kind: EdgeKind::Valley,
                }),
                flat: Some(FoldAssignmentTarget::ImportAs {
                    edge_kind: EdgeKind::Auxiliary,
                }),
                unassigned: Some(FoldAssignmentTarget::Ignore),
                cut: Some(FoldAssignmentTarget::ImportAs {
                    edge_kind: EdgeKind::Cut,
                }),
                join: Some(FoldAssignmentTarget::Ignore),
            },
            millimetres_per_unit: 1.0,
        }
    }

    #[test]
    fn all_formats_are_byte_deterministic_and_report_exact_metadata() {
        let (pattern, paper) = sample_pattern();
        for (format, media_type, extension) in [
            (
                CreasePatternExportFormat::Fold12,
                "application/json",
                "fold",
            ),
            (CreasePatternExportFormat::Svg, "image/svg+xml", "svg"),
            (CreasePatternExportFormat::Pdf17, "application/pdf", "pdf"),
            (
                CreasePatternExportFormat::Dxf2007Ascii,
                "image/vnd.dxf",
                "dxf",
            ),
        ] {
            let first =
                export_crease_pattern(format, "Deterministic", &pattern, &paper).expect("export");
            let second =
                export_crease_pattern(format, "Deterministic", &pattern, &paper).expect("export");
            assert_eq!(first.bytes, second.bytes);
            assert_eq!(first.format, format);
            assert_eq!(first.media_type, media_type);
            assert_eq!(first.file_extension, extension);
            assert!(first.has_cuts);
        }
    }

    #[test]
    fn display_unit_never_changes_external_export_bytes() {
        let (pattern, paper) = sample_pattern();
        for format in [
            CreasePatternExportFormat::Fold12,
            CreasePatternExportFormat::Svg,
            CreasePatternExportFormat::Pdf17,
            CreasePatternExportFormat::Dxf2007Ascii,
        ] {
            let baseline = export_crease_pattern(format, "units", &pattern, &paper)
                .expect("baseline export")
                .bytes;
            for unit in [
                LengthDisplayUnit::Centimeter,
                LengthDisplayUnit::Inch,
                LengthDisplayUnit::PaperEdgeRatio {
                    reference_edge: pattern.edges[0].id,
                },
            ] {
                let changed = Paper {
                    length_display_unit: unit,
                    ..paper.clone()
                };
                let bytes = export_crease_pattern(format, "units", &pattern, &changed)
                    .expect("unit-independent export")
                    .bytes;
                assert_eq!(bytes, baseline, "{format:?} must remain millimetre based");
            }
        }
    }

    #[test]
    fn svg_styles_remain_distinct_when_every_stroke_is_printed_black() {
        let styles = [
            svg_edge_style(EdgeKind::Boundary),
            svg_edge_style(EdgeKind::Mountain),
            svg_edge_style(EdgeKind::Valley),
            svg_edge_style(EdgeKind::Auxiliary),
            svg_edge_style(EdgeKind::Cut),
        ];

        let colours = styles
            .iter()
            .map(|style| style.stroke)
            .collect::<BTreeSet<_>>();
        assert_eq!(colours.len(), styles.len(), "screen colours must be unique");

        let monochrome_patterns = styles
            .iter()
            .map(|style| style.dash_attribute)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            monochrome_patterns.len(),
            styles.len(),
            "five line kinds must remain visually unique after colour is removed"
        );
        assert_eq!(
            styles.map(|style| (style.dash_attribute, style.line_cap, style.semantic)),
            [
                ("", "butt", "boundary"),
                (" stroke-dasharray=\"6 2 1 2\"", "butt", "mountain"),
                (" stroke-dasharray=\"3 1.5\"", "butt", "valley"),
                (" stroke-dasharray=\"0.5 1.5\"", "round", "auxiliary"),
                (" stroke-dasharray=\"8 2 1 2 1 2\"", "butt", "cut"),
            ]
        );

        let (pattern, paper) = sample_pattern();
        let artifact = export_crease_pattern(
            CreasePatternExportFormat::Svg,
            "monochrome styles",
            &pattern,
            &paper,
        )
        .expect("SVG export");
        let svg = std::str::from_utf8(&artifact.bytes).expect("UTF-8 SVG");
        for style in styles {
            assert!(
                svg.lines().any(|line| {
                    line.contains(&format!("stroke=\"{}\"", style.stroke))
                        && line.contains(style.dash_attribute)
                        && line.contains(&format!("stroke-linecap=\"{}\"", style.line_cap))
                        && line.contains(&format!("data-origami-kind=\"{}\"", style.semantic))
                }),
                "serialized SVG is missing the {:?} style",
                style.semantic
            );
        }
    }

    #[test]
    fn malicious_title_is_data_not_markup_or_json_structure() {
        let (pattern, paper) = sample_pattern();
        let title = "</title><script>alert(&\"')</script>\r";

        let svg = export_crease_pattern(CreasePatternExportFormat::Svg, title, &pattern, &paper)
            .expect("SVG export");
        let svg_text = std::str::from_utf8(&svg.bytes).expect("UTF-8 SVG");
        assert!(!svg_text.contains("</title><script>"));
        assert!(svg_text.contains("&lt;/title&gt;&lt;script&gt;"));
        assert_eq!(
            read_svg_preview(&svg.bytes).expect("read own SVG").title(),
            Some(title)
        );

        let fold =
            export_crease_pattern(CreasePatternExportFormat::Fold12, title, &pattern, &paper)
                .expect("FOLD export");
        let json: serde_json::Value = serde_json::from_slice(&fold.bytes).expect("JSON");
        assert_eq!(json["file_title"], title);
        assert_eq!(json["file_creator"], "ORIGAMI2");
        assert_eq!(json["file_classes"], serde_json::json!(["singleModel"]));
        assert_eq!(json["frame_attributes"], serde_json::json!(["2D", "cuts"]));
    }

    #[test]
    fn rejects_non_finite_missing_and_cutting_policy_violations() {
        let (pattern, paper) = sample_pattern();

        let mut non_finite = pattern.clone();
        non_finite.vertices[0].position.x = f64::NAN;
        assert!(matches!(
            export_crease_pattern(
                CreasePatternExportFormat::Fold12,
                "invalid",
                &non_finite,
                &paper
            ),
            Err(CreasePatternExportError::NonFiniteVertex { vertex_index: 0 })
        ));

        let mut missing = pattern.clone();
        missing.edges[0].start = VertexId::new();
        assert!(matches!(
            export_crease_pattern(CreasePatternExportFormat::Svg, "invalid", &missing, &paper),
            Err(CreasePatternExportError::MissingEdgeEndpoint {
                edge_index: 0,
                endpoint: CreasePatternExportEndpoint::Start
            })
        ));

        let cutting_disabled = Paper {
            cutting_allowed: false,
            ..paper.clone()
        };
        assert!(matches!(
            export_crease_pattern(
                CreasePatternExportFormat::Fold12,
                "invalid",
                &pattern,
                &cutting_disabled
            ),
            Err(CreasePatternExportError::CuttingDisabled { edge_index: 7 })
        ));
    }

    #[test]
    fn enforces_count_title_xml_and_output_limits() {
        let (pattern, paper) = sample_pattern();
        let exact_limits = CreasePatternExportLimits {
            max_vertices: pattern.vertices.len(),
            max_edges: pattern.edges.len(),
            max_boundary_vertices: paper.boundary_vertices.len(),
            ..CreasePatternExportLimits::default()
        };
        export_crease_pattern_with_limits(
            CreasePatternExportFormat::Fold12,
            "at limits",
            &pattern,
            &paper,
            exact_limits,
        )
        .expect("counts equal to their limits are accepted");

        let mut limits = CreasePatternExportLimits {
            max_vertices: pattern.vertices.len() - 1,
            ..CreasePatternExportLimits::default()
        };
        assert!(matches!(
            export_crease_pattern_with_limits(
                CreasePatternExportFormat::Fold12,
                "limited",
                &pattern,
                &paper,
                limits
            ),
            Err(CreasePatternExportError::TooManyVertices { .. })
        ));
        limits = CreasePatternExportLimits {
            max_edges: pattern.edges.len() - 1,
            ..CreasePatternExportLimits::default()
        };
        assert!(matches!(
            export_crease_pattern_with_limits(
                CreasePatternExportFormat::Fold12,
                "limited",
                &pattern,
                &paper,
                limits
            ),
            Err(CreasePatternExportError::TooManyEdges { .. })
        ));
        limits = CreasePatternExportLimits {
            max_boundary_vertices: paper.boundary_vertices.len() - 1,
            ..CreasePatternExportLimits::default()
        };
        assert!(matches!(
            export_crease_pattern_with_limits(
                CreasePatternExportFormat::Fold12,
                "limited",
                &pattern,
                &paper,
                limits
            ),
            Err(CreasePatternExportError::TooManyBoundaryVertices { .. })
        ));

        limits = CreasePatternExportLimits::default();
        limits.max_title_chars = 2;
        for format in [
            CreasePatternExportFormat::Fold12,
            CreasePatternExportFormat::Svg,
            CreasePatternExportFormat::Pdf17,
            CreasePatternExportFormat::Dxf2007Ascii,
        ] {
            assert!(matches!(
                export_crease_pattern_with_limits(format, "三文字", &pattern, &paper, limits),
                Err(CreasePatternExportError::TitleTooLong {
                    actual: 3,
                    maximum: 2
                })
            ));
        }
        let too_many_scalars = "🙂".repeat(MAX_CREASE_PATTERN_EXPORT_TITLE_CHARS + 1);
        assert!(matches!(
            export_crease_pattern(
                CreasePatternExportFormat::Pdf17,
                &too_many_scalars,
                &pattern,
                &paper
            ),
            Err(CreasePatternExportError::TitleTooLong {
                actual: 513,
                maximum: MAX_CREASE_PATTERN_EXPORT_TITLE_CHARS
            })
        ));
        assert!(matches!(
            export_crease_pattern(
                CreasePatternExportFormat::Svg,
                "bad\u{0}title",
                &pattern,
                &paper
            ),
            Err(CreasePatternExportError::InvalidXmlCharacter {
                character_index: 3,
                code_point: 0
            })
        ));

        for format in [
            CreasePatternExportFormat::Fold12,
            CreasePatternExportFormat::Svg,
            CreasePatternExportFormat::Pdf17,
            CreasePatternExportFormat::Dxf2007Ascii,
        ] {
            let full = export_crease_pattern(format, "limited", &pattern, &paper)
                .expect("unlimited export");
            limits = CreasePatternExportLimits {
                max_output_bytes: full.bytes.len() - 1,
                ..CreasePatternExportLimits::default()
            };
            assert!(matches!(
                export_crease_pattern_with_limits(format, "limited", &pattern, &paper, limits),
                Err(CreasePatternExportError::OutputTooLarge {
                    actual,
                    maximum
                }) if actual == full.bytes.len() && maximum == full.bytes.len() - 1
            ));
            limits.max_output_bytes = full.bytes.len();
            export_crease_pattern_with_limits(format, "limited", &pattern, &paper, limits)
                .expect("byte length equal to its limit is accepted");
        }
    }

    #[test]
    fn rejects_excess_intersection_work_and_static_format_vertex_loss() {
        let (pattern, paper) = sample_pattern();
        let positions = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<HashMap<_, _>>();
        let mut candidate_bounds = pattern
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                IntersectionCandidateBounds::new(
                    index,
                    positions[&edge.start],
                    positions[&edge.end],
                )
            })
            .collect::<Vec<_>>();
        let mut exact_candidates = 0_usize;
        loop {
            if validate_intersection_candidate_limit(&mut candidate_bounds, exact_candidates)
                .is_ok()
            {
                break;
            }
            exact_candidates += 1;
        }
        assert!(exact_candidates > 0);

        let exact_limits = CreasePatternExportLimits {
            max_intersection_candidates: exact_candidates,
            ..CreasePatternExportLimits::default()
        };
        export_crease_pattern_with_limits(
            CreasePatternExportFormat::Fold12,
            "candidate limit",
            &pattern,
            &paper,
            exact_limits,
        )
        .expect("candidate count equal to its limit is accepted");
        let limits = CreasePatternExportLimits {
            max_intersection_candidates: exact_candidates - 1,
            ..exact_limits
        };
        assert!(matches!(
            export_crease_pattern_with_limits(
                CreasePatternExportFormat::Fold12,
                "candidate limit",
                &pattern,
                &paper,
                limits
            ),
            Err(CreasePatternExportError::TooManyIntersectionCandidates { maximum })
                if maximum == exact_candidates - 1
        ));

        let mut with_isolated_vertex = pattern.clone();
        with_isolated_vertex.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(5.0, 7.0),
        });
        export_crease_pattern(
            CreasePatternExportFormat::Fold12,
            "isolated FOLD vertex",
            &with_isolated_vertex,
            &paper,
        )
        .expect("FOLD preserves isolated vertices");
        assert!(matches!(
            export_crease_pattern(
                CreasePatternExportFormat::Svg,
                "isolated SVG vertex",
                &with_isolated_vertex,
                &paper
            ),
            Err(CreasePatternExportError::UnreferencedSvgVertex { vertex_index: 5 })
        ));
        assert!(matches!(
            export_crease_pattern(
                CreasePatternExportFormat::Pdf17,
                "isolated PDF vertex",
                &with_isolated_vertex,
                &paper
            ),
            Err(CreasePatternExportError::UnreferencedPdfVertex { vertex_index: 5 })
        ));
        assert!(matches!(
            export_crease_pattern(
                CreasePatternExportFormat::Dxf2007Ascii,
                "isolated DXF vertex",
                &with_isolated_vertex,
                &paper
            ),
            Err(CreasePatternExportError::UnreferencedDxfVertex { vertex_index: 5 })
        ));
    }

    #[test]
    fn fold12_round_trip_preserves_geometry_and_all_assignments() {
        let (pattern, paper) = sample_pattern();
        let artifact = export_crease_pattern(
            CreasePatternExportFormat::Fold12,
            "FOLD round trip",
            &pattern,
            &paper,
        )
        .expect("FOLD export");
        let preview = read_fold_preview(&artifact.bytes).expect("read own FOLD");
        assert_eq!(preview.file_spec(), Some(1.2));
        assert_eq!(preview.recommended_millimetres_per_unit(), Some(1.0));
        assert!(matches!(
            preview.warnings(),
            [crate::FoldPreviewWarning::IgnoredFields { names }]
                if names == &["file_classes", "file_creator", "frame_attributes"]
        ));
        let counts = preview.assignment_counts();
        assert_eq!(
            (
                counts.boundary,
                counts.mountain,
                counts.valley,
                counts.flat,
                counts.cut
            ),
            (4, 1, 1, 1, 1)
        );

        let converted = preview.convert(&fold_options()).expect("convert own FOLD");
        assert_eq!(
            edge_signatures(converted.crease_pattern()),
            edge_signatures(&pattern)
        );
        assert_eq!(converted.boundary_vertices().len(), 4);
    }

    #[test]
    fn svg_round_trip_preserves_geometry_semantics_scale_and_cuts() {
        let (pattern, paper) = sample_pattern();
        let artifact = export_crease_pattern(
            CreasePatternExportFormat::Svg,
            "SVG round trip",
            &pattern,
            &paper,
        )
        .expect("SVG export");
        let preview = read_svg_preview(&artifact.bytes).expect("read own SVG");
        assert_eq!(preview.title(), Some("SVG round trip"));
        assert_eq!(preview.recommended_millimetres_per_unit(), Some(1.0));
        assert!(preview.warnings().is_empty());
        assert_eq!(
            preview.root_view_box(),
            Some(crate::SvgRootViewBox {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0
            })
        );
        let mappings = preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: match group.semantic.as_deref() {
                    Some("boundary") => SvgGroupTarget::Boundary,
                    Some("mountain") => SvgGroupTarget::Mountain,
                    Some("valley") => SvgGroupTarget::Valley,
                    Some("auxiliary") => SvgGroupTarget::Auxiliary,
                    Some("cut") => SvgGroupTarget::Cut,
                    semantic => panic!("unexpected semantic {semantic:?}"),
                },
            })
            .collect();
        let converted = preview
            .convert(&SvgConversionOptions {
                millimetres_per_unit: 1.0,
                group_mappings: mappings,
                boundary_candidate: None,
            })
            .expect("convert own SVG");
        assert_eq!(
            edge_signatures(converted.crease_pattern()),
            edge_signatures(&pattern)
        );
        assert_eq!(converted.boundary_vertices().len(), 4);
        assert!(converted.has_cuts());
    }

    #[test]
    fn svg_view_box_preserves_negative_paper_origin() {
        let (mut pattern, paper) = sample_pattern();
        for vertex in &mut pattern.vertices {
            vertex.position.x -= 30.0;
            vertex.position.y -= 40.0;
        }
        let artifact = export_crease_pattern(
            CreasePatternExportFormat::Svg,
            "Negative origin",
            &pattern,
            &paper,
        )
        .expect("SVG export");
        let preview = read_svg_preview(&artifact.bytes).expect("read own SVG");
        assert_eq!(
            preview.root_view_box(),
            Some(crate::SvgRootViewBox {
                x: -30.0,
                y: -40.0,
                width: 20.0,
                height: 20.0
            })
        );
        assert_eq!(preview.recommended_millimetres_per_unit(), Some(1.0));
    }

    #[test]
    fn typed_generation_provenance_is_embedded_in_every_export() {
        let (pattern, paper) = sample_pattern();
        let provenance = BeginnerGenerationProvenanceV1 {
            fold_path_certificate_sha256: Some([0x5au8; 32]),
            schema_version: 1,
            topology_authority_sha256: [0xabu8; 32],
            confidence_score: 87,
            confidence_reasons: vec!["native_topology_witness".to_owned()],
            explicit_override: false,
            source_asset_fingerprint: "asset:test".to_owned(),
            semantic_landmark_provenance: None,
            generic_tree: None,
        };
        for format in [
            CreasePatternExportFormat::Fold12,
            CreasePatternExportFormat::Svg,
            CreasePatternExportFormat::Pdf17,
            CreasePatternExportFormat::Dxf2007Ascii,
        ] {
            let plain =
                export_crease_pattern_with_provenance(format, "provenance", &pattern, &paper, None)
                    .unwrap();
            assert_eq!(
                plain,
                export_crease_pattern(format, "provenance", &pattern, &paper).unwrap()
            );
            let artifact = export_crease_pattern_with_provenance(
                format,
                "provenance",
                &pattern,
                &paper,
                Some(&provenance),
            )
            .unwrap();
            assert_eq!(
                read_crease_pattern_generation_provenance(format, &artifact.bytes).unwrap(),
                Some(provenance.clone())
            );
            if format == CreasePatternExportFormat::Fold12 {
                let value: serde_json::Value = serde_json::from_slice(&artifact.bytes).unwrap();
                assert_eq!(
                    value["origami2_generation_provenance"]["confidence_score"],
                    87
                );
                assert_eq!(
                    value["origami2_generation_provenance"]["fold_path_certificate_sha256"]
                        .as_array()
                        .map(Vec::len),
                    Some(32)
                );
            } else {
                let text = String::from_utf8_lossy(&artifact.bytes);
                assert!(
                    text.contains("ORIGAMI2_GENERATION_PROVENANCE")
                        || text.contains("origami2-generation-provenance")
                );
            }
        }
    }

    #[test]
    fn generation_provenance_reader_rejects_ambiguous_or_tampered_metadata() {
        let (pattern, paper) = sample_pattern();
        let provenance = BeginnerGenerationProvenanceV1 {
            fold_path_certificate_sha256: Some([0x3cu8; 32]),
            schema_version: 1,
            topology_authority_sha256: [7; 32],
            confidence_score: 91,
            confidence_reasons: vec!["bounded_witness".to_owned(), "image_outline".to_owned()],
            explicit_override: true,
            source_asset_fingerprint: "sha256:0123456789abcdef".to_owned(),
            semantic_landmark_provenance: None,
            generic_tree: None,
        };
        for format in [
            CreasePatternExportFormat::Fold12,
            CreasePatternExportFormat::Svg,
            CreasePatternExportFormat::Pdf17,
            CreasePatternExportFormat::Dxf2007Ascii,
        ] {
            let plain = export_crease_pattern(format, "legacy", &pattern, &paper).unwrap();
            assert_eq!(
                read_crease_pattern_generation_provenance(format, &plain.bytes).unwrap(),
                None
            );
            let mut artifact = export_crease_pattern_with_provenance(
                format,
                "strict",
                &pattern,
                &paper,
                Some(&provenance),
            )
            .unwrap();
            let marker = match format {
                CreasePatternExportFormat::Fold12 => {
                    b"\"origami2_generation_provenance\"".as_slice()
                }
                CreasePatternExportFormat::Svg => {
                    b"<metadata id=\"origami2-generation-provenance\">".as_slice()
                }
                CreasePatternExportFormat::Pdf17 => b"% ORIGAMI2_GENERATION_PROVENANCE ".as_slice(),
                CreasePatternExportFormat::Dxf2007Ascii => {
                    b"ORIGAMI2_GENERATION_PROVENANCE ".as_slice()
                }
            };
            artifact.bytes.extend_from_slice(marker);
            assert!(matches!(
                read_crease_pattern_generation_provenance(format, &artifact.bytes),
                Err(CreasePatternExportError::DuplicateGenerationProvenance)
            ));
        }

        let fold = export_crease_pattern_with_provenance(
            CreasePatternExportFormat::Fold12,
            "unknown",
            &pattern,
            &paper,
            Some(&provenance),
        )
        .unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&fold.bytes).unwrap();
        value["origami2_generation_provenance"]["unknown"] = serde_json::json!(true);
        assert!(matches!(
            read_crease_pattern_generation_provenance(
                CreasePatternExportFormat::Fold12,
                &serde_json::to_vec(&value).unwrap(),
            ),
            Err(CreasePatternExportError::MalformedGenerationProvenance)
        ));

        let malformed = b"<metadata id=\"origami2-generation-provenance\">0G</metadata>";
        assert!(matches!(
            read_crease_pattern_generation_provenance(CreasePatternExportFormat::Svg, malformed),
            Err(CreasePatternExportError::MalformedGenerationProvenance)
        ));
        let one_short = vec![b'x'; MAX_CREASE_PATTERN_EXPORT_BYTES - 1];
        assert_eq!(
            read_crease_pattern_generation_provenance(CreasePatternExportFormat::Pdf17, &one_short)
                .unwrap(),
            None
        );
        let oversized = vec![b'x'; MAX_CREASE_PATTERN_EXPORT_BYTES + 1];
        assert!(matches!(
            read_crease_pattern_generation_provenance(CreasePatternExportFormat::Pdf17, &oversized),
            Err(CreasePatternExportError::OutputTooLarge { .. })
        ));
    }
}
