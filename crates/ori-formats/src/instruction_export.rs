//! Bounded, deterministic exports of an authored instruction timeline.

mod font;
mod layout;
mod pdf;
mod svg_zip;

use ori_domain::{CreasePattern, InstructionTimeline, Paper};
use ori_instructions::build_instruction_diagram_plan_with_limits;
pub use ori_instructions::{InstructionDiagramError, InstructionDiagramLimits};
use ori_topology::TopologySnapshot;
use thiserror::Error;

pub const MAX_INSTRUCTION_EXPORT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_INSTRUCTION_EXPORT_PAGES: usize = 2_048;
pub const MAX_INSTRUCTION_EXPORT_GLYPHS: usize = 500_000;
pub const MAX_INSTRUCTION_EXPORT_PAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_INSTRUCTION_EXPORT_TITLE_CHARS: usize = 120;
pub const INSTRUCTION_EXPORT_PROFILE: &str = "instruction_export_v1";
pub const INSTRUCTION_PROJECTION_PROFILE: &str = "orthographic_isometric_v1";
const UNTITLED_INSTRUCTION_TITLE: &str = "無題";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstructionExportWarning {
    FixedAutomaticCamera,
    VisualEffectsOmitted,
    AuthoredGuidesOmitted,
    DiscreteStepEndpointsOnly,
}

impl InstructionExportWarning {
    #[must_use]
    pub const fn category(self) -> &'static str {
        match self {
            Self::FixedAutomaticCamera => "fixed_automatic_camera",
            Self::VisualEffectsOmitted => "visual_effects_omitted",
            Self::AuthoredGuidesOmitted => "authored_guides_omitted",
            Self::DiscreteStepEndpointsOnly => "discrete_step_endpoints_only",
        }
    }

    #[must_use]
    pub const fn message_ja(self) -> &'static str {
        match self {
            Self::FixedAutomaticCamera => {
                "固定自動カメラで生成され、現在のカメラや作家指定カメラは使用されません。"
            }
            Self::VisualEffectsOmitted => {
                "テクスチャ、照明、影、透明効果を省略し、単色の表裏色と白背景で描画します。"
            }
            Self::AuthoredGuidesOmitted => {
                "カメラ遷移、矢印、注目箇所、指先、つまみ、押さえ、手の移動、持ち替えは出力されません。"
            }
            Self::DiscreteStepEndpointsOnly => {
                "各手順は保存済みの終端姿勢のみを表し、手順間の連続動作は出力されません。"
            }
        }
    }
}

pub const INSTRUCTION_EXPORT_WARNINGS: [InstructionExportWarning; 4] = [
    InstructionExportWarning::FixedAutomaticCamera,
    InstructionExportWarning::VisualEffectsOmitted,
    InstructionExportWarning::AuthoredGuidesOmitted,
    InstructionExportWarning::DiscreteStepEndpointsOnly,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionExportFormat {
    Pdf17,
    SvgPageZip,
}

impl InstructionExportFormat {
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Pdf17 => "application/pdf",
            Self::SvgPageZip => "application/zip",
        }
    }

    #[must_use]
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Pdf17 => "pdf",
            Self::SvgPageZip => "zip",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionExportLimits {
    pub max_output_bytes: usize,
    pub max_pages: usize,
    pub max_glyphs: usize,
    pub max_page_bytes: usize,
    pub diagram: InstructionDiagramLimits,
}

impl Default for InstructionExportLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: MAX_INSTRUCTION_EXPORT_BYTES,
            max_pages: MAX_INSTRUCTION_EXPORT_PAGES,
            max_glyphs: MAX_INSTRUCTION_EXPORT_GLYPHS,
            max_page_bytes: MAX_INSTRUCTION_EXPORT_PAGE_BYTES,
            diagram: InstructionDiagramLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionExportArtifact {
    pub format: InstructionExportFormat,
    pub media_type: &'static str,
    pub file_extension: &'static str,
    pub profile: &'static str,
    pub projection_profile: &'static str,
    pub bytes: Vec<u8>,
    pub step_count: usize,
    pub page_count: usize,
    pub glyph_count: usize,
    pub projected_vertex_visits: usize,
    pub caution_count: usize,
    pub warnings: Vec<InstructionExportWarning>,
}

#[derive(Debug, Clone, PartialEq)]
struct CanonicalInstructionPlanV1 {
    profile: &'static str,
    projection_profile: &'static str,
    title: String,
    pages: Vec<layout::InstructionPage>,
    step_count: usize,
    glyph_count: usize,
    projected_vertex_visits: usize,
    warnings: Vec<InstructionExportWarning>,
}

impl CanonicalInstructionPlanV1 {
    fn validate(&self, font: &font::InstructionFont<'_>) -> Result<(), InstructionExportError> {
        if self.profile != INSTRUCTION_EXPORT_PROFILE
            || self.projection_profile != INSTRUCTION_PROJECTION_PROFILE
            || self.title.is_empty()
            || self.pages.is_empty()
            || self.step_count == 0
            || self.projected_vertex_visits == 0
            || self.warnings != INSTRUCTION_EXPORT_WARNINGS
        {
            return Err(InstructionExportError::StructureNotRepresentable);
        }

        let mut current_step = 0_usize;
        let mut current_continuation = 0_usize;
        let mut actual_glyph_count = 0_usize;
        for page in &self.pages {
            if page.continuation_number == 0 {
                current_step = current_step
                    .checked_add(1)
                    .ok_or(InstructionExportError::StructureNotRepresentable)?;
                current_continuation = 0;
                if page.step_number != current_step {
                    return Err(InstructionExportError::StructureNotRepresentable);
                }
            } else {
                current_continuation = current_continuation
                    .checked_add(1)
                    .ok_or(InstructionExportError::StructureNotRepresentable)?;
                if page.step_number != current_step
                    || page.continuation_number != current_continuation
                {
                    return Err(InstructionExportError::StructureNotRepresentable);
                }
            }
            if !page.has_white_page_background() {
                return Err(InstructionExportError::StructureNotRepresentable);
            }
            for text in &page.texts {
                text.validate()?;
                actual_glyph_count = actual_glyph_count
                    .checked_add(text.glyphs.len())
                    .ok_or(InstructionExportError::StructureNotRepresentable)?;
                for glyph in &text.glyphs {
                    if font.glyph_id(glyph.scalar)?.0 != glyph.glyph_id {
                        return Err(InstructionExportError::StructureNotRepresentable);
                    }
                }
            }
        }
        if current_step != self.step_count || actual_glyph_count != self.glyph_count {
            return Err(InstructionExportError::StructureNotRepresentable);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum InstructionExportError {
    #[error("the project title has {actual} characters; the limit is {maximum}")]
    TitleTooLong { actual: usize, maximum: usize },
    #[error("the project title contains an unsupported control character")]
    InvalidTitle,
    #[error(transparent)]
    Diagram(#[from] InstructionDiagramError),
    #[error("the bundled instruction font is invalid")]
    InvalidBundledFont,
    #[error("the bundled instruction font or license does not match the pinned asset digest")]
    FontAssetMismatch,
    #[error(
        "instruction text contains a glyph unavailable in the bundled font: U+{code_point:04X}"
    )]
    UnsupportedGlyph { code_point: u32 },
    #[error("instruction layout exceeds the configured page or glyph limit")]
    LayoutLimitExceeded,
    #[error("one instruction page exceeds the {maximum}-byte limit")]
    PageTooLarge { maximum: usize },
    #[error("the instruction export is {actual} bytes; the limit is {maximum} bytes")]
    OutputTooLarge { actual: usize, maximum: usize },
    #[error("the instruction export could not be represented")]
    StructureNotRepresentable,
    #[error("instruction image archive serialization failed: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("instruction image archive I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("instruction image manifest serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn export_instruction_document(
    format: InstructionExportFormat,
    title: &str,
    current_fold_model_fingerprint: &str,
    pattern: &CreasePattern,
    paper: &Paper,
    timeline: &InstructionTimeline,
    topology: &TopologySnapshot,
) -> Result<InstructionExportArtifact, InstructionExportError> {
    export_instruction_document_with_limits(
        format,
        title,
        current_fold_model_fingerprint,
        pattern,
        paper,
        timeline,
        topology,
        InstructionExportLimits::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn export_instruction_document_with_limits(
    format: InstructionExportFormat,
    title: &str,
    current_fold_model_fingerprint: &str,
    pattern: &CreasePattern,
    paper: &Paper,
    timeline: &InstructionTimeline,
    topology: &TopologySnapshot,
    limits: InstructionExportLimits,
) -> Result<InstructionExportArtifact, InstructionExportError> {
    validate_title(title)?;
    let title = canonical_instruction_title(title);
    let (plan, font) = build_canonical_instruction_plan(
        title,
        current_fold_model_fingerprint,
        pattern,
        paper,
        timeline,
        topology,
        limits,
    )?;
    let bytes = match format {
        InstructionExportFormat::Pdf17 => pdf::serialize_instruction_pdf(
            &plan,
            &font,
            limits.max_page_bytes,
            limits.max_output_bytes,
        )?,
        InstructionExportFormat::SvgPageZip => svg_zip::serialize_instruction_svg_zip(
            &plan,
            &font,
            limits.max_page_bytes,
            limits.max_output_bytes,
        )?,
    };
    if bytes.len() > limits.max_output_bytes {
        return Err(InstructionExportError::OutputTooLarge {
            actual: bytes.len(),
            maximum: limits.max_output_bytes,
        });
    }
    Ok(InstructionExportArtifact {
        format,
        media_type: format.media_type(),
        file_extension: format.file_extension(),
        profile: plan.profile,
        projection_profile: plan.projection_profile,
        bytes,
        step_count: plan.step_count,
        page_count: plan.pages.len(),
        glyph_count: plan.glyph_count,
        projected_vertex_visits: plan.projected_vertex_visits,
        caution_count: timeline
            .steps
            .iter()
            .filter(|step| !step.caution.is_empty())
            .count(),
        warnings: plan.warnings,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_canonical_instruction_plan(
    title: &str,
    current_fold_model_fingerprint: &str,
    pattern: &CreasePattern,
    paper: &Paper,
    timeline: &InstructionTimeline,
    topology: &TopologySnapshot,
    limits: InstructionExportLimits,
) -> Result<(CanonicalInstructionPlanV1, font::InstructionFont<'static>), InstructionExportError> {
    let diagram = build_instruction_diagram_plan_with_limits(
        current_fold_model_fingerprint,
        pattern,
        paper,
        timeline,
        topology,
        limits.diagram,
    )?;
    let font = font::InstructionFont::load()?;
    let layout = layout::layout_instruction_pages(title, timeline, &diagram, &font, limits)?;
    let plan = CanonicalInstructionPlanV1 {
        profile: INSTRUCTION_EXPORT_PROFILE,
        projection_profile: INSTRUCTION_PROJECTION_PROFILE,
        title: title.to_owned(),
        pages: layout.pages,
        step_count: timeline.steps.len(),
        glyph_count: layout.glyph_count,
        projected_vertex_visits: diagram.projected_vertex_visits,
        warnings: INSTRUCTION_EXPORT_WARNINGS.to_vec(),
    };
    plan.validate(&font)?;
    Ok((plan, font))
}

fn canonical_instruction_title(title: &str) -> &str {
    if title.trim().is_empty() {
        UNTITLED_INSTRUCTION_TITLE
    } else {
        title
    }
}

fn validate_title(title: &str) -> Result<(), InstructionExportError> {
    let actual = title.chars().count();
    if actual > MAX_INSTRUCTION_EXPORT_TITLE_CHARS {
        return Err(InstructionExportError::TitleTooLong {
            actual,
            maximum: MAX_INSTRUCTION_EXPORT_TITLE_CHARS,
        });
    }
    if title.chars().any(|character| {
        character.is_control() || character == '\u{2028}' || character == '\u{2029}'
    }) {
        return Err(InstructionExportError::InvalidTitle);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::Write as _,
        fs,
        io::{Cursor, Read},
    };

    use ori_domain::{
        Edge, EdgeId, EdgeKind, InstructionHingeAngle, InstructionPose, InstructionPoseModel,
        InstructionStep, InstructionStepId, Point2, ProjectId, Vertex, VertexId,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};
    use sha2::{Digest, Sha256};
    use zip::ZipArchive;

    use super::*;

    const FINGERPRINT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct Fixture {
        pattern: CreasePattern,
        paper: Paper,
        topology: TopologySnapshot,
        fold: EdgeId,
    }

    fn fixture_vertex_id(index: u64) -> VertexId {
        serde_json::from_str(&format!("\"00000000-0000-4000-8000-{index:012x}\""))
            .expect("fixed vertex id")
    }

    fn fixture_edge_id(index: u64) -> EdgeId {
        serde_json::from_str(&format!("\"00000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed edge id")
    }

    fn fixture_project_id() -> ProjectId {
        serde_json::from_str("\"00000000-0000-4000-a000-000000000001\"").expect("fixed project id")
    }

    fn fixture() -> Fixture {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(50.0, 0.0),
            Point2::new(100.0, 0.0),
            Point2::new(100.0, 100.0),
            Point2::new(50.0, 100.0),
            Point2::new(0.0, 100.0),
        ];
        let vertices = positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| Vertex {
                id: fixture_vertex_id(index as u64 + 1),
                position,
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: fixture_edge_id(index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let fold = Edge {
            id: fixture_edge_id(7),
            start: boundary[1],
            end: boundary[4],
            kind: EdgeKind::Mountain,
        };
        let fold_id = fold.id;
        edges.push(fold);
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: fixture_project_id(),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        Fixture {
            pattern,
            paper,
            topology: report.snapshot.expect("topology"),
            fold: fold_id,
        }
    }

    fn timeline(fixture: &Fixture, description: String) -> InstructionTimeline {
        InstructionTimeline {
            steps: vec![
                InstructionStep {
                    id: InstructionStepId::new(),
                    title: "中央を山折りする".to_owned(),
                    description,
                    caution: "角を正確に合わせてください。".to_owned(),
                    duration_ms: 1_250,
                    visual: Default::default(),
                    pose: InstructionPose {
                        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: FINGERPRINT.to_owned(),
                        fixed_face: Some(fixture.topology.faces[0].id),
                        hinge_angles: vec![InstructionHingeAngle {
                            edge: fixture.fold,
                            angle_degrees: 90.0,
                        }],
                    },
                },
                InstructionStep {
                    id: InstructionStepId::new(),
                    title: "折り目を開く".to_owned(),
                    description: "形を確認します。".to_owned(),
                    caution: String::new(),
                    duration_ms: 800,
                    visual: Default::default(),
                    pose: InstructionPose {
                        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: FINGERPRINT.to_owned(),
                        fixed_face: Some(fixture.topology.faces[0].id),
                        hinge_angles: vec![InstructionHingeAngle {
                            edge: fixture.fold,
                            angle_degrees: 0.0,
                        }],
                    },
                },
            ],
        }
    }

    fn canonical_plan_digest(plan: &CanonicalInstructionPlanV1) -> String {
        let mut canonical = String::new();
        write!(
            canonical,
            "{}|{}|{}:{}:{}:{}:{}|{}",
            plan.profile,
            plan.projection_profile,
            plan.title.len(),
            plan.title,
            plan.step_count,
            plan.pages.len(),
            plan.glyph_count,
            plan.projected_vertex_visits
        )
        .expect("plan header");
        for warning in &plan.warnings {
            write!(
                canonical,
                "|w{}:{}",
                warning.category(),
                warning.message_ja()
            )
            .expect("warning");
        }
        for page in &plan.pages {
            write!(
                canonical,
                "|p{}:{}:{}:{}:{}",
                page.step_number,
                page.continuation_number,
                page.polygons.len(),
                page.lines.len(),
                page.texts.len()
            )
            .expect("page");
            for polygon in &page.polygons {
                write!(
                    canonical,
                    "|g{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:016x}:{}",
                    polygon.fill.red,
                    polygon.fill.green,
                    polygon.fill.blue,
                    polygon.stroke.red,
                    polygon.stroke.green,
                    polygon.stroke.blue,
                    polygon.stroke_width.to_bits(),
                    polygon.points.len()
                )
                .expect("polygon");
                for point in &polygon.points {
                    write!(
                        canonical,
                        ":{:016x}{:016x}",
                        point.x.to_bits(),
                        point.y.to_bits()
                    )
                    .expect("polygon point");
                }
            }
            for line in &page.lines {
                let dash = match line.dash {
                    layout::PageLineDash::Solid => 's',
                    layout::PageLineDash::Dashed => 'd',
                    layout::PageLineDash::DashDot => 'm',
                };
                write!(
                    canonical,
                    "|l{dash}{:02x}{:02x}{:02x}{:016x}:{:016x}{:016x}{:016x}{:016x}",
                    line.color.red,
                    line.color.green,
                    line.color.blue,
                    line.width.to_bits(),
                    line.start.x.to_bits(),
                    line.start.y.to_bits(),
                    line.end.x.to_bits(),
                    line.end.y.to_bits()
                )
                .expect("line");
            }
            for text in &page.texts {
                write!(
                    canonical,
                    "|t{:02x}{:02x}{:02x}{:016x}{:016x}:{}",
                    text.color.red,
                    text.color.green,
                    text.color.blue,
                    text.baseline_y.to_bits(),
                    text.font_size.to_bits(),
                    text.glyphs.len()
                )
                .expect("text");
                for glyph in &text.glyphs {
                    write!(
                        canonical,
                        ":{:08x}{:04x}{:016x}{:016x}",
                        u32::from(glyph.scalar),
                        glyph.glyph_id,
                        glyph.x.to_bits(),
                        glyph.advance.to_bits()
                    )
                    .expect("glyph");
                }
            }
        }
        format!("{:x}", Sha256::digest(canonical.as_bytes()))
    }

    #[test]
    fn both_formats_are_deterministic_and_report_complete_metadata() {
        let fixture = fixture();
        let timeline = timeline(&fixture, "辺と辺を合わせます。".to_owned());
        let diagram = build_instruction_diagram_plan_with_limits(
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
            InstructionDiagramLimits::default(),
        )
        .expect("diagram");
        let font = font::InstructionFont::load().expect("font");
        let layout = layout::layout_instruction_pages(
            "鶴の試作",
            &timeline,
            &diagram,
            &font,
            InstructionExportLimits::default(),
        )
        .expect("layout");
        let first = &layout.pages[0];
        assert!(first.has_white_page_background());
        assert_eq!(first.polygons[1].fill, layout::PageColor::WHITE);
        let labels = first
            .texts
            .iter()
            .map(layout::PageText::scalar_text)
            .collect::<Vec<_>>();
        let positions = [
            "鶴の試作",
            "手順 1 / 2",
            "中央を山折りする",
            "所要時間: 1.25秒 / 今回動かす折り線: 1本 / 3D姿勢: 再計算済み（経路未証明）",
            "説明",
            "注意事項",
        ]
        .map(|label| {
            labels
                .iter()
                .position(|actual| actual == label)
                .expect("ordered page label")
        });
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        let diagram_bottom = first.polygons[1]
            .points
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let metadata_baseline = first.texts[positions[3]].baseline_y;
        assert!(metadata_baseline > diagram_bottom);

        for format in [
            InstructionExportFormat::Pdf17,
            InstructionExportFormat::SvgPageZip,
        ] {
            let first = export_instruction_document(
                format,
                "鶴の試作",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
            )
            .expect("export");
            let second = export_instruction_document(
                format,
                "鶴の試作",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
            )
            .expect("repeat export");
            assert_eq!(first, second);
            assert_eq!(first.step_count, 2);
            assert_eq!(first.caution_count, 1);
            assert_eq!(first.profile, INSTRUCTION_EXPORT_PROFILE);
            assert_eq!(first.projection_profile, INSTRUCTION_PROJECTION_PROFILE);
            assert_eq!(first.warnings, INSTRUCTION_EXPORT_WARNINGS);
            assert_eq!(first.projected_vertex_visits, 20);
            assert!(first.glyph_count > 0);
            assert!(first.page_count >= 2);
            assert!(!first.bytes.is_empty());
            if let Some(directory) = std::env::var_os("ORIGAMI2_INSTRUCTION_EXPORT_AUDIT_DIRECTORY")
            {
                fs::create_dir_all(&directory).expect("create audit directory");
                fs::write(
                    std::path::Path::new(&directory)
                        .join(format!("instruction-sample.{}", first.file_extension)),
                    &first.bytes,
                )
                .expect("write audit artifact");
            }
        }
    }

    #[test]
    fn complete_canonical_plan_has_a_cross_platform_golden_digest() {
        let fixture = fixture();
        let timeline = timeline(&fixture, "山折りして角を合わせます。\n".repeat(45));
        assert!(timeline.steps.iter().all(|step| {
            step.visual.camera.is_none()
                && step.visual.arrows.is_empty()
                && step.visual.focus_points.is_empty()
                && step.visual.hand_guides.is_empty()
        }));
        let (plan, font) = build_canonical_instruction_plan(
            "鶴の試作",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
            InstructionExportLimits::default(),
        )
        .expect("canonical instruction plan");
        plan.validate(&font).expect("valid canonical plan");
        assert!(plan.pages.len() > plan.step_count);
        assert_eq!(plan.warnings, INSTRUCTION_EXPORT_WARNINGS);
        assert_eq!(
            canonical_plan_digest(&plan),
            "f016ddaef0a4094e3713a97bfb9a3c26e6bcf7ab6949041d951baf6126dc76b8"
        );
    }

    #[test]
    fn page_glyph_title_and_output_limits_accept_the_boundary_and_reject_one_more() {
        let fixture = fixture();
        let timeline = timeline(&fixture, "境界値を確認します。".to_owned());
        let (plan, _) = build_canonical_instruction_plan(
            "境界",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
            InstructionExportLimits::default(),
        )
        .expect("measure canonical plan");
        let exact = InstructionExportLimits {
            max_pages: plan.pages.len(),
            max_glyphs: plan.glyph_count,
            ..InstructionExportLimits::default()
        };
        export_instruction_document_with_limits(
            InstructionExportFormat::Pdf17,
            "境界",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
            exact,
        )
        .expect("page and glyph counts equal to their limits");

        for one_too_many in [
            InstructionExportLimits {
                max_pages: exact.max_pages - 1,
                ..exact
            },
            InstructionExportLimits {
                max_glyphs: exact.max_glyphs - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                export_instruction_document_with_limits(
                    InstructionExportFormat::Pdf17,
                    "境界",
                    FINGERPRINT,
                    &fixture.pattern,
                    &fixture.paper,
                    &timeline,
                    &fixture.topology,
                    one_too_many,
                ),
                Err(InstructionExportError::LayoutLimitExceeded)
            ));
        }

        let exact_title = "題".repeat(MAX_INSTRUCTION_EXPORT_TITLE_CHARS);
        export_instruction_document(
            InstructionExportFormat::Pdf17,
            &exact_title,
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("title equal to its character limit");
        let one_too_many_title = "題".repeat(MAX_INSTRUCTION_EXPORT_TITLE_CHARS + 1);
        assert!(matches!(
            export_instruction_document(
                InstructionExportFormat::SvgPageZip,
                &one_too_many_title,
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
            ),
            Err(InstructionExportError::TitleTooLong {
                actual,
                maximum: MAX_INSTRUCTION_EXPORT_TITLE_CHARS,
            }) if actual == MAX_INSTRUCTION_EXPORT_TITLE_CHARS + 1
        ));

        for format in [
            InstructionExportFormat::Pdf17,
            InstructionExportFormat::SvgPageZip,
        ] {
            let complete = export_instruction_document(
                format,
                "境界",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
            )
            .expect("measure complete artifact");
            let exact_output = InstructionExportLimits {
                max_output_bytes: complete.bytes.len(),
                ..InstructionExportLimits::default()
            };
            export_instruction_document_with_limits(
                format,
                "境界",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
                exact_output,
            )
            .expect("output equal to its byte limit");
            assert!(matches!(
                export_instruction_document_with_limits(
                    format,
                    "境界",
                    FINGERPRINT,
                    &fixture.pattern,
                    &fixture.paper,
                    &timeline,
                    &fixture.topology,
                    InstructionExportLimits {
                        max_output_bytes: complete.bytes.len() - 1,
                        ..InstructionExportLimits::default()
                    },
                ),
                Err(InstructionExportError::OutputTooLarge { maximum, .. })
                    if maximum == complete.bytes.len() - 1
            ));
        }
    }

    #[test]
    fn long_text_continues_without_truncation_and_limits_reject_all_output() {
        let fixture = fixture();
        let timeline = timeline(&fixture, "山折りして角を合わせます。\n".repeat(140));
        let artifact = export_instruction_document(
            InstructionExportFormat::SvgPageZip,
            "長文",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("long export");
        assert!(artifact.page_count > artifact.step_count);
        let page_count = artifact.page_count;
        let mut archive = ZipArchive::new(Cursor::new(artifact.bytes)).expect("ZIP");
        let mut all_pages = String::new();
        for page_number in 1..=page_count {
            archive
                .by_name(&format!("pages/page-{page_number:04}.svg"))
                .expect("page")
                .read_to_string(&mut all_pages)
                .expect("read page");
        }
        assert!(all_pages.contains("aria-label=\"手順 1 / 2（続き 1）\""));
        assert!(all_pages.contains("aria-label=\"継続セクション: 説明\""));
        assert_eq!(
            all_pages
                .matches("aria-label=\"山折りして角を合わせます。\"")
                .count(),
            140
        );

        let mut limits = InstructionExportLimits {
            max_pages: 1,
            ..InstructionExportLimits::default()
        };
        assert!(matches!(
            export_instruction_document_with_limits(
                InstructionExportFormat::SvgPageZip,
                "長文",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
                limits,
            ),
            Err(InstructionExportError::LayoutLimitExceeded)
        ));
        limits.max_pages = MAX_INSTRUCTION_EXPORT_PAGES;
        limits.max_glyphs = 1;
        assert!(matches!(
            export_instruction_document_with_limits(
                InstructionExportFormat::Pdf17,
                "長文",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
                limits,
            ),
            Err(InstructionExportError::LayoutLimitExceeded)
        ));
    }

    #[test]
    fn stale_timeline_and_invalid_title_are_rejected_before_any_artifact() {
        let fixture = fixture();
        let timeline = timeline(&fixture, String::new());
        assert!(matches!(
            export_instruction_document(
                InstructionExportFormat::Pdf17,
                "不正\r題名",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
            ),
            Err(InstructionExportError::InvalidTitle)
        ));
        assert!(matches!(
            export_instruction_document(
                InstructionExportFormat::Pdf17,
                "旧工程",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
            ),
            Err(InstructionExportError::Diagram(
                InstructionDiagramError::StaleStep { step_index: 0 }
            ))
        ));
    }

    #[test]
    fn empty_title_is_canonicalized_once_for_pages_pdf_and_svg_metadata() {
        let fixture = fixture();
        let timeline = timeline(&fixture, "説明".to_owned());
        let pdf = export_instruction_document(
            InstructionExportFormat::Pdf17,
            "",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("PDF");
        let pdf_body = std::str::from_utf8(&pdf.bytes[15..]).expect("ASCII PDF body");
        assert!(pdf_body.contains("/Title <FEFF7121984C>"));

        let svg = export_instruction_document(
            InstructionExportFormat::SvgPageZip,
            "　",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
        )
        .expect("SVG ZIP");
        let mut archive = ZipArchive::new(Cursor::new(svg.bytes)).expect("ZIP");
        let mut manifest = String::new();
        archive
            .by_name("manifest.json")
            .expect("manifest")
            .read_to_string(&mut manifest)
            .expect("read manifest");
        let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("JSON");
        assert_eq!(manifest["title"], UNTITLED_INSTRUCTION_TITLE);
        let mut page = String::new();
        archive
            .by_name("pages/page-0001.svg")
            .expect("page")
            .read_to_string(&mut page)
            .expect("read page");
        assert!(page.contains("<title>無題 — 1 / 2</title>"));
        assert!(page.contains("aria-label=\"無題\""));
    }
}
