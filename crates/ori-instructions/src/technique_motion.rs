//! Narrow, fail-closed execution boundary for a named straight-line book fold.
//!
//! Technique files remain inert. This module only compiles the one physical
//! operation whose endpoints have already been joined by a native collision-
//! and closure-certified path. The returned timeline is still preview data;
//! it never grants project-mutation authority.

use ori_collision::CertifiedPoseGraphPathCertificateV1;
use ori_domain::{
    EdgeId, FaceId, InstructionHingeAngle, InstructionPose, InstructionPoseModel, InstructionStep,
    InstructionStepId, InstructionTimeline, InstructionVisual, MIN_INSTRUCTION_DURATION_MS,
    PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1, PathCertificateReferenceV1,
    validate_instruction_timeline,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FoldTechniqueActionV1, FoldTechniqueCapabilityV1, FoldTechniqueExecutionSupportV1,
    FoldTechniqueFileV1, FoldTechniqueParameterTypeV1,
};

const TARGET_ANGLE_ROLE: &str = "target_angle";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalTechniqueCompilerV1 {
    BookFold,
    InsideReverseFold,
    OutsideReverseFold,
    SinkFold,
    LayerSelective,
}

#[must_use]
pub const fn physical_technique_compiler_v1(
    action: &FoldTechniqueActionV1,
) -> Option<PhysicalTechniqueCompilerV1> {
    match action {
        FoldTechniqueActionV1::InstructionCue { .. } => None,
        FoldTechniqueActionV1::StraightLineStackedFold => {
            Some(PhysicalTechniqueCompilerV1::BookFold)
        }
        FoldTechniqueActionV1::InsideReverseFold => {
            Some(PhysicalTechniqueCompilerV1::InsideReverseFold)
        }
        FoldTechniqueActionV1::OutsideReverseFold => {
            Some(PhysicalTechniqueCompilerV1::OutsideReverseFold)
        }
        FoldTechniqueActionV1::SinkFold { .. } => Some(PhysicalTechniqueCompilerV1::SinkFold),
        FoldTechniqueActionV1::LayerSelectiveManipulation { .. } => {
            Some(PhysicalTechniqueCompilerV1::LayerSelective)
        }
    }
}

/// Host-owned inputs for compiling one certified book fold.
pub struct BookFoldMotionRequestV1<'a> {
    pub technique_file: &'a FoldTechniqueFileV1,
    pub technique_id: &'a str,
    pub source_model_fingerprint: &'a str,
    pub fixed_face: FaceId,
    pub fold_edge: EdgeId,
    pub source_hinge_angles: &'a [InstructionHingeAngle],
    pub target_angle_microdegrees: i64,
    pub path_certificate: &'a CertifiedPoseGraphPathCertificateV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicFoldKindV1 {
    Mountain,
    Valley,
}

/// A named mountain/valley fold backed by the same certified straight-line
/// primitive as a book fold. The kind is explicit and never inferred from a
/// translated title.
pub struct BasicFoldMotionRequestV1<'a> {
    pub kind: BasicFoldKindV1,
    pub straight_fold: BookFoldMotionRequestV1<'a>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BookFoldMotionError {
    #[error("the requested named technique is missing or is not the supported book fold")]
    UnsupportedTechnique,
    #[error("the target angle does not satisfy the technique's typed parameter boundary")]
    InvalidTargetAngle,
    #[error("the source model or hinge vector is stale or invalid")]
    InvalidSourcePose,
    #[error("the native path certificate is absent, empty, or bound to different endpoints")]
    PathCertificateMismatch,
    #[error("the compiled instruction timeline failed validation")]
    InvalidTimeline,
}

pub fn compile_certified_basic_fold_timeline_v1(
    request: BasicFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, BookFoldMotionError> {
    let technique = request
        .straight_fold
        .technique_file
        .document()
        .techniques
        .iter()
        .find(|technique| technique.id == request.straight_fold.technique_id)
        .ok_or(BookFoldMotionError::UnsupportedTechnique)?;
    let expected_name = match request.kind {
        BasicFoldKindV1::Mountain => ["山折り", "Mountain fold"],
        BasicFoldKindV1::Valley => ["谷折り", "Valley fold"],
    };
    if !technique
        .names
        .iter()
        .any(|name| expected_name.contains(&name.text.as_str()))
    {
        return Err(BookFoldMotionError::UnsupportedTechnique);
    }
    compile_certified_book_fold_timeline_v1(request.straight_fold)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReverseFoldKindV1 {
    Inside,
    Outside,
}

pub struct ReverseFoldMotionRequestV1<'a> {
    pub technique_file: &'a FoldTechniqueFileV1,
    pub technique_id: &'a str,
    pub kind: ReverseFoldKindV1,
    pub source_model_fingerprint: &'a str,
    pub fixed_face: FaceId,
    pub first_edge: EdgeId,
    pub second_edge: EdgeId,
    pub source_hinge_angles: &'a [InstructionHingeAngle],
    pub intermediate_angle_microdegrees: i64,
    pub target_angle_microdegrees: i64,
    pub first_path_certificate: &'a CertifiedPoseGraphPathCertificateV1,
    pub second_path_certificate: &'a CertifiedPoseGraphPathCertificateV1,
}

pub struct SinkFoldMotionRequestV1<'a> {
    pub technique_file: &'a FoldTechniqueFileV1,
    pub technique_id: &'a str,
    pub source_model_fingerprint: &'a str,
    pub fixed_face: FaceId,
    pub first_edge: EdgeId,
    pub second_edge: EdgeId,
    pub source_hinge_angles: &'a [InstructionHingeAngle],
    pub intermediate_angle_microdegrees: i64,
    pub target_angle_microdegrees: i64,
    pub first_path_certificate: &'a CertifiedPoseGraphPathCertificateV1,
    pub second_path_certificate: &'a CertifiedPoseGraphPathCertificateV1,
}

pub type LayerSelectiveMotionRequestV1<'a> = SinkFoldMotionRequestV1<'a>;
/// A squash fold authored as the validated V1 open/closed-sink primitive.
/// Instruction cues alone never enter this physical compiler.
pub type SquashFoldMotionRequestV1<'a> = SinkFoldMotionRequestV1<'a>;
/// A crimp fold represented by exactly two ordered straight-line operations.
pub type CrimpFoldMotionRequestV1<'a> = SinkFoldMotionRequestV1<'a>;
/// Reserved request shape for a future certified petal-fold compiler.
pub type PetalFoldMotionRequestV1<'a> = SinkFoldMotionRequestV1<'a>;

/// Proof inputs for the deliberately narrow regular-quad petal primitive.
/// The native transaction boundary proves the regular four-edge flap and
/// continuous layer authority before constructing this request; this compiler
/// still revalidates each of the three ordered path-certificate endpoints.
pub type RegularQuadPetalFoldMotionRequestV1<'a> = AccordionFoldMotionRequestV1<'a>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetalFoldMissingPremiseV1 {
    ThreeSegmentGraphChain,
    LiftedFlapAuthority,
    AdjacentFaceOpeningAuthority,
    FinalFlatteningAuthority,
    ContinuousLayerAuthority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PetalFoldCertificationAuditV1 {
    pub supported: bool,
    pub minimum_graph_segments: u8,
    pub missing_premises: &'static [PetalFoldMissingPremiseV1],
}

const PETAL_FOLD_MISSING_PREMISES_V1: &[PetalFoldMissingPremiseV1] = &[
    PetalFoldMissingPremiseV1::ThreeSegmentGraphChain,
    PetalFoldMissingPremiseV1::LiftedFlapAuthority,
    PetalFoldMissingPremiseV1::AdjacentFaceOpeningAuthority,
    PetalFoldMissingPremiseV1::FinalFlatteningAuthority,
    PetalFoldMissingPremiseV1::ContinuousLayerAuthority,
];

/// The V1 graph certificate binds one or two endpoint paths, but carries no
/// single-vertex flap/open/flatten topology or continuous layer authority.
pub const fn audit_certified_petal_fold_v1() -> PetalFoldCertificationAuditV1 {
    PetalFoldCertificationAuditV1 {
        supported: false,
        minimum_graph_segments: 3,
        missing_premises: PETAL_FOLD_MISSING_PREMISES_V1,
    }
}

/// Compiles the proof-carrying sink primitive used by a named squash fold.
/// Missing capabilities or either missing path segment remain fail-closed.
pub fn compile_certified_squash_fold_timeline_v1(
    request: SquashFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    compile_certified_sink_fold_timeline_v1(request)
}

/// Compiles a crimp only when the technique declares exactly two validated
/// straight-line fold primitives and both native path segments bind exactly.
pub fn compile_certified_crimp_fold_timeline_v1(
    request: CrimpFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    let technique = request
        .technique_file
        .document()
        .techniques
        .iter()
        .find(|technique| technique.id == request.technique_id)
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    let physical = technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                FoldTechniqueActionV1::StraightLineStackedFold
            )
        })
        .collect::<Vec<_>>();
    if physical.len() != 2
        || physical.iter().any(|operation| {
            !operation
                .required_capabilities
                .contains(&FoldTechniqueCapabilityV1::StraightLineStackedFoldV1)
                || operation.execution_support != FoldTechniqueExecutionSupportV1::DeclarativeOnly
        })
    {
        return Err(ReverseFoldMotionError::UnsupportedTechnique);
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.as_str())
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    compile_two_segment_motion(
        title,
        request.source_model_fingerprint,
        request.fixed_face,
        request.first_edge,
        request.second_edge,
        request.source_hinge_angles,
        request.intermediate_angle_microdegrees,
        request.target_angle_microdegrees,
        request.first_path_certificate,
        request.second_path_certificate,
    )
}

/// Petal folding needs a lifted flap, adjacent-face opening, and final
/// flattening relation that V1 primitives do not jointly prove. Never
/// reinterpret a reverse/sink certificate as petal-fold authority.
pub const fn compile_certified_petal_fold_timeline_v1(
    _request: PetalFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    let _audit = audit_certified_petal_fold_v1();
    Err(ReverseFoldMotionError::UnsupportedTechnique)
}

/// Compiles only the three-stage path portion of a native-authenticated,
/// regular-quad petal fold. General petal requests continue to use
/// [`compile_certified_petal_fold_timeline_v1`] and remain unsupported.
pub fn compile_certified_regular_quad_petal_fold_timeline_v1(
    request: RegularQuadPetalFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, AccordionFoldMotionError> {
    if request.ordered_edges.len() != 3
        || request.ordered_target_angles_microdegrees.len() != 3
        || request.ordered_path_certificates.len() != 3
    {
        return Err(AccordionFoldMotionError::InvalidSegments);
    }
    compile_certified_accordion_fold_timeline_v1(request)
}

pub fn compile_certified_layer_selective_timeline_v1(
    request: LayerSelectiveMotionRequestV1<'_>,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    let technique = request
        .technique_file
        .document()
        .techniques
        .iter()
        .find(|technique| technique.id == request.technique_id)
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    let operation = technique
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.action,
                FoldTechniqueActionV1::LayerSelectiveManipulation { .. }
            )
        })
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    if !operation
        .required_capabilities
        .contains(&FoldTechniqueCapabilityV1::LayerSelectiveMotionV1)
    {
        return Err(ReverseFoldMotionError::UnsupportedTechnique);
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.as_str())
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    compile_two_segment_motion(
        title,
        request.source_model_fingerprint,
        request.fixed_face,
        request.first_edge,
        request.second_edge,
        request.source_hinge_angles,
        request.intermediate_angle_microdegrees,
        request.target_angle_microdegrees,
        request.first_path_certificate,
        request.second_path_certificate,
    )
}

/// Compiles the closest validated V1 equivalent to a squash/petal motion: an
/// open or closed sink fold. Both native segments remain exact proof premises.
pub fn compile_certified_sink_fold_timeline_v1(
    request: SinkFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    let technique = request
        .technique_file
        .document()
        .techniques
        .iter()
        .find(|technique| technique.id == request.technique_id)
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    let operation = technique
        .operations
        .iter()
        .find(|operation| matches!(operation.action, FoldTechniqueActionV1::SinkFold { .. }))
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    if !operation
        .required_capabilities
        .contains(&FoldTechniqueCapabilityV1::SinkFoldMotionV1)
    {
        return Err(ReverseFoldMotionError::UnsupportedTechnique);
    }
    compile_two_segment_motion(
        technique
            .names
            .iter()
            .find(|text| text.locale == "ja")
            .or_else(|| technique.names.first())
            .map(|text| text.text.as_str())
            .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?,
        request.source_model_fingerprint,
        request.fixed_face,
        request.first_edge,
        request.second_edge,
        request.source_hinge_angles,
        request.intermediate_angle_microdegrees,
        request.target_angle_microdegrees,
        request.first_path_certificate,
        request.second_path_certificate,
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_two_segment_motion(
    title: &str,
    model: &str,
    fixed_face: FaceId,
    first_edge: EdgeId,
    second_edge: EdgeId,
    source_angles: &[InstructionHingeAngle],
    intermediate_angle: i64,
    target_angle: i64,
    first: &CertifiedPoseGraphPathCertificateV1,
    second: &CertifiedPoseGraphPathCertificateV1,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    if first_edge == second_edge
        || !(0..=180_000_000).contains(&intermediate_angle)
        || !(0..=180_000_000).contains(&target_angle)
    {
        return Err(ReverseFoldMotionError::InvalidAngle);
    }
    let mut source = source_angles.to_vec();
    source.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    if source.windows(2).any(|pair| pair[0].edge == pair[1].edge) || model.len() != 64 {
        return Err(ReverseFoldMotionError::InvalidSourcePose);
    }
    let mut middle = source.clone();
    set_hinge_angle(&mut middle, first_edge, intermediate_angle);
    let mut target = middle.clone();
    set_hinge_angle(&mut target, second_edge, target_angle);
    let hash = |angles: &[InstructionHingeAngle]| {
        instruction_pose_fingerprint_v1(model, fixed_face, angles)
    };
    if first.edges().is_empty()
        || second.edges().is_empty()
        || first.source() != hash(&source)
        || first.target() != hash(&middle)
        || second.source() != hash(&middle)
        || second.target() != hash(&target)
    {
        return Err(ReverseFoldMotionError::PathSegmentMismatch);
    }
    let certificate_references = [
        path_certificate_reference_v1(first, model),
        path_certificate_reference_v1(second, model),
    ];
    let certificate_visuals = [
        path_certificate_visual_v1(first, model),
        path_certificate_visual_v1(second, model),
    ];
    let steps = [("開始", source), ("沈め", middle), ("完了", target)]
        .into_iter()
        .enumerate()
        .map(|(index, (suffix, hinge_angles))| InstructionStep {
            id: InstructionStepId::new(),
            title: format!("{title}：{suffix}"),
            description: if index == 0 {
                "認証済み二段階技法の開始姿勢です。".to_owned()
            } else {
                format!(
                    "衝突・層順序証明に結合された区間{index}の終端です。経路証明 SHA-256: {}",
                    certificate_references[index - 1]
                )
            },
            caution: String::new(),
            duration_ms: 1_000,
            visual: if index == 0 {
                InstructionVisual::default()
            } else {
                certificate_visuals[index - 1].clone()
            },
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: model.to_owned(),
                fixed_face: Some(fixed_face),
                hinge_angles,
            },
        })
        .collect();
    let timeline = InstructionTimeline { steps };
    validate_instruction_timeline(&timeline)
        .map_err(|_| ReverseFoldMotionError::InvalidTimeline)?;
    Ok(timeline)
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReverseFoldMotionError {
    #[error("the requested named technique is not the selected reverse-fold kind")]
    UnsupportedTechnique,
    #[error("a reverse-fold angle is outside the native instruction boundary")]
    InvalidAngle,
    #[error("the source pose is invalid")]
    InvalidSourcePose,
    #[error("a path segment is empty, discontinuous, stale, or endpoint-mismatched")]
    PathSegmentMismatch,
    #[error("the compiled reverse-fold timeline failed validation")]
    InvalidTimeline,
}

pub struct AccordionFoldMotionRequestV1<'a> {
    pub technique_file: &'a FoldTechniqueFileV1,
    pub technique_id: &'a str,
    pub source_model_fingerprint: &'a str,
    pub fixed_face: FaceId,
    pub source_hinge_angles: &'a [InstructionHingeAngle],
    pub ordered_edges: &'a [EdgeId],
    pub ordered_target_angles_microdegrees: &'a [i64],
    pub ordered_path_certificates: &'a [CertifiedPoseGraphPathCertificateV1],
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AccordionFoldMotionError {
    #[error("an accordion fold requires at least three ordered straight-fold operations")]
    UnsupportedTechnique,
    #[error(
        "the ordered accordion segment inputs are invalid or exceed the bounded operation count"
    )]
    InvalidSegments,
    #[error("the source pose is invalid")]
    InvalidSourcePose,
    #[error("an accordion path segment is empty, discontinuous, or endpoint-mismatched")]
    PathSegmentMismatch,
    #[error("the compiled accordion timeline failed validation")]
    InvalidTimeline,
}

/// Compiles a pleat/accordion as three or more ordered certified segments.
/// Each certificate is checked against the exact previous and next pose, so
/// segment reordering or an ABA replacement cannot preserve authority.
pub fn compile_certified_accordion_fold_timeline_v1(
    request: AccordionFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, AccordionFoldMotionError> {
    let technique = request
        .technique_file
        .document()
        .techniques
        .iter()
        .find(|technique| technique.id == request.technique_id)
        .ok_or(AccordionFoldMotionError::UnsupportedTechnique)?;
    let straight_operations = technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                FoldTechniqueActionV1::StraightLineStackedFold
            ) && operation
                .required_capabilities
                .contains(&FoldTechniqueCapabilityV1::StraightLineStackedFoldV1)
        })
        .count();
    let count = request.ordered_edges.len();
    if straight_operations < 3 || straight_operations != count {
        return Err(AccordionFoldMotionError::UnsupportedTechnique);
    }
    if count > 31
        || request.ordered_target_angles_microdegrees.len() != count
        || request.ordered_path_certificates.len() != count
        || request
            .ordered_edges
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != count
        || request
            .ordered_target_angles_microdegrees
            .iter()
            .any(|angle| !(0..=180_000_000).contains(angle))
    {
        return Err(AccordionFoldMotionError::InvalidSegments);
    }
    let mut pose_angles = request.source_hinge_angles.to_vec();
    pose_angles.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    if pose_angles
        .windows(2)
        .any(|pair| pair[0].edge == pair[1].edge)
        || !pose_angles
            .iter()
            .all(|hinge| hinge.angle_degrees.is_finite())
        || request.source_model_fingerprint.len() != 64
    {
        return Err(AccordionFoldMotionError::InvalidSourcePose);
    }
    let mut poses = vec![pose_angles.clone()];
    for ((edge, angle), certificate) in request
        .ordered_edges
        .iter()
        .zip(request.ordered_target_angles_microdegrees)
        .zip(request.ordered_path_certificates)
    {
        let source_hash = instruction_pose_fingerprint_v1(
            request.source_model_fingerprint,
            request.fixed_face,
            &pose_angles,
        );
        set_hinge_angle(&mut pose_angles, *edge, *angle);
        let target_hash = instruction_pose_fingerprint_v1(
            request.source_model_fingerprint,
            request.fixed_face,
            &pose_angles,
        );
        if certificate.edges().is_empty()
            || certificate.source() != source_hash
            || certificate.target() != target_hash
        {
            return Err(AccordionFoldMotionError::PathSegmentMismatch);
        }
        poses.push(pose_angles.clone());
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.clone())
        .ok_or(AccordionFoldMotionError::UnsupportedTechnique)?;
    let certificate_references = request
        .ordered_path_certificates
        .iter()
        .map(|certificate| {
            path_certificate_reference_v1(certificate, request.source_model_fingerprint)
        })
        .collect::<Vec<_>>();
    let certificate_visuals = request
        .ordered_path_certificates
        .iter()
        .map(|certificate| {
            path_certificate_visual_v1(certificate, request.source_model_fingerprint)
        })
        .collect::<Vec<_>>();
    let steps = poses
        .into_iter()
        .enumerate()
        .map(|(index, hinge_angles)| InstructionStep {
            id: InstructionStepId::new(),
            title: if index == 0 {
                format!("{title}：開始")
            } else {
                format!("{title}：折り{index}")
            },
            description: if index == 0 {
                "認証済み蛇腹折りの開始姿勢です。".to_owned()
            } else {
                format!(
                    "認証済み区間{index}の終端姿勢です。経路証明 SHA-256: {}",
                    certificate_references[index - 1]
                )
            },
            caution: "区間を入れ替えず順番どおりに折ってください。".to_owned(),
            duration_ms: 1_000,
            visual: if index == 0 {
                InstructionVisual::default()
            } else {
                certificate_visuals[index - 1].clone()
            },
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: request.source_model_fingerprint.to_owned(),
                fixed_face: Some(request.fixed_face),
                hinge_angles,
            },
        })
        .collect();
    let timeline = InstructionTimeline { steps };
    validate_instruction_timeline(&timeline)
        .map_err(|_| AccordionFoldMotionError::InvalidTimeline)?;
    Ok(timeline)
}

/// Compiles an inside/outside reverse fold as two independently certified
/// native path segments. The intermediate endpoint is hashed once and must be
/// exactly the target of segment one and source of segment two.
pub fn compile_certified_reverse_fold_timeline_v1(
    request: ReverseFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    let technique = request
        .technique_file
        .document()
        .techniques
        .iter()
        .find(|technique| technique.id == request.technique_id)
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    let expected_action = |action: &FoldTechniqueActionV1| match request.kind {
        ReverseFoldKindV1::Inside => matches!(action, FoldTechniqueActionV1::InsideReverseFold),
        ReverseFoldKindV1::Outside => matches!(action, FoldTechniqueActionV1::OutsideReverseFold),
    };
    let physical = technique
        .operations
        .iter()
        .filter(|operation| expected_action(&operation.action))
        .collect::<Vec<_>>();
    let expected_capability = match request.kind {
        ReverseFoldKindV1::Inside => FoldTechniqueCapabilityV1::InsideReverseFoldMotionV1,
        ReverseFoldKindV1::Outside => FoldTechniqueCapabilityV1::OutsideReverseFoldMotionV1,
    };
    if physical.len() != 1
        || !physical[0]
            .required_capabilities
            .contains(&expected_capability)
    {
        return Err(ReverseFoldMotionError::UnsupportedTechnique);
    }
    if request.first_edge == request.second_edge
        || !(0..=180_000_000).contains(&request.intermediate_angle_microdegrees)
        || !(0..=180_000_000).contains(&request.target_angle_microdegrees)
    {
        return Err(ReverseFoldMotionError::InvalidAngle);
    }
    let mut source = request.source_hinge_angles.to_vec();
    source.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    if source.windows(2).any(|pair| pair[0].edge == pair[1].edge)
        || !source.iter().all(|hinge| hinge.angle_degrees.is_finite())
        || request.source_model_fingerprint.len() != 64
    {
        return Err(ReverseFoldMotionError::InvalidSourcePose);
    }
    let mut intermediate = source.clone();
    set_hinge_angle(
        &mut intermediate,
        request.first_edge,
        request.intermediate_angle_microdegrees,
    );
    let mut target = intermediate.clone();
    set_hinge_angle(
        &mut target,
        request.second_edge,
        request.target_angle_microdegrees,
    );
    let fingerprint = |angles: &[InstructionHingeAngle]| {
        instruction_pose_fingerprint_v1(
            request.source_model_fingerprint,
            request.fixed_face,
            angles,
        )
    };
    let source_hash = fingerprint(&source);
    let intermediate_hash = fingerprint(&intermediate);
    let target_hash = fingerprint(&target);
    let first = request.first_path_certificate;
    let second = request.second_path_certificate;
    if first.edges().is_empty()
        || second.edges().is_empty()
        || first.source() != source_hash
        || first.target() != intermediate_hash
        || second.source() != intermediate_hash
        || second.target() != target_hash
        || first.target() != second.source()
    {
        return Err(ReverseFoldMotionError::PathSegmentMismatch);
    }
    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.clone())
        .ok_or(ReverseFoldMotionError::UnsupportedTechnique)?;
    let pose = |hinge_angles| InstructionPose {
        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
        source_model_fingerprint: request.source_model_fingerprint.to_owned(),
        fixed_face: Some(request.fixed_face),
        hinge_angles,
    };
    let first_reference = path_certificate_reference_v1(first, request.source_model_fingerprint);
    let second_reference = path_certificate_reference_v1(second, request.source_model_fingerprint);
    let step = |suffix: &str, description: &str, visual, angles| InstructionStep {
        id: InstructionStepId::new(),
        title: format!("{title}：{suffix}"),
        description: description.to_owned(),
        caution: "認証済みの2区間を順番どおりに操作してください。".to_owned(),
        duration_ms: 1_000,
        visual,
        pose: pose(angles),
    };
    let timeline = InstructionTimeline {
        steps: vec![
            step(
                "開始",
                "逆折りの開始姿勢です。",
                InstructionVisual::default(),
                source,
            ),
            step(
                "反転",
                &format!(
                    "第1の衝突・層順序証明区間の終端です。経路証明 SHA-256: {first_reference}"
                ),
                path_certificate_visual_v1(first, request.source_model_fingerprint),
                intermediate,
            ),
            step(
                "完了",
                &format!(
                    "第2の衝突・層順序証明区間の終端です。経路証明 SHA-256: {second_reference}"
                ),
                path_certificate_visual_v1(second, request.source_model_fingerprint),
                target,
            ),
        ],
    };
    validate_instruction_timeline(&timeline)
        .map_err(|_| ReverseFoldMotionError::InvalidTimeline)?;
    Ok(timeline)
}

fn set_hinge_angle(angles: &mut Vec<InstructionHingeAngle>, edge: EdgeId, microdegrees: i64) {
    let degrees = microdegrees as f64 / 1_000_000.0;
    if let Some(hinge) = angles.iter_mut().find(|hinge| hinge.edge == edge) {
        hinge.angle_degrees = degrees;
    } else {
        angles.push(InstructionHingeAngle {
            edge,
            angle_degrees: degrees,
        });
        angles.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    }
}

fn path_certificate_reference_v1(
    certificate: &CertifiedPoseGraphPathCertificateV1,
    source_model_fingerprint: &str,
) -> String {
    let certificate = certificate
        .binding_fingerprint_v1()
        .into_iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{certificate} / 元モデル SHA-256: {source_model_fingerprint}")
}

fn path_certificate_visual_v1(
    certificate: &CertifiedPoseGraphPathCertificateV1,
    source_model_fingerprint: &str,
) -> InstructionVisual {
    InstructionVisual {
        path_certificate_reference_v1: path_certificate_reference_from_native_v1(
            certificate,
            source_model_fingerprint,
        ),
        ..InstructionVisual::default()
    }
}

/// Converts a live native path certificate into the persisted, non-authorizing
/// reference shared by instruction archives and PDF/SVG export validation.
/// Empty certificates and non-canonical model fingerprints fail closed.
#[must_use]
pub fn path_certificate_reference_from_native_v1(
    certificate: &CertifiedPoseGraphPathCertificateV1,
    source_model_fingerprint: &str,
) -> Option<PathCertificateReferenceV1> {
    if certificate.edges().is_empty()
        || certificate.source() == certificate.target()
        || source_model_fingerprint.len() != 64
        || !source_model_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut model_hash = Sha256::new();
    model_hash.update(b"path_certificate_source_model_binding_v1");
    model_hash.update(source_model_fingerprint.as_bytes());
    Some(PathCertificateReferenceV1 {
        version: 1,
        model_id: PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1.to_owned(),
        binding_sha256: certificate.binding_fingerprint_v1(),
        source_pose_sha256: certificate.source(),
        target_pose_sha256: certificate.target(),
        source_model_binding_sha256: model_hash.finalize().into(),
        transition_count: certificate.edges().len(),
    })
}

/// Atomically appends a certified dyadic pose-graph path to an instruction
/// timeline. The caller installs the returned clone only after its project
/// mutation succeeds, so every error leaves the original timeline untouched.
pub fn append_certified_dyadic_path_timeline_v1(
    timeline: &InstructionTimeline,
    title: &str,
    source_model_fingerprint: &str,
    fixed_face: FaceId,
    source_hinge_angles: &[InstructionHingeAngle],
    target_hinge_angles: &[Vec<InstructionHingeAngle>],
    certificate: &CertifiedPoseGraphPathCertificateV1,
) -> Result<InstructionTimeline, ReverseFoldMotionError> {
    let reference =
        path_certificate_reference_from_native_v1(certificate, source_model_fingerprint)
            .ok_or(ReverseFoldMotionError::PathSegmentMismatch)?;
    if title.trim().is_empty()
        || certificate.edges().len() != target_hinge_angles.len()
        || graph_pose_fingerprint_v1(source_hinge_angles) != certificate.source()
    {
        return Err(ReverseFoldMotionError::PathSegmentMismatch);
    }
    let mut previous = source_hinge_angles;
    for (edge, target) in certificate.edges().iter().zip(target_hinge_angles) {
        if edge.source() != graph_pose_fingerprint_v1(previous)
            || edge.target() != graph_pose_fingerprint_v1(target)
        {
            return Err(ReverseFoldMotionError::PathSegmentMismatch);
        }
        previous = target;
    }

    let mut candidate = timeline.clone();
    candidate.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: format!("「{title}」の開始姿勢"),
        description: "構造化証明の始点姿勢です。".to_owned(),
        caution: String::new(),
        duration_ms: MIN_INSTRUCTION_DURATION_MS,
        visual: InstructionVisual::default(),
        pose: pose_from_angles(source_model_fingerprint, fixed_face, source_hinge_angles),
    });
    let binding = reference
        .binding_sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    for (index, target) in target_hinge_angles.iter().enumerate() {
        let edge = &certificate.edges()[index];
        let mut step_reference = reference.clone();
        step_reference.source_pose_sha256 = edge.source();
        step_reference.target_pose_sha256 = edge.target();
        candidate.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: if index == 0 { title.to_owned() } else { format!("{title} {}", index + 1) },
            description: format!(
                "認証済みの連続折り経路で「{title}」を適用します。経路証明 SHA-256: {binding} / 元モデル SHA-256: {source_model_fingerprint}"
            ),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual {
                path_certificate_reference_v1: Some(step_reference),
                ..InstructionVisual::default()
            },
            pose: pose_from_angles(source_model_fingerprint, fixed_face, target),
        });
    }
    validate_instruction_timeline(&candidate)
        .map_err(|_| ReverseFoldMotionError::InvalidTimeline)?;
    Ok(candidate)
}

fn pose_from_angles(
    model: &str,
    fixed_face: FaceId,
    angles: &[InstructionHingeAngle],
) -> InstructionPose {
    InstructionPose {
        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
        source_model_fingerprint: model.to_owned(),
        fixed_face: Some(fixed_face),
        hinge_angles: angles.to_vec(),
    }
}

fn graph_pose_fingerprint_v1(angles: &[InstructionHingeAngle]) -> [u8; 32] {
    let mut canonical = angles.to_vec();
    canonical.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    let mut hash = Sha256::new();
    hash.update(b"stacked_fold_certified_path_graph_state_v1");
    hash.update((canonical.len() as u64).to_be_bytes());
    for hinge in canonical {
        hash.update(hinge.edge.canonical_bytes());
        hash.update(hinge.angle_degrees.to_bits().to_be_bytes());
    }
    hash.finalize().into()
}

/// Compiles a validated named straight-line fold into a two-pose timeline.
///
/// The certificate must contain at least one native-certified transition and
/// bind the exact canonical source and target pose fingerprints. Collision,
/// closure, stale, and tamper failures therefore fail closed before a preview
/// can be persisted or exported.
pub fn compile_certified_book_fold_timeline_v1(
    request: BookFoldMotionRequestV1<'_>,
) -> Result<InstructionTimeline, BookFoldMotionError> {
    let technique = request
        .technique_file
        .document()
        .techniques
        .iter()
        .find(|technique| technique.id == request.technique_id)
        .ok_or(BookFoldMotionError::UnsupportedTechnique)?;
    let physical = technique
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.action,
                FoldTechniqueActionV1::StraightLineStackedFold
            )
        })
        .collect::<Vec<_>>();
    if physical.len() != 1
        || !physical[0]
            .required_capabilities
            .contains(&FoldTechniqueCapabilityV1::StraightLineStackedFoldV1)
        || physical[0].execution_support != FoldTechniqueExecutionSupportV1::DeclarativeOnly
    {
        return Err(BookFoldMotionError::UnsupportedTechnique);
    }
    let angle_parameter_id = physical[0]
        .parameter_bindings
        .iter()
        .find(|binding| binding.role == TARGET_ANGLE_ROLE)
        .map(|binding| binding.parameter_id.as_str())
        .ok_or(BookFoldMotionError::UnsupportedTechnique)?;
    let valid_angle = technique.parameters.iter().any(|parameter| {
        parameter.id == angle_parameter_id
            && matches!(
                parameter.parameter_type,
                FoldTechniqueParameterTypeV1::AngleMicrodegrees { minimum, maximum, .. }
                    if (minimum..=maximum).contains(&request.target_angle_microdegrees)
            )
    });
    if !valid_angle || !(0..=180_000_000).contains(&request.target_angle_microdegrees) {
        return Err(BookFoldMotionError::InvalidTargetAngle);
    }

    let mut source = request.source_hinge_angles.to_vec();
    source.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    if source.windows(2).any(|pair| pair[0].edge == pair[1].edge)
        || !source.iter().all(|hinge| hinge.angle_degrees.is_finite())
        || request.source_model_fingerprint.len() != 64
        || !request
            .source_model_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(BookFoldMotionError::InvalidSourcePose);
    }
    let mut target = source.clone();
    let target_degrees = request.target_angle_microdegrees as f64 / 1_000_000.0;
    match target
        .iter_mut()
        .find(|hinge| hinge.edge == request.fold_edge)
    {
        Some(hinge) => hinge.angle_degrees = target_degrees,
        None => target.push(InstructionHingeAngle {
            edge: request.fold_edge,
            angle_degrees: target_degrees,
        }),
    }
    target.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());

    let source_fingerprint = instruction_pose_fingerprint_v1(
        request.source_model_fingerprint,
        request.fixed_face,
        &source,
    );
    let target_fingerprint = instruction_pose_fingerprint_v1(
        request.source_model_fingerprint,
        request.fixed_face,
        &target,
    );
    if request.path_certificate.edges().is_empty()
        || request.path_certificate.source() != source_fingerprint
        || request.path_certificate.target() != target_fingerprint
    {
        return Err(BookFoldMotionError::PathCertificateMismatch);
    }

    let title = technique
        .names
        .iter()
        .find(|text| text.locale == "ja")
        .or_else(|| technique.names.first())
        .map(|text| text.text.clone())
        .ok_or(BookFoldMotionError::UnsupportedTechnique)?;
    let pose = |hinge_angles| InstructionPose {
        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
        source_model_fingerprint: request.source_model_fingerprint.to_owned(),
        fixed_face: Some(request.fixed_face),
        hinge_angles,
    };
    let certificate_reference =
        path_certificate_reference_v1(request.path_certificate, request.source_model_fingerprint);
    let timeline = InstructionTimeline {
        steps: vec![
            InstructionStep {
                id: InstructionStepId::new(),
                title: format!("{title}：開始"),
                description: "認証済み経路の開始姿勢です。".to_owned(),
                caution: String::new(),
                duration_ms: 500,
                visual: InstructionVisual::default(),
                pose: pose(source),
            },
            InstructionStep {
                id: InstructionStepId::new(),
                title: format!("{title}：完了"),
                description: format!(
                    "衝突・閉包証明に結合された経路で折ります。経路証明 SHA-256: {certificate_reference}"
                ),
                caution: "証明対象外の紙や姿勢には適用しないでください。".to_owned(),
                duration_ms: 1_500,
                visual: path_certificate_visual_v1(
                    request.path_certificate,
                    request.source_model_fingerprint,
                ),
                pose: pose(target),
            },
        ],
    };
    validate_instruction_timeline(&timeline).map_err(|_| BookFoldMotionError::InvalidTimeline)?;
    Ok(timeline)
}

/// Canonical endpoint binding shared with a native path-certificate issuer.
#[must_use]
pub fn instruction_pose_fingerprint_v1(
    source_model_fingerprint: &str,
    fixed_face: FaceId,
    hinge_angles: &[InstructionHingeAngle],
) -> [u8; 32] {
    let mut canonical = hinge_angles.to_vec();
    canonical.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    let mut hash = Sha256::new();
    hash.update(b"origami2_instruction_pose_fingerprint_v1");
    hash.update(source_model_fingerprint.as_bytes());
    hash.update(fixed_face.canonical_bytes());
    for hinge in canonical {
        hash.update(hinge.edge.canonical_bytes());
        hash.update(hinge.angle_degrees.to_bits().to_be_bytes());
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use ori_collision::{
        CertifiedPathGraphSearchResultV1, CertifiedPathTransitionCandidateV1,
        CertifiedPathTransitionEvidenceV1, search_certified_pose_graph_v1,
    };

    use super::*;
    use crate::{
        FOLD_TECHNIQUE_FILE_SCHEMA_V1, FOLD_TECHNIQUE_FILE_VERSION_V1, FoldTechniqueFileDocumentV1,
        FoldTechniqueLocalizedTextV1, FoldTechniqueMetadataV1, FoldTechniqueOperationV1,
        FoldTechniqueParameterBindingV1, FoldTechniqueParameterDefinitionV1, FoldTechniqueSourceV1,
        FoldTechniqueTemplateV1, FoldTechniqueUnsupportedPhysicalOperationV1,
        validate_fold_technique_file_v1,
    };

    fn text(value: &str) -> Vec<FoldTechniqueLocalizedTextV1> {
        vec![FoldTechniqueLocalizedTextV1 {
            locale: "ja".to_owned(),
            text: value.to_owned(),
        }]
    }

    #[test]
    fn every_physical_v1_action_has_one_proof_bearing_compiler_family() {
        let cue = FoldTechniqueActionV1::InstructionCue {
            instructions: text("説明"),
        };
        assert_eq!(physical_technique_compiler_v1(&cue), None);
        let cases = [
            (
                FoldTechniqueActionV1::StraightLineStackedFold,
                PhysicalTechniqueCompilerV1::BookFold,
            ),
            (
                FoldTechniqueActionV1::InsideReverseFold,
                PhysicalTechniqueCompilerV1::InsideReverseFold,
            ),
            (
                FoldTechniqueActionV1::OutsideReverseFold,
                PhysicalTechniqueCompilerV1::OutsideReverseFold,
            ),
            (
                FoldTechniqueActionV1::SinkFold {
                    sink_kind: crate::FoldTechniqueSinkKindV1::Open,
                },
                PhysicalTechniqueCompilerV1::SinkFold,
            ),
            (
                FoldTechniqueActionV1::SinkFold {
                    sink_kind: crate::FoldTechniqueSinkKindV1::Closed,
                },
                PhysicalTechniqueCompilerV1::SinkFold,
            ),
            (
                FoldTechniqueActionV1::LayerSelectiveManipulation {
                    instructions: text("層を選ぶ"),
                },
                PhysicalTechniqueCompilerV1::LayerSelective,
            ),
        ];
        for (action, expected) in cases {
            assert_eq!(physical_technique_compiler_v1(&action), Some(expected));
        }
    }

    fn book_fold_file() -> FoldTechniqueFileV1 {
        validate_fold_technique_file_v1(FoldTechniqueFileDocumentV1 {
            schema: FOLD_TECHNIQUE_FILE_SCHEMA_V1.to_owned(),
            version: FOLD_TECHNIQUE_FILE_VERSION_V1,
            package_id: "user.test.book-fold".to_owned(),
            metadata: FoldTechniqueMetadataV1 {
                authors: vec!["Test".to_owned()],
                source: FoldTechniqueSourceV1::UserAuthored,
                license_spdx_id: "MIT".to_owned(),
            },
            techniques: vec![FoldTechniqueTemplateV1 {
                id: "book-fold".to_owned(),
                version: 1,
                names: text("二つ折り"),
                descriptions: text("直線に沿う一枚の二つ折り"),
                parameters: vec![FoldTechniqueParameterDefinitionV1 {
                    id: "target_angle".to_owned(),
                    names: text("目標角度"),
                    descriptions: text("折り終わりの角度"),
                    parameter_type: FoldTechniqueParameterTypeV1::AngleMicrodegrees {
                        minimum: 1,
                        maximum: 180_000_000,
                        default: 90_000_000,
                    },
                }],
                preconditions: vec![],
                operations: vec![
                    FoldTechniqueOperationV1 {
                        id: "prepare".to_owned(),
                        names: text("準備"),
                        action: FoldTechniqueActionV1::InstructionCue {
                            instructions: text("紙を置く"),
                        },
                        parameter_bindings: vec![],
                        precondition_ids: vec![],
                        required_capabilities: vec![
                            FoldTechniqueCapabilityV1::HumanInterpretationV1,
                        ],
                        execution_support: FoldTechniqueExecutionSupportV1::DeclarativeOnly,
                    },
                    FoldTechniqueOperationV1 {
                        id: "fold".to_owned(),
                        names: text("折る"),
                        action: FoldTechniqueActionV1::StraightLineStackedFold,
                        parameter_bindings: vec![FoldTechniqueParameterBindingV1 {
                            role: TARGET_ANGLE_ROLE.to_owned(),
                            parameter_id: "target_angle".to_owned(),
                        }],
                        precondition_ids: vec![],
                        required_capabilities: vec![
                            FoldTechniqueCapabilityV1::StraightLineStackedFoldV1,
                        ],
                        execution_support: FoldTechniqueExecutionSupportV1::DeclarativeOnly,
                    },
                ],
            }],
        })
        .expect("valid book fold")
    }

    fn certificate(source: [u8; 32], target: [u8; 32]) -> CertifiedPoseGraphPathCertificateV1 {
        let candidate = CertifiedPathTransitionCandidateV1 {
            source,
            target,
            candidate_key: [7; 32],
        };
        match search_certified_pose_graph_v1(
            &[source, target],
            &[candidate],
            source,
            target,
            |edge| {
                Some(CertifiedPathTransitionEvidenceV1::from_native_oracle(
                    edge.source,
                    edge.target,
                    [1; 32],
                    [2; 32],
                    [3; 32],
                ))
            },
        ) {
            CertifiedPathGraphSearchResultV1::Certified(certificate) => certificate,
            other => panic!("expected certificate, got {other:?}"),
        }
    }

    #[test]
    fn shared_sink_and_layer_timeline_binds_each_certified_segment_only_to_its_endpoint() {
        let face = FaceId::new();
        let first_edge = EdgeId::new();
        let second_edge = EdgeId::new();
        let model = "78".repeat(32);
        let source = Vec::new();
        let mut middle = source.clone();
        set_hinge_angle(&mut middle, first_edge, 45_000_000);
        let mut target = middle.clone();
        set_hinge_angle(&mut target, second_edge, 90_000_000);
        let first = certificate(
            instruction_pose_fingerprint_v1(&model, face, &source),
            instruction_pose_fingerprint_v1(&model, face, &middle),
        );
        let second = certificate(
            instruction_pose_fingerprint_v1(&model, face, &middle),
            instruction_pose_fingerprint_v1(&model, face, &target),
        );
        let persisted = path_certificate_reference_from_native_v1(&first, &model)
            .expect("native certificate becomes a persisted DTO");
        assert_eq!(persisted.binding_sha256, first.binding_fingerprint_v1());
        assert_eq!(persisted.source_pose_sha256, first.source());
        assert_eq!(persisted.target_pose_sha256, first.target());
        assert_eq!(persisted.transition_count, first.edges().len());
        assert!(path_certificate_reference_from_native_v1(&first, "ABC").is_none());
        let timeline = compile_two_segment_motion(
            "二段階技法",
            &model,
            face,
            first_edge,
            second_edge,
            &source,
            45_000_000,
            90_000_000,
            &first,
            &second,
        )
        .expect("two certified segments");

        assert!(!timeline.steps[0].description.contains("経路証明 SHA-256:"));
        assert!(
            timeline.steps[1]
                .description
                .contains(&path_certificate_reference_v1(&first, &model))
        );
        assert_eq!(
            timeline.steps[1]
                .visual
                .path_certificate_reference_v1
                .as_ref()
                .map(|reference| reference.binding_sha256),
            Some(first.binding_fingerprint_v1())
        );
        assert!(
            timeline.steps[2]
                .description
                .contains(&path_certificate_reference_v1(&second, &model))
        );
    }

    #[test]
    fn atomic_dyadic_timeline_append_is_proof_bearing_or_noop() {
        let face = FaceId::new();
        let edge = EdgeId::new();
        let model = "5a".repeat(32);
        let source = vec![InstructionHingeAngle {
            edge,
            angle_degrees: 5.0,
        }];
        let target = vec![InstructionHingeAngle {
            edge,
            angle_degrees: 45.0,
        }];
        let certificate = certificate(
            graph_pose_fingerprint_v1(&source),
            graph_pose_fingerprint_v1(&target),
        );
        let original = InstructionTimeline::default();
        let appended = append_certified_dyadic_path_timeline_v1(
            &original,
            "atomic dyadic fold",
            &model,
            face,
            &source,
            std::slice::from_ref(&target),
            &certificate,
        )
        .expect("certified dyadic timeline");
        assert!(original.steps.is_empty());
        assert_eq!(appended.steps.len(), 2);
        let persisted = appended.steps[1]
            .visual
            .path_certificate_reference_v1
            .as_ref()
            .expect("structured path reference");
        assert_eq!(
            persisted.binding_sha256,
            certificate.binding_fingerprint_v1()
        );
        assert_eq!(persisted.source_pose_sha256, certificate.source());
        assert_eq!(persisted.target_pose_sha256, certificate.target());

        let tampered = vec![InstructionHingeAngle {
            edge,
            angle_degrees: 46.0,
        }];
        assert_eq!(
            append_certified_dyadic_path_timeline_v1(
                &original,
                "atomic dyadic fold",
                &model,
                face,
                &source,
                &[tampered],
                &certificate,
            ),
            Err(ReverseFoldMotionError::PathSegmentMismatch)
        );
        assert!(original.steps.is_empty());
    }

    fn reverse_fold_file(kind: ReverseFoldKindV1) -> FoldTechniqueFileV1 {
        let mut document = book_fold_file().document().clone();
        let technique = &mut document.techniques[0];
        technique.names = text(match kind {
            ReverseFoldKindV1::Inside => "中割り折り",
            ReverseFoldKindV1::Outside => "かぶせ折り",
        });
        let operation = &mut technique.operations[1];
        let (action, capability, unsupported) = match kind {
            ReverseFoldKindV1::Inside => (
                FoldTechniqueActionV1::InsideReverseFold,
                FoldTechniqueCapabilityV1::InsideReverseFoldMotionV1,
                FoldTechniqueUnsupportedPhysicalOperationV1::InsideReverseFoldMotionV1,
            ),
            ReverseFoldKindV1::Outside => (
                FoldTechniqueActionV1::OutsideReverseFold,
                FoldTechniqueCapabilityV1::OutsideReverseFoldMotionV1,
                FoldTechniqueUnsupportedPhysicalOperationV1::OutsideReverseFoldMotionV1,
            ),
        };
        operation.action = action;
        operation.required_capabilities = vec![capability];
        operation.execution_support =
            FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                operation: unsupported,
            };
        validate_fold_technique_file_v1(document).expect("valid reverse fold")
    }

    fn accordion_fold_file() -> FoldTechniqueFileV1 {
        let mut document = book_fold_file().document().clone();
        let technique = &mut document.techniques[0];
        technique.names = text("蛇腹折り");
        let physical = technique.operations[1].clone();
        technique.operations = (0..3)
            .map(|index| {
                let mut operation = physical.clone();
                operation.id = format!("pleat-{}", index + 1);
                operation
            })
            .collect();
        validate_fold_technique_file_v1(document).expect("valid accordion fold")
    }

    #[test]
    fn accordion_fold_binds_three_ordered_pose_segments() {
        let file = accordion_fold_file();
        let face = FaceId::new();
        let edges = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
        let targets = [45_000_000, 90_000_000, 135_000_000];
        let model = "56".repeat(32);
        let source = Vec::new();
        let mut previous = source.clone();
        let certificates = edges
            .iter()
            .zip(targets)
            .map(|(edge, angle)| {
                let source_hash = instruction_pose_fingerprint_v1(&model, face, &previous);
                set_hinge_angle(&mut previous, *edge, angle);
                let target_hash = instruction_pose_fingerprint_v1(&model, face, &previous);
                certificate(source_hash, target_hash)
            })
            .collect::<Vec<_>>();
        let timeline = compile_certified_accordion_fold_timeline_v1(AccordionFoldMotionRequestV1 {
            technique_file: &file,
            technique_id: "book-fold",
            source_model_fingerprint: &model,
            fixed_face: face,
            source_hinge_angles: &source,
            ordered_edges: &edges,
            ordered_target_angles_microdegrees: &targets,
            ordered_path_certificates: &certificates,
        })
        .expect("three continuous segments");
        assert_eq!(timeline.steps.len(), 4);
        assert_eq!(timeline.steps[3].pose.hinge_angles, previous);
        assert!(!timeline.steps[0].description.contains("経路証明 SHA-256:"));
        for (index, certificate) in certificates.iter().enumerate() {
            assert!(
                timeline.steps[index + 1]
                    .description
                    .contains(&path_certificate_reference_v1(certificate, &model))
            );
            assert_eq!(
                timeline.steps[index + 1]
                    .visual
                    .path_certificate_reference_v1
                    .as_ref()
                    .map(|reference| reference.binding_sha256),
                Some(certificate.binding_fingerprint_v1())
            );
        }
        let petal = compile_certified_regular_quad_petal_fold_timeline_v1(
            RegularQuadPetalFoldMotionRequestV1 {
                technique_file: &file,
                technique_id: "book-fold",
                source_model_fingerprint: &model,
                fixed_face: face,
                source_hinge_angles: &source,
                ordered_edges: &edges,
                ordered_target_angles_microdegrees: &targets,
                ordered_path_certificates: &certificates,
            },
        )
        .expect("regular-quad petal keeps the exact three-segment proof chain");
        assert_eq!(petal.steps.len(), 4);
        assert_eq!(petal.steps[3].pose.hinge_angles, previous);
    }

    #[test]
    fn accordion_fold_rejects_reordered_certificate_segments() {
        let file = accordion_fold_file();
        let face = FaceId::new();
        let edges = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
        let targets = [45_000_000, 90_000_000, 135_000_000];
        let model = "78".repeat(32);
        let mut previous = Vec::new();
        let mut certificates = edges
            .iter()
            .zip(targets)
            .map(|(edge, angle)| {
                let source_hash = instruction_pose_fingerprint_v1(&model, face, &previous);
                set_hinge_angle(&mut previous, *edge, angle);
                certificate(
                    source_hash,
                    instruction_pose_fingerprint_v1(&model, face, &previous),
                )
            })
            .collect::<Vec<_>>();
        certificates.swap(0, 1);
        assert_eq!(
            compile_certified_accordion_fold_timeline_v1(AccordionFoldMotionRequestV1 {
                technique_file: &file,
                technique_id: "book-fold",
                source_model_fingerprint: &model,
                fixed_face: face,
                source_hinge_angles: &[],
                ordered_edges: &edges,
                ordered_target_angles_microdegrees: &targets,
                ordered_path_certificates: &certificates,
            }),
            Err(AccordionFoldMotionError::PathSegmentMismatch)
        );
        assert_eq!(
            compile_certified_regular_quad_petal_fold_timeline_v1(
                RegularQuadPetalFoldMotionRequestV1 {
                    technique_file: &file,
                    technique_id: "book-fold",
                    source_model_fingerprint: &model,
                    fixed_face: face,
                    source_hinge_angles: &[],
                    ordered_edges: &edges,
                    ordered_target_angles_microdegrees: &targets,
                    ordered_path_certificates: &certificates,
                },
            ),
            Err(AccordionFoldMotionError::PathSegmentMismatch),
            "a petal certificate cannot be reordered or substituted",
        );
    }

    #[test]
    fn inside_and_outside_reverse_folds_require_two_continuous_certified_segments() {
        for kind in [ReverseFoldKindV1::Inside, ReverseFoldKindV1::Outside] {
            let file = reverse_fold_file(kind);
            let face = FaceId::new();
            let first_edge = EdgeId::new();
            let second_edge = EdgeId::new();
            let model = "12".repeat(32);
            let source = vec![InstructionHingeAngle {
                edge: first_edge,
                angle_degrees: 0.0,
            }];
            let mut intermediate = source.clone();
            set_hinge_angle(&mut intermediate, first_edge, 45_000_000);
            let mut target = intermediate.clone();
            set_hinge_angle(&mut target, second_edge, 90_000_000);
            let source_hash = instruction_pose_fingerprint_v1(&model, face, &source);
            let intermediate_hash = instruction_pose_fingerprint_v1(&model, face, &intermediate);
            let target_hash = instruction_pose_fingerprint_v1(&model, face, &target);
            let first = certificate(source_hash, intermediate_hash);
            let second = certificate(intermediate_hash, target_hash);
            let timeline = compile_certified_reverse_fold_timeline_v1(ReverseFoldMotionRequestV1 {
                technique_file: &file,
                technique_id: "book-fold",
                kind,
                source_model_fingerprint: &model,
                fixed_face: face,
                first_edge,
                second_edge,
                source_hinge_angles: &source,
                intermediate_angle_microdegrees: 45_000_000,
                target_angle_microdegrees: 90_000_000,
                first_path_certificate: &first,
                second_path_certificate: &second,
            })
            .expect("two certified segments");
            assert_eq!(timeline.steps.len(), 3);
            assert_eq!(timeline.steps[1].pose.hinge_angles, intermediate);
            assert_eq!(timeline.steps[2].pose.hinge_angles, target);
            assert!(!timeline.steps[0].description.contains("経路証明 SHA-256:"));
            assert!(
                timeline.steps[1]
                    .description
                    .contains(&path_certificate_reference_v1(&first, &model))
            );
            assert_eq!(
                timeline.steps[1]
                    .visual
                    .path_certificate_reference_v1
                    .as_ref()
                    .map(|reference| reference.binding_sha256),
                Some(first.binding_fingerprint_v1())
            );
            assert!(
                timeline.steps[2]
                    .description
                    .contains(&path_certificate_reference_v1(&second, &model))
            );
        }
    }

    #[test]
    fn reverse_fold_rejects_discontinuous_or_reordered_segment_authority() {
        let file = reverse_fold_file(ReverseFoldKindV1::Inside);
        let face = FaceId::new();
        let first_edge = EdgeId::new();
        let second_edge = EdgeId::new();
        let model = "34".repeat(32);
        let source = vec![InstructionHingeAngle {
            edge: first_edge,
            angle_degrees: 0.0,
        }];
        let source_hash = instruction_pose_fingerprint_v1(&model, face, &source);
        let unrelated = [0x55; 32];
        let first = certificate(source_hash, unrelated);
        let second = certificate(unrelated, [0x66; 32]);
        assert_eq!(
            compile_certified_reverse_fold_timeline_v1(ReverseFoldMotionRequestV1 {
                technique_file: &file,
                technique_id: "book-fold",
                kind: ReverseFoldKindV1::Inside,
                source_model_fingerprint: &model,
                fixed_face: face,
                first_edge,
                second_edge,
                source_hinge_angles: &source,
                intermediate_angle_microdegrees: 45_000_000,
                target_angle_microdegrees: 90_000_000,
                first_path_certificate: &first,
                second_path_certificate: &second,
            }),
            Err(ReverseFoldMotionError::PathSegmentMismatch)
        );
    }

    #[test]
    fn certified_book_fold_compiles_exact_native_endpoints() {
        let file = book_fold_file();
        let face = FaceId::new();
        let edge = EdgeId::new();
        let source_angles = vec![InstructionHingeAngle {
            edge,
            angle_degrees: 0.0,
        }];
        let fingerprint = "ab".repeat(32);
        let source = instruction_pose_fingerprint_v1(&fingerprint, face, &source_angles);
        let target_angles = vec![InstructionHingeAngle {
            edge,
            angle_degrees: 90.0,
        }];
        let target = instruction_pose_fingerprint_v1(&fingerprint, face, &target_angles);
        let proof = certificate(source, target);
        let timeline = compile_certified_book_fold_timeline_v1(BookFoldMotionRequestV1 {
            technique_file: &file,
            technique_id: "book-fold",
            source_model_fingerprint: &fingerprint,
            fixed_face: face,
            fold_edge: edge,
            source_hinge_angles: &source_angles,
            target_angle_microdegrees: 90_000_000,
            path_certificate: &proof,
        })
        .expect("certified fold timeline");
        assert_eq!(timeline.steps.len(), 2);
        assert_eq!(timeline.steps[1].pose.hinge_angles[0].angle_degrees, 90.0);
        let reference = proof
            .binding_fingerprint_v1()
            .into_iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(timeline.steps[1].description.contains(&reference));
        assert_eq!(
            timeline.steps[1]
                .visual
                .path_certificate_reference_v1
                .as_ref()
                .map(|value| value.binding_sha256),
            Some(proof.binding_fingerprint_v1())
        );
    }

    #[test]
    fn stale_tampered_or_uncertified_endpoints_fail_closed() {
        let file = book_fold_file();
        let face = FaceId::new();
        let edge = EdgeId::new();
        let source_angles = vec![InstructionHingeAngle {
            edge,
            angle_degrees: 0.0,
        }];
        let fingerprint = "cd".repeat(32);
        let source = instruction_pose_fingerprint_v1(&fingerprint, face, &source_angles);
        let target = instruction_pose_fingerprint_v1(
            &fingerprint,
            face,
            &[InstructionHingeAngle {
                edge,
                angle_degrees: 90.0,
            }],
        );
        let proof = certificate(source, target);
        for (model, angle) in [
            ("ef".repeat(32), 90_000_000),
            (fingerprint.clone(), 91_000_000),
        ] {
            assert_eq!(
                compile_certified_book_fold_timeline_v1(BookFoldMotionRequestV1 {
                    technique_file: &file,
                    technique_id: "book-fold",
                    source_model_fingerprint: &model,
                    fixed_face: face,
                    fold_edge: edge,
                    source_hinge_angles: &source_angles,
                    target_angle_microdegrees: angle,
                    path_certificate: &proof,
                }),
                Err(BookFoldMotionError::PathCertificateMismatch),
            );
        }
        assert_eq!(
            compile_certified_book_fold_timeline_v1(BookFoldMotionRequestV1 {
                technique_file: &file,
                technique_id: "book-fold",
                source_model_fingerprint: &fingerprint,
                fixed_face: face,
                fold_edge: edge,
                source_hinge_angles: &source_angles,
                target_angle_microdegrees: 180_000_001,
                path_certificate: &proof,
            }),
            Err(BookFoldMotionError::InvalidTargetAngle),
        );
    }

    #[test]
    fn petal_fold_audit_keeps_every_unproven_physical_premise_closed() {
        let audit = audit_certified_petal_fold_v1();
        assert!(!audit.supported);
        assert_eq!(audit.minimum_graph_segments, 3);
        assert_eq!(audit.missing_premises, PETAL_FOLD_MISSING_PREMISES_V1);
        assert!(
            audit
                .missing_premises
                .contains(&PetalFoldMissingPremiseV1::ContinuousLayerAuthority)
        );
    }
}
