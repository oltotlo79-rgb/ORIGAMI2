use std::collections::VecDeque;
use std::sync::Arc;

use ori_collision::CertifiedPoseGraphPathCertificateV1;
use ori_domain::{
    FaceId, InstructionPoseModel, InstructionStepId, InstructionTimeline, MAX_INSTRUCTION_STEPS,
    PathCertificateReferenceV1,
};
use ori_formats::PathCertificateExportAttestationV1;
use sha2::{Digest, Sha256};

pub(super) const MAX_TRUSTED_PATH_CERTIFICATE_REFERENCES_V1: usize = MAX_INSTRUCTION_STEPS;

pub(super) struct TrustedPathCertificateEntryV1 {
    project_instance_id: ori_domain::ProjectId,
    project_id: ori_domain::ProjectId,
    step_id: InstructionStepId,
    source_step_id: InstructionStepId,
    source_fixed_face: FaceId,
    target_fixed_face: FaceId,
    source_pose_binding_sha256: [u8; 32],
    target_pose_binding_sha256: [u8; 32],
    reference: PathCertificateReferenceV1,
    certificate: Arc<CertifiedPoseGraphPathCertificateV1>,
}

fn try_clone_path_certificate_reference_v1(
    reference: &PathCertificateReferenceV1,
) -> Result<PathCertificateReferenceV1, ()> {
    let mut model_id = String::new();
    model_id
        .try_reserve_exact(reference.model_id.len())
        .map_err(|_| ())?;
    model_id.push_str(&reference.model_id);
    Ok(PathCertificateReferenceV1 {
        version: reference.version,
        model_id,
        binding_sha256: reference.binding_sha256,
        source_pose_sha256: reference.source_pose_sha256,
        target_pose_sha256: reference.target_pose_sha256,
        source_model_binding_sha256: reference.source_model_binding_sha256,
        transition_count: reference.transition_count,
    })
}

impl TrustedPathCertificateEntryV1 {
    fn try_clone_v1(&self) -> Result<Self, ()> {
        Ok(Self {
            project_instance_id: self.project_instance_id,
            project_id: self.project_id,
            step_id: self.step_id,
            source_step_id: self.source_step_id,
            source_fixed_face: self.source_fixed_face,
            target_fixed_face: self.target_fixed_face,
            source_pose_binding_sha256: self.source_pose_binding_sha256,
            target_pose_binding_sha256: self.target_pose_binding_sha256,
            reference: try_clone_path_certificate_reference_v1(&self.reference)?,
            certificate: Arc::clone(&self.certificate),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct LivePathCertificateStepBindingV1 {
    source_step_id: InstructionStepId,
    source_fixed_face: FaceId,
    target_fixed_face: FaceId,
    source_pose_binding_sha256: [u8; 32],
    target_pose_binding_sha256: [u8; 32],
}

#[cfg(test)]
fn graph_pose_fingerprint_v1(pose: &ori_domain::InstructionPose) -> Option<[u8; 32]> {
    let mut angles = Vec::new();
    angles.try_reserve_exact(pose.hinge_angles.len()).ok()?;
    angles.extend_from_slice(&pose.hinge_angles);
    angles.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    let mut hash = Sha256::new();
    hash.update(b"stacked_fold_certified_path_graph_state_v1");
    hash.update((angles.len() as u64).to_be_bytes());
    for hinge in angles {
        hash.update(hinge.edge.canonical_bytes());
        hash.update(hinge.angle_degrees.to_bits().to_be_bytes());
    }
    Some(hash.finalize().into())
}

fn source_model_binding_v1(source_model_fingerprint: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"path_certificate_source_model_binding_v1");
    hash.update(source_model_fingerprint.as_bytes());
    hash.finalize().into()
}

fn live_step_binding_v1(
    timeline: &InstructionTimeline,
    step_index: usize,
    reference: &PathCertificateReferenceV1,
    certificate: &CertifiedPoseGraphPathCertificateV1,
) -> Option<LivePathCertificateStepBindingV1> {
    let source_index = step_index.checked_sub(1)?;
    let source = timeline.steps.get(source_index)?;
    let target = timeline.steps.get(step_index)?;
    let source_fixed_face = source.pose.fixed_face?;
    let target_fixed_face = target.pose.fixed_face?;
    let source_pose_binding_sha256 = ori_instructions::instruction_pose_fingerprint_v1(
        &source.pose.source_model_fingerprint,
        source_fixed_face,
        &source.pose.hinge_angles,
    );
    let target_pose_binding_sha256 = ori_instructions::instruction_pose_fingerprint_v1(
        &target.pose.source_model_fingerprint,
        target_fixed_face,
        &target.pose.hinge_angles,
    );
    if target.visual.path_certificate_reference_v1.as_ref() != Some(reference)
        || source.pose.model != InstructionPoseModel::AbsoluteHingeAnglesV1
        || target.pose.model != InstructionPoseModel::AbsoluteHingeAnglesV1
        || source.pose.source_model_fingerprint != target.pose.source_model_fingerprint
        || source_model_binding_v1(&source.pose.source_model_fingerprint)
            != reference.source_model_binding_sha256
        || source_pose_binding_sha256 != reference.source_pose_sha256
        || target_pose_binding_sha256 != reference.target_pose_sha256
        || certificate.native_source_model_binding_v1()
            != Some(reference.source_model_binding_sha256)
        || !certificate.matches_path_certificate_reference_v1(reference)
    {
        return None;
    }
    if source_fixed_face != target_fixed_face {
        return None;
    }
    if certificate.native_fixed_face_v1() != Some(source_fixed_face) {
        return None;
    }
    Some(LivePathCertificateStepBindingV1 {
        source_step_id: source.id,
        source_fixed_face,
        target_fixed_face,
        source_pose_binding_sha256,
        target_pose_binding_sha256,
    })
}

fn entry_matches_live_step_v1(
    entry: &TrustedPathCertificateEntryV1,
    timeline: &InstructionTimeline,
    step_index: usize,
) -> bool {
    if !entry.certificate.is_native_attestable_v1() {
        return false;
    }
    let Some(step) = timeline.steps.get(step_index) else {
        return false;
    };
    if step.id != entry.step_id
        || step.visual.path_certificate_reference_v1.as_ref() != Some(&entry.reference)
    {
        return false;
    }
    live_step_binding_v1(timeline, step_index, &entry.reference, &entry.certificate).is_some_and(
        |binding| {
            binding.source_step_id == entry.source_step_id
                && binding.source_fixed_face == entry.source_fixed_face
                && binding.target_fixed_face == entry.target_fixed_face
                && binding.source_pose_binding_sha256 == entry.source_pose_binding_sha256
                && binding.target_pose_binding_sha256 == entry.target_pose_binding_sha256
        },
    )
}

#[derive(Default)]
pub(super) struct TrustedPathCertificateRegistryV1 {
    entries: VecDeque<TrustedPathCertificateEntryV1>,
}

impl TrustedPathCertificateRegistryV1 {
    pub(super) fn prepare_entries_for_timeline_suffix_v1(
        project_instance_id: ori_domain::ProjectId,
        project_id: ori_domain::ProjectId,
        timeline: &InstructionTimeline,
        suffix_start: usize,
        certificate: Option<&CertifiedPoseGraphPathCertificateV1>,
    ) -> Result<Vec<TrustedPathCertificateEntryV1>, ()> {
        let suffix = timeline.steps.get(suffix_start..).ok_or(())?;
        let reference_count = suffix
            .iter()
            .filter(|step| step.visual.path_certificate_reference_v1.is_some())
            .count();
        if reference_count == 0 {
            return Ok(Vec::new());
        }
        let certificate = certificate.ok_or(())?;
        if !certificate.is_native_attestable_v1()
            || certificate.native_source_model_binding_v1().is_none()
            || certificate.edges().is_empty()
            || certificate.edges().len() > ori_domain::MAX_PATH_CERTIFICATE_REFERENCE_TRANSITIONS_V1
            || certificate.authorizes_project_mutation()
        {
            return Err(());
        }

        let whole = Arc::new(certificate.try_clone_v1().ok_or(())?);
        let candidate_count = certificate.edges().len().checked_add(1).ok_or(())?;
        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(candidate_count)
            .map_err(|_| ())?;
        candidates.push(whole);
        for index in 0..certificate.edges().len() {
            let segment = certificate.segment_certificate_v1(index).ok_or(())?;
            if candidates
                .iter()
                .all(|candidate| candidate.as_ref() != &segment)
            {
                candidates.push(Arc::new(segment));
            }
        }

        let mut entries = Vec::new();
        entries.try_reserve_exact(reference_count).map_err(|_| ())?;
        for (step_index, step) in timeline.steps.iter().enumerate().skip(suffix_start) {
            let Some(reference) = step.visual.path_certificate_reference_v1.as_ref() else {
                continue;
            };
            let mut matching = candidates
                .iter()
                .filter(|candidate| candidate.matches_path_certificate_reference_v1(reference));
            let certificate = Arc::clone(matching.next().ok_or(())?);
            if matching.next().is_some() {
                return Err(());
            }
            let binding =
                live_step_binding_v1(timeline, step_index, reference, &certificate).ok_or(())?;
            entries.push(TrustedPathCertificateEntryV1 {
                project_instance_id,
                project_id,
                step_id: step.id,
                source_step_id: binding.source_step_id,
                source_fixed_face: binding.source_fixed_face,
                target_fixed_face: binding.target_fixed_face,
                source_pose_binding_sha256: binding.source_pose_binding_sha256,
                target_pose_binding_sha256: binding.target_pose_binding_sha256,
                reference: try_clone_path_certificate_reference_v1(reference)?,
                certificate,
            });
        }
        Ok(entries)
    }

    pub(super) fn with_registered_timeline_v1(
        &self,
        project_instance_id: ori_domain::ProjectId,
        project_id: ori_domain::ProjectId,
        timeline: &InstructionTimeline,
        new_entries: Vec<TrustedPathCertificateEntryV1>,
    ) -> Result<Self, ()> {
        if new_entries.iter().any(|entry| {
            entry.project_instance_id != project_instance_id
                || entry.project_id != project_id
                || !entry.certificate.is_native_attestable_v1()
                || !timeline.steps.iter().enumerate().any(|(step_index, step)| {
                    step.id == entry.step_id
                        && entry_matches_live_step_v1(entry, timeline, step_index)
                })
        }) {
            return Err(());
        }

        let prepared_capacity = self
            .entries
            .len()
            .checked_add(new_entries.len())
            .ok_or(())?;
        let mut prepared_entries = VecDeque::new();
        prepared_entries
            .try_reserve_exact(prepared_capacity)
            .map_err(|_| ())?;
        for entry in &self.entries {
            prepared_entries.push_back(entry.try_clone_v1()?);
        }
        let mut prepared = Self {
            entries: prepared_entries,
        };
        prepared.entries.retain(|entry| {
            entry.project_instance_id == project_instance_id
                && entry.project_id == project_id
                && timeline.steps.iter().enumerate().any(|(step_index, step)| {
                    step.id == entry.step_id
                        && entry_matches_live_step_v1(entry, timeline, step_index)
                })
        });
        for entry in new_entries {
            prepared.entries.retain(|current| {
                current.project_instance_id != entry.project_instance_id
                    || current.project_id != entry.project_id
                    || current.step_id != entry.step_id
            });
            prepared.entries.push_back(entry);
            while prepared.entries.len() > MAX_TRUSTED_PATH_CERTIFICATE_REFERENCES_V1 {
                prepared.entries.pop_front();
            }
        }
        Ok(prepared)
    }

    pub(super) fn export_attestation_v1(
        &self,
        project_instance_id: ori_domain::ProjectId,
        project_id: ori_domain::ProjectId,
        timeline: &InstructionTimeline,
    ) -> Result<Option<PathCertificateExportAttestationV1>, ()> {
        let reference_count = timeline
            .steps
            .iter()
            .filter(|step| step.visual.path_certificate_reference_v1.is_some())
            .count();
        if reference_count == 0 {
            return Ok(None);
        }
        let mut certificates = Vec::new();
        certificates
            .try_reserve_exact(reference_count)
            .map_err(|_| ())?;
        for (step_index, step) in timeline.steps.iter().enumerate() {
            let Some(reference) = step.visual.path_certificate_reference_v1.as_ref() else {
                continue;
            };
            let entry = self
                .entries
                .iter()
                .find(|entry| {
                    entry.project_instance_id == project_instance_id
                        && entry.project_id == project_id
                        && entry.step_id == step.id
                        && entry.reference == *reference
                        && entry_matches_live_step_v1(entry, timeline, step_index)
                })
                .ok_or(())?;
            certificates.push(entry.certificate.as_ref());
        }
        PathCertificateExportAttestationV1::from_native_path_certificates_v1(
            timeline,
            &certificates,
        )
        .map(Some)
        .map_err(|_| ())
    }

    pub(super) fn downgrade_untrusted_references_v1(
        &self,
        project_instance_id: ori_domain::ProjectId,
        project_id: ori_domain::ProjectId,
        timeline: &mut InstructionTimeline,
    ) {
        let step_count = timeline.steps.len();
        let mut trusted_steps = Vec::new();
        if trusted_steps.try_reserve_exact(step_count).is_err() {
            for step in &mut timeline.steps {
                step.visual.path_certificate_reference_v1 = None;
                step.visual.named_technique_compiler_v1 = None;
            }
            return;
        }
        trusted_steps.resize(step_count, true);
        let mut compiler_steps_to_downgrade = Vec::new();
        if compiler_steps_to_downgrade
            .try_reserve_exact(step_count)
            .is_err()
        {
            for step in &mut timeline.steps {
                step.visual.path_certificate_reference_v1 = None;
                step.visual.named_technique_compiler_v1 = None;
            }
            return;
        }
        compiler_steps_to_downgrade.resize(step_count, false);
        let mut clear_all_compiler_provenance = false;
        for (step_index, step) in timeline.steps.iter().enumerate() {
            let Some(reference) = step.visual.path_certificate_reference_v1.as_ref() else {
                continue;
            };
            let trusted = self.entries.iter().any(|entry| {
                entry.project_instance_id == project_instance_id
                    && entry.project_id == project_id
                    && entry.step_id == step.id
                    && entry.reference == *reference
                    && entry_matches_live_step_v1(entry, timeline, step_index)
            });
            trusted_steps[step_index] = trusted;
            if !trusted {
                if let Some(metadata) = step.visual.named_technique_compiler_v1.as_ref() {
                    match step_index
                        .checked_sub(metadata.segment_index)
                        .and_then(|start| {
                            start
                                .checked_add(metadata.segment_count)
                                .map(|end| (start, end))
                        })
                        .filter(|(_, end)| *end <= step_count)
                    {
                        Some((start, end)) => {
                            compiler_steps_to_downgrade[start..end].fill(true);
                        }
                        None => clear_all_compiler_provenance = true,
                    }
                }
            }
        }
        for (step_index, step) in timeline.steps.iter_mut().enumerate() {
            if !trusted_steps[step_index] {
                step.visual.path_certificate_reference_v1 = None;
            }
            if clear_all_compiler_provenance || compiler_steps_to_downgrade[step_index] {
                step.visual.named_technique_compiler_v1 = None;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn len_v1(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use ori_collision::{
        CertifiedPathGraphSearchResultV1, CertifiedPathTransitionCandidateV1,
        certify_scheduled_cycle_transition_v1, issue_instruction_bound_single_transition_path_v1,
        private_petal_e2e_transition_fixture_v1, search_certified_pose_graph_v1,
    };
    use ori_core::Command;
    use ori_domain::{
        EdgeId, EdgeKind, InstructionHingeAngle, InstructionPose, InstructionPoseModel,
        InstructionStep, InstructionVisual, MIN_INSTRUCTION_DURATION_MS,
        PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1,
    };
    use ori_kinematics::{
        CanonicalHingeAngles, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, HingeAngle,
        MaterialHingeGraphAudit, MaterialHingeGraphGeometry, MultiHingePathCandidateLimitsV1,
        TreeKinematicsLimits, generate_linear_multi_hinge_path_candidate_v1,
    };

    use super::*;

    fn certificate_pair_fixture_v1() -> (
        CertifiedPoseGraphPathCertificateV1,
        CertifiedPoseGraphPathCertificateV1,
        FaceId,
        EdgeId,
        String,
    ) {
        let mut project = crate::initial_project_state();
        let edge = EdgeId::new();
        let boundary = project.editor.paper().boundary_vertices.clone();
        project
            .editor
            .execute(
                0,
                Command::AddEdge {
                    id: edge,
                    start: boundary[0],
                    end: boundary[2],
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("add one material hinge");
        let model_fingerprint = project.editor.fold_model_fingerprint_v1();
        let topology_analysis = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let topology = topology_analysis
            .simulation_snapshot()
            .expect("one-hinge topology");
        let fixed_face = topology.faces[0].id;
        let geometry = MaterialHingeGraphGeometry::prepare(
            project.editor.pattern(),
            project.editor.paper(),
            topology,
            TreeKinematicsLimits::default(),
        )
        .expect("one-hinge graph model");
        let audit = MaterialHingeGraphAudit::prepare(topology, TreeKinematicsLimits::default())
            .expect("one-hinge graph audit");
        let source_angles = CanonicalHingeAngles::new(vec![
            HingeAngle::new(edge, 5.0).expect("finite source angle"),
        ])
        .expect("canonical source");
        let target_angles = CanonicalHingeAngles::new(vec![
            HingeAngle::new(edge, 45.0).expect("finite target angle"),
        ])
        .expect("canonical target");
        let generated = generate_linear_multi_hinge_path_candidate_v1(
            &geometry,
            &audit,
            fixed_face,
            &source_angles,
            &target_angles,
            MultiHingePathCandidateLimitsV1::default(),
        )
        .expect("one-hinge schedule");
        let schedule_limits = CycleScheduleLimitsV1 {
            max_work: 1_048_576,
            ..CycleScheduleLimitsV1::default()
        };
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed_face,
                generated.schedule(),
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 8,
                    max_leaves: 256,
                    max_work: 1_048_576,
                    schedule_limits,
                },
            )
            .expect("one-hinge full-domain closure");
        let evidence = certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed_face, &generated, &closure, 32,
        )
        .expect("native scheduled transition");
        let source = evidence.source();
        let target = evidence.target();
        let candidate = CertifiedPathTransitionCandidateV1 {
            source,
            target,
            candidate_key: evidence.schedule_certificate(),
        };
        let raw_certificate = match search_certified_pose_graph_v1(
            &[source, target],
            &[candidate],
            source,
            target,
            |_| Some(evidence),
        ) {
            CertifiedPathGraphSearchResultV1::Certified(certificate) => certificate,
            CertifiedPathGraphSearchResultV1::Indeterminate { .. } => {
                panic!("one certified edge must produce a path")
            }
        };
        let certificate = issue_instruction_bound_single_transition_path_v1(
            &raw_certificate,
            &model_fingerprint,
            fixed_face,
            &source_angles,
            &target_angles,
        )
        .expect("instruction-bound native certificate");
        assert!(certificate.is_native_attestable_v1());
        (
            raw_certificate,
            certificate,
            fixed_face,
            edge,
            model_fingerprint,
        )
    }

    fn certificate_fixture_v1() -> (CertifiedPoseGraphPathCertificateV1, FaceId, EdgeId, String) {
        let (_, certificate, fixed_face, edge, model_fingerprint) = certificate_pair_fixture_v1();
        (certificate, fixed_face, edge, model_fingerprint)
    }

    fn reference_v1(
        certificate: &CertifiedPoseGraphPathCertificateV1,
        source_model_fingerprint: &str,
    ) -> PathCertificateReferenceV1 {
        PathCertificateReferenceV1 {
            version: 1,
            model_id: PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1.to_owned(),
            binding_sha256: certificate.binding_fingerprint_v1(),
            source_pose_sha256: certificate.source(),
            target_pose_sha256: certificate.target(),
            source_model_binding_sha256: source_model_binding_v1(source_model_fingerprint),
            transition_count: certificate.edges().len(),
        }
    }

    fn step_v1(
        source_model_fingerprint: &str,
        fixed_face: FaceId,
        edge: EdgeId,
        angle_degrees: f64,
        reference: Option<PathCertificateReferenceV1>,
    ) -> InstructionStep {
        let description = reference.as_ref().map_or_else(String::new, |reference| {
            let binding = reference
                .binding_sha256
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("経路証明 SHA-256: {binding} / 元モデル SHA-256: {source_model_fingerprint}")
        });
        InstructionStep {
            id: InstructionStepId::new(),
            title: "trusted path".to_owned(),
            description,
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual {
                path_certificate_reference_v1: reference,
                ..InstructionVisual::default()
            },
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: source_model_fingerprint.to_owned(),
                fixed_face: Some(fixed_face),
                hinge_angles: vec![InstructionHingeAngle {
                    edge,
                    angle_degrees,
                }],
            },
        }
    }

    fn timeline_fixture_v1() -> (CertifiedPoseGraphPathCertificateV1, InstructionTimeline) {
        let (certificate, fixed_face, edge, model) = certificate_fixture_v1();
        let reference = reference_v1(&certificate, &model);
        let timeline = InstructionTimeline {
            steps: vec![
                step_v1(&model, fixed_face, edge, 5.0, None),
                step_v1(&model, fixed_face, edge, 45.0, Some(reference)),
            ],
        };
        (certificate, timeline)
    }

    #[test]
    fn exact_live_reference_attests_but_foreign_and_coordinated_tamper_fail_closed_v1() {
        let instance = ori_domain::ProjectId::new();
        let project = ori_domain::ProjectId::new();
        let (certificate, timeline) = timeline_fixture_v1();
        let entries = TrustedPathCertificateRegistryV1::prepare_entries_for_timeline_suffix_v1(
            instance,
            project,
            &timeline,
            0,
            Some(&certificate),
        )
        .expect("native certificate matches exact timeline reference");
        let registry = TrustedPathCertificateRegistryV1::default()
            .with_registered_timeline_v1(instance, project, &timeline, entries)
            .expect("bounded live registry");
        assert!(
            registry
                .export_attestation_v1(instance, project, &timeline)
                .unwrap()
                .is_some()
        );
        assert!(
            registry
                .export_attestation_v1(ori_domain::ProjectId::new(), project, &timeline)
                .is_err()
        );
        assert!(
            registry
                .export_attestation_v1(instance, ori_domain::ProjectId::new(), &timeline)
                .is_err()
        );

        let mut tampered = timeline.clone();
        let tampered_reference = tampered.steps[1]
            .visual
            .path_certificate_reference_v1
            .as_mut()
            .expect("structured reference");
        tampered_reference.binding_sha256[0] ^= 1;
        let tampered_binding = tampered_reference
            .binding_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let tampered_model = tampered.steps[1].pose.source_model_fingerprint.clone();
        tampered.steps[1].description =
            format!("経路証明 SHA-256: {tampered_binding} / 元モデル SHA-256: {tampered_model}");
        let replacement_entries =
            TrustedPathCertificateRegistryV1::prepare_entries_for_timeline_suffix_v1(
                instance,
                project,
                &timeline,
                0,
                Some(&certificate),
            )
            .expect("rebuild bounded exact entries without an unchecked registry clone");
        assert!(
            registry
                .with_registered_timeline_v1(instance, project, &tampered, replacement_entries)
                .is_err(),
            "a failed coordinated-tamper registration must not produce a new registry"
        );
        assert_eq!(registry.len_v1(), 1);
        assert!(
            registry
                .export_attestation_v1(instance, project, &timeline)
                .unwrap()
                .is_some(),
            "failed registration must leave the original live registry unchanged"
        );
        assert!(
            registry
                .export_attestation_v1(instance, project, &tampered)
                .is_err()
        );
        registry.downgrade_untrusted_references_v1(instance, project, &mut tampered);
        assert!(
            tampered.steps[1]
                .visual
                .path_certificate_reference_v1
                .is_none()
        );
    }

    #[test]
    fn public_feature_fixture_is_explicitly_untrusted_at_both_boundaries_v1() {
        let (native, mut timeline) = timeline_fixture_v1();
        let source = graph_pose_fingerprint_v1(&timeline.steps[0].pose)
            .expect("bounded fixture source fingerprint");
        let target = graph_pose_fingerprint_v1(&timeline.steps[1].pose)
            .expect("bounded fixture target fingerprint");
        let candidate = CertifiedPathTransitionCandidateV1 {
            source,
            target,
            candidate_key: [0x33; 32],
        };
        let untrusted = match search_certified_pose_graph_v1(
            &[source, target],
            &[candidate],
            source,
            target,
            |edge| {
                Some(private_petal_e2e_transition_fixture_v1(
                    edge.source,
                    edge.target,
                    [0x44; 32],
                    [0x55; 32],
                    [0x66; 32],
                ))
            },
        ) {
            CertifiedPathGraphSearchResultV1::Certified(certificate) => certificate,
            CertifiedPathGraphSearchResultV1::Indeterminate { .. } => {
                panic!("fixture graph search")
            }
        };
        assert!(native.is_native_attestable_v1());
        assert!(!untrusted.is_native_attestable_v1());
        let model = timeline.steps[1].pose.source_model_fingerprint.clone();
        timeline.steps[1].visual.path_certificate_reference_v1 =
            Some(reference_v1(&untrusted, &model));
        let instance = ori_domain::ProjectId::new();
        let project = ori_domain::ProjectId::new();
        assert!(
            TrustedPathCertificateRegistryV1::prepare_entries_for_timeline_suffix_v1(
                instance,
                project,
                &timeline,
                0,
                Some(&untrusted),
            )
            .is_err()
        );
        assert!(
            PathCertificateExportAttestationV1::from_native_path_certificates_v1(
                &timeline,
                &[&untrusted],
            )
            .is_err()
        );
    }

    #[test]
    fn raw_native_graph_certificate_cannot_cross_registry_or_export_boundary_v1() {
        let (raw, bound, fixed_face, edge, model) = certificate_pair_fixture_v1();
        assert!(raw.is_native_attestable_v1());
        assert_eq!(raw.native_source_model_binding_v1(), None);
        assert!(bound.native_source_model_binding_v1().is_some());
        let raw_reference = reference_v1(&raw, &model);
        let raw_timeline = InstructionTimeline {
            steps: vec![
                step_v1(&model, fixed_face, edge, 5.0, None),
                step_v1(&model, fixed_face, edge, 45.0, Some(raw_reference)),
            ],
        };
        let instance = ori_domain::ProjectId::new();
        let project = ori_domain::ProjectId::new();
        assert!(
            TrustedPathCertificateRegistryV1::prepare_entries_for_timeline_suffix_v1(
                instance,
                project,
                &raw_timeline,
                0,
                Some(&raw),
            )
            .is_err(),
            "raw graph evidence has no instruction source-model binding"
        );
        assert!(
            PathCertificateExportAttestationV1::from_native_path_certificates_v1(
                &raw_timeline,
                &[&raw],
            )
            .is_err(),
            "formats attestation accepts only instruction-bound native evidence"
        );
    }

    #[test]
    fn coordinated_fixed_face_drift_invalidates_exact_predecessor_binding_v1() {
        let instance = ori_domain::ProjectId::new();
        let project = ori_domain::ProjectId::new();
        let (certificate, timeline) = timeline_fixture_v1();
        let entries = TrustedPathCertificateRegistryV1::prepare_entries_for_timeline_suffix_v1(
            instance,
            project,
            &timeline,
            0,
            Some(&certificate),
        )
        .expect("exact native fixed-face binding");
        let registry = TrustedPathCertificateRegistryV1::default()
            .with_registered_timeline_v1(instance, project, &timeline, entries)
            .expect("register exact native fixed-face binding");

        let mut drifted = timeline.clone();
        let foreign_fixed_face = FaceId::new();
        drifted.steps[0].pose.fixed_face = Some(foreign_fixed_face);
        drifted.steps[1].pose.fixed_face = Some(foreign_fixed_face);
        assert!(
            PathCertificateExportAttestationV1::from_native_path_certificates_v1(
                &drifted,
                &[&certificate],
            )
            .is_err(),
            "the format boundary must compare the issuer fixed face directly"
        );
        assert!(
            registry
                .export_attestation_v1(instance, project, &drifted)
                .is_err(),
            "coordinated source/target fixed-face replacement must not reuse the live attestation"
        );

        let mut angle_drifted = timeline.clone();
        angle_drifted.steps[1].pose.hinge_angles[0].angle_degrees = 46.0;
        assert!(
            PathCertificateExportAttestationV1::from_native_path_certificates_v1(
                &angle_drifted,
                &[&certificate],
            )
            .is_err(),
            "a native certificate must match the exact live instruction endpoint"
        );

        let mut model_drifted = timeline.clone();
        let foreign_model = "0".repeat(64);
        assert_ne!(
            foreign_model, model_drifted.steps[0].pose.source_model_fingerprint,
            "fixture model must differ from the coordinated replacement"
        );
        model_drifted.steps[0].pose.source_model_fingerprint = foreign_model.clone();
        model_drifted.steps[1].pose.source_model_fingerprint = foreign_model.clone();
        let fixed_face = model_drifted.steps[1]
            .pose
            .fixed_face
            .expect("fixture fixed face");
        let source_pose_sha256 = ori_instructions::instruction_pose_fingerprint_v1(
            &foreign_model,
            fixed_face,
            &model_drifted.steps[0].pose.hinge_angles,
        );
        let target_pose_sha256 = ori_instructions::instruction_pose_fingerprint_v1(
            &foreign_model,
            fixed_face,
            &model_drifted.steps[1].pose.hinge_angles,
        );
        let model_reference = model_drifted.steps[1]
            .visual
            .path_certificate_reference_v1
            .as_mut()
            .expect("fixture structured reference");
        model_reference.source_pose_sha256 = source_pose_sha256;
        model_reference.target_pose_sha256 = target_pose_sha256;
        model_reference.source_model_binding_sha256 = source_model_binding_v1(&foreign_model);
        let binding = model_reference
            .binding_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        model_drifted.steps[1].description =
            format!("経路証明 SHA-256: {binding} / 元モデル SHA-256: {foreign_model}");
        assert!(
            PathCertificateExportAttestationV1::from_native_path_certificates_v1(
                &model_drifted,
                &[&certificate],
            )
            .is_err(),
            "synchronized timeline/reference/description replacement cannot relabel the native model"
        );
        assert!(
            registry
                .export_attestation_v1(instance, project, &model_drifted)
                .is_err(),
            "the live registry must compare the issuer-retained source-model binding"
        );
        registry.downgrade_untrusted_references_v1(instance, project, &mut drifted);
        assert!(
            drifted.steps[1]
                .visual
                .path_certificate_reference_v1
                .is_none()
        );
    }

    #[test]
    fn undo_redo_timeline_visibility_preserves_only_the_live_in_process_attestation_v1() {
        let instance = ori_domain::ProjectId::new();
        let project = ori_domain::ProjectId::new();
        let (certificate, timeline) = timeline_fixture_v1();
        let entries = TrustedPathCertificateRegistryV1::prepare_entries_for_timeline_suffix_v1(
            instance,
            project,
            &timeline,
            0,
            Some(&certificate),
        )
        .unwrap();
        let registry = TrustedPathCertificateRegistryV1::default()
            .with_registered_timeline_v1(instance, project, &timeline, entries)
            .unwrap();
        assert!(
            registry
                .export_attestation_v1(instance, project, &InstructionTimeline::default())
                .unwrap()
                .is_none()
        );
        assert!(
            registry
                .export_attestation_v1(instance, project, &timeline)
                .unwrap()
                .is_some()
        );
        assert!(
            TrustedPathCertificateRegistryV1::default()
                .export_attestation_v1(instance, project, &timeline)
                .is_err(),
            "archive reopen starts with an empty non-persisted registry"
        );
    }

    #[test]
    fn registry_evicts_oldest_reference_at_the_hard_bound_v1() {
        let instance = ori_domain::ProjectId::new();
        let project = ori_domain::ProjectId::new();
        let (certificate, fixed_face, edge, model) = certificate_fixture_v1();
        let reference = reference_v1(&certificate, &model);
        let timeline = InstructionTimeline {
            steps: (0..=MAX_TRUSTED_PATH_CERTIFICATE_REFERENCES_V1)
                .flat_map(|_| {
                    [
                        step_v1(&model, fixed_face, edge, 5.0, None),
                        step_v1(&model, fixed_face, edge, 45.0, Some(reference.clone())),
                    ]
                })
                .collect(),
        };
        let entries = TrustedPathCertificateRegistryV1::prepare_entries_for_timeline_suffix_v1(
            instance,
            project,
            &timeline,
            0,
            Some(&certificate),
        )
        .unwrap();
        let registry = TrustedPathCertificateRegistryV1::default()
            .with_registered_timeline_v1(instance, project, &timeline, entries)
            .unwrap();
        assert_eq!(
            registry.len_v1(),
            MAX_TRUSTED_PATH_CERTIFICATE_REFERENCES_V1
        );
        let mut visible = timeline;
        registry.downgrade_untrusted_references_v1(instance, project, &mut visible);
        assert!(
            visible.steps[1]
                .visual
                .path_certificate_reference_v1
                .is_none()
        );
        assert!(
            visible.steps[3..]
                .iter()
                .step_by(2)
                .all(|step| step.visual.path_certificate_reference_v1.is_some())
        );
    }
}
