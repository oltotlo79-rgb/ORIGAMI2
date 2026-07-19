//! Versioned persistence and interchange adapters.

mod crease_pattern_export;
mod fold;
mod instruction_export;
mod mesh_export;
mod ori2;
mod svg;

use ori_domain::{
    ConstraintId, CreasePattern, EdgeId, GeometricConstraintDocumentV1,
    GeometricConstraintDocumentValidationErrorV1, GeometricConstraintKindV1, InstructionTimeline,
    InstructionTimelineValidationError, Paper, ProjectId, ProjectLayerDocumentV1,
    ProjectLayerDocumentValidationErrorV1, VertexId, validate_geometric_constraint_document_v1,
    validate_instruction_timeline, validate_project_layer_document_against_pattern_v1,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crease_pattern_export::{
    CreasePatternExportArtifact, CreasePatternExportEndpoint, CreasePatternExportError,
    CreasePatternExportFormat, CreasePatternExportLimits,
    MAX_CREASE_PATTERN_EXPORT_BOUNDARY_VERTICES, MAX_CREASE_PATTERN_EXPORT_BYTES,
    MAX_CREASE_PATTERN_EXPORT_EDGES, MAX_CREASE_PATTERN_EXPORT_INTERSECTION_CANDIDATES,
    MAX_CREASE_PATTERN_EXPORT_TITLE_CHARS, MAX_CREASE_PATTERN_EXPORT_VERTICES,
    export_crease_pattern, export_crease_pattern_with_limits,
};
pub use fold::{
    FoldAssignmentCounts, FoldAssignmentMapping, FoldAssignmentTarget, FoldConversionError,
    FoldConversionOptions, FoldCreasePatternConversion, FoldEdgeAssignment, FoldFrameUnit,
    FoldImportError, FoldImportLimits, FoldPreview, FoldPreviewEdge, FoldPreviewVertex,
    FoldPreviewWarning, read_fold_preview, read_fold_preview_with_limits,
};
pub use instruction_export::{
    INSTRUCTION_EXPORT_PROFILE, INSTRUCTION_EXPORT_WARNINGS, INSTRUCTION_PROJECTION_PROFILE,
    InstructionDiagramError, InstructionDiagramLimits, InstructionExportArtifact,
    InstructionExportError, InstructionExportFormat, InstructionExportLimits,
    InstructionExportWarning, MAX_INSTRUCTION_EXPORT_BYTES, MAX_INSTRUCTION_EXPORT_GLYPHS,
    MAX_INSTRUCTION_EXPORT_PAGE_BYTES, MAX_INSTRUCTION_EXPORT_PAGES,
    MAX_INSTRUCTION_EXPORT_TITLE_CHARS, export_instruction_document,
    export_instruction_document_with_limits,
};
pub use mesh_export::{
    INDEXED_TRIANGLE_MESH_SCHEMA_VERSION_V1, IndexedTriangleMeshV1, MAX_STATIC_MESH_EXPORT_BYTES,
    MAX_STATIC_MESH_NAME_BYTES, MAX_STATIC_MESH_NAME_CHARS, MAX_STATIC_MESH_TRIANGLES,
    MAX_STATIC_MESH_VERTICES, STATIC_MESH_SOURCE_AXIS, STATIC_MESH_SOURCE_UNIT,
    StaticMeshEncodedPrecision, StaticMeshExportArtifact, StaticMeshExportError,
    StaticMeshExportFormat, StaticMeshExportLimits, ValidatedIndexedTriangleMesh,
    export_static_triangle_mesh, export_static_triangle_mesh_with_limits,
    validate_indexed_triangle_mesh, validate_indexed_triangle_mesh_with_limits,
};
pub use ori_domain::{
    DEFAULT_MAX_CONSTRAINT_EDGES as MAX_PROJECT_CONSTRAINT_INDEX_EDGES,
    DEFAULT_MAX_CONSTRAINT_VERTICES as MAX_PROJECT_CONSTRAINT_INDEX_VERTICES,
};
pub use ori2::{
    CURRENT_ORI2_CONTAINER_VERSION, MAX_EDITOR_HISTORY_JSON_BYTES, ORI2_CONTAINER_IDENTIFIER,
    ORI2_EDITOR_HISTORY_PATH, ORI2_FEATURE_EDITOR_HISTORY_V1,
    ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1, ORI2_FEATURE_INSTRUCTION_TIMELINE_V1,
    ORI2_FEATURE_LAYERS_V1, ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1, ORI2_MANIFEST_PATH,
    ORI2_PROJECT_PATH, Ori2EditorHistoryEntry, Ori2Limits, Ori2Manifest, Ori2ProjectArchive,
    Ori2ProjectEntry, read_project_archive_ori2, read_project_archive_ori2_with_limits,
    read_project_ori2, read_project_ori2_with_limits, write_project_archive_ori2,
    write_project_archive_ori2_with_limits, write_project_ori2, write_project_ori2_with_limits,
};
pub use svg::{
    SvgBoundaryCandidate, SvgBoundaryCandidateId, SvgBoundaryCandidateKind, SvgConversionError,
    SvgConversionOptions, SvgConvertedGroup, SvgCreasePatternConversion, SvgDashPattern,
    SvgGroupMapping, SvgGroupTarget, SvgImportError, SvgImportLimits, SvgLineCap, SvgPreview,
    SvgPreviewEdge, SvgPreviewVertex, SvgPreviewWarning, SvgRootLengthUnit, SvgRootPhysicalSize,
    SvgRootViewBox, SvgStyleGroup, SvgStyleGroupId, SvgWarningKind, read_svg_preview,
    read_svg_preview_with_limits,
};

pub const CURRENT_FORMAT_VERSION: u32 = 1;
pub const PROJECT_NUMERIC_EXPRESSIONS_SCHEMA_VERSION: u32 = 1;
pub const MAX_PROJECT_NUMERIC_EXPRESSION_SOURCE_BYTES: usize = 4_096;
/// Non-relaxable byte ceiling for directly supplied `project.json` input.
pub const MAX_PROJECT_JSON_BYTES: usize = 128 * 1024 * 1024;

/// Resource limits applied before parsing a directly supplied `project.json`.
///
/// A caller may lower `max_input_size`, but values above
/// [`MAX_PROJECT_JSON_BYTES`] never relax the format's hard ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectJsonLimits {
    pub max_input_size: usize,
}

impl Default for ProjectJsonLimits {
    fn default() -> Self {
        Self {
            max_input_size: MAX_PROJECT_JSON_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RectangularPaperCreationExpressions {
    pub schema_version: u32,
    pub width_source: String,
    pub height_source: String,
    pub adopted_width_mm: f64,
    pub adopted_height_mm: f64,
}

impl RectangularPaperCreationExpressions {
    #[must_use]
    pub fn new(
        width_source: impl Into<String>,
        height_source: impl Into<String>,
        adopted_width_mm: f64,
        adopted_height_mm: f64,
    ) -> Self {
        Self {
            schema_version: PROJECT_NUMERIC_EXPRESSIONS_SCHEMA_VERSION,
            width_source: width_source.into(),
            height_source: height_source.into(),
            adopted_width_mm,
            adopted_height_mm,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectNumericExpressions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rectangular_paper_creation: Option<RectangularPaperCreationExpressions>,
}

impl ProjectNumericExpressions {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rectangular_paper_creation.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDocument {
    pub format_version: u32,
    pub project_id: ProjectId,
    pub name: String,
    #[serde(default)]
    pub paper: Paper,
    pub crease_pattern: CreasePattern,
    #[serde(default)]
    pub instruction_timeline: InstructionTimeline,
    #[serde(default, skip_serializing_if = "ProjectNumericExpressions::is_empty")]
    pub numeric_expressions: ProjectNumericExpressions,
    #[serde(
        default,
        skip_serializing_if = "GeometricConstraintDocumentV1::is_empty"
    )]
    pub geometric_constraints: GeometricConstraintDocumentV1,
    /// Ordered project layers and explicit non-default edge assignments.
    ///
    /// The exact default is omitted so legacy V1 JSON remains byte-stable.
    #[serde(default, skip_serializing_if = "ProjectLayerDocumentV1::is_default")]
    pub layers: ProjectLayerDocumentV1,
}

impl ProjectDocument {
    #[must_use]
    pub fn new(name: impl Into<String>, crease_pattern: CreasePattern) -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            project_id: ProjectId::new(),
            name: name.into(),
            paper: Paper::default(),
            crease_pattern,
            instruction_timeline: InstructionTimeline::default(),
            numeric_expressions: ProjectNumericExpressions::default(),
            geometric_constraints: GeometricConstraintDocumentV1::default(),
            layers: ProjectLayerDocumentV1::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("project JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("project JSON is {actual} bytes; the limit is {limit} bytes")]
    ProjectJsonTooLarge { actual: usize, limit: usize },
    #[error("project_id must not be the nil UUID")]
    NilProjectId,
    #[error(".ori2 manifest JSON is invalid: {0}")]
    InvalidManifestJson(#[source] serde_json::Error),
    #[error(".ori2 editor-history JSON is invalid: {0}")]
    InvalidEditorHistoryJson(#[source] serde_json::Error),
    #[error(".ori2 editor-history is not a valid history for the current project: {0}")]
    InvalidEditorHistory(#[source] ori_core::EditorHistoryErrorV1),
    #[error(".ori2 ZIP data is invalid: {0}")]
    InvalidZip(#[from] zip::result::ZipError),
    #[error(".ori2 ZIP end-of-central-directory record is missing or invalid")]
    InvalidZipFooter,
    #[error("multi-disk .ori2 ZIP archives are not supported")]
    MultiDiskZipNotSupported,
    #[error("ZIP64 .ori2 archives are not supported")]
    Zip64NotSupported,
    #[error(
        ".ori2 ZIP declares {declared} entries, but {parsed} unique entries were parsed; duplicate names are not allowed"
    )]
    ArchiveEntryCountMismatch { declared: usize, parsed: usize },
    #[error("could not read or write .ori2 data: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "project format version {found} is not supported; latest supported version is {latest}"
    )]
    UnsupportedVersion { found: u32, latest: u32 },
    #[error(".ori2 archive is {actual} bytes; the limit is {limit} bytes")]
    ContainerTooLarge { actual: u64, limit: u64 },
    #[error(".ori2 archive has {actual} entries; the limit is {limit}")]
    TooManyEntries { actual: usize, limit: usize },
    #[error(".ori2 entry path is {actual} bytes; the limit is {limit} bytes")]
    EntryPathTooLong { actual: usize, limit: usize },
    #[error(".ori2 entry path is not safe: {path:?}")]
    UnsafeEntryPath { path: String },
    #[error(".ori2 entry path is not valid UTF-8")]
    NonUtf8EntryPath,
    #[error(".ori2 is missing the required entry: {path}")]
    MissingEntry { path: &'static str },
    #[error("required .ori2 entry is a directory: {path}")]
    RequiredEntryIsDirectory { path: &'static str },
    #[error("encrypted .ori2 entries are not supported: {path}")]
    EncryptedEntry { path: String },
    #[error(".ori2 entry {path} is {actual} bytes; the limit is {limit} bytes")]
    EntryTooLarge {
        path: String,
        actual: u64,
        limit: u64,
    },
    #[error(".ori2 expands to {actual} bytes; the limit is {limit} bytes")]
    ExpandedArchiveTooLarge { actual: u64, limit: u64 },
    #[error("unexpected .ori2 container identifier {found:?}")]
    InvalidContainerIdentifier { found: String },
    #[error(
        ".ori2 container version {found} is not supported; latest supported version is {latest}"
    )]
    UnsupportedContainerVersion { found: u32, latest: u32 },
    #[error(".ori2 requires unsupported features: {features:?}")]
    UnsupportedRequiredFeatures { features: Vec<String> },
    #[error(".ori2 project content requires manifest feature {feature:?}")]
    MissingRequiredFeature { feature: &'static str },
    #[error(".ori2 manifest references an invalid project path: {found:?}")]
    InvalidManifestProjectPath { found: String },
    #[error(".ori2 manifest references an invalid editor-history path: {found:?}")]
    InvalidManifestEditorHistoryPath { found: String },
    #[error(
        ".ori2 editor-history schema version {found} is not supported; latest supported version is {latest}"
    )]
    UnsupportedEditorHistorySchemaVersion { found: u32, latest: u32 },
    #[error(
        ".ori2 editor-history feature and manifest descriptor must either both be present or both be absent"
    )]
    EditorHistoryFeatureDescriptorMismatch,
    #[error(".ori2 contains editor-history data without a manifest descriptor")]
    UnexpectedEditorHistoryEntry,
    #[error(
        ".ori2 manifest declares editor-history size {declared} bytes, but editor-history.json is {actual} bytes"
    )]
    EditorHistorySizeMismatch { declared: u64, actual: u64 },
    #[error(".ori2 editor-history checksum differs (expected {expected}, actual {actual})")]
    EditorHistoryHashMismatch { expected: String, actual: String },
    #[error(".ori2 editor-history is bound to a different project checksum")]
    EditorHistoryProjectHashMismatch,
    #[error(".ori2 editor-history is bound to a different project ID")]
    EditorHistoryProjectIdMismatch,
    #[error(
        "the document-only .ori2 API cannot discard persisted editor history; use the project-archive API"
    )]
    EditorHistoryRequiresArchiveApi,
    #[error(
        ".ori2 manifest declares project size {declared} bytes, but project.json is {actual} bytes"
    )]
    ProjectSizeMismatch { declared: u64, actual: u64 },
    #[error(".ori2 project checksum differs (expected {expected}, actual {actual})")]
    ProjectHashMismatch { expected: String, actual: String },
    #[error(
        ".ori2 manifest declares project format version {manifest}, but project.json declares {project}"
    )]
    ManifestProjectVersionMismatch { manifest: u32, project: u32 },
    #[error("folding instruction timeline is invalid: {0}")]
    InvalidInstructionTimeline(#[from] InstructionTimelineValidationError),
    #[error("project numeric-expression metadata is invalid")]
    InvalidNumericExpressions,
    #[error("project geometric-constraint metadata is invalid: {0}")]
    InvalidGeometricConstraints(#[from] GeometricConstraintDocumentValidationErrorV1),
    #[error("project layer metadata is invalid: {0}")]
    InvalidProjectLayers(#[from] ProjectLayerDocumentValidationErrorV1),
    #[error(
        "cannot validate geometric constraints against {actual} crease-pattern vertices; the hard maximum is {maximum}"
    )]
    TooManyConstraintIndexVertices { actual: usize, maximum: usize },
    #[error(
        "cannot validate geometric constraints against {actual} crease-pattern edges; the hard maximum is {maximum}"
    )]
    TooManyConstraintIndexEdges { actual: usize, maximum: usize },
    #[error("memory for the geometric-constraint {index_name} index could not be reserved")]
    ConstraintIndexAllocationFailed { index_name: &'static str },
    #[error(
        "geometric constraint {constraint:?} references missing crease-pattern vertex {vertex:?}"
    )]
    MissingConstraintVertex {
        constraint: ConstraintId,
        vertex: VertexId,
    },
    #[error("geometric constraint {constraint:?} references missing crease-pattern edge {edge:?}")]
    MissingConstraintEdge {
        constraint: ConstraintId,
        edge: EdgeId,
    },
}

pub fn write_project_json(document: &ProjectDocument) -> Result<Vec<u8>, FormatError> {
    write_project_json_with_size_limit(document, MAX_PROJECT_JSON_BYTES)
}

fn write_project_json_with_size_limit(
    document: &ProjectDocument,
    requested_limit: usize,
) -> Result<Vec<u8>, FormatError> {
    validate_project_envelope(document)?;
    validate_instruction_timeline(&document.instruction_timeline)?;
    validate_numeric_expressions(&document.numeric_expressions)?;
    validate_project_geometric_constraints(document)?;
    validate_project_layer_document_against_pattern_v1(&document.layers, &document.crease_pattern)?;
    let bytes = serde_json::to_vec_pretty(document)?;
    ensure_project_json_size(bytes.len(), requested_limit)?;
    Ok(bytes)
}

pub fn read_project_json(bytes: &[u8]) -> Result<ProjectDocument, FormatError> {
    read_project_json_with_limits(bytes, ProjectJsonLimits::default())
}

/// Reads project JSON with a caller-selected byte limit.
///
/// The byte count is checked before serde sees the input. The caller-selected
/// limit can tighten, but cannot relax, [`MAX_PROJECT_JSON_BYTES`].
pub fn read_project_json_with_limits(
    bytes: &[u8],
    limits: ProjectJsonLimits,
) -> Result<ProjectDocument, FormatError> {
    ensure_project_json_size(bytes.len(), limits.max_input_size)?;
    let document: ProjectDocument = serde_json::from_slice(bytes)?;
    validate_project_envelope(&document)?;
    validate_instruction_timeline(&document.instruction_timeline)?;
    validate_numeric_expressions(&document.numeric_expressions)?;
    validate_project_geometric_constraints(&document)?;
    validate_project_layer_document_against_pattern_v1(&document.layers, &document.crease_pattern)?;
    Ok(document)
}

fn validate_project_envelope(document: &ProjectDocument) -> Result<(), FormatError> {
    if document.format_version != CURRENT_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: document.format_version,
            latest: CURRENT_FORMAT_VERSION,
        });
    }
    if document.project_id.canonical_bytes() == [0; 16] {
        return Err(FormatError::NilProjectId);
    }
    Ok(())
}

fn ensure_project_json_size(actual: usize, requested_limit: usize) -> Result<(), FormatError> {
    let limit = requested_limit.min(MAX_PROJECT_JSON_BYTES);
    if actual > limit {
        return Err(FormatError::ProjectJsonTooLarge { actual, limit });
    }
    Ok(())
}

fn canonical_edge_reference_pair(first: EdgeId, second: EdgeId) -> [EdgeId; 2] {
    if first.canonical_bytes() <= second.canonical_bytes() {
        [first, second]
    } else {
        [second, first]
    }
}

fn canonical_vertex_reference_pair(first: VertexId, second: VertexId) -> [VertexId; 2] {
    if first.canonical_bytes() <= second.canonical_bytes() {
        [first, second]
    } else {
        [second, first]
    }
}

/// Validates only persistence metadata and entity existence.
///
/// Geometry-dependent checks intentionally stay out of this adapter so an
/// incomplete or degenerate crease pattern remains loadable for EDT-011 repair.
/// Empty constraint documents return before any crease-pattern size ceiling is
/// applied. For non-empty documents, the bounded sorted indexes and sorted
/// constraint traversal cost `O((V + E + C) log(V + E + C))` time and
/// `O(V + E + C)` storage.
fn validate_project_geometric_constraints(document: &ProjectDocument) -> Result<(), FormatError> {
    validate_geometric_constraint_document_v1(&document.geometric_constraints)?;
    if document.geometric_constraints.is_empty() {
        return Ok(());
    }

    let vertex_count = document.crease_pattern.vertices.len();
    if vertex_count > MAX_PROJECT_CONSTRAINT_INDEX_VERTICES {
        return Err(FormatError::TooManyConstraintIndexVertices {
            actual: vertex_count,
            maximum: MAX_PROJECT_CONSTRAINT_INDEX_VERTICES,
        });
    }
    let edge_count = document.crease_pattern.edges.len();
    if edge_count > MAX_PROJECT_CONSTRAINT_INDEX_EDGES {
        return Err(FormatError::TooManyConstraintIndexEdges {
            actual: edge_count,
            maximum: MAX_PROJECT_CONSTRAINT_INDEX_EDGES,
        });
    }

    let mut vertices = Vec::new();
    vertices.try_reserve_exact(vertex_count).map_err(|_| {
        FormatError::ConstraintIndexAllocationFailed {
            index_name: "vertex",
        }
    })?;
    vertices.extend(
        document
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id.canonical_bytes()),
    );
    vertices.sort_unstable();
    vertices.dedup();

    let mut edges = Vec::new();
    edges
        .try_reserve_exact(edge_count)
        .map_err(|_| FormatError::ConstraintIndexAllocationFailed { index_name: "edge" })?;
    edges.extend(
        document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id.canonical_bytes()),
    );
    edges.sort_unstable();
    edges.dedup();

    let constraint_count = document.geometric_constraints.constraints.len();
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(constraint_count)
        .map_err(|_| FormatError::ConstraintIndexAllocationFailed {
            index_name: "constraint-order",
        })?;
    constraints.extend(document.geometric_constraints.constraints.iter());
    constraints.sort_unstable_by_key(|record| record.id.canonical_bytes());

    for record in constraints {
        let require_vertex = |vertex: VertexId| {
            if vertices.binary_search(&vertex.canonical_bytes()).is_ok() {
                Ok(())
            } else {
                Err(FormatError::MissingConstraintVertex {
                    constraint: record.id,
                    vertex,
                })
            }
        };
        let require_edge = |edge: EdgeId| {
            if edges.binary_search(&edge.canonical_bytes()).is_ok() {
                Ok(())
            } else {
                Err(FormatError::MissingConstraintEdge {
                    constraint: record.id,
                    edge,
                })
            }
        };

        match &record.constraint {
            GeometricConstraintKindV1::FixedLength { edge, .. }
            | GeometricConstraintKindV1::Horizontal { edge }
            | GeometricConstraintKindV1::Vertical { edge } => require_edge(*edge)?,
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                ..
            } => {
                require_vertex(*vertex)?;
                for edge in canonical_edge_reference_pair(*first_edge, *second_edge) {
                    require_edge(edge)?;
                }
            }
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            }
            | GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } => {
                for edge in canonical_edge_reference_pair(*first_edge, *second_edge) {
                    require_edge(edge)?;
                }
            }
            GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
                require_vertex(*vertex)?;
                require_edge(*line_edge)?;
            }
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                axis_edge,
            } => {
                for vertex in canonical_vertex_reference_pair(*first_vertex, *second_vertex) {
                    require_vertex(vertex)?;
                }
                require_edge(*axis_edge)?;
            }
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex,
                source_vertex,
                target_vertex,
                ..
            } => {
                require_vertex(*center_vertex)?;
                require_vertex(*source_vertex)?;
                require_vertex(*target_vertex)?;
            }
            GeometricConstraintKindV1::AngleBisector {
                vertex,
                first_edge,
                second_edge,
                bisector_edge,
            } => {
                require_vertex(*vertex)?;
                for edge in canonical_edge_reference_pair(*first_edge, *second_edge) {
                    require_edge(edge)?;
                }
                require_edge(*bisector_edge)?;
            }
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ..
            } => {
                require_edge(*numerator_edge)?;
                require_edge(*denominator_edge)?;
            }
        }
    }

    Ok(())
}

fn validate_numeric_expressions(
    expressions: &ProjectNumericExpressions,
) -> Result<(), FormatError> {
    let Some(rectangular) = &expressions.rectangular_paper_creation else {
        return Ok(());
    };
    if rectangular.schema_version != PROJECT_NUMERIC_EXPRESSIONS_SCHEMA_VERSION
        || !valid_numeric_expression_source(&rectangular.width_source)
        || !valid_numeric_expression_source(&rectangular.height_source)
        || !rectangular.adopted_width_mm.is_finite()
        || rectangular.adopted_width_mm <= 0.0
        || !rectangular.adopted_height_mm.is_finite()
        || rectangular.adopted_height_mm <= 0.0
    {
        return Err(FormatError::InvalidNumericExpressions);
    }
    Ok(())
}

fn valid_numeric_expression_source(source: &str) -> bool {
    !source.trim().is_empty()
        && source.len() <= MAX_PROJECT_NUMERIC_EXPRESSION_SOURCE_BYTES
        && !source.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        AssetId, Edge, EdgeId, EdgeKind, EdgeLayerAssignmentV1, FaceId,
        GeometricConstraintRecordV1, InstructionHingeAngle, InstructionPose, InstructionPoseModel,
        InstructionStep, InstructionStepId, LayerContentKindV1, LayerId, LayerRecordV1,
        LengthDisplayUnit, PaperAppearance, Point2, RgbaColor, Vertex, VertexId,
    };

    use super::*;

    fn sample_document() -> ProjectDocument {
        let start = VertexId::new();
        let end = VertexId::new();
        ProjectDocument::new(
            "Mountain fold",
            CreasePattern {
                vertices: vec![
                    Vertex {
                        id: start,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: end,
                        position: Point2::new(10.0, 4.0),
                    },
                ],
                edges: vec![Edge {
                    id: EdgeId::new(),
                    start,
                    end,
                    kind: EdgeKind::Mountain,
                }],
            },
        )
    }

    fn project_id_from_wire(value: &str) -> ProjectId {
        serde_json::from_str(&format!("\"{value}\"")).expect("project ID fixture")
    }

    fn fixed_constraint_id(index: usize) -> ConstraintId {
        serde_json::from_str(&format!("\"10000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed constraint ID")
    }

    fn fixed_vertex_id(index: usize) -> VertexId {
        serde_json::from_str(&format!("\"20000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed vertex ID")
    }

    fn fixed_edge_id(index: usize) -> EdgeId {
        serde_json::from_str(&format!("\"30000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed edge ID")
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
                    kind: EdgeKind::Mountain,
                })
                .collect(),
        };

        let kinds = [
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
        ];
        document.geometric_constraints.constraints = kinds
            .into_iter()
            .map(|constraint| GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            })
            .collect();
    }

    #[test]
    fn json_round_trip_preserves_ids_geometry_and_kinds() {
        let original = sample_document();
        let bytes = write_project_json(&original).expect("write project");
        assert!(bytes.len() <= MAX_PROJECT_JSON_BYTES);
        let restored = read_project_json(&bytes).expect("read project");
        assert_eq!(restored, original);
    }

    #[test]
    fn json_reader_and_writer_reject_the_nil_project_id_with_a_typed_error() {
        let mut document = sample_document();
        document.project_id = project_id_from_wire("00000000-0000-0000-0000-000000000000");

        assert!(matches!(
            write_project_json(&document),
            Err(FormatError::NilProjectId)
        ));

        let bytes = serde_json::to_vec(&document).expect("serialize nil-ID read fixture");
        assert!(matches!(
            read_project_json(&bytes),
            Err(FormatError::NilProjectId)
        ));
    }

    #[test]
    fn json_round_trip_accepts_non_nil_uuid_versions_and_variants() {
        for wire in [
            "10000000-0000-0000-0000-000000000001",
            "10000000-0000-1000-8000-000000000001",
            "10000000-0000-f000-c000-000000000001",
            "10000000-0000-7000-e000-000000000001",
        ] {
            let mut document = sample_document();
            document.project_id = project_id_from_wire(wire);

            let bytes = write_project_json(&document).expect("write non-nil project ID");
            let restored = read_project_json(&bytes).expect("read non-nil project ID");
            assert_eq!(restored.project_id, document.project_id, "{wire}");
        }
    }

    #[test]
    fn empty_geometric_constraints_are_omitted_and_legacy_json_defaults_to_empty() {
        let document = sample_document();
        let bytes = write_project_json(&document).expect("write project");
        let mut value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("parse project JSON");
        assert!(
            value.get("geometric_constraints").is_none(),
            "an empty optional feature must not alter canonical legacy JSON"
        );

        value
            .as_object_mut()
            .expect("project object")
            .remove("geometric_constraints");
        let legacy = serde_json::to_vec(&value).expect("serialize legacy fixture");
        let restored = read_project_json(&legacy).expect("read legacy project");
        assert!(restored.geometric_constraints.is_empty());
        let rewritten = write_project_json(&restored).expect("rewrite legacy project");
        assert!(
            !String::from_utf8(rewritten)
                .expect("JSON is UTF-8")
                .contains("geometric_constraints")
        );
    }

    #[test]
    fn project_document_rejects_unknown_envelope_fields() {
        let mut value = serde_json::to_value(sample_document()).expect("serialize project");
        value.as_object_mut().expect("project object").insert(
            "future_format_extension".to_owned(),
            serde_json::json!(true),
        );
        let bytes = serde_json::to_vec(&value).expect("serialize future project");

        assert!(matches!(
            read_project_json(&bytes),
            Err(FormatError::InvalidJson(_))
        ));
    }

    #[test]
    fn project_json_size_guard_has_exact_and_one_over_hard_ceiling_semantics() {
        ensure_project_json_size(MAX_PROJECT_JSON_BYTES, usize::MAX)
            .expect("equality with the hard ceiling must succeed");
        assert!(matches!(
            ensure_project_json_size(MAX_PROJECT_JSON_BYTES + 1, usize::MAX),
            Err(FormatError::ProjectJsonTooLarge {
                actual,
                limit: MAX_PROJECT_JSON_BYTES
            }) if actual == MAX_PROJECT_JSON_BYTES + 1
        ));

        ensure_project_json_size(7, 7).expect("equality with a tighter limit must succeed");
        assert!(matches!(
            ensure_project_json_size(8, 7),
            Err(FormatError::ProjectJsonTooLarge {
                actual: 8,
                limit: 7
            })
        ));

        let document = sample_document();
        let expected = serde_json::to_vec_pretty(&document).expect("serialize size fixture");
        assert_eq!(
            write_project_json_with_size_limit(&document, expected.len())
                .expect("writer accepts its exact generated size"),
            expected
        );
        assert!(matches!(
            write_project_json_with_size_limit(&document, expected.len() - 1),
            Err(FormatError::ProjectJsonTooLarge { actual, limit })
                if actual == expected.len() && limit == expected.len() - 1
        ));

        // The public reader invokes this same guard before serde.
        assert!(matches!(
            read_project_json_with_limits(b"!", ProjectJsonLimits { max_input_size: 0 }),
            Err(FormatError::ProjectJsonTooLarge {
                actual: 1,
                limit: 0
            })
        ));
    }

    #[test]
    fn direct_json_reader_honours_a_smaller_custom_limit_at_the_exact_boundary() {
        let original = sample_document();
        let bytes = write_project_json(&original).expect("write project");
        let exact = ProjectJsonLimits {
            max_input_size: bytes.len(),
        };
        assert_eq!(
            read_project_json_with_limits(&bytes, exact).expect("exact custom limit"),
            original
        );

        let smaller = ProjectJsonLimits {
            max_input_size: bytes.len() - 1,
        };
        assert!(matches!(
            read_project_json_with_limits(&bytes, smaller),
            Err(FormatError::ProjectJsonTooLarge { actual, limit })
                if actual == bytes.len() && limit == bytes.len() - 1
        ));
    }

    #[test]
    fn json_round_trip_preserves_all_eleven_geometric_constraint_kinds() {
        let mut original = sample_document();
        add_all_geometric_constraint_kinds(&mut original);

        let bytes = write_project_json(&original).expect("write constraints");
        assert!(
            String::from_utf8(bytes.clone())
                .expect("JSON is UTF-8")
                .contains("\"geometric_constraints\"")
        );
        let restored = read_project_json(&bytes).expect("read constraints");

        assert_eq!(restored, original);
        assert_eq!(restored.geometric_constraints.constraints.len(), 11);
    }

    #[test]
    fn json_reader_and_writer_reject_invalid_geometric_constraint_metadata() {
        let mut document = sample_document();
        add_all_geometric_constraint_kinds(&mut document);
        document.geometric_constraints.schema_version += 1;

        let write_error =
            write_project_json(&document).expect_err("writer must reject future metadata");
        assert!(matches!(
            write_error,
            FormatError::InvalidGeometricConstraints(
                GeometricConstraintDocumentValidationErrorV1::UnsupportedSchemaVersion { .. }
            )
        ));

        let bytes = serde_json::to_vec(&document).expect("serialize invalid fixture directly");
        let read_error = read_project_json(&bytes).expect_err("reader must reject future metadata");
        assert!(matches!(
            read_error,
            FormatError::InvalidGeometricConstraints(
                GeometricConstraintDocumentValidationErrorV1::UnsupportedSchemaVersion { .. }
            )
        ));
    }

    #[test]
    fn json_reader_and_writer_reject_dangling_constraint_references() {
        let mut missing_vertex = sample_document();
        add_all_geometric_constraint_kinds(&mut missing_vertex);
        let absent_vertex = VertexId::new();
        let constraint_id = missing_vertex.geometric_constraints.constraints[1].id;
        let GeometricConstraintKindV1::FixedAngle { vertex, .. } =
            &mut missing_vertex.geometric_constraints.constraints[1].constraint
        else {
            panic!("second fixture is fixed angle");
        };
        *vertex = absent_vertex;
        assert!(matches!(
            write_project_json(&missing_vertex),
            Err(FormatError::MissingConstraintVertex {
                constraint,
                vertex
            }) if constraint == constraint_id && vertex == absent_vertex
        ));
        let bytes = serde_json::to_vec(&missing_vertex).expect("serialize dangling vertex fixture");
        assert!(matches!(
            read_project_json(&bytes),
            Err(FormatError::MissingConstraintVertex {
                constraint,
                vertex
            }) if constraint == constraint_id && vertex == absent_vertex
        ));

        let mut missing_edge = sample_document();
        add_all_geometric_constraint_kinds(&mut missing_edge);
        let absent_edge = EdgeId::new();
        let constraint_id = missing_edge.geometric_constraints.constraints[0].id;
        let GeometricConstraintKindV1::FixedLength { edge, .. } =
            &mut missing_edge.geometric_constraints.constraints[0].constraint
        else {
            panic!("first fixture is fixed length");
        };
        *edge = absent_edge;
        assert!(matches!(
            write_project_json(&missing_edge),
            Err(FormatError::MissingConstraintEdge { constraint, edge })
                if constraint == constraint_id && edge == absent_edge
        ));
        let bytes = serde_json::to_vec(&missing_edge).expect("serialize dangling edge fixture");
        assert!(matches!(
            read_project_json(&bytes),
            Err(FormatError::MissingConstraintEdge { constraint, edge })
                if constraint == constraint_id && edge == absent_edge
        ));
    }

    #[test]
    fn every_reference_role_across_all_eleven_kinds_requires_an_existing_entity() {
        #[derive(Clone, Copy)]
        enum Reference {
            Vertex(VertexId),
            Edge(EdgeId),
        }

        let mut original = sample_document();
        add_all_geometric_constraint_kinds(&mut original);
        for record in original.geometric_constraints.constraints.clone() {
            let references = match record.constraint {
                GeometricConstraintKindV1::FixedLength { edge, .. }
                | GeometricConstraintKindV1::Horizontal { edge }
                | GeometricConstraintKindV1::Vertical { edge } => {
                    vec![Reference::Edge(edge)]
                }
                GeometricConstraintKindV1::FixedAngle {
                    vertex,
                    first_edge,
                    second_edge,
                    ..
                } => vec![
                    Reference::Vertex(vertex),
                    Reference::Edge(first_edge),
                    Reference::Edge(second_edge),
                ],
                GeometricConstraintKindV1::EqualLength {
                    first_edge,
                    second_edge,
                }
                | GeometricConstraintKindV1::Parallel {
                    first_edge,
                    second_edge,
                } => vec![Reference::Edge(first_edge), Reference::Edge(second_edge)],
                GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
                    vec![Reference::Vertex(vertex), Reference::Edge(line_edge)]
                }
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex,
                    second_vertex,
                    axis_edge,
                } => vec![
                    Reference::Vertex(first_vertex),
                    Reference::Vertex(second_vertex),
                    Reference::Edge(axis_edge),
                ],
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex,
                    source_vertex,
                    target_vertex,
                    ..
                } => vec![
                    Reference::Vertex(center_vertex),
                    Reference::Vertex(source_vertex),
                    Reference::Vertex(target_vertex),
                ],
                GeometricConstraintKindV1::AngleBisector {
                    vertex,
                    first_edge,
                    second_edge,
                    bisector_edge,
                } => vec![
                    Reference::Vertex(vertex),
                    Reference::Edge(first_edge),
                    Reference::Edge(second_edge),
                    Reference::Edge(bisector_edge),
                ],
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge,
                    denominator_edge,
                    ..
                } => vec![
                    Reference::Edge(numerator_edge),
                    Reference::Edge(denominator_edge),
                ],
            };

            for reference in references {
                let mut candidate = original.clone();
                candidate.geometric_constraints.constraints = vec![record.clone()];
                match reference {
                    Reference::Vertex(vertex) => {
                        candidate
                            .crease_pattern
                            .vertices
                            .retain(|candidate| candidate.id != vertex);
                        assert!(matches!(
                            validate_project_geometric_constraints(&candidate),
                            Err(FormatError::MissingConstraintVertex {
                                constraint,
                                vertex: missing
                            }) if constraint == record.id && missing == vertex
                        ));
                    }
                    Reference::Edge(edge) => {
                        candidate
                            .crease_pattern
                            .edges
                            .retain(|candidate| candidate.id != edge);
                        assert!(matches!(
                            validate_project_geometric_constraints(&candidate),
                            Err(FormatError::MissingConstraintEdge {
                                constraint,
                                edge: missing
                            }) if constraint == record.id && missing == edge
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn dangling_reference_failure_is_deterministic_across_record_order() {
        let first_id = ConstraintId::new();
        let second_id = ConstraintId::new();
        let first_edge = EdgeId::new();
        let second_edge = EdgeId::new();
        let expected_constraint = if first_id.canonical_bytes() < second_id.canonical_bytes() {
            first_id
        } else {
            second_id
        };
        let records = vec![
            GeometricConstraintRecordV1 {
                id: first_id,
                constraint: GeometricConstraintKindV1::Horizontal { edge: first_edge },
            },
            GeometricConstraintRecordV1 {
                id: second_id,
                constraint: GeometricConstraintKindV1::Vertical { edge: second_edge },
            },
        ];
        let mut document = ProjectDocument::new("Deterministic error", CreasePattern::empty());
        document.geometric_constraints.constraints = records;

        for reverse in [false, true] {
            if reverse {
                document.geometric_constraints.constraints.reverse();
            }
            assert!(matches!(
                validate_project_geometric_constraints(&document),
                Err(FormatError::MissingConstraintEdge { constraint, .. })
                    if constraint == expected_constraint
            ));
        }
    }

    #[test]
    fn unordered_constraint_reference_failures_use_canonical_id_order() {
        #[derive(Debug, Clone, Copy)]
        enum MissingReference {
            Edge(EdgeId),
            Vertex(VertexId),
        }

        let low_edge = fixed_edge_id(1);
        let high_edge = fixed_edge_id(2);
        let existing_edge = fixed_edge_id(9);
        let low_vertex = fixed_vertex_id(1);
        let high_vertex = fixed_vertex_id(2);
        let existing_vertex = fixed_vertex_id(9);
        let cases = vec![
            (
                "fixed_angle",
                GeometricConstraintKindV1::FixedAngle {
                    vertex: existing_vertex,
                    first_edge: low_edge,
                    second_edge: high_edge,
                    angle_degrees: 90.0,
                },
                GeometricConstraintKindV1::FixedAngle {
                    vertex: existing_vertex,
                    first_edge: high_edge,
                    second_edge: low_edge,
                    angle_degrees: 90.0,
                },
                MissingReference::Edge(low_edge),
            ),
            (
                "equal_length",
                GeometricConstraintKindV1::EqualLength {
                    first_edge: low_edge,
                    second_edge: high_edge,
                },
                GeometricConstraintKindV1::EqualLength {
                    first_edge: high_edge,
                    second_edge: low_edge,
                },
                MissingReference::Edge(low_edge),
            ),
            (
                "parallel",
                GeometricConstraintKindV1::Parallel {
                    first_edge: low_edge,
                    second_edge: high_edge,
                },
                GeometricConstraintKindV1::Parallel {
                    first_edge: high_edge,
                    second_edge: low_edge,
                },
                MissingReference::Edge(low_edge),
            ),
            (
                "mirror_symmetry",
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: low_vertex,
                    second_vertex: high_vertex,
                    axis_edge: existing_edge,
                },
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: high_vertex,
                    second_vertex: low_vertex,
                    axis_edge: existing_edge,
                },
                MissingReference::Vertex(low_vertex),
            ),
            (
                "angle_bisector",
                GeometricConstraintKindV1::AngleBisector {
                    vertex: existing_vertex,
                    first_edge: low_edge,
                    second_edge: high_edge,
                    bisector_edge: existing_edge,
                },
                GeometricConstraintKindV1::AngleBisector {
                    vertex: existing_vertex,
                    first_edge: high_edge,
                    second_edge: low_edge,
                    bisector_edge: existing_edge,
                },
                MissingReference::Edge(low_edge),
            ),
        ];

        for (name, forward, reversed, expected) in cases {
            for constraint in [forward, reversed] {
                let mut document = ProjectDocument::new(
                    "Canonical references",
                    CreasePattern {
                        vertices: vec![Vertex {
                            id: existing_vertex,
                            position: Point2::new(0.0, 0.0),
                        }],
                        edges: vec![Edge {
                            id: existing_edge,
                            start: existing_vertex,
                            end: existing_vertex,
                            kind: EdgeKind::Mountain,
                        }],
                    },
                );
                let constraint_id = fixed_constraint_id(1);
                document.geometric_constraints.constraints = vec![GeometricConstraintRecordV1 {
                    id: constraint_id,
                    constraint,
                }];
                match (expected, validate_project_geometric_constraints(&document)) {
                    (
                        MissingReference::Edge(expected_edge),
                        Err(FormatError::MissingConstraintEdge { constraint, edge }),
                    ) => {
                        assert_eq!(constraint, constraint_id, "{name}");
                        assert_eq!(edge, expected_edge, "{name}");
                    }
                    (
                        MissingReference::Vertex(expected_vertex),
                        Err(FormatError::MissingConstraintVertex { constraint, vertex }),
                    ) => {
                        assert_eq!(constraint, constraint_id, "{name}");
                        assert_eq!(vertex, expected_vertex, "{name}");
                    }
                    (expected, actual) => {
                        panic!("{name}: expected {expected:?}, got {actual:?}")
                    }
                }
            }
        }
    }

    #[test]
    fn constraint_existence_check_preserves_repairable_degenerate_geometry() {
        let missing_endpoint = VertexId::new();
        let edge = EdgeId::new();
        let mut document = ProjectDocument::new(
            "Repairable",
            CreasePattern {
                vertices: Vec::new(),
                edges: vec![Edge {
                    id: edge,
                    start: missing_endpoint,
                    end: missing_endpoint,
                    kind: EdgeKind::Mountain,
                }],
            },
        );
        document.geometric_constraints.constraints = vec![GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::FixedLength {
                edge,
                length_mm: 1.0,
            },
        }];

        let bytes = write_project_json(&document)
            .expect("an existing edge remains saveable despite unusable endpoints");
        let restored =
            read_project_json(&bytes).expect("repairable incomplete geometry remains loadable");
        assert_eq!(restored, document);
    }

    #[test]
    fn constraint_reference_indexes_enforce_exact_hard_ceilings_only_when_needed() {
        let endpoint = VertexId::new();
        let referenced_edge = EdgeId::new();
        let mut document = ProjectDocument::new(
            "Bounded constraint indexes",
            CreasePattern {
                vertices: (0..MAX_PROJECT_CONSTRAINT_INDEX_VERTICES)
                    .map(|index| Vertex {
                        id: endpoint,
                        position: Point2::new(index as f64, 0.0),
                    })
                    .collect(),
                edges: (0..MAX_PROJECT_CONSTRAINT_INDEX_EDGES)
                    .map(|_| Edge {
                        id: referenced_edge,
                        start: endpoint,
                        end: endpoint,
                        kind: EdgeKind::Mountain,
                    })
                    .collect(),
            },
        );
        document.geometric_constraints.constraints = vec![GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: referenced_edge,
            },
        }];

        validate_project_geometric_constraints(&document).expect("exact ceilings are accepted");
        document.crease_pattern.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 0.0),
        });
        assert!(matches!(
            validate_project_geometric_constraints(&document),
            Err(FormatError::TooManyConstraintIndexVertices {
                actual,
                maximum: MAX_PROJECT_CONSTRAINT_INDEX_VERTICES
            }) if actual == MAX_PROJECT_CONSTRAINT_INDEX_VERTICES + 1
        ));

        document.crease_pattern.vertices.pop();
        document.crease_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: endpoint,
            end: endpoint,
            kind: EdgeKind::Mountain,
        });
        assert!(matches!(
            validate_project_geometric_constraints(&document),
            Err(FormatError::TooManyConstraintIndexEdges {
                actual,
                maximum: MAX_PROJECT_CONSTRAINT_INDEX_EDGES
            }) if actual == MAX_PROJECT_CONSTRAINT_INDEX_EDGES + 1
        ));

        document.geometric_constraints = GeometricConstraintDocumentV1::default();
        validate_project_geometric_constraints(&document)
            .expect("unrelated oversized repair geometry is ignored without constraints");
    }

    #[test]
    fn constraint_reference_indexes_use_the_domain_shared_hard_ceilings() {
        assert_eq!(
            MAX_PROJECT_CONSTRAINT_INDEX_VERTICES,
            ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES
        );
        assert_eq!(
            MAX_PROJECT_CONSTRAINT_INDEX_EDGES,
            ori_domain::DEFAULT_MAX_CONSTRAINT_EDGES
        );
    }

    fn add_sample_instruction(document: &mut ProjectDocument) {
        let edge = document.crease_pattern.edges[0].id;
        document.instruction_timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: "半分に折る".to_owned(),
            description: "辺を正確に重ねます。".to_owned(),
            caution: "強く折りすぎないでください。".to_owned(),
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

    #[test]
    fn json_round_trip_preserves_instruction_timeline() {
        let mut original = sample_document();
        add_sample_instruction(&mut original);

        let bytes = write_project_json(&original).expect("write instructions");
        let restored = read_project_json(&bytes).expect("read instructions");

        assert_eq!(restored.instruction_timeline, original.instruction_timeline);
    }

    #[test]
    fn json_round_trip_preserves_versioned_creation_dimension_expressions() {
        let mut original = sample_document();
        original.numeric_expressions.rectangular_paper_creation =
            Some(RectangularPaperCreationExpressions::new(
                "200 * sqrt(2)",
                "400 / 3",
                282.842_712_474_619,
                133.333_333_333_333_34,
            ));

        let bytes = write_project_json(&original).expect("write expressions");
        let restored = read_project_json(&bytes).expect("read expressions");

        assert_eq!(restored.numeric_expressions, original.numeric_expressions);
        assert!(
            String::from_utf8(bytes)
                .expect("JSON is UTF-8")
                .contains("\"schema_version\": 1")
        );
    }

    #[test]
    fn new_project_uses_default_paper() {
        let document = ProjectDocument::new("Blank", CreasePattern::empty());
        assert_eq!(document.paper, Paper::default());
        assert!(document.instruction_timeline.steps.is_empty());
    }

    #[test]
    fn legacy_json_without_paper_uses_safe_defaults() {
        let document = sample_document();
        let mut value = serde_json::to_value(&document).expect("serialize project value");
        value
            .as_object_mut()
            .expect("project is a JSON object")
            .remove("paper");
        let bytes = serde_json::to_vec(&value).expect("serialize legacy project");

        let restored = read_project_json(&bytes).expect("read legacy project");
        assert_eq!(restored.paper, Paper::default());
    }

    #[test]
    fn legacy_json_without_length_display_unit_defaults_to_millimetres() {
        let document = sample_document();
        let mut value = serde_json::to_value(&document).expect("serialize project value");
        value["paper"]
            .as_object_mut()
            .expect("paper is a JSON object")
            .remove("length_display_unit");
        let bytes = serde_json::to_vec(&value).expect("serialize legacy project");

        let restored = read_project_json(&bytes).expect("read legacy project");
        assert_eq!(
            restored.paper.length_display_unit,
            LengthDisplayUnit::Millimeter
        );
    }

    #[test]
    fn legacy_json_without_instruction_timeline_uses_empty_default() {
        let document = sample_document();
        let mut value = serde_json::to_value(&document).expect("serialize project value");
        value
            .as_object_mut()
            .expect("project is a JSON object")
            .remove("instruction_timeline");
        let bytes = serde_json::to_vec(&value).expect("serialize legacy project");

        let restored = read_project_json(&bytes).expect("read legacy project");
        assert!(restored.instruction_timeline.steps.is_empty());
    }

    #[test]
    fn legacy_json_without_numeric_expressions_migrates_to_an_empty_binding() {
        let document = sample_document();
        let mut value = serde_json::to_value(&document).expect("serialize project value");
        value
            .as_object_mut()
            .expect("project is a JSON object")
            .remove("numeric_expressions");
        let bytes = serde_json::to_vec(&value).expect("serialize legacy project");

        let restored = read_project_json(&bytes).expect("read legacy project");
        assert!(restored.numeric_expressions.is_empty());
        let rewritten = write_project_json(&restored).expect("rewrite migrated project");
        assert!(
            !String::from_utf8(rewritten)
                .expect("JSON is UTF-8")
                .contains("numeric_expressions")
        );
    }

    #[test]
    fn reader_and_writer_reject_invalid_numeric_expression_metadata() {
        for invalid in [
            RectangularPaperCreationExpressions {
                schema_version: PROJECT_NUMERIC_EXPRESSIONS_SCHEMA_VERSION + 1,
                width_source: "400".to_owned(),
                height_source: "400".to_owned(),
                adopted_width_mm: 400.0,
                adopted_height_mm: 400.0,
            },
            RectangularPaperCreationExpressions::new(" ", "400", 400.0, 400.0),
            RectangularPaperCreationExpressions::new("400", "1\n+ 2", 400.0, 400.0),
            RectangularPaperCreationExpressions::new("400", "400", 0.0, 400.0),
            RectangularPaperCreationExpressions::new(
                "1".repeat(MAX_PROJECT_NUMERIC_EXPRESSION_SOURCE_BYTES + 1),
                "400",
                400.0,
                400.0,
            ),
        ] {
            let mut document = sample_document();
            document.numeric_expressions.rectangular_paper_creation = Some(invalid);
            assert!(matches!(
                write_project_json(&document),
                Err(FormatError::InvalidNumericExpressions)
            ));
            let bytes = serde_json::to_vec(&document).expect("serialize invalid direct fixture");
            assert!(matches!(
                read_project_json(&bytes),
                Err(FormatError::InvalidNumericExpressions)
            ));
        }
    }

    #[test]
    fn json_reader_and_writer_reject_invalid_instruction_timeline() {
        let mut document = sample_document();
        add_sample_instruction(&mut document);
        document.instruction_timeline.steps[0].title.clear();

        let write_error =
            write_project_json(&document).expect_err("writer must reject invalid timeline");
        assert!(matches!(
            write_error,
            FormatError::InvalidInstructionTimeline(
                InstructionTimelineValidationError::EmptyTitle { step_index: 0 }
            )
        ));

        let bytes = serde_json::to_vec(&document).expect("serialize invalid document directly");
        let read_error =
            read_project_json(&bytes).expect_err("reader must reject invalid timeline");
        assert!(matches!(
            read_error,
            FormatError::InvalidInstructionTimeline(
                InstructionTimelineValidationError::EmptyTitle { step_index: 0 }
            )
        ));
    }

    #[test]
    fn json_round_trip_preserves_complete_paper_definition() {
        let mut original = sample_document();
        let boundary_vertices = original
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect();
        let front_texture = AssetId::new();
        let back_texture = AssetId::new();
        original.paper = Paper {
            boundary_vertices,
            thickness_mm: 0.235,
            length_display_unit: LengthDisplayUnit::PaperEdgeRatio {
                reference_edge: original.crease_pattern.edges[0].id,
            },
            cutting_allowed: true,
            front: PaperAppearance {
                color: RgbaColor {
                    red: 18,
                    green: 52,
                    blue: 86,
                    alpha: 240,
                },
                texture_asset: Some(front_texture),
            },
            back: PaperAppearance {
                color: RgbaColor {
                    red: 240,
                    green: 230,
                    blue: 210,
                    alpha: 255,
                },
                texture_asset: Some(back_texture),
            },
        };

        let bytes = write_project_json(&original).expect("write project with paper");
        let restored = read_project_json(&bytes).expect("read project with paper");

        assert_eq!(restored.paper, original.paper);
        assert_eq!(
            restored.paper.length_display_unit,
            original.paper.length_display_unit
        );
        assert_eq!(restored.paper.front.texture_asset, Some(front_texture));
        assert_eq!(restored.paper.back.texture_asset, Some(back_texture));
    }

    #[test]
    fn reader_and_writer_reject_unknown_format_version() {
        let mut document = sample_document();
        document.format_version = CURRENT_FORMAT_VERSION + 1;
        assert!(matches!(
            write_project_json(&document),
            Err(FormatError::UnsupportedVersion {
                found: 2,
                latest: 1
            })
        ));

        let bytes = serde_json::to_vec(&document).expect("serialize future project");
        let error = read_project_json(&bytes).expect_err("future version must fail");
        assert!(matches!(
            error,
            FormatError::UnsupportedVersion {
                found: 2,
                latest: 1
            }
        ));
    }

    #[test]
    fn legacy_json_migrates_to_the_fixed_default_layer_without_rewriting_empty_json() {
        let document = sample_document();
        let bytes = write_project_json(&document).expect("write default-layer project");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("project JSON");
        assert!(
            value.get("layers").is_none(),
            "the semantic default must preserve legacy canonical JSON"
        );

        let restored = read_project_json(&bytes).expect("read legacy-compatible project");
        assert_eq!(restored.layers, ProjectLayerDocumentV1::default());

        let mut legacy = value;
        legacy
            .as_object_mut()
            .expect("project object")
            .remove("layers");
        let migrated = read_project_json(&serde_json::to_vec(&legacy).expect("legacy bytes"))
            .expect("migrate");
        assert_eq!(migrated.layers, ProjectLayerDocumentV1::default());
        assert_eq!(
            write_project_json(&migrated).expect("rewrite migrated project"),
            bytes
        );
    }

    #[test]
    fn authored_layers_round_trip_and_nested_unknown_or_dangling_data_fail_closed() {
        let mut document = sample_document();
        let edge = document.crease_pattern.edges[0].id;
        let layer = LayerRecordV1 {
            id: LayerId::new(),
            name: "Details".to_owned(),
            content_kind: LayerContentKindV1::CreasePattern,
        };
        document.layers.layers.push(layer.clone());
        document
            .layers
            .edge_assignments
            .push(EdgeLayerAssignmentV1 {
                edge,
                layer: layer.id,
            });

        let bytes = write_project_json(&document).expect("write authored layers");
        assert_eq!(
            read_project_json(&bytes).expect("read authored layers"),
            document
        );

        let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).expect("project JSON");
        unknown["layers"]["unexpected"] = serde_json::json!(true);
        assert!(matches!(
            read_project_json(&serde_json::to_vec(&unknown).expect("unknown-field bytes")),
            Err(FormatError::InvalidJson(_))
        ));

        let mut dangling = document.clone();
        dangling.layers.edge_assignments[0].edge = EdgeId::new();
        assert!(matches!(
            write_project_json(&dangling),
            Err(FormatError::InvalidProjectLayers(
                ProjectLayerDocumentValidationErrorV1::MissingAssignedEdge { .. }
            ))
        ));
        let dangling_bytes = serde_json::to_vec(&dangling).expect("raw dangling JSON");
        assert!(matches!(
            read_project_json(&dangling_bytes),
            Err(FormatError::InvalidProjectLayers(
                ProjectLayerDocumentValidationErrorV1::MissingAssignedEdge { .. }
            ))
        ));
    }
}
