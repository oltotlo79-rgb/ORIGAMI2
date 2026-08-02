//! Checked resource policy for shared relief and sealed aggregation.

use super::*;
use num_rational::BigRational;

// Fixed-quad exact-operation proofs. A hinge has 11 setup operations and two
// cells of 92 (polygon validation) + 40 (finite axial/side validation) + 59
// (the parallel finite-strip clip retains at most four vertices): 393. Policy
// validation adds 5, then an intentional 40-operation per-hinge reserve is
// charged and checked against observed work before publication. A vertex has
// 2 shared-origin conversions and two cells of 92 + 27 (wedge construction) +
// 63 (generic convex-quad half-plane clip): 366 per shared-vertex pair;
// policy validation adds 2 per distinct vertex-policy record.
const HINGE_EXACT_CLIP_CHARGE_V2: usize = 11 + 2 * (92 + 40 + 59) + 5 + 40;
const VERTEX_PAIR_EXACT_CLIP_CHARGE_V2: usize = 2 + 2 * (92 + 27 + 63);
const VERTEX_POLICY_EXACT_CLIP_CHARGE_V2: usize = 2;

mod preflight;
pub(super) use preflight::preflight_limits_v2;

pub(super) fn preflight_observed_ordinary_v2(
    input: &ReliefAggregateInputV2<'_>,
    ordinary: OrdinaryIntervalResourcesV2,
) -> Result<(), ReliefAggregateErrorV2> {
    let carrier_upper = carrier_upper_bound_v2(input)?;
    let pending = input
        .limits
        .max_collision_leaves
        .checked_mul(size_of::<DyadicLeafV2>())
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let scratch = exact_scratch_upper_bound_v2(input.limits.max_exact_value_bits)?;
    let relief_phase = ordinary
        .charged_temporary_bytes
        .checked_add(carrier_upper)
        .and_then(|value| value.checked_add(pending))
        .and_then(|value| value.checked_add(scratch))
        .and_then(|value| value.checked_add(input.vertex_policies.len()))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let temporary = ordinary.charged_temporary_bytes.max(
        size_of::<OrdinaryIntervalEvidenceV2>()
            .checked_add(relief_phase)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
    );
    let aggregate = temporary.max(
        publication_bytes_v2()?
            .checked_add(carrier_upper)
            .and_then(|value| value.checked_add(ordinary.charged_session_steady_retained_bytes))
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
    );
    if input.limits.max_temporary_bytes < temporary
        || input.limits.max_aggregate_peak_bytes < aggregate
    {
        Err(ReliefAggregateErrorV2::ResourceLimit)
    } else {
        Ok(())
    }
}

pub(super) fn charge_v2(
    counter: &mut usize,
    amount: usize,
    cap: usize,
) -> Result<(), ReliefAggregateErrorV2> {
    *counter = counter
        .checked_add(amount)
        .filter(|value| *value <= cap)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    Ok(())
}

pub(super) fn retained_carrier_bytes_v2(
    pairs: &Vec<PreparedSharedPairV2>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<usize, ReliefAggregateErrorV2> {
    let mut bytes = pairs
        .capacity()
        .checked_mul(size_of::<PreparedSharedPairV2>())
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    for pair in pairs {
        relief_checkpoint_v2(checkpoint)?;
        for cell in [&pair.left, &pair.right] {
            relief_checkpoint_v2(checkpoint)?;
            bytes = bytes
                .checked_add(
                    cell.ring
                        .capacity()
                        .checked_mul(size_of::<[OutwardIntervalV1; 2]>())
                        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
                )
                .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
        }
    }
    Ok(bytes)
}

pub(super) fn finish_resource_accounting_v2(
    input: &ReliefAggregateInputV2<'_>,
    ordinary: OrdinaryIntervalResourcesV2,
    resources: &mut ReliefAggregateResourcesV2,
) -> Result<(), ReliefAggregateErrorV2> {
    let charged_nodes = charged_pair_node_tests_v2(input)?;
    let charged_projection = charged_axis_projection_work_v2(input)?;
    let charged_clip = charged_exact_clip_operations_v2(input)?;
    let charged_carrier_vertices = charged_rest_carrier_vertices_v2(input)?;
    let charged_conversion = charged_carrier_conversion_work_v2(input)?;
    let charged_hash = charged_hash_work_v2(input, resources.vertex_incident_face_occurrences)?;
    if resources.shared_pair_node_tests > charged_nodes
        || resources.axis_projection_work > charged_projection
        || resources.exact_clip_operations > charged_clip
        || resources.rest_carrier_vertices > charged_carrier_vertices
        || resources.carrier_conversion_work > charged_conversion
        || resources.hash_work > charged_hash
    {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    resources.shared_pair_node_tests = charged_nodes;
    resources.axis_projection_work = charged_projection;
    resources.exact_clip_operations = charged_clip;
    resources.rest_carrier_vertices = charged_carrier_vertices;
    resources.carrier_conversion_work = charged_conversion;
    resources.hash_work = charged_hash;
    resources.logical_work = [
        resources.pair_membership_tests,
        resources.pair_hinge_tests,
        resources.scope_and_policy_validation_work,
        resources.convexity_segment_tests,
        resources.exact_clip_operations,
        resources.shared_pair_node_tests,
        resources.axis_projection_work,
        resources.carrier_conversion_work,
        resources.hash_work,
        resources
            .sqrt_calls
            .checked_mul(input.limits.max_sqrt_operations_per_call)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let pending_bytes = input
        .limits
        .max_collision_leaves
        .checked_mul(size_of::<DyadicLeafV2>())
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let carrier_upper = carrier_upper_bound_v2(input)?;
    if resources.retained_carrier_bytes > carrier_upper {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    resources.exact_scratch_bytes =
        exact_scratch_upper_bound_v2(input.limits.max_exact_value_bits)?;
    let relief_phase = ordinary
        .charged_temporary_bytes
        .checked_add(carrier_upper)
        .and_then(|value| value.checked_add(pending_bytes))
        .and_then(|value| value.checked_add(resources.exact_scratch_bytes))
        .and_then(|value| value.checked_add(input.vertex_policies.len()))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    resources.temporary_bytes = ordinary.charged_temporary_bytes.max(
        size_of::<OrdinaryIntervalEvidenceV2>()
            .checked_add(relief_phase)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
    );
    resources.publication_bytes = publication_bytes_v2()?;
    resources.aggregate_peak_bytes = resources.temporary_bytes.max(
        resources
            .publication_bytes
            .checked_add(carrier_upper)
            .and_then(|value| value.checked_add(ordinary.charged_session_steady_retained_bytes))
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?,
    );
    let limits = input.limits;
    let within = resources.hinge_policy_records <= limits.max_hinge_policy_records
        && resources.vertex_policy_records <= limits.max_vertex_policy_records
        && resources.vertex_incident_face_occurrences
            <= limits.max_vertex_incident_face_occurrences
        && resources.shared_pairs <= limits.max_shared_pairs
        && resources.pair_membership_tests <= limits.max_pair_membership_tests
        && resources.pair_hinge_tests <= limits.max_pair_hinge_tests
        && resources.scope_and_policy_validation_work
            <= limits.max_scope_and_policy_validation_work
        && resources.convexity_segment_tests <= limits.max_convexity_segment_tests
        && resources.rest_carrier_vertices <= limits.max_rest_carrier_vertices
        && resources.exact_clip_operations <= limits.max_exact_clip_operations
        && resources.sqrt_calls <= limits.max_sqrt_calls
        && resources.exact_scratch_bytes <= limits.max_exact_scratch_bytes
        && resources.shared_pair_node_tests <= limits.max_shared_pair_node_tests
        && resources.axis_projection_work <= limits.max_axis_projection_work
        && resources.carrier_conversion_work <= limits.max_carrier_conversion_work
        && resources.hash_work <= limits.max_hash_work
        && resources.logical_work <= limits.max_logical_work
        && resources.temporary_bytes <= limits.max_temporary_bytes
        && resources.publication_bytes <= limits.max_publication_bytes
        && resources.aggregate_peak_bytes <= limits.max_aggregate_peak_bytes;
    if within {
        Ok(())
    } else {
        Err(ReliefAggregateErrorV2::ResourceLimit)
    }
}

fn exact_scratch_upper_bound_v2(max_bits: usize) -> Result<usize, ReliefAggregateErrorV2> {
    // At most 128 exact scalars are simultaneously live in one quad clip or
    // one bounded sqrt. Eight times the payload covers numerator/denominator
    // limbs, allocator growth, and unreduced arithmetic temporaries.
    max_bits
        .checked_add(7)
        .and_then(|value| value.checked_div(8))
        .and_then(|bytes| bytes.checked_add(size_of::<BigRational>()))
        .and_then(|bytes| bytes.checked_mul(8))
        .and_then(|bytes| bytes.checked_mul(128))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn publication_bytes_v2() -> Result<usize, ReliefAggregateErrorV2> {
    size_of::<WholeParentPositiveThicknessEvidenceV2>()
        .checked_add(size_of::<OrdinaryIntervalEvidenceV2>())
        .and_then(|value| value.checked_add(size_of::<SharedReliefEvidenceV2>()))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn carrier_upper_bound_v2(
    input: &ReliefAggregateInputV2<'_>,
) -> Result<usize, ReliefAggregateErrorV2> {
    let pairs = input.ordinary.excluded_shared_pairs.len();
    let vertices = charged_rest_carrier_vertices_v2(input)?;
    pairs
        .checked_mul(size_of::<PreparedSharedPairV2>())
        .and_then(|value| {
            vertices
                .checked_mul(size_of::<[OutwardIntervalV1; 2]>())
                .and_then(|rings| value.checked_add(rings))
        })
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn charged_pair_node_tests_v2(
    input: &ReliefAggregateInputV2<'_>,
) -> Result<usize, ReliefAggregateErrorV2> {
    input
        .limits
        .max_collision_leaves
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .and_then(|nodes| nodes.checked_mul(input.ordinary.excluded_shared_pairs.len()))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn charged_axis_projection_work_v2(
    input: &ReliefAggregateInputV2<'_>,
) -> Result<usize, ReliefAggregateErrorV2> {
    let vertices = charged_rest_carrier_vertices_v2(input)?;
    input
        .limits
        .max_collision_leaves
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .and_then(|nodes| nodes.checked_mul(vertices))
        .and_then(|value| value.checked_mul(78))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn charged_rest_carrier_vertices_v2(
    input: &ReliefAggregateInputV2<'_>,
) -> Result<usize, ReliefAggregateErrorV2> {
    input
        .ordinary
        .excluded_shared_pairs
        .len()
        .checked_mul(10)
        .and_then(|value| {
            input
                .ordinary
                .geometry
                .hinges()
                .len()
                .checked_mul(2)
                .and_then(|hinges| value.checked_sub(hinges))
        })
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn charged_carrier_conversion_work_v2(
    input: &ReliefAggregateInputV2<'_>,
) -> Result<usize, ReliefAggregateErrorV2> {
    charged_rest_carrier_vertices_v2(input)?
        .checked_mul(2)
        .and_then(|value| {
            input
                .ordinary
                .excluded_shared_pairs
                .len()
                .checked_mul(4)
                .and_then(|axes| value.checked_add(axes))
        })
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn charged_exact_clip_operations_v2(
    input: &ReliefAggregateInputV2<'_>,
) -> Result<usize, ReliefAggregateErrorV2> {
    let hinges = input.ordinary.geometry.hinges().len();
    let vertices = input
        .ordinary
        .excluded_shared_pairs
        .len()
        .checked_sub(hinges)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    hinges
        .checked_mul(HINGE_EXACT_CLIP_CHARGE_V2)
        .and_then(|value| {
            vertices
                .checked_mul(VERTEX_PAIR_EXACT_CLIP_CHARGE_V2)
                .and_then(|other| value.checked_add(other))
        })
        .and_then(|value| {
            input
                .vertex_policies
                .len()
                .checked_mul(VERTEX_POLICY_EXACT_CLIP_CHARGE_V2)
                .and_then(|policy| value.checked_add(policy))
        })
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}

fn charged_hash_work_v2(
    input: &ReliefAggregateInputV2<'_>,
    vertex_incident_face_occurrences: usize,
) -> Result<usize, ReliefAggregateErrorV2> {
    input
        .ordinary
        .excluded_shared_pairs
        .len()
        .checked_mul(2)
        .and_then(|value| {
            input
                .ordinary
                .geometry
                .hinges()
                .len()
                .checked_mul(4)
                .and_then(|work| value.checked_add(work))
        })
        .and_then(|value| {
            input
                .vertex_policies
                .len()
                .checked_mul(4)
                .and_then(|work| value.checked_add(work))
        })
        .and_then(|value| value.checked_add(vertex_incident_face_occurrences))
        // Three variable registry counts, then two fields for every possible
        // accepted leaf and two final partition counters.
        .and_then(|value| value.checked_add(3))
        .and_then(|value| {
            input
                .limits
                .max_collision_leaves
                .checked_mul(2)
                .and_then(|work| value.checked_add(work))
        })
        .and_then(|value| value.checked_add(2))
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)
}
