//! Fresh exhaustive topology classification and relief-policy binding.

use super::*;

pub(super) fn validate_relief_input_v2<'a>(
    input: &ReliefAggregateInputV2<'a>,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<ValidatedReliefV2<'a>, ReliefAggregateErrorV2> {
    resources::preflight_limits_v2(input, checkpoint)?;
    relief_checkpoint_v2(checkpoint)?;
    let ordinary = super::super::resources::validate_input_v2(&input.ordinary, checkpoint)
        .map_err(map_ordinary_error_v2)?;
    resources::preflight_observed_ordinary_v2(input, ordinary.resources)?;
    let preflight_scope_work = input
        .ordinary
        .geometry
        .face_ids()
        .len()
        .checked_mul(2)
        .and_then(|value| {
            input
                .vertex_policies
                .len()
                .checked_mul(2)
                .and_then(|policies| value.checked_add(policies))
        })
        .filter(|value| *value <= input.limits.max_scope_and_policy_validation_work)
        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    let mut result_resources = ReliefAggregateResourcesV2 {
        hinge_policy_records: input.hinge_policies.len(),
        vertex_policy_records: input.vertex_policies.len(),
        // Represents both the whole-parent preflight and the post-ordinary
        // classification preflight; private test entry points preserve that
        // same two-pass contract.
        scope_and_policy_validation_work: preflight_scope_work,
        ..ReliefAggregateResourcesV2::default()
    };
    for record in input.vertex_policies {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut result_resources.scope_and_policy_validation_work,
            1,
            input.limits.max_scope_and_policy_validation_work,
        )?;
        result_resources.vertex_incident_face_occurrences = result_resources
            .vertex_incident_face_occurrences
            .checked_add(record.incident_faces.len())
            .filter(|value| *value <= input.limits.max_vertex_incident_face_occurrences)
            .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
    }
    validate_policy_registries_v2(input, &mut result_resources, checkpoint)?;
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(input.ordinary.excluded_shared_pairs.len())
        .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
    if pairs.capacity() > input.ordinary.excluded_shared_pairs.len() {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    let mut used_vertex_policies = Vec::new();
    used_vertex_policies
        .try_reserve_exact(input.vertex_policies.len())
        .map_err(|_| ReliefAggregateErrorV2::ResourceLimit)?;
    if used_vertex_policies.capacity() > input.vertex_policies.len() {
        return Err(ReliefAggregateErrorV2::ResourceLimit);
    }
    for _ in input.vertex_policies {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut result_resources.scope_and_policy_validation_work,
            1,
            input.limits.max_scope_and_policy_validation_work,
        )?;
        used_vertex_policies.push(0_u8);
    }
    let geometry = input.ordinary.geometry;
    let mut excluded_cursor = 0usize;
    let mut shared_hash = Sha256::new();
    shared_hash.update(b"origami2/dynamic-general-n/shared-feature-registry/v2");
    for first_index in 0..geometry.face_ids().len() {
        relief_checkpoint_v2(checkpoint)?;
        for second_index in first_index + 1..geometry.face_ids().len() {
            relief_checkpoint_v2(checkpoint)?;
            let pair = OrdinaryIntervalFacePairV2 {
                first: geometry.face_ids()[first_index],
                second: geometry.face_ids()[second_index],
            };
            let first = geometry
                .face_boundary_vertices(pair.first)
                .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
            let second = geometry
                .face_boundary_vertices(pair.second)
                .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
            let mut shared = [None, None, None];
            let mut shared_count = 0usize;
            for left in first {
                relief_checkpoint_v2(checkpoint)?;
                for right in second {
                    relief_checkpoint_v2(checkpoint)?;
                    resources::charge_v2(
                        &mut result_resources.pair_membership_tests,
                        1,
                        input.limits.max_pair_membership_tests,
                    )?;
                    if left == right {
                        if shared_count >= shared.len() {
                            return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
                        }
                        shared[shared_count] = Some(*left);
                        shared_count += 1;
                    }
                }
            }
            let submitted = input
                .ordinary
                .excluded_shared_pairs
                .get(excluded_cursor)
                .is_some_and(|submitted| *submitted == pair);
            if (shared_count > 0) != submitted {
                return Err(ReliefAggregateErrorV2::InvalidInput);
            }
            if shared_count == 0 {
                continue;
            }
            let mut matching_hinge = None;
            for (index, hinge) in geometry.hinges().iter().enumerate() {
                relief_checkpoint_v2(checkpoint)?;
                resources::charge_v2(
                    &mut result_resources.pair_hinge_tests,
                    1,
                    input.limits.max_pair_hinge_tests,
                )?;
                if canonical_face_pair_v2(hinge.left_face(), hinge.right_face()) == Some(pair)
                    && matching_hinge.replace(index).is_some()
                {
                    return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology);
                }
            }
            let prepared = match (shared_count, matching_hinge) {
                (2, Some(hinge_index)) => {
                    let hinge = &geometry.hinges()[hinge_index];
                    let vertices = [shared[0].unwrap(), shared[1].unwrap()];
                    exact_clip::verify_hinge_endpoints_v2(geometry, hinge, vertices)?;
                    let policy = find_hinge_policy_v2(input.hinge_policies, hinge.edge())?;
                    let (left, right) = exact_clip::prepare_hinge_cells_v2(
                        input,
                        pair,
                        hinge,
                        policy,
                        &mut result_resources,
                        checkpoint,
                    )?;
                    result_resources.shared_hinge_pairs = result_resources
                        .shared_hinge_pairs
                        .checked_add(1)
                        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
                    PreparedSharedPairV2 {
                        pair,
                        feature: SharedFeatureV2::Hinge { edge: hinge.edge() },
                        left,
                        right,
                    }
                }
                (1, None) => {
                    let vertex = shared[0].unwrap();
                    let (policy_index, policy) =
                        find_vertex_policy_v2(input.vertex_policies, vertex)?;
                    used_vertex_policies[policy_index] = 1;
                    let (left, right) = exact_clip::prepare_vertex_cells_v2(
                        input,
                        pair,
                        vertex,
                        policy,
                        &mut result_resources,
                        checkpoint,
                    )?;
                    result_resources.shared_vertex_pairs = result_resources
                        .shared_vertex_pairs
                        .checked_add(1)
                        .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
                    PreparedSharedPairV2 {
                        pair,
                        feature: SharedFeatureV2::Vertex { vertex },
                        left,
                        right,
                    }
                }
                _ => return Err(ReliefAggregateErrorV2::UnsupportedSharedTopology),
            };
            result_resources.shared_pairs = result_resources
                .shared_pairs
                .checked_add(1)
                .filter(|value| *value <= input.limits.max_shared_pairs)
                .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
            result_resources.rest_carrier_vertices = result_resources
                .rest_carrier_vertices
                .checked_add(prepared.left.ring.len())
                .and_then(|value| value.checked_add(prepared.right.ring.len()))
                .filter(|value| *value <= input.limits.max_rest_carrier_vertices)
                .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
            shared_hash.update(pair.first.canonical_bytes());
            shared_hash.update(pair.second.canonical_bytes());
            resources::charge_v2(
                &mut result_resources.hash_work,
                2,
                input.limits.max_hash_work,
            )?;
            pairs.push(prepared);
            excluded_cursor = excluded_cursor
                .checked_add(1)
                .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
        }
    }
    if excluded_cursor != input.ordinary.excluded_shared_pairs.len()
        || pairs.len() != result_resources.shared_pairs
    {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    if result_resources.shared_hinge_pairs != input.hinge_policies.len() {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    for used in &used_vertex_policies {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut result_resources.scope_and_policy_validation_work,
            1,
            input.limits.max_scope_and_policy_validation_work,
        )?;
        if *used != 1 {
            return Err(ReliefAggregateErrorV2::InvalidInput);
        }
    }
    update_usize_v2(&mut shared_hash, pairs.len()).map_err(map_ordinary_error_v2)?;
    resources::charge_v2(
        &mut result_resources.hash_work,
        1,
        input.limits.max_hash_work,
    )?;
    let shared_pair_digest = shared_hash.finalize().into();
    if shared_pair_digest != ordinary.excluded_shared_pair_digest {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    let policy_digest = policy_digest_v2(input, &mut result_resources, checkpoint)?;
    result_resources.retained_carrier_bytes =
        resources::retained_carrier_bytes_v2(&pairs, checkpoint)?;
    Ok(ValidatedReliefV2 {
        ordinary,
        pairs,
        shared_pair_digest,
        policy_digest,
        resources: result_resources,
    })
}

fn validate_policy_registries_v2(
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), ReliefAggregateErrorV2> {
    if input.hinge_policies.len() != input.ordinary.geometry.hinges().len() {
        return Err(ReliefAggregateErrorV2::InvalidInput);
    }
    for pair in input.hinge_policies.windows(2) {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut resources.scope_and_policy_validation_work,
            1,
            input.limits.max_scope_and_policy_validation_work,
        )?;
        if pair[0].edge.canonical_bytes() >= pair[1].edge.canonical_bytes() {
            return Err(ReliefAggregateErrorV2::InvalidInput);
        }
    }
    for pair in input.vertex_policies.windows(2) {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut resources.scope_and_policy_validation_work,
            1,
            input.limits.max_scope_and_policy_validation_work,
        )?;
        if pair[0].vertex.canonical_bytes() >= pair[1].vertex.canonical_bytes() {
            return Err(ReliefAggregateErrorV2::InvalidInput);
        }
    }
    for (policy, hinge) in input
        .hinge_policies
        .iter()
        .zip(input.ordinary.geometry.hinges())
    {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut resources.scope_and_policy_validation_work,
            1,
            input.limits.max_scope_and_policy_validation_work,
        )?;
        exact_clip::validate_hinge_policy_v2(policy, input, resources)?;
        if hinge.edge() != policy.edge {
            return Err(ReliefAggregateErrorV2::InvalidInput);
        }
    }
    for policy in input.vertex_policies {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(
            &mut resources.scope_and_policy_validation_work,
            1,
            input.limits.max_scope_and_policy_validation_work,
        )?;
        exact_clip::validate_vertex_policy_v2(policy, input, resources)?;
        let mut previous_face = None;
        for face in &policy.incident_faces {
            relief_checkpoint_v2(checkpoint)?;
            resources::charge_v2(
                &mut resources.scope_and_policy_validation_work,
                1,
                input.limits.max_scope_and_policy_validation_work,
            )?;
            if previous_face.is_some_and(|previous: FaceId| {
                previous.canonical_bytes() >= face.canonical_bytes()
            }) {
                return Err(ReliefAggregateErrorV2::InvalidInput);
            }
            previous_face = Some(*face);
        }
        let mut cursor = 0usize;
        for face in input.ordinary.geometry.face_ids() {
            relief_checkpoint_v2(checkpoint)?;
            resources::charge_v2(
                &mut resources.scope_and_policy_validation_work,
                1,
                input.limits.max_scope_and_policy_validation_work,
            )?;
            let boundary = input
                .ordinary
                .geometry
                .face_boundary_vertices(*face)
                .ok_or(ReliefAggregateErrorV2::InvalidInput)?;
            let mut incident = false;
            for vertex in boundary {
                relief_checkpoint_v2(checkpoint)?;
                resources::charge_v2(
                    &mut resources.scope_and_policy_validation_work,
                    1,
                    input.limits.max_scope_and_policy_validation_work,
                )?;
                incident |= *vertex == policy.vertex;
            }
            if incident {
                if policy.incident_faces.get(cursor) != Some(face) {
                    return Err(ReliefAggregateErrorV2::InvalidInput);
                }
                cursor = cursor
                    .checked_add(1)
                    .ok_or(ReliefAggregateErrorV2::ResourceLimit)?;
            }
        }
        if cursor != policy.incident_faces.len() {
            return Err(ReliefAggregateErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn canonical_face_pair_v2(first: FaceId, second: FaceId) -> Option<OrdinaryIntervalFacePairV2> {
    OrdinaryIntervalFacePairV2::new(first, second)
}

fn find_hinge_policy_v2(
    policies: &[HingeReliefPolicyRecordV1],
    edge: EdgeId,
) -> Result<&HingeReliefPolicyRecordV1, ReliefAggregateErrorV2> {
    policies
        .binary_search_by_key(&edge.canonical_bytes(), |policy| {
            policy.edge.canonical_bytes()
        })
        .ok()
        .map(|index| &policies[index])
        .ok_or(ReliefAggregateErrorV2::InvalidInput)
}

fn find_vertex_policy_v2(
    policies: &[VertexReliefPolicyRecordV1],
    vertex: VertexId,
) -> Result<(usize, &VertexReliefPolicyRecordV1), ReliefAggregateErrorV2> {
    policies
        .binary_search_by_key(&vertex.canonical_bytes(), |policy| {
            policy.vertex.canonical_bytes()
        })
        .ok()
        .map(|index| (index, &policies[index]))
        .ok_or(ReliefAggregateErrorV2::InvalidInput)
}

fn policy_digest_v2(
    input: &ReliefAggregateInputV2<'_>,
    resources: &mut ReliefAggregateResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<[u8; 32], ReliefAggregateErrorV2> {
    let mut hash = Sha256::new();
    hash.update(b"origami2/dynamic-general-n/relief-policy/v2");
    update_usize_v2(&mut hash, input.hinge_policies.len()).map_err(map_ordinary_error_v2)?;
    resources::charge_v2(&mut resources.hash_work, 1, input.limits.max_hash_work)?;
    for policy in input.hinge_policies {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(&mut resources.hash_work, 4, input.limits.max_hash_work)?;
        hash.update(policy.edge.canonical_bytes());
        hash.update(policy.cutout_width_mm.to_bits().to_le_bytes());
        hash.update(policy.bevel_angle_degrees.to_bits().to_le_bytes());
        hash.update(policy.material_thickness_mm.to_bits().to_le_bytes());
    }
    update_usize_v2(&mut hash, input.vertex_policies.len()).map_err(map_ordinary_error_v2)?;
    resources::charge_v2(&mut resources.hash_work, 1, input.limits.max_hash_work)?;
    for policy in input.vertex_policies {
        relief_checkpoint_v2(checkpoint)?;
        resources::charge_v2(&mut resources.hash_work, 4, input.limits.max_hash_work)?;
        hash.update(policy.vertex.canonical_bytes());
        hash.update(policy.cutout_radius_mm.to_bits().to_le_bytes());
        hash.update(policy.material_thickness_mm.to_bits().to_le_bytes());
        update_usize_v2(&mut hash, policy.incident_faces.len()).map_err(map_ordinary_error_v2)?;
        for face in &policy.incident_faces {
            relief_checkpoint_v2(checkpoint)?;
            resources::charge_v2(&mut resources.hash_work, 1, input.limits.max_hash_work)?;
            hash.update(face.canonical_bytes());
        }
    }
    Ok(hash.finalize().into())
}
