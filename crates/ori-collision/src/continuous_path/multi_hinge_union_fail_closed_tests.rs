use super::{
    MultiHingeReliefUnionErrorV2, MultiHingeReliefUnionLimitsV2,
    certify_multi_hinge_relief_union_v2, diagnose_multi_hinge_relief_union_gaps_v2,
    tests::{relief, segmented_crease},
};
use crate::ContinuousPairCoverageKindV1;

#[test]
fn two_and_three_shared_hinges_stay_resource_bounded_and_non_authorizing() {
    for hinge_count in [2_usize, 3] {
        let (geometry, audit, schedule, fixed) = segmented_crease(hinge_count, 1);

        let registry =
            crate::diagnose_continuous_pair_coverage_v1(&geometry, &audit, fixed, &schedule)
                .expect("production pair registry");
        assert_eq!(registry.entries().len(), 1);
        assert_eq!(
            registry.entries()[0].kind(),
            ContinuousPairCoverageKindV1::SharedHingeNeedsCorridor
        );
        assert!(!registry.authorizes_continuous_motion());
        assert!(!registry.authorizes_project_mutation());

        // V1 now binds the complete canonical hinge set, but the report stays
        // diagnostic-only and cannot become collision or mutation authority.
        let shared_hinge_gaps = crate::diagnose_shared_hinge_continuous_corridor_gaps_v1(
            &registry, &geometry, &audit, fixed, &schedule, 0.1,
        )
        .expect("bound diagnostic-only gap report");
        assert_eq!(shared_hinge_gaps.gaps().len(), hinge_count);
        assert!(shared_hinge_gaps.gaps().windows(2).all(|pair| {
            pair[0].pair() == pair[1].pair()
                && pair[0].hinge().canonical_bytes() < pair[1].hinge().canonical_bytes()
        }));
        assert!(!shared_hinge_gaps.authorizes_continuous_motion());
        assert!(!shared_hinge_gaps.authorizes_project_mutation());

        let limits = MultiHingeReliefUnionLimitsV2::default();
        let gaps = diagnose_multi_hinge_relief_union_gaps_v2(
            &geometry, &audit, fixed, &schedule, 0.1, limits,
        )
        .expect("bounded multi-hinge union gap report");
        assert_eq!(gaps.gaps().len(), 1);
        assert_eq!(gaps.gaps()[0].hinges().len(), hinge_count);
        assert!(!gaps.authorizes_continuous_motion());
        assert!(!gaps.authorizes_collision_free_classification());
        assert!(!gaps.authorizes_shared_hinge_admission());
        assert!(!gaps.authorizes_simulation_admission());
        assert!(!gaps.authorizes_project_mutation());
        assert!(!gaps.authorizes_persistence());

        assert!(gaps.work_used() > 1);
        assert!(matches!(
            diagnose_multi_hinge_relief_union_gaps_v2(
                &geometry,
                &audit,
                fixed,
                &schedule,
                0.1,
                MultiHingeReliefUnionLimitsV2 {
                    max_work: gaps.work_used() - 1,
                    ..limits
                },
            ),
            Err(MultiHingeReliefUnionErrorV2::ResourceLimit)
        ));
        if hinge_count == 3 {
            assert!(matches!(
                diagnose_multi_hinge_relief_union_gaps_v2(
                    &geometry,
                    &audit,
                    fixed,
                    &schedule,
                    0.1,
                    MultiHingeReliefUnionLimitsV2 {
                        max_hinges_per_pair: 2,
                        ..limits
                    },
                ),
                Err(MultiHingeReliefUnionErrorV2::ResourceLimit)
            ));
        }

        let (policies, schedules, prerequisite, local, policy_limits) = relief(&gaps, &geometry);
        let certificate = certify_multi_hinge_relief_union_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        )
        .expect("complete local-union evidence");
        assert!(certificate.covers_every_reported_hinge_neighbourhood());
        assert!(!certificate.authorizes_continuous_motion());
        assert!(!certificate.authorizes_collision_free_classification());
        assert!(!certificate.authorizes_shared_hinge_admission());
        assert!(!certificate.authorizes_simulation_admission());
        assert!(!certificate.authorizes_project_mutation());
        assert!(!certificate.authorizes_persistence());
    }
}
