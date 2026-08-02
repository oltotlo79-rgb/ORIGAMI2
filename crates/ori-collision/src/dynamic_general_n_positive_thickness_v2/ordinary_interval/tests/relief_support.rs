//! Phase 3E fixture policy and exact-limit helpers.

use std::collections::HashSet;

use crate::{HingeReliefPolicyRecordV1, VertexReliefPolicyRecordV1};

use super::super::relief_aggregate::*;
use super::support::{OrdinaryFixtureV2, input_v2, strict_limits_v2};

#[derive(Clone)]
pub(crate) struct ReliefFixtureInputV2 {
    pub(crate) hinge: Vec<HingeReliefPolicyRecordV1>,
    pub(crate) vertex: Vec<VertexReliefPolicyRecordV1>,
}

pub(crate) fn relief_policies_v2(fixture: &OrdinaryFixtureV2) -> ReliefFixtureInputV2 {
    let geometry = &fixture.fixture.geometry;
    let thickness = fixture.fixture.paper.thickness_mm;
    let mut hinge = geometry
        .hinges()
        .iter()
        .map(|item| HingeReliefPolicyRecordV1 {
            edge: item.edge(),
            cutout_width_mm: 13.0,
            bevel_angle_degrees: 1.0,
            material_thickness_mm: thickness,
        })
        .collect::<Vec<_>>();
    hinge.sort_unstable_by_key(|record| record.edge.canonical_bytes());
    let hinge_pairs = geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let mut pair = [hinge.left_face(), hinge.right_face()];
            pair.sort_unstable_by_key(ori_domain::FaceId::canonical_bytes);
            pair
        })
        .collect::<HashSet<_>>();
    let mut vertices = HashSet::new();
    for first in 0..geometry.face_ids().len() {
        for second in first + 1..geometry.face_ids().len() {
            let pair = [geometry.face_ids()[first], geometry.face_ids()[second]];
            if hinge_pairs.contains(&pair) {
                continue;
            }
            let left = geometry.face_boundary_vertices(pair[0]).unwrap();
            let right = geometry.face_boundary_vertices(pair[1]).unwrap();
            let shared = left
                .iter()
                .copied()
                .filter(|vertex| right.contains(vertex))
                .collect::<Vec<_>>();
            if shared.len() == 1 {
                vertices.insert(shared[0]);
            }
        }
    }
    let mut vertices = vertices.into_iter().collect::<Vec<_>>();
    vertices.sort_unstable_by_key(ori_domain::VertexId::canonical_bytes);
    let vertex = vertices
        .into_iter()
        .map(|vertex| {
            let incident_faces = geometry
                .face_ids()
                .iter()
                .copied()
                .filter(|face| {
                    geometry
                        .face_boundary_vertices(*face)
                        .unwrap()
                        .contains(&vertex)
                })
                .collect();
            VertexReliefPolicyRecordV1 {
                vertex,
                cutout_radius_mm: 13.0,
                material_thickness_mm: thickness,
                incident_faces,
            }
        })
        .collect();
    ReliefFixtureInputV2 { hinge, vertex }
}

pub(super) fn generous_relief_limits_v2(fixture: &OrdinaryFixtureV2) -> ReliefAggregateLimitsV2 {
    let profile_maximum = fixture.fixture.profile.maximum_v2();
    let max_vertex_occurrences = profile_maximum.face_count_v2() * 4;
    ReliefAggregateLimitsV2 {
        max_hinge_policy_records: profile_maximum.hinge_count_v2(),
        max_vertex_policy_records: max_vertex_occurrences,
        max_vertex_incident_face_occurrences: max_vertex_occurrences,
        max_shared_pairs: 1 << 20,
        max_pair_membership_tests: 1 << 26,
        max_pair_hinge_tests: 1 << 26,
        max_scope_and_policy_validation_work: 1 << 26,
        max_convexity_segment_tests: 1 << 24,
        max_rest_carrier_vertices: 1 << 20,
        max_exact_clip_operations: 1 << 28,
        max_sqrt_calls: 1 << 20,
        max_sqrt_operations_per_call: 20_000,
        max_exact_value_bits: 32_768,
        max_exact_scratch_bytes: 1 << 28,
        max_collision_depth: 6,
        max_collision_leaves: 64,
        max_shared_pair_node_tests: 1 << 24,
        max_axis_projection_work: 1 << 30,
        max_carrier_conversion_work: 1 << 26,
        max_hash_work: 1 << 26,
        max_logical_work: 1 << 30,
        max_temporary_bytes: 1 << 30,
        max_publication_bytes: 1 << 20,
        max_aggregate_peak_bytes: 1 << 30,
    }
}

pub(super) fn relief_input_v2<'a>(
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    limits: ReliefAggregateLimitsV2,
) -> ReliefAggregateInputV2<'a> {
    ReliefAggregateInputV2 {
        ordinary: input_v2(fixture, strict_limits_v2(fixture)),
        hinge_policies: &policies.hinge,
        vertex_policies: &policies.vertex,
        limits,
    }
}

pub(super) fn strict_relief_limits_v2(
    generous: ReliefAggregateLimitsV2,
    resources: ReliefAggregateResourcesV2,
) -> ReliefAggregateLimitsV2 {
    ReliefAggregateLimitsV2 {
        max_hinge_policy_records: resources.hinge_policy_records,
        max_vertex_policy_records: resources.vertex_policy_records,
        max_vertex_incident_face_occurrences: resources.vertex_incident_face_occurrences,
        max_shared_pairs: resources.shared_pairs,
        max_pair_membership_tests: resources.pair_membership_tests,
        max_pair_hinge_tests: resources.pair_hinge_tests,
        max_scope_and_policy_validation_work: resources.scope_and_policy_validation_work,
        max_convexity_segment_tests: resources.convexity_segment_tests,
        max_rest_carrier_vertices: resources.rest_carrier_vertices,
        max_exact_clip_operations: resources.exact_clip_operations,
        max_sqrt_calls: resources.sqrt_calls,
        max_exact_scratch_bytes: resources.exact_scratch_bytes,
        max_shared_pair_node_tests: resources.shared_pair_node_tests,
        max_axis_projection_work: resources.axis_projection_work,
        max_carrier_conversion_work: resources.carrier_conversion_work,
        max_hash_work: resources.hash_work,
        max_logical_work: resources.logical_work,
        max_temporary_bytes: resources.temporary_bytes,
        max_publication_bytes: resources.publication_bytes,
        max_aggregate_peak_bytes: resources.aggregate_peak_bytes,
        ..generous
    }
}
