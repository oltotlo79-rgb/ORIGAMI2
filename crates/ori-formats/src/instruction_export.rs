//! Bounded, deterministic exports of an authored instruction timeline.

mod font;
mod layout;
mod pdf;
mod svg_zip;

use ori_domain::{CreasePattern, InstructionTimeline, Paper};
pub use ori_instructions::{InstructionDiagramError, InstructionDiagramLimits};
use ori_instructions::{
    build_instruction_diagram_plan_with_limits, instruction_pose_fingerprint_v1,
};
use ori_topology::TopologySnapshot;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const MAX_INSTRUCTION_EXPORT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_INSTRUCTION_EXPORT_PAGES: usize = 2_048;
pub const MAX_INSTRUCTION_EXPORT_GLYPHS: usize = 500_000;
pub const MAX_INSTRUCTION_EXPORT_PAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_INSTRUCTION_EXPORT_TITLE_CHARS: usize = 120;
pub const INSTRUCTION_EXPORT_PROFILE: &str = "instruction_export_v1";
pub const INSTRUCTION_PROJECTION_PROFILE: &str = "orthographic_isometric_v1";
const UNTITLED_INSTRUCTION_TITLE: &str = "無題";
const PATH_CERTIFICATE_REFERENCE_LABEL: &str = "経路証明 SHA-256: ";
const CERTIFIED_CONTINUOUS_PATH_CLAIM: &str = "認証済みの連続折り経路";
const SOURCE_MODEL_REFERENCE_LABEL: &str = " / 元モデル SHA-256: ";

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
    #[error("instruction step {step_index} contains a malformed path-certificate reference")]
    InvalidPathCertificateReference { step_index: usize },
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
    validate_path_certificate_references(timeline)?;
    let diagram = build_instruction_diagram_plan_with_limits(
        current_fold_model_fingerprint,
        pattern,
        paper,
        timeline,
        topology,
        limits.diagram,
    )?;
    let font = font::InstructionFont::load()?;
    let layout = layout::layout_instruction_pages(
        title,
        current_fold_model_fingerprint,
        timeline,
        &diagram,
        &font,
        limits,
    )?;
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

fn validate_path_certificate_references(
    timeline: &InstructionTimeline,
) -> Result<(), InstructionExportError> {
    for (step_index, step) in timeline.steps.iter().enumerate() {
        let claims_certified_path = [&step.title, &step.description, &step.caution]
            .iter()
            .any(|text| text.contains(CERTIFIED_CONTINUOUS_PATH_CLAIM));
        let mut described_binding = None;
        let mut reference_count = 0_usize;
        for text in [&step.title, &step.description, &step.caution] {
            let mut remainder = text.as_str();
            while let Some(offset) = remainder.find(PATH_CERTIFICATE_REFERENCE_LABEL) {
                reference_count += 1;
                let value = &remainder[offset + PATH_CERTIFICATE_REFERENCE_LABEL.len()..];
                if value.len() < 64
                    || !value.as_bytes()[..64]
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                {
                    return Err(InstructionExportError::InvalidPathCertificateReference {
                        step_index,
                    });
                }
                described_binding = decode_lower_hex_32(&value[..64]);
                let model_value = value[64..]
                    .strip_prefix(SOURCE_MODEL_REFERENCE_LABEL)
                    .ok_or(InstructionExportError::InvalidPathCertificateReference {
                        step_index,
                    })?;
                if model_value.len() < 64
                    || !model_value.as_bytes()[..64]
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                    || model_value[..64] != step.pose.source_model_fingerprint
                    || model_value
                        .as_bytes()
                        .get(64)
                        .is_some_and(|byte| !byte.is_ascii_whitespace())
                {
                    return Err(InstructionExportError::InvalidPathCertificateReference {
                        step_index,
                    });
                }
                remainder = &model_value[64..];
            }
        }
        match &step.visual.path_certificate_reference_v1 {
            None if reference_count == 0 && !claims_certified_path => {}
            Some(reference) if reference_count == 1 => {
                let fixed_face = step.pose.fixed_face.ok_or(
                    InstructionExportError::InvalidPathCertificateReference { step_index },
                )?;
                let previous = step_index
                    .checked_sub(1)
                    .and_then(|index| timeline.steps.get(index))
                    .filter(|previous| {
                        previous.pose.model
                            == ori_domain::InstructionPoseModel::AbsoluteHingeAnglesV1
                            && previous.pose.source_model_fingerprint
                                == step.pose.source_model_fingerprint
                    })
                    .ok_or(InstructionExportError::InvalidPathCertificateReference {
                        step_index,
                    })?;
                let mut model_hash = Sha256::new();
                model_hash.update(b"path_certificate_source_model_binding_v1");
                model_hash.update(step.pose.source_model_fingerprint.as_bytes());
                if described_binding != Some(reference.binding_sha256)
                    || reference.source_model_binding_sha256
                        != <[u8; 32]>::from(model_hash.finalize())
                    || reference.source_pose_sha256
                        != instruction_pose_fingerprint_v1(
                            &step.pose.source_model_fingerprint,
                            fixed_face,
                            &previous.pose.hinge_angles,
                        )
                    || reference.target_pose_sha256
                        != instruction_pose_fingerprint_v1(
                            &step.pose.source_model_fingerprint,
                            fixed_face,
                            &step.pose.hinge_angles,
                        )
                {
                    return Err(InstructionExportError::InvalidPathCertificateReference {
                        step_index,
                    });
                }
            }
            _ => {
                return Err(InstructionExportError::InvalidPathCertificateReference { step_index });
            }
        }
    }
    Ok(())
}

fn decode_lower_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        decoded[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Some(decoded)
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
            FINGERPRINT,
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
            "cca573674692aa7f9017d943376186c3bc89c321033ee8fc64b5f9ce2dac0f84"
        );
    }

    #[test]
    fn certified_path_reference_survives_step_layout_and_both_final_formats() {
        let fixture = fixture();
        let certificate_reference = "7c".repeat(32);
        let mut timeline = timeline(&fixture, "開始姿勢です。".to_owned());
        timeline.steps[1].description = format!(
            "衝突・閉包証明に結合された区間です。経路証明 SHA-256: {certificate_reference} / 元モデル SHA-256: {FINGERPRINT}"
        );
        let fixed_face = timeline.steps[1].pose.fixed_face.expect("fixed face");
        let mut model_hash = Sha256::new();
        model_hash.update(b"path_certificate_source_model_binding_v1");
        model_hash.update(FINGERPRINT.as_bytes());
        timeline.steps[1].visual.path_certificate_reference_v1 =
            Some(ori_domain::PathCertificateReferenceV1 {
                version: 1,
                model_id: ori_domain::PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1.to_owned(),
                binding_sha256: [0x7c; 32],
                source_pose_sha256: instruction_pose_fingerprint_v1(
                    FINGERPRINT,
                    fixed_face,
                    &timeline.steps[0].pose.hinge_angles,
                ),
                target_pose_sha256: instruction_pose_fingerprint_v1(
                    FINGERPRINT,
                    fixed_face,
                    &timeline.steps[1].pose.hinge_angles,
                ),
                source_model_binding_sha256: model_hash.finalize().into(),
                transition_count: 1,
            });
        let (plan, font) = build_canonical_instruction_plan(
            "証明付き手順",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &timeline,
            &fixture.topology,
            InstructionExportLimits::default(),
        )
        .expect("proof-bearing canonical plan");
        plan.validate(&font).expect("valid proof-bearing plan");
        let proof_step_text = plan
            .pages
            .iter()
            .filter(|page| page.step_number == 2)
            .flat_map(|page| page.texts.iter())
            .map(layout::PageText::scalar_text)
            .collect::<String>();
        assert!(proof_step_text.contains(&certificate_reference));
        let reference = timeline.steps[1]
            .visual
            .path_certificate_reference_v1
            .as_ref()
            .expect("structured proof reference");
        let short = |hash: &[u8; 32]| {
            hash[..6]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        assert!(proof_step_text.contains("v1 / transitions=1"));
        assert!(proof_step_text.contains(&format!("cert={}", short(&reference.binding_sha256))));
        assert!(
            proof_step_text.contains(&format!("source={}", short(&reference.source_pose_sha256)))
        );
        assert!(
            proof_step_text.contains(&format!("target={}", short(&reference.target_pose_sha256)))
        );
        assert_eq!(
            canonical_plan_digest(&plan),
            "a2af684997f5f35baf9270f7faf8d03a858860b29a0d3d7a294643886a4871b7"
        );
        let mut glyph_limited = InstructionExportLimits::default();
        glyph_limited.max_glyphs = plan.glyph_count - 1;
        assert!(matches!(
            build_canonical_instruction_plan(
                "証明付き手順",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &timeline,
                &fixture.topology,
                glyph_limited,
            ),
            Err(InstructionExportError::LayoutLimitExceeded)
        ));
        let archived = serde_json::to_vec(&timeline).expect("archive proof-bearing timeline");
        let reopened: InstructionTimeline =
            serde_json::from_slice(&archived).expect("reopen proof-bearing timeline");
        assert_eq!(reopened, timeline);
        let (reopened_plan, reopened_font) = build_canonical_instruction_plan(
            "証明付き手順",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &reopened,
            &fixture.topology,
            InstructionExportLimits::default(),
        )
        .expect("reopened proof-bearing canonical plan");
        reopened_plan
            .validate(&reopened_font)
            .expect("valid reopened proof-bearing plan");
        assert_eq!(
            canonical_plan_digest(&reopened_plan),
            canonical_plan_digest(&plan)
        );
        let reopened_proof_step_text = reopened_plan
            .pages
            .iter()
            .filter(|page| page.step_number == 2)
            .flat_map(|page| page.texts.iter())
            .map(layout::PageText::scalar_text)
            .collect::<String>();
        assert_eq!(reopened_proof_step_text, proof_step_text);

        let mut multiple = reopened.clone();
        let mut third = multiple.steps[1].clone();
        third.id = InstructionStepId::new();
        third.title = "証明付きの追加区間".to_owned();
        third.description = format!(
            "経路証明 SHA-256: {} / 元モデル SHA-256: {FINGERPRINT}",
            "6d".repeat(32)
        );
        third.pose.hinge_angles[0].angle_degrees = 45.0;
        let fixed_face = third.pose.fixed_face.expect("fixed face");
        let mut model_hash = Sha256::new();
        model_hash.update(b"path_certificate_source_model_binding_v1");
        model_hash.update(FINGERPRINT.as_bytes());
        third.visual.path_certificate_reference_v1 = Some(ori_domain::PathCertificateReferenceV1 {
            version: 1,
            model_id: ori_domain::PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1.to_owned(),
            binding_sha256: [0x6d; 32],
            source_pose_sha256: instruction_pose_fingerprint_v1(
                FINGERPRINT,
                fixed_face,
                &multiple.steps[1].pose.hinge_angles,
            ),
            target_pose_sha256: instruction_pose_fingerprint_v1(
                FINGERPRINT,
                fixed_face,
                &third.pose.hinge_angles,
            ),
            source_model_binding_sha256: model_hash.finalize().into(),
            transition_count: 2,
        });
        multiple.steps.push(third);
        let (multiple_plan, _) = build_canonical_instruction_plan(
            "複数証明手順",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &multiple,
            &fixture.topology,
            InstructionExportLimits::default(),
        )
        .expect("multiple proof layout");
        let multiple_text = multiple_plan
            .pages
            .iter()
            .flat_map(|page| page.texts.iter())
            .map(layout::PageText::scalar_text)
            .collect::<String>();
        assert_eq!(multiple_text.matches("構造化経路証明").count(), 2);
        assert!(multiple_text.contains("v1 / transitions=2 / cert=6d6d6d6d6d6d"));
        let exact_multiple_limits = InstructionExportLimits {
            max_pages: multiple_plan.pages.len(),
            max_glyphs: multiple_plan.glyph_count,
            ..InstructionExportLimits::default()
        };
        build_canonical_instruction_plan(
            "複数証明手順",
            FINGERPRINT,
            &fixture.pattern,
            &fixture.paper,
            &multiple,
            &fixture.topology,
            exact_multiple_limits,
        )
        .expect("exact multiple proof limits");
        assert!(matches!(
            build_canonical_instruction_plan(
                "複数証明手順",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &multiple,
                &fixture.topology,
                InstructionExportLimits {
                    max_pages: multiple_plan.pages.len() - 1,
                    ..exact_multiple_limits
                },
            ),
            Err(InstructionExportError::LayoutLimitExceeded)
        ));

        for format in [
            InstructionExportFormat::Pdf17,
            InstructionExportFormat::SvgPageZip,
        ] {
            let artifact = export_instruction_document(
                format,
                "証明付き手順",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &reopened,
                &fixture.topology,
            )
            .expect("proof-bearing export");
            assert_eq!(artifact.step_count, reopened.steps.len());
            assert_eq!(artifact.page_count, plan.pages.len());
        }

        let mut malformed_reference = reopened.clone();
        malformed_reference.steps[1].description = format!(
            "経路証明 SHA-256: {}G",
            &certificate_reference[..certificate_reference.len() - 1]
        );
        assert!(matches!(
            export_instruction_document(
                InstructionExportFormat::Pdf17,
                "証明付き手順",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &malformed_reference,
                &fixture.topology,
            ),
            Err(InstructionExportError::InvalidPathCertificateReference { step_index: 1 })
        ));

        let mut mismatched_model_reference = reopened.clone();
        mismatched_model_reference.steps[1].description = format!(
            "経路証明 SHA-256: {certificate_reference} / 元モデル SHA-256: {}",
            "b".repeat(64)
        );
        assert!(matches!(
            export_instruction_document(
                InstructionExportFormat::Pdf17,
                "証明付き手順",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &mismatched_model_reference,
                &fixture.topology,
            ),
            Err(InstructionExportError::InvalidPathCertificateReference { step_index: 1 })
        ));

        let mut foreign_model = reopened;
        foreign_model.steps[1].pose.source_model_fingerprint = "b".repeat(64);
        foreign_model.steps[1].description = format!(
            "経路証明 SHA-256: {certificate_reference} / 元モデル SHA-256: {}",
            "b".repeat(64)
        );
        assert!(matches!(
            export_instruction_document(
                InstructionExportFormat::SvgPageZip,
                "証明付き手順",
                FINGERPRINT,
                &fixture.pattern,
                &fixture.paper,
                &foreign_model,
                &fixture.topology,
            ),
            Err(InstructionExportError::InvalidPathCertificateReference { step_index: 1 })
        ));

        let mut tampered_endpoint = timeline;
        tampered_endpoint.steps[1]
            .visual
            .path_certificate_reference_v1
            .as_mut()
            .expect("structured proof reference")
            .target_pose_sha256[0] ^= 1;
        for format in [
            InstructionExportFormat::Pdf17,
            InstructionExportFormat::SvgPageZip,
        ] {
            assert!(matches!(
                export_instruction_document(
                    format,
                    "証明付き手順",
                    FINGERPRINT,
                    &fixture.pattern,
                    &fixture.paper,
                    &tampered_endpoint,
                    &fixture.topology,
                ),
                Err(InstructionExportError::InvalidPathCertificateReference { step_index: 1 })
            ));
        }

        let mut unproven_named_technique = tampered_endpoint.clone();
        unproven_named_technique.steps[1].description =
            "認証済みの連続折り経路で名前付き技法を適用します。".to_owned();
        unproven_named_technique.steps[1]
            .visual
            .path_certificate_reference_v1 = None;
        for format in [
            InstructionExportFormat::Pdf17,
            InstructionExportFormat::SvgPageZip,
        ] {
            assert!(matches!(
                export_instruction_document(
                    format,
                    "未証明の名前付き技法",
                    FINGERPRINT,
                    &fixture.pattern,
                    &fixture.paper,
                    &unproven_named_technique,
                    &fixture.topology,
                ),
                Err(InstructionExportError::InvalidPathCertificateReference { step_index: 1 })
            ));
        }
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
