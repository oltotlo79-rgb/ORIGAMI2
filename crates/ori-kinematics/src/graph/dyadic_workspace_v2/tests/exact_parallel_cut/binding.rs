use super::*;

#[allow(clippy::too_many_arguments)]
fn binding(
    fixture: &ExactParallelCutFixture,
    policy: DyadicIntervalClosureWorkspaceLimitsV2,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
    partition: &[(u32, u64)],
    canonical_edges: &[EdgeId],
    exact_parallel_cut: bool,
) -> [u8; 32] {
    compute_partition_binding_with_checkpoint_v2(
        fixture.fixed_face,
        fixture.schedule.certificate_binding_fingerprint_v2(),
        fixture.schedule.graph_binding_fingerprint_v1(),
        0.0_f64.to_bits(),
        policy,
        partition,
        canonical_edges,
        resources,
        exact_parallel_cut,
        &mut || Ok(()),
    )
    .unwrap()
}

#[test]
fn binding_separates_exact_path_and_every_theorem_policy_resource_field() {
    let fixture = exact_parallel_cut_fixture();
    let generous = generous_limits(fixture.schedule_limits);
    let first = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            0.0,
            generous,
            || Ok(()),
        )
        .unwrap();
    let resources = first.resources();
    let policy = exact_limits(generous, resources);
    let exact = binding(
        &fixture,
        policy,
        resources,
        first.partition(),
        first.canonical_checked_hinges(),
        true,
    );
    let fallback = binding(
        &fixture,
        policy,
        resources,
        first.partition(),
        first.canonical_checked_hinges(),
        false,
    );
    assert_ne!(exact, fallback);

    let policy_mutations = [
        DyadicIntervalClosureWorkspaceLimitsV2 {
            max_theorem_recognizer_work: policy.max_theorem_recognizer_work.checked_add(1).unwrap(),
            ..policy
        },
        DyadicIntervalClosureWorkspaceLimitsV2 {
            max_theorem_recognizer_workspace_bytes: policy
                .max_theorem_recognizer_workspace_bytes
                .checked_add(1)
                .unwrap(),
            ..policy
        },
    ];
    for mutated in policy_mutations {
        assert_ne!(
            binding(
                &fixture,
                mutated,
                resources,
                first.partition(),
                first.canonical_checked_hinges(),
                true,
            ),
            exact
        );
    }

    let resource_mutations = [
        DyadicIntervalClosureWorkspaceResourcesV2 {
            charged_theorem_recognizer_work: resources
                .charged_theorem_recognizer_work
                .checked_add(1)
                .unwrap(),
            ..resources
        },
        DyadicIntervalClosureWorkspaceResourcesV2 {
            charged_theorem_recognizer_upper_bound_bytes: resources
                .charged_theorem_recognizer_upper_bound_bytes
                .checked_add(1)
                .unwrap(),
            ..resources
        },
    ];
    for mutated in resource_mutations {
        assert_ne!(
            binding(
                &fixture,
                policy,
                mutated,
                first.partition(),
                first.canonical_checked_hinges(),
                true,
            ),
            exact
        );
    }
}
