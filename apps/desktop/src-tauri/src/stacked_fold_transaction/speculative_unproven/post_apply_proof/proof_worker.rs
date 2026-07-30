fn run_attempt_v1(
    job_token: &ProjectId,
    premise: PostApplyProofPremiseV1,
    sample_intervals: usize,
    cancellation: &AtomicBool,
    proof_deadline: Instant,
) -> PostApplyProofWorkerAttemptV1 {
    #[cfg(not(test))]
    let _ = job_token;
    let control = CooperativeOperationControlV1::new(Some(cancellation), proof_deadline);
    if let Some(terminal) = cooperative_stop_terminal_v1(&control) {
        return stopped_worker_attempt_v1(premise, terminal);
    }
    let limits = StackedFoldPathDiagnosticLimitsV1 {
        sample_intervals,
        static_collision: Default::default(),
    };
    // These are deliberately independent observations. The admitted
    // layer-order diagnostic may supply a blocking witness, but only the
    // admission-free issuer can supply positive typed authority.
    let diagnostic = catch_unwind(AssertUnwindSafe(|| {
        diagnose_admitted_post_apply_path_with_control_v1(
            &premise.requested,
            premise.paper_thickness_mm,
            limits,
            &premise.initial_layer_order,
            &control,
        )
        .map_err(|_| ())
    }))
    .unwrap_or(Err(()));
    if let Some(terminal) = cooperative_stop_terminal_v1(&control) {
        return stopped_worker_attempt_v1(premise, terminal);
    }
    let native_certificate = catch_unwind(AssertUnwindSafe(|| {
        run_direct_certificate_v1(&premise, limits, &control)
    }))
    .unwrap_or(Err(
        PostApplyProofDirectCertificateErrorV1::ResourceUnavailable,
    ));
    // The issuer may have completed its final internal checkpoint just before
    // a lifecycle stop arrived. Do not start the one-shot binder in that
    // interval: it is both unnecessary after a terminal win and would make
    // the stop's responsiveness depend on binder work.
    if let Some(terminal) = cooperative_stop_terminal_v1(&control) {
        return stopped_worker_attempt_v1(premise, terminal);
    }

    #[cfg(test)]
    let premise = {
        let mut premise_owner = Some(premise);
        if let Some(certificate) =
            inject_post_apply_binder_fault_for_test_v1(job_token, &mut premise_owner)
        {
            return PostApplyProofWorkerAttemptV1 {
                diagnostic,
                certificate,
            };
        }
        premise_owner
            .take()
            .expect("an unconsumed binder-dispatch premise remains owned")
    };
    let certificate = match native_certificate {
        Ok(Some(certificate)) => {
            bind_tree_certificate_to_premise_v1(premise, certificate, &control)
        }
        // The ordinary issuer deliberately has no authority for a flat
        // initial stack. Only then may the narrow, independently typed
        // three-face/two-hinge theorem be attempted. All other shapes,
        // schedules, and positive-thickness inputs retain the prior
        // ordinary `Uncertified` result.
        Ok(None) if is_layered_three_face_fallback_candidate_v1(&premise) => {
            run_layered_three_face_fallback_v1(premise, &control)
        }
        Ok(None) if is_layered_four_face_fallback_candidate_v1(&premise) => {
            run_layered_four_face_fallback_v1(premise, &control)
        }
        Ok(None) => PostApplyProofWorkerCertificateV1::Uncertified(premise),
        Err(PostApplyProofDirectCertificateErrorV1::BindingRejected) => {
            PostApplyProofWorkerCertificateV1::BindingRejected(premise)
        }
        Err(PostApplyProofDirectCertificateErrorV1::ResourceUnavailable) => {
            PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
        }
        Err(PostApplyProofDirectCertificateErrorV1::Cancelled) => {
            PostApplyProofWorkerCertificateV1::Cancelled(premise)
        }
        Err(PostApplyProofDirectCertificateErrorV1::DeadlineExceeded) => {
            PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
        }
    };
    PostApplyProofWorkerAttemptV1 {
        diagnostic,
        certificate,
    }
}

fn stopped_worker_attempt_v1(
    premise: PostApplyProofPremiseV1,
    terminal: PostApplyProofTerminalV1,
) -> PostApplyProofWorkerAttemptV1 {
    let certificate = match terminal {
        PostApplyProofTerminalV1::UnknownCancelled => {
            PostApplyProofWorkerCertificateV1::Cancelled(premise)
        }
        PostApplyProofTerminalV1::UnknownDeadlineReached => {
            PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
        }
        _ => unreachable!("cooperative control only stops as cancelled or deadline"),
    };
    PostApplyProofWorkerAttemptV1 {
        diagnostic: Err(()),
        certificate,
    }
}

fn diagnose_admitted_post_apply_path_with_control_v1(
    requested: &PreparedStackedFoldRequestedPoseV1,
    paper_thickness_mm: f64,
    limits: StackedFoldPathDiagnosticLimitsV1,
    initial_layer_order: &StackedFoldInitialLayerOrderV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    cooperative_path_checkpoint_v1(control)?;
    let initial = requested.initial();
    let source_angles = initial.pose().hinge_angles();
    let target_angles = requested.pose().hinge_angles();
    if source_angles.len() != target_angles.len()
        || source_angles
            .iter()
            .zip(target_angles)
            .any(|(source, target)| source.edge() != target.edge())
    {
        return Err(StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch);
    }
    let moving_hinges = source_angles
        .iter()
        .zip(target_angles)
        .filter_map(|(source, target)| {
            (source.angle_degrees().to_bits() != target.angle_degrees().to_bits())
                .then_some(source.edge())
        })
        .collect::<Vec<_>>();
    if moving_hinges.is_empty()
        || source_angles
            .iter()
            .zip(target_angles)
            .filter(|(source, target)| {
                source.angle_degrees().to_bits() != target.angle_degrees().to_bits()
            })
            .any(|(_, target)| {
                target.angle_degrees().to_bits() != requested.requested_angle_degrees().to_bits()
            })
    {
        return Err(StackedFoldPathDiagnosticErrorV1::InvalidPath);
    }
    let admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        initial.target().model(),
        initial.pose(),
        paper_thickness_mm,
        limits.static_collision,
        initial_layer_order,
        control,
    )?;
    cooperative_path_checkpoint_v1(control)?;
    diagnose_collective_hinge_path_with_initial_sample_layer_admission_with_control_v1(
        initial.target().model(),
        initial.pose(),
        &moving_hinges,
        requested.requested_angle_degrees(),
        paper_thickness_mm,
        limits,
        &admission,
        control,
    )
}

fn cooperative_path_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), StackedFoldPathDiagnosticErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => StackedFoldPathDiagnosticErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            StackedFoldPathDiagnosticErrorV1::DeadlineExceeded
        }
    })
}

fn cooperative_stop_terminal_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Option<PostApplyProofTerminalV1> {
    match control.checkpoint() {
        Ok(()) => None,
        Err(CooperativeOperationStopV1::Cancelled) => {
            Some(PostApplyProofTerminalV1::UnknownCancelled)
        }
        Err(CooperativeOperationStopV1::DeadlineExceeded) => {
            Some(PostApplyProofTerminalV1::UnknownDeadlineReached)
        }
    }
}

#[cfg(test)]
fn take_post_apply_worker_panic_for_test_v1(job_token: &ProjectId) -> bool {
    take_process_global_one_shot_fault_if_v1(&PANIC_NEXT_POST_APPLY_WORKER_V1, |target| {
        target == job_token
    })
    .is_some()
}

fn inject_post_apply_worker_panic_for_test_v1(job_token: &ProjectId) {
    #[cfg(not(test))]
    let _ = job_token;
    #[cfg(test)]
    if take_post_apply_worker_panic_for_test_v1(job_token) {
        panic!("injected post-Apply proof worker panic");
    }
}

fn bind_tree_certificate_to_premise_v1(
    premise: PostApplyProofPremiseV1,
    certificate: StackedFoldTreeContinuousCertificateV1,
    control: &CooperativeOperationControlV1<'_>,
) -> PostApplyProofWorkerCertificateV1 {
    let PostApplyProofPremiseV1 {
        resolution_ticket,
        binding,
        requested,
        initial_layer_order,
        target_revision,
        target_fingerprint,
        target_pose_generation,
        paper_thickness_mm,
    } = premise;
    match bind_speculative_unproven_tree_continuous_proof_with_control_v1(
        resolution_ticket,
        &requested,
        certificate,
        control,
    ) {
        Ok(proof) => PostApplyProofWorkerCertificateV1::Certified(
            PostApplyProofCertifiedAuthorityV1::Tree(proof),
        ),
        Err(failure) => {
            let error = failure.error().clone();
            let (_, resolution_ticket, recovered_certificate) = failure.into_parts();
            // The native certificate can be deterministically reissued at the
            // next bounded stage. Retaining it would add an uncharged pair of
            // hinge-angle allocations to the post-Apply registry; the
            // one-shot resolution ticket, by contrast, must never be dropped.
            drop(recovered_certificate);
            let premise = PostApplyProofPremiseV1 {
                resolution_ticket,
                binding,
                requested,
                initial_layer_order,
                target_revision,
                target_fingerprint,
                target_pose_generation,
                paper_thickness_mm,
            };
            match error {
                SpeculativeUnprovenFoldCertificationErrorV1::Cancelled => {
                    PostApplyProofWorkerCertificateV1::Cancelled(premise)
                }
                SpeculativeUnprovenFoldCertificationErrorV1::DeadlineExceeded => {
                    PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
                }
                SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed
                | SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked
                | SpeculativeUnprovenFoldCertificationErrorV1::ResourceUnavailable => {
                    PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
                }
                _ => PostApplyProofWorkerCertificateV1::BindingRejected(premise),
            }
        }
    }
}

#[cfg(test)]
fn inject_post_apply_binder_fault_for_test_v1(
    job_token: &ProjectId,
    premise_owner: &mut Option<PostApplyProofPremiseV1>,
) -> Option<PostApplyProofWorkerCertificateV1> {
    match take_injected_post_apply_binder_fault_v1(job_token) {
        Some(InjectedPostApplyBinderFaultV1::Allocation) => {
            let premise = premise_owner
                .take()
                .expect("the binder allocation fault owns its exact premise");
            Some(PostApplyProofWorkerCertificateV1::ResourceUnavailable(
                premise,
            ))
        }
        Some(InjectedPostApplyBinderFaultV1::ValidationPanic) => {
            // Catch the real injected unwind at the common binder-dispatch
            // boundary, before selecting the tree or layered binder and
            // before moving the one-shot premise into either implementation.
            let injected: Result<(), _> = catch_unwind(AssertUnwindSafe(|| {
                panic!("injected post-Apply binder validation panic");
            }));
            assert!(
                injected.is_err(),
                "the injected binder validation panic must unwind"
            );
            let _previous =
                OBSERVED_POST_APPLY_BINDER_VALIDATION_PANICS_V1.fetch_add(1, Ordering::AcqRel);
            let premise = premise_owner
                .take()
                .expect("the binder validation fault owns its exact premise");
            Some(PostApplyProofWorkerCertificateV1::ResourceUnavailable(
                premise,
            ))
        }
        None => None,
    }
}

#[cfg(test)]
fn take_injected_post_apply_binder_fault_v1(
    job_token: &ProjectId,
) -> Option<InjectedPostApplyBinderFaultV1> {
    take_process_global_one_shot_fault_if_v1(&NEXT_POST_APPLY_BINDER_FAULT_V1, |target| {
        target.job_token == *job_token
    })
    .map(|target| target.fault)
}

#[cfg(test)]
fn observed_post_apply_binder_validation_panics_for_test_v1() -> usize {
    OBSERVED_POST_APPLY_BINDER_VALIDATION_PANICS_V1.load(Ordering::Acquire)
}

fn run_direct_certificate_v1(
    premise: &PostApplyProofPremiseV1,
    limits: StackedFoldPathDiagnosticLimitsV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<Option<StackedFoldTreeContinuousCertificateV1>, PostApplyProofDirectCertificateErrorV1>
{
    let target_angles = target_angles_for_premise_v1(premise, control)?;
    cooperative_path_checkpoint_v1(control).map_err(classify_direct_certificate_error_v1)?;
    certify_tree_continuous_path_from_pose_with_control_v1(
        premise.requested.initial().target().model(),
        premise.requested.initial().pose(),
        &target_angles,
        premise.paper_thickness_mm,
        limits,
        control,
    )
    .map_err(classify_direct_certificate_error_v1)
}

fn classify_direct_certificate_error_v1(
    error: StackedFoldPathDiagnosticErrorV1,
) -> PostApplyProofDirectCertificateErrorV1 {
    match error {
        StackedFoldPathDiagnosticErrorV1::InvalidLimits
        | StackedFoldPathDiagnosticErrorV1::InvalidPath
        | StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch => {
            PostApplyProofDirectCertificateErrorV1::BindingRejected
        }
        StackedFoldPathDiagnosticErrorV1::PoseUnavailable
        | StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable
        | StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable
        | StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult
        | StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable
        | StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit => {
            PostApplyProofDirectCertificateErrorV1::ResourceUnavailable
        }
        StackedFoldPathDiagnosticErrorV1::Cancelled => {
            PostApplyProofDirectCertificateErrorV1::Cancelled
        }
        StackedFoldPathDiagnosticErrorV1::DeadlineExceeded => {
            PostApplyProofDirectCertificateErrorV1::DeadlineExceeded
        }
    }
}
