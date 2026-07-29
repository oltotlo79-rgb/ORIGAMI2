use ori_collision::{
    CooperativeOperationControlV1, StackedFoldPathDiagnosticErrorV1,
    StackedFoldPathDiagnosticLimitsV1, certify_tree_continuous_path_from_pose_v1,
};
use ori_domain::{EdgeKind, ProjectId};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, analyze_global_flat_foldability,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces, analyze_local_flat_foldability};

use crate::{
    AppliedPoseLimitsV1, FaceLineageLimits, PreparedStackedFoldRequestedPoseV1,
    SpeculativeApproximateBlockingObservationV1, StackedFoldGeometryLimitsV1,
    StackedFoldTopologyBuildLimitsV1, create_rectangular_sheet, prepare_applied_pose_v1,
    stacked_fold::{
        ExpectedStackedFoldCreaseV1, prepare_stacked_fold_geometry_candidate_v1,
        prepare_stacked_fold_initial_pose_v1, prepare_stacked_fold_requested_pose_v1,
        prepare_stacked_fold_target_model_v1,
    },
};

use super::*;

mod layered;

struct CertificationFixture {
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    requested: PreparedStackedFoldRequestedPoseV1,
    same_semantics_other_requested: PreparedStackedFoldRequestedPoseV1,
    certificate: StackedFoldTreeContinuousCertificateV1,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolutionTicketSnapshotV1 {
    editor_instance_anchor: *const (),
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: Revision,
    target_geometry_fingerprint: [u8; 32],
    target_applied_pose: AppliedPoseV1,
}

impl ResolutionTicketSnapshotV1 {
    fn capture(ticket: &SpeculativeUnprovenFoldResolutionTicketV1) -> Self {
        Self {
            editor_instance_anchor: Arc::as_ptr(&ticket.editor_instance_anchor),
            binding: ticket.binding.clone(),
            target_revision: ticket.target_revision,
            target_geometry_fingerprint: ticket.target_geometry_fingerprint,
            target_applied_pose: ticket.target_applied_pose.clone(),
        }
    }
}

struct CertificationTestFaultGuardV1;

impl CertificationTestFaultGuardV1 {
    fn set(fault: CertificationTestFaultV1) -> Self {
        CERTIFICATION_TEST_FAULT_V1.with(|active| {
            assert_eq!(active.replace(Some(fault)), None);
        });
        Self
    }
}

impl Drop for CertificationTestFaultGuardV1 {
    fn drop(&mut self) {
        CERTIFICATION_TEST_FAULT_V1.with(|active| active.set(None));
    }
}

fn assert_recoverable_failure_v1(
    result: Result<
        SpeculativeUnprovenFoldCertifiedProofV1,
        SpeculativeUnprovenFoldCertificationFailureV1,
    >,
    expected_error: SpeculativeUnprovenFoldCertificationErrorV1,
    expected_ticket: &ResolutionTicketSnapshotV1,
) -> (
    SpeculativeUnprovenFoldResolutionTicketV1,
    StackedFoldTreeContinuousCertificateV1,
) {
    let failure = result.expect_err("certification binding must fail closed");
    assert_eq!(failure.error(), &expected_error);
    let (error, ticket, certificate) = failure.into_parts();
    assert_eq!(error, expected_error);
    let returned_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
    assert_eq!(&returned_ticket, expected_ticket);
    (ticket, certificate)
}

fn certification_fixture(angle_degrees: f64) -> CertificationFixture {
    let identity = ProjectId::new();
    let source_revision = 0;
    let sheet = create_rectangular_sheet(80.0, 60.0, false).expect("rectangular sheet");
    let (source_pattern, mut source_paper) = sheet.into_parts();
    // This core fixture has no explicit positive-thickness relief. Its
    // post-Apply binder contract is exercised at exact zero thickness;
    // ori-collision separately covers its relief-aware 0.1 mm issuer.
    source_paper.thickness_mm = 0.0;
    let source_topology = analyze_faces(FaceExtractionInput {
        identity_namespace: identity,
        source_revision,
        paper: &source_paper,
        pattern: &source_pattern,
    })
    .snapshot
    .expect("source topology");
    let local = analyze_local_flat_foldability(&source_paper, &source_pattern);
    let global = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            identity,
            &source_paper,
            &source_pattern,
            &source_topology,
            &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("global source proof");
    let source_layer_order = global.layer_order().expect("source layer order").clone();
    let start = source_pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == source_paper.boundary_vertices[0])
        .expect("first corner")
        .position;
    let end = source_pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == source_paper.boundary_vertices[2])
        .expect("opposite corner")
        .position;
    let geometry = prepare_stacked_fold_geometry_candidate_v1(
        identity,
        source_revision,
        &source_pattern,
        &source_paper,
        &source_layer_order,
        &[ExpectedStackedFoldCreaseV1 {
            start,
            end,
            kind: EdgeKind::Mountain,
        }],
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("prepared target geometry");
    let same_semantics_other_geometry = geometry.clone();
    let target = prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
        .expect("target tree model");
    let same_semantics_other_target = prepare_stacked_fold_target_model_v1(
        same_semantics_other_geometry,
        TreeKinematicsLimits::default(),
    )
    .expect("independently re-prepared target tree model");
    let source_model = MaterialTreeKinematicsModel::prepare(
        &source_pattern,
        &source_paper,
        &source_topology,
        TreeKinematicsLimits::default(),
    )
    .expect("source tree model");
    let source_pose = source_model
        .solve(
            None,
            &CanonicalHingeAngles::new(Vec::new()).expect("empty source angles"),
        )
        .expect("source pose");
    let initial = prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
        .expect("lift source pose");
    let same_semantics_other_initial = prepare_stacked_fold_initial_pose_v1(
        same_semantics_other_target,
        &source_model,
        &source_pose,
    )
    .expect("independently lift the same source pose");
    let requested = prepare_stacked_fold_requested_pose_v1(initial, angle_degrees)
        .expect("requested target pose");
    let same_semantics_other_requested =
        prepare_stacked_fold_requested_pose_v1(same_semantics_other_initial, angle_degrees)
            .expect("independently re-prepare the same requested target pose");
    let requested_angles = CanonicalHingeAngles::new(requested.pose().hinge_angles().to_vec())
        .expect("canonical requested angles");
    let certificate = certify_tree_continuous_path_from_pose_v1(
        requested.initial().target().model(),
        requested.initial().pose(),
        &requested_angles,
        source_paper.thickness_mm,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("continuous diagnosis")
    .expect("simple one-hinge path is continuously certified");

    let lineage = requested.initial().target().geometry().proof().lineage();
    let binding = SpeculativeUnprovenFoldBindingV1::new(
        ProjectId::new(),
        identity,
        source_revision,
        lineage.source_fingerprint().to_hex(),
        1,
        ProjectId::new(),
        source_paper.thickness_mm,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("binding");
    let hinge_ids = requested
        .pose()
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let hinge_angles = requested
        .pose()
        .hinge_angles()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    let target_applied_pose = prepare_applied_pose_v1(
        requested.pose().face_ids(),
        &hinge_ids,
        requested.pose().fixed_face(),
        &hinge_angles,
        AppliedPoseLimitsV1::default(),
    )
    .expect("target semantic pose");
    let ticket = SpeculativeUnprovenFoldResolutionTicketV1::new(
        Arc::new(()),
        binding,
        lineage.target_revision(),
        lineage.target_fingerprint().0,
        target_applied_pose,
        Some(PreparedStackedFoldRequestIssuerSealV1::capture(&requested)),
    );
    CertificationFixture {
        ticket,
        requested,
        same_semantics_other_requested,
        certificate,
    }
}

#[test]
fn exact_ticket_request_and_certificate_mint_one_typed_proof() {
    let fixture = certification_fixture(37.0);
    bind_speculative_unproven_tree_continuous_proof_v1(
        fixture.ticket,
        &fixture.requested,
        fixture.certificate,
    )
    .expect("exact post-Apply certification binding");
}

#[test]
fn tree_ticket_rejects_same_semantics_prepared_request_aba_and_remains_exactly_retryable() {
    let CertificationFixture {
        ticket,
        requested,
        same_semantics_other_requested,
        certificate,
    } = certification_fixture(37.0);
    let owner_model = requested.initial().target().model();
    let other_model = same_semantics_other_requested.initial().target().model();
    assert_eq!(
        requested.initial().target().geometry(),
        same_semantics_other_requested.initial().target().geometry()
    );
    assert_eq!(
        requested.pose().hinge_angles(),
        same_semantics_other_requested.pose().hinge_angles()
    );
    assert!(
        !owner_model.owns_pose(same_semantics_other_requested.initial().pose())
            && !owner_model.owns_pose(same_semantics_other_requested.pose())
            && !other_model.owns_pose(requested.initial().pose())
            && !other_model.owns_pose(requested.pose()),
        "the ABA request must be semantically equal but independently issued"
    );

    let other_target_angles = CanonicalHingeAngles::new(
        same_semantics_other_requested
            .pose()
            .hinge_angles()
            .to_vec(),
    )
    .expect("canonical independently prepared target angles");
    let other_certificate = certify_tree_continuous_path_from_pose_v1(
        other_model,
        same_semantics_other_requested.initial().pose(),
        &other_target_angles,
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("independent continuous diagnosis")
    .expect("the semantically equal independent path is continuously certified");
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, other_certificate) = assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            ticket,
            &same_semantics_other_requested,
            other_certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::PreparedRequestIssuerMismatch,
        &snapshot,
    );
    assert!(
        other_certificate.is_for(
            other_model,
            same_semantics_other_requested.initial().pose(),
            &other_target_angles,
            0.0,
        ),
        "the rejected independently issued certificate must be returned unchanged"
    );
    bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        .expect("the recovered owner ticket remains exactly retryable with its owner request");
}

#[test]
fn controlled_pre_cancelled_binding_recovers_inputs_and_allows_retry() {
    let CertificationFixture {
        ticket,
        requested,
        certificate,
        ..
    } = certification_fixture(37.0);
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        std::time::Instant::now() + std::time::Duration::from_secs(1),
    );

    let (ticket, certificate) = assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_with_control_v1(
            ticket,
            &requested,
            certificate,
            &control,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::Cancelled,
        &expected_ticket,
    );
    bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        .expect("a cancelled binding must not consume its retry authority");
}

#[test]
fn controlled_expired_deadline_recovers_inputs_and_allows_retry() {
    let CertificationFixture {
        ticket,
        requested,
        certificate,
        ..
    } = certification_fixture(37.0);
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
    let control = CooperativeOperationControlV1::new(None, std::time::Instant::now());

    let (ticket, certificate) = assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_with_control_v1(
            ticket,
            &requested,
            certificate,
            &control,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::DeadlineExceeded,
        &expected_ticket,
    );
    bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        .expect("an expired binding must not consume its retry authority");
}

#[test]
fn late_tree_cancellation_after_native_revalidation_recovers_inputs_and_allows_retry() {
    let CertificationFixture {
        ticket,
        requested,
        certificate,
        ..
    } = certification_fixture(37.0);
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
    let result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::LateOwnershipCancellation);
        bind_speculative_unproven_tree_continuous_proof_with_control_v1(
            ticket,
            &requested,
            certificate,
            &CooperativeOperationControlV1::unbounded(),
        )
    };

    let (ticket, certificate) = assert_recoverable_failure_v1(
        result,
        SpeculativeUnprovenFoldCertificationErrorV1::Cancelled,
        &expected_ticket,
    );
    bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        .expect("late cancellation must return the exact retry authority");
}

#[test]
fn continuous_certificate_stop_and_resource_reasons_map_to_certification_errors() {
    assert_eq!(
        map_continuous_certificate_revalidation_error_v1(
            StackedFoldPathDiagnosticErrorV1::Cancelled
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::Cancelled
    );
    assert_eq!(
        map_continuous_certificate_revalidation_error_v1(
            StackedFoldPathDiagnosticErrorV1::DeadlineExceeded
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::DeadlineExceeded
    );
    for error in [
        StackedFoldPathDiagnosticErrorV1::PoseUnavailable,
        StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable,
        StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable,
        StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult,
        StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable,
        StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit,
    ] {
        assert_eq!(
            map_continuous_certificate_revalidation_error_v1(error),
            SpeculativeUnprovenFoldCertificationErrorV1::ResourceUnavailable
        );
    }
}

#[test]
fn requested_angle_allocation_failure_returns_unchanged_inputs_for_retry() {
    let CertificationFixture {
        ticket,
        requested,
        certificate,
        ..
    } = certification_fixture(37.0);
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
    let result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::TargetAngleAllocation);
        bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
    };
    let (ticket, certificate) = assert_recoverable_failure_v1(
        result,
        SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed,
        &expected_ticket,
    );
    bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        .expect("the exact returned ticket and certificate remain retryable");
}

#[test]
fn validation_panic_is_caught_before_one_shot_inputs_are_consumed() {
    let CertificationFixture {
        ticket,
        requested,
        certificate,
        ..
    } = certification_fixture(37.0);
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
    let result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::NativeRevalidationPanic);
        bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
    };
    let (ticket, certificate) = assert_recoverable_failure_v1(
        result,
        SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked,
        &expected_ticket,
    );
    bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        .expect("the internally caught panic must leave both inputs retryable");
}

#[test]
fn source_fingerprint_comparison_is_exact_and_allocation_free() {
    let bytes = [0xa5; 32];
    assert!(lowercase_sha256_matches_v1(bytes, &"a5".repeat(32)));
    assert!(!lowercase_sha256_matches_v1(bytes, &"A5".repeat(32)));
    assert!(!lowercase_sha256_matches_v1(bytes, &"a5".repeat(31)));
    assert!(!lowercase_sha256_matches_v1([0x5a; 32], &"a5".repeat(32)));
}

#[test]
fn ticket_and_certificate_drift_fail_closed() {
    let mut source_revision = certification_fixture(37.0);
    let original = &source_revision.ticket.binding;
    source_revision.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
        original.project_instance_id(),
        original.project_id(),
        original.source_revision() + 1,
        original.source_geometry_fingerprint_sha256().to_owned(),
        original.pose_generation(),
        original.request_generation_id(),
        f64::from_bits(original.paper_thickness_bits()),
        original.approximate_blocking_observation(),
    )
    .expect("well-formed source-revision drift");
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&source_revision.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            source_revision.ticket,
            &source_revision.requested,
            source_revision.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::SourceRevisionMismatch,
        &expected_ticket,
    );

    let mut revision = certification_fixture(37.0);
    revision.ticket.target_revision += 1;
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&revision.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            revision.ticket,
            &revision.requested,
            revision.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::TargetRevisionMismatch,
        &expected_ticket,
    );

    let mut fingerprint = certification_fixture(37.0);
    fingerprint.ticket.target_geometry_fingerprint[0] ^= 1;
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&fingerprint.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            fingerprint.ticket,
            &fingerprint.requested,
            fingerprint.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::TargetGeometryFingerprintMismatch,
        &expected_ticket,
    );

    let mut lineage = certification_fixture(37.0);
    let original = &lineage.ticket.binding;
    lineage.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
        original.project_instance_id(),
        ProjectId::new(),
        original.source_revision(),
        original.source_geometry_fingerprint_sha256().to_owned(),
        original.pose_generation(),
        original.request_generation_id(),
        f64::from_bits(original.paper_thickness_bits()),
        original.approximate_blocking_observation(),
    )
    .expect("well-formed foreign project binding");
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&lineage.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            lineage.ticket,
            &lineage.requested,
            lineage.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::ProjectLineageMismatch,
        &expected_ticket,
    );

    let mut source_fingerprint = certification_fixture(37.0);
    let original = &source_fingerprint.ticket.binding;
    source_fingerprint.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
        original.project_instance_id(),
        original.project_id(),
        original.source_revision(),
        "00".repeat(32),
        original.pose_generation(),
        original.request_generation_id(),
        f64::from_bits(original.paper_thickness_bits()),
        original.approximate_blocking_observation(),
    )
    .expect("well-formed source-fingerprint drift");
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&source_fingerprint.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            source_fingerprint.ticket,
            &source_fingerprint.requested,
            source_fingerprint.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::SourceGeometryFingerprintMismatch,
        &expected_ticket,
    );

    let mut thickness = certification_fixture(37.0);
    let original = &thickness.ticket.binding;
    thickness.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
        original.project_instance_id(),
        original.project_id(),
        original.source_revision(),
        original.source_geometry_fingerprint_sha256().to_owned(),
        original.pose_generation(),
        original.request_generation_id(),
        f64::from_bits(original.paper_thickness_bits() + 1),
        original.approximate_blocking_observation(),
    )
    .expect("well-formed thickness-drifted binding");
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&thickness.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            thickness.ticket,
            &thickness.requested,
            thickness.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::PaperThicknessBitsMismatch,
        &expected_ticket,
    );

    let mut observation = certification_fixture(37.0);
    let original = &observation.ticket.binding;
    observation.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
        original.project_instance_id(),
        original.project_id(),
        original.source_revision(),
        original.source_geometry_fingerprint_sha256().to_owned(),
        original.pose_generation(),
        original.request_generation_id(),
        f64::from_bits(original.paper_thickness_bits()),
        SpeculativeApproximateBlockingObservationV1::blocking_sample_observed(12.5)
            .expect("valid blocking observation"),
    )
    .expect("well-formed blocking-observation drift");
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&observation.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            observation.ticket,
            &observation.requested,
            observation.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::ApproximateBlockingObservationMismatch,
        &expected_ticket,
    );

    let mut semantic_pose = certification_fixture(37.0);
    let native = semantic_pose.requested.pose();
    let hinge_ids = native
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let changed_angles = native
        .hinge_angles()
        .iter()
        .map(|angle| {
            (
                angle.edge(),
                f64::from_bits(angle.angle_degrees().to_bits() + 1),
            )
        })
        .collect::<Vec<_>>();
    semantic_pose.ticket.target_applied_pose = prepare_applied_pose_v1(
        native.face_ids(),
        &hinge_ids,
        native.fixed_face(),
        &changed_angles,
        AppliedPoseLimitsV1::default(),
    )
    .expect("different but valid semantic pose");
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&semantic_pose.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            semantic_pose.ticket,
            &semantic_pose.requested,
            semantic_pose.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::TargetAppliedPoseMismatch,
        &expected_ticket,
    );

    let certificate = certification_fixture(37.0);
    let wrong_angles = CanonicalHingeAngles::new(
        certificate
            .requested
            .pose()
            .hinge_angles()
            .iter()
            .map(|angle| {
                HingeAngle::new(angle.edge(), angle.angle_degrees() + 1.0)
                    .expect("different target angle")
            })
            .collect(),
    )
    .expect("canonical different target angles");
    let wrong_certificate = certify_tree_continuous_path_from_pose_v1(
        certificate.requested.initial().target().model(),
        certificate.requested.initial().pose(),
        &wrong_angles,
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("different continuous diagnosis")
    .expect("different simple path is certified");
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&certificate.ticket);
    let failure = bind_speculative_unproven_tree_continuous_proof_v1(
        certificate.ticket,
        &certificate.requested,
        wrong_certificate,
    )
    .expect_err("the wrong target certificate must be rejected");
    assert_eq!(
        failure.error(),
        &SpeculativeUnprovenFoldCertificationErrorV1::ContinuousCertificateMismatch
    );
    let returned_ticket = failure.into_ticket();
    assert_eq!(
        ResolutionTicketSnapshotV1::capture(&returned_ticket),
        expected_ticket
    );
    bind_speculative_unproven_tree_continuous_proof_v1(
        returned_ticket,
        &certificate.requested,
        certificate.certificate,
    )
    .expect("the unchanged ticket can be retried with a new exact certificate");

    let exact = certification_fixture(37.0);
    let foreign = certification_fixture(37.0);
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&exact.ticket);
    assert_recoverable_failure_v1(
        bind_speculative_unproven_tree_continuous_proof_v1(
            exact.ticket,
            &exact.requested,
            foreign.certificate,
        ),
        SpeculativeUnprovenFoldCertificationErrorV1::ContinuousCertificateMismatch,
        &expected_ticket,
    );

    let exact = certification_fixture(37.0);
    let foreign = certification_fixture(37.0);
    let expected_ticket = ResolutionTicketSnapshotV1::capture(&exact.ticket);
    let failure = bind_speculative_unproven_tree_continuous_proof_v1(
        exact.ticket,
        &foreign.requested,
        exact.certificate,
    )
    .expect_err("a foreign prepared request must be rejected");
    assert_eq!(
        failure.error(),
        &SpeculativeUnprovenFoldCertificationErrorV1::PreparedRequestIssuerMismatch
    );
    let (_, ticket, _) = failure.into_parts();
    assert_eq!(
        ResolutionTicketSnapshotV1::capture(&ticket),
        expected_ticket
    );
}
