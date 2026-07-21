//! Narrow, fail-closed execution boundary for a named straight-line book fold.
//!
//! Technique files remain inert. This module only compiles the one physical
//! operation whose endpoints have already been joined by a native collision-
//! and closure-certified path. The returned timeline is still preview data;
//! it never grants project-mutation authority.

use ori_collision::CertifiedPoseGraphPathCertificateV1;
use ori_domain::{
    EdgeId, FaceId, InstructionHingeAngle, InstructionPose, InstructionPoseModel, InstructionStep,
    InstructionStepId, InstructionTimeline, InstructionVisual, validate_instruction_timeline,
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
        path_certificate_reference_v1(first),
        path_certificate_reference_v1(second),
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
            visual: InstructionVisual::default(),
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
        .map(path_certificate_reference_v1)
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
            visual: InstructionVisual::default(),
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
    let first_reference = path_certificate_reference_v1(first);
    let second_reference = path_certificate_reference_v1(second);
    let step = |suffix: &str, description: &str, angles| InstructionStep {
        id: InstructionStepId::new(),
        title: format!("{title}：{suffix}"),
        description: description.to_owned(),
        caution: "認証済みの2区間を順番どおりに操作してください。".to_owned(),
        duration_ms: 1_000,
        visual: InstructionVisual::default(),
        pose: pose(angles),
    };
    let timeline = InstructionTimeline {
        steps: vec![
            step("開始", "逆折りの開始姿勢です。", source),
            step(
                "反転",
                &format!(
                    "第1の衝突・層順序証明区間の終端です。経路証明 SHA-256: {first_reference}"
                ),
                intermediate,
            ),
            step(
                "完了",
                &format!(
                    "第2の衝突・層順序証明区間の終端です。経路証明 SHA-256: {second_reference}"
                ),
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

fn path_certificate_reference_v1(certificate: &CertifiedPoseGraphPathCertificateV1) -> String {
    certificate
        .binding_fingerprint_v1()
        .into_iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    let certificate_reference = path_certificate_reference_v1(request.path_certificate);
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
                visual: InstructionVisual::default(),
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
                .contains(&path_certificate_reference_v1(&first))
        );
        assert!(
            timeline.steps[2]
                .description
                .contains(&path_certificate_reference_v1(&second))
        );
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
                    .contains(&path_certificate_reference_v1(certificate))
            );
        }
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
                    .contains(&path_certificate_reference_v1(&first))
            );
            assert!(
                timeline.steps[2]
                    .description
                    .contains(&path_certificate_reference_v1(&second))
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
}
