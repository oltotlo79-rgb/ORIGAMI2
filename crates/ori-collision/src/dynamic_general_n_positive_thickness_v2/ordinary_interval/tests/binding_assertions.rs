//! Binding checks extracted from the single heavy ordinary-kernel replay.

use super::super::*;

pub(super) fn assert_ordinary_binding_covers_boundary_and_resource_fields_v2<'a>(
    input: OrdinaryIntervalInputV2<'a>,
    validated: &mut ValidatedInputV2<'a>,
    evidence: &OrdinaryIntervalEvidenceV2,
) {
    assert_eq!(evidence.root_lower_boundary_accepted_leaf_count, 1);
    assert_eq!(evidence.root_upper_boundary_accepted_leaf_count, 1);
    let mut run = ProofRunV2 {
        collision_partition_digest: evidence.collision_partition_digest,
        accepted_leaf_count: evidence.accepted_leaf_count,
        processed_interval_node_count: evidence.processed_interval_node_count,
        maximum_accepted_depth: evidence.maximum_accepted_depth,
        certified_ordinary_pair_leaf_count: evidence.certified_ordinary_pair_leaf_count,
        root_lower_boundary_accepted_leaf_count: evidence.root_lower_boundary_accepted_leaf_count,
        root_upper_boundary_accepted_leaf_count: evidence.root_upper_boundary_accepted_leaf_count,
    };
    let base_binding = binding::binding_fingerprint_v2(&input, validated, &run).unwrap();
    assert_eq!(base_binding, evidence.binding_fingerprint);

    run.root_lower_boundary_accepted_leaf_count += 1;
    assert_ne!(
        binding::binding_fingerprint_v2(&input, validated, &run).unwrap(),
        base_binding,
        "lower root-boundary coverage count must be bound"
    );
    run.root_lower_boundary_accepted_leaf_count -= 1;
    run.root_upper_boundary_accepted_leaf_count += 1;
    assert_ne!(
        binding::binding_fingerprint_v2(&input, validated, &run).unwrap(),
        base_binding,
        "upper root-boundary coverage count must be bound"
    );
    run.root_upper_boundary_accepted_leaf_count -= 1;

    macro_rules! resource_binding_drift {
        ($field:ident) => {{
            validated.resources.$field += 1;
            assert_ne!(
                binding::binding_fingerprint_v2(&input, validated, &run).unwrap(),
                base_binding,
                "resource field {} must be bound",
                stringify!($field)
            );
            validated.resources.$field -= 1;
        }};
    }
    resource_binding_drift!(charged_bridge_retained_bytes);
    resource_binding_drift!(charged_bridge_revalidation_peak_bytes);
    resource_binding_drift!(charged_schedule_retained_bytes);
    resource_binding_drift!(charged_session_shell_bytes);
    resource_binding_drift!(charged_session_steady_retained_bytes);
    resource_binding_drift!(charged_bridge_revalidation_phase_peak_bytes);
    resource_binding_drift!(charged_bridge_partition_search_work);
    resource_binding_drift!(charged_leaf_wrapper_overhead_bytes);
    resource_binding_drift!(charged_leaf_retained_bytes);

    macro_rules! limit_binding_drift {
        ($field:ident) => {{
            let mut changed = input;
            changed.limits.$field += 1;
            assert_ne!(
                binding::binding_fingerprint_v2(&changed, validated, &run).unwrap(),
                base_binding,
                "limit field {} must be bound",
                stringify!($field)
            );
        }};
    }
    limit_binding_drift!(max_bridge_retained_bytes);
    limit_binding_drift!(max_bridge_revalidation_peak_bytes);
    limit_binding_drift!(max_schedule_retained_bytes);
    limit_binding_drift!(max_session_shell_bytes);
    limit_binding_drift!(max_bridge_partition_search_work_per_node);
}
