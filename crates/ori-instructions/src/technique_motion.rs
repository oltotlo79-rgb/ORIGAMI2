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
                description: "衝突・閉包証明に結合された経路で折ります。".to_owned(),
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
        FoldTechniqueTemplateV1, validate_fold_technique_file_v1,
    };

    fn text(value: &str) -> Vec<FoldTechniqueLocalizedTextV1> {
        vec![FoldTechniqueLocalizedTextV1 {
            locale: "ja".to_owned(),
            text: value.to_owned(),
        }]
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
