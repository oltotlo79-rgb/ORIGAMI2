//! Versioned persistence and interchange adapters.

mod crease_pattern_export;
mod fold;
mod instruction_export;
mod ori2;
mod svg;

use ori_domain::{
    CreasePattern, InstructionTimeline, InstructionTimelineValidationError, Paper, ProjectId,
    validate_instruction_timeline,
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
pub use ori2::{
    CURRENT_ORI2_CONTAINER_VERSION, ORI2_CONTAINER_IDENTIFIER,
    ORI2_FEATURE_INSTRUCTION_TIMELINE_V1, ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1, ORI2_MANIFEST_PATH,
    ORI2_PROJECT_PATH, Ori2Limits, Ori2Manifest, Ori2ProjectEntry, read_project_ori2,
    read_project_ori2_with_limits, write_project_ori2, write_project_ori2_with_limits,
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
        }
    }
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("project JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error(".ori2 manifest JSON is invalid: {0}")]
    InvalidManifestJson(#[source] serde_json::Error),
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
}

pub fn write_project_json(document: &ProjectDocument) -> Result<Vec<u8>, FormatError> {
    validate_instruction_timeline(&document.instruction_timeline)?;
    validate_numeric_expressions(&document.numeric_expressions)?;
    Ok(serde_json::to_vec_pretty(document)?)
}

pub fn read_project_json(bytes: &[u8]) -> Result<ProjectDocument, FormatError> {
    let document: ProjectDocument = serde_json::from_slice(bytes)?;
    if document.format_version != CURRENT_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: document.format_version,
            latest: CURRENT_FORMAT_VERSION,
        });
    }
    validate_instruction_timeline(&document.instruction_timeline)?;
    validate_numeric_expressions(&document.numeric_expressions)?;
    Ok(document)
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
        AssetId, Edge, EdgeId, EdgeKind, FaceId, InstructionHingeAngle, InstructionPose,
        InstructionPoseModel, InstructionStep, InstructionStepId, LengthDisplayUnit,
        PaperAppearance, Point2, RgbaColor, Vertex, VertexId,
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

    #[test]
    fn json_round_trip_preserves_ids_geometry_and_kinds() {
        let original = sample_document();
        let bytes = write_project_json(&original).expect("write project");
        let restored = read_project_json(&bytes).expect("read project");
        assert_eq!(restored, original);
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
    fn rejects_unknown_format_version() {
        let mut document = sample_document();
        document.format_version = CURRENT_FORMAT_VERSION + 1;
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
}
