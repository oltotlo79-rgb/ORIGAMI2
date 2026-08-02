use super::*;

fn issue_with_checkpoint(
    fixture: &ExactParallelCutFixture,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
    checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2, DyadicIntervalClosureControlErrorV1>
{
    fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            schedule,
            tolerance,
            limits,
            checkpoint,
        )
}

fn expected_peak(resources: DyadicIntervalClosureWorkspaceResourcesV2) -> usize {
    let base = resources
        .charged_carrier_index_workspace_upper_bound_bytes
        .checked_add(resources.charged_partition_workspace_upper_bound_bytes)
        .and_then(|bytes| bytes.checked_add(resources.charged_retained_material_upper_bound_bytes))
        .unwrap();
    let proof = resources
        .charged_schedule_evaluation_workspace_upper_bound_bytes
        .checked_add(resources.charged_interval_closure_workspace_upper_bound_bytes)
        .unwrap();
    base.checked_add(
        proof
            .max(resources.charged_theorem_recognizer_upper_bound_bytes)
            .max(resources.charged_publication_workspace_upper_bound_bytes),
    )
    .unwrap()
}

#[test]
fn exact_path_honors_every_checkpoint_with_exact_stop_class() {
    let fixture = exact_parallel_cut_fixture();
    let limits = generous_limits(fixture.schedule_limits);
    let mut successful_polls = 0usize;
    issue_with_checkpoint(&fixture, &fixture.schedule, 0.0, limits, || {
        successful_polls += 1;
        Ok(())
    })
    .unwrap();
    assert!(successful_polls > 1);

    for stop in [
        DyadicIntervalClosureStopV1::Cancelled,
        DyadicIntervalClosureStopV1::DeadlineExceeded,
    ] {
        for stop_at in 1..=successful_polls {
            let mut polls = 0usize;
            let result = issue_with_checkpoint(&fixture, &fixture.schedule, 0.0, limits, || {
                polls += 1;
                if polls == stop_at { Err(stop) } else { Ok(()) }
            });
            assert_eq!(
                result.unwrap_err(),
                match stop {
                    DyadicIntervalClosureStopV1::Cancelled => {
                        DyadicIntervalClosureControlErrorV1::Cancelled
                    }
                    DyadicIntervalClosureStopV1::DeadlineExceeded => {
                        DyadicIntervalClosureControlErrorV1::DeadlineExceeded
                    }
                }
            );
        }
    }
}

#[test]
fn postallocation_not_applicable_fallback_retains_theorem_charges_and_peak() {
    let fixture = exact_parallel_cut_fixture();
    let target = fixture.moving_edges[0];
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let noncoaxial_end = Point3::new(1.0, 1.0, 0.0).unwrap();
    let geometry = exact_parallel_cut_geometry_with_mutation(
        &fixture,
        target,
        FoldAssignment::Mountain,
        origin,
        noncoaxial_end,
        axis,
    );
    let schedule = exact_parallel_cut_schedule_for_geometry_with_overrides(
        &fixture,
        &geometry,
        &fixture.moving_edges,
        None,
    );
    let recognition = recognize_exact_parallel_cut_with_checkpoint_v2(
        &geometry,
        &schedule,
        &fixture.canonical_hinge_indices,
        &fixture.canonical_edges,
        1_000_000,
        1_000_000,
        &mut || Ok(()),
    )
    .unwrap();
    assert!(matches!(
        recognition,
        ExactParallelCutRecognitionV2::NotApplicable {
            workspace_bytes: 1..,
            ..
        }
    ));

    let mut generous = generous_limits(fixture.schedule_limits);
    generous.max_depth = 2;
    generous.max_leaves = 4;
    let fallback_tolerance = 10.0;
    let material = geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            fallback_tolerance,
            generous,
            || Ok(()),
        )
        .expect("the exactly closing motion should admit the adaptive fallback");
    let resources = material.resources();
    assert!(resources.charged_theorem_recognizer_work > 0);
    assert!(resources.charged_theorem_recognizer_upper_bound_bytes > 0);
    assert!(resources.visited_partition_nodes > 1);
    assert!(resources.issued_leaves > 1);
    assert_eq!(
        resources.charged_peak_workspace_upper_bound_bytes,
        expected_peak(resources)
    );

    let mut theorem_workspace_short = generous;
    theorem_workspace_short.max_theorem_recognizer_workspace_bytes = resources
        .charged_theorem_recognizer_upper_bound_bytes
        .checked_sub(1)
        .unwrap();
    assert!(matches!(
        geometry.prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            fallback_tolerance,
            theorem_workspace_short,
            || Ok(()),
        ),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
}

#[test]
fn exact_path_peak_is_base_plus_max_phase_and_one_short_fails() {
    let fixture = exact_parallel_cut_fixture();
    let generous = generous_limits(fixture.schedule_limits);
    let material =
        issue_with_checkpoint(&fixture, &fixture.schedule, 0.0, generous, || Ok(())).unwrap();
    let resources = material.resources();
    assert_eq!(
        resources.charged_peak_workspace_upper_bound_bytes,
        expected_peak(resources)
    );
    let mut one_short = exact_limits(generous, resources);
    one_short.max_peak_workspace_bytes = resources
        .charged_peak_workspace_upper_bound_bytes
        .checked_sub(1)
        .unwrap();
    assert!(matches!(
        issue_with_checkpoint(&fixture, &fixture.schedule, 0.0, one_short, || Ok(())),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
}
