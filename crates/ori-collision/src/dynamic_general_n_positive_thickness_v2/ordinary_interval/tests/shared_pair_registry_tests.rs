use super::super::*;
use super::support::{N33, N34, n33_fixture_v2, n34_fixture_v2, strict_limits_v2};

#[test]
fn n33_n34_derived_shared_pair_registry_matches_existing_inventory() {
    for (fixture, expected_pair_count) in [(n33_fixture_v2(), 724), (n34_fixture_v2(), 746)] {
        let limits = strict_limits_v2(fixture);
        let derived = derive_v2(
            &fixture.fixture.geometry,
            limits.max_excluded_shared_pairs,
            limits.max_shared_feature_membership_tests,
            || Ok(()),
        )
        .expect("strict N33/N34 shared-pair registry");
        assert_eq!(derived, fixture.excluded_shared_pairs);
        assert_eq!(derived.len(), expected_pair_count);
        let mut no_stop = || Ok(());
        assert_eq!(
            super::super::geometry::validate_exact_shared_pair_registry_v2(
                &fixture.fixture.geometry,
                &derived,
                limits.max_shared_feature_membership_tests,
                &mut no_stop,
            )
            .expect("derived registry remains admitted"),
            super::super::geometry::validate_exact_shared_pair_registry_v2(
                &fixture.fixture.geometry,
                &fixture.excluded_shared_pairs,
                limits.max_shared_feature_membership_tests,
                &mut no_stop,
            )
            .expect("existing registry remains admitted"),
        );
    }
    assert_eq!(
        n33_fixture_v2().fixture.profile.actual_block_count_v2(),
        N33
    );
    assert_eq!(
        n34_fixture_v2().fixture.profile.actual_block_count_v2(),
        N34
    );
}

#[test]
fn shared_pair_registry_exact_combined_membership_cap_and_one_short_fail_closed() {
    let fixture = n33_fixture_v2();
    let limits = strict_limits_v2(fixture);
    let boundary_occurrences = fixture
        .fixture
        .geometry
        .face_ids()
        .iter()
        .try_fold(0usize, |total, face| {
            total.checked_add(
                fixture
                    .fixture
                    .geometry
                    .face_boundary_vertices(*face)
                    .expect("fixture boundary")
                    .len(),
            )
        })
        .expect("fixture boundary occurrences");
    let exact_combined_membership_cap = boundary_occurrences
        .checked_mul(
            boundary_occurrences
                .checked_sub(1)
                .expect("nonempty fixture"),
        )
        .map(|value| value / 2)
        .expect("fixture combined membership cap");

    assert_eq!(
        derive_v2(
            &fixture.fixture.geometry,
            limits.max_excluded_shared_pairs,
            exact_combined_membership_cap,
            || Ok(()),
        )
        .expect("exact combined cap"),
        fixture.excluded_shared_pairs,
    );
    assert_eq!(
        derive_v2(
            &fixture.fixture.geometry,
            limits.max_excluded_shared_pairs,
            exact_combined_membership_cap - 1,
            || Ok(()),
        )
        .unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit,
    );
    assert_eq!(
        derive_v2(
            &fixture.fixture.geometry,
            limits.max_excluded_shared_pairs - 1,
            exact_combined_membership_cap,
            || Ok(()),
        )
        .unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit,
    );
}

#[test]
fn shared_pair_registry_propagates_cancellation_and_deadline() {
    let fixture = n33_fixture_v2();
    let limits = strict_limits_v2(fixture);
    assert_eq!(
        derive_v2(
            &fixture.fixture.geometry,
            limits.max_excluded_shared_pairs,
            limits.max_shared_feature_membership_tests,
            || Err(OrdinaryIntervalStopV2::Cancelled),
        )
        .unwrap_err(),
        OrdinaryIntervalErrorV2::Cancelled,
    );
    assert_eq!(
        derive_v2(
            &fixture.fixture.geometry,
            limits.max_excluded_shared_pairs,
            limits.max_shared_feature_membership_tests,
            || Err(OrdinaryIntervalStopV2::DeadlineExceeded),
        )
        .unwrap_err(),
        OrdinaryIntervalErrorV2::DeadlineExceeded,
    );
}

fn derive_v2(
    geometry: &MaterialHingeGraphGeometry,
    pair_cap: usize,
    membership_test_cap: usize,
    checkpoint: impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<Vec<OrdinaryIntervalFacePairV2>, OrdinaryIntervalErrorV2> {
    let mut checkpoint = checkpoint;
    super::super::geometry::derive_exact_shared_pair_registry_v2(
        geometry,
        pair_cap,
        membership_test_cap,
        &mut checkpoint,
    )
}
