use std::collections::HashSet;
use std::mem::size_of;
use std::sync::OnceLock;

use crate::common_articulation_clearance_v2::test_support::{
    MiuraFixtureV2, miura_fixture_v2, miura_fixture_v2_with_profile,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, ClosedMaterialHingeGraphPose,
    CommonArticulationDynamicClosureBridgeInputV2, CommonArticulationDynamicClosureBridgeLimitsV2,
    CommonArticulationDynamicClosureBridgeRevalidationInputV2,
    CommonArticulationDynamicClosureBridgeV2, CommonArticulationPoseAuthorityV2,
    CommonArticulationPoseInputV2, CycleScheduleEntryInputV1, CycleScheduleLimitsV1,
    DyadicIntervalClosureStopV1, IntervalFaceTransformWorkspaceLimitsV2, IntervalRigidTransformV1,
    OutwardIntervalV1, RationalCoefficientV1, prove_common_articulation_dynamic_closure_bridge_v2,
    prove_common_articulation_pose_authority_v2,
};

use super::super::*;

pub(super) const N33: usize = 33;
pub(super) const N34: usize = 34;
const INTERVAL_TRANSFORM_OPERATIONAL_WORK_CAP: usize = 1 << 20;

pub(super) struct OrdinaryFixtureV2 {
    pub fixture: MiuraFixtureV2,
    pub schedule: CanonicalCycleScheduleV1,
    pub pose: ClosedMaterialHingeGraphPose,
    pub common_pose: CommonArticulationPoseAuthorityV2,
    pub bridge: CommonArticulationDynamicClosureBridgeV2,
    pub excluded_shared_pairs: Vec<OrdinaryIntervalFacePairV2>,
}

static N33_FIXTURE: OnceLock<OrdinaryFixtureV2> = OnceLock::new();
static N34_FIXTURE: OnceLock<OrdinaryFixtureV2> = OnceLock::new();

pub(super) fn n33_fixture_v2() -> &'static OrdinaryFixtureV2 {
    N33_FIXTURE.get_or_init(|| ordinary_fixture_v2(miura_fixture_v2()))
}

pub(super) fn n34_fixture_v2() -> &'static OrdinaryFixtureV2 {
    N34_FIXTURE.get_or_init(|| ordinary_fixture_v2(miura_fixture_v2_with_profile(N34, N34)))
}

pub(super) fn fresh_n33_fixture_v2() -> OrdinaryFixtureV2 {
    ordinary_fixture_v2(miura_fixture_v2())
}

fn ordinary_fixture_v2(fixture: MiuraFixtureV2) -> OrdinaryFixtureV2 {
    let schedule = nonstationary_schedule_v2(&fixture);
    let pose = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            fixture.parent_fixed_face,
            &schedule.evaluate(0.0).expect("midpoint schedule"),
            1.0e-8,
        )
        .expect("nonzero general-N midpoint pose");
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &pose,
        decomposition: &fixture.decomposition,
        paper_thickness_mm: fixture.paper.thickness_mm,
        profile: &fixture.profile,
    })
    .expect("nonzero general-N common pose");
    let bridge = prove_common_articulation_dynamic_closure_bridge_v2(
        CommonArticulationDynamicClosureBridgeInputV2 {
            geometry: &fixture.geometry,
            audit: &fixture.audit,
            pose: &pose,
            parent_fixed_face: fixture.parent_fixed_face,
            parent_schedule: &schedule,
            decomposition: &fixture.decomposition,
            common_pose: &common_pose,
            paper_thickness_mm: fixture.paper.thickness_mm,
            closure_tolerance: fixture.closure_tolerance,
            profile: &fixture.profile,
            limits: bridge_limits_v2(fixture.profile.actual_block_count_v2()),
        },
    )
    .expect("nonzero general-N dynamic closure bridge");
    let excluded_shared_pairs = exact_shared_pairs_v2(&fixture.geometry);
    OrdinaryFixtureV2 {
        fixture,
        schedule,
        pose,
        common_pose,
        bridge,
        excluded_shared_pairs,
    }
}

pub(super) fn bridge_limits_v2(
    max_blocks: usize,
) -> CommonArticulationDynamicClosureBridgeLimitsV2 {
    let ceiling = 1 << 30;
    CommonArticulationDynamicClosureBridgeLimitsV2 {
        max_blocks,
        max_validation_work: ceiling,
        max_total_restriction_work: ceiling,
        max_total_restricted_schedule_retained_bytes: ceiling,
        max_total_block_closure_retained_bytes: ceiling,
        max_total_block_leaves: ceiling,
        max_parent_schedule_retained_bytes: ceiling,
        max_parent_closure_retained_bytes: ceiling,
        max_parent_leaves: ceiling,
        max_bundle_retained_bytes: ceiling,
        max_issuance_peak_bytes: ceiling,
        max_revalidation_peak_bytes: ceiling * 3,
        max_schedule_degree: 1,
        max_schedule_coefficient_bits: 53,
        max_dyadic_depth: 2,
        max_dyadic_leaves_per_closure: 4,
        max_dyadic_work_per_closure: ceiling,
    }
}

pub(super) fn schedule_limits_v2(fixture: &OrdinaryFixtureV2) -> CycleScheduleLimitsV1 {
    CycleScheduleLimitsV1 {
        max_hinges: fixture.fixture.geometry.hinges().len(),
        max_degree: 1,
        max_coefficient_bits: 53,
        max_work: 15,
    }
}

pub(super) fn strict_limits_v2(fixture: &OrdinaryFixtureV2) -> OrdinaryIntervalLimitsV2 {
    let geometry = &fixture.fixture.geometry;
    let face_count = geometry.face_ids().len();
    let hinge_count = geometry.hinges().len();
    let schedule_limits = schedule_limits_v2(fixture);
    let max_collision_depth = 6;
    let max_collision_leaves = 64usize;
    let charged_interval_nodes = max_collision_leaves * 2 - 1;
    let schedule_bound = fixture
        .schedule
        .checked_dyadic_workspace_upper_bound_v2(max_collision_depth, schedule_limits)
        .expect("checked schedule workspace");
    // Representation-derived discovery caps for the one registry preflight;
    // the returned physical inventory is then used as the strict policy.
    let session_resources = fixture
        .bridge
        .checked_interval_transform_session_resources_v2(&fixture.schedule)
        .expect("checked session resources");
    let registry_shell = session_resources.interval_registry_shell_bytes();
    let pose_bytes = size_of::<Option<IntervalRigidTransformV1>>() * face_count;
    let registry_workspace_discovery = registry_shell
        + size_of::<usize>() * hinge_count
        + size_of::<Vec<(usize, usize, bool)>>() * face_count
        + size_of::<(usize, usize, bool)>() * fixture.fixture.audit.spanning_hinges().len() * 2
        + size_of::<usize>() * face_count
        + pose_bytes
        + size_of::<usize>() * face_count;
    let registry_retained_discovery = registry_shell + pose_bytes;
    let registry_validation_discovery = face_count
        .checked_add(hinge_count)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_mul(value))
        .and_then(|value| value.checked_mul(8))
        .expect("fixture-derived registry validation cap");
    let hinge_bit_length = usize::BITS as usize - hinge_count.leading_zeros() as usize;
    let registry_sort_discovery = hinge_count * hinge_bit_length * 3;
    let discovered_registry = geometry
        .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
            &fixture.fixture.audit,
            fixture.fixture.parent_fixed_face,
            IntervalFaceTransformWorkspaceLimitsV2 {
                max_work: INTERVAL_TRANSFORM_OPERATIONAL_WORK_CAP,
                max_validation_work: registry_validation_discovery,
                max_sort_comparisons: registry_sort_discovery,
                max_workspace_bytes: registry_workspace_discovery,
                max_retained_bytes: registry_retained_discovery,
            },
            || Ok(()),
        )
        .expect("checked registry workspace");
    let registry = discovered_registry.checked_resources();
    let boundary_vertex_occurrences = geometry
        .face_ids()
        .iter()
        .map(|face| geometry.face_boundary_vertices(*face).unwrap().len())
        .sum();
    let total_face_pairs = face_count * (face_count - 1) / 2;
    let ordinary_face_pairs = total_face_pairs - fixture.excluded_shared_pairs.len();
    let charged_shared_feature_membership_tests =
        boundary_vertex_occurrences * boundary_vertex_occurrences;
    let charged_ordinary_pair_node_tests = charged_interval_nodes * ordinary_face_pairs;
    let charged_axis_tests = charged_ordinary_pair_node_tests * AXIS_COUNT_V2;
    let charged_surface_vertex_visits =
        charged_interval_nodes * boundary_vertex_occurrences * THICK_SURFACE_COUNT_V2;
    let charged_pair_classification_visits = charged_interval_nodes * total_face_pairs;
    let charged_schedule_work = charged_interval_nodes
        * hinge_count
        * (schedule_limits.max_degree + 1)
        * schedule_limits.max_work;
    let charged_transform_work = charged_interval_nodes * INTERVAL_TRANSFORM_OPERATIONAL_WORK_CAP;
    let charged_registry_validation =
        (charged_interval_nodes + 1) * registry.validation_work_upper_bound();
    let charged_registry_sort = charged_interval_nodes * registry.sort_comparison_upper_bound();
    let charged_coverage_work =
        charged_interval_nodes * session_resources.coverage_search_comparison_upper_bound();
    let charged_logical_work = [
        charged_shared_feature_membership_tests,
        charged_schedule_work,
        charged_transform_work,
        charged_registry_validation,
        charged_registry_sort,
        charged_coverage_work,
        charged_surface_vertex_visits,
        charged_pair_classification_visits,
        charged_axis_tests,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .expect("fixture-derived logical work");

    let pending_bytes = max_collision_leaves * size_of::<DyadicLeafV2>();
    let angle_box_bytes = hinge_count * size_of::<(ori_domain::EdgeId, OutwardIntervalV1)>();
    let leaf_overhead = size_of::<
        ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafV2<'static>,
    >() - registry_shell;
    let leaf_retained = registry.retained_registry_bytes() + leaf_overhead;
    let face_aabb_bytes = face_count * size_of::<ThickFaceAabbV2>();
    let schedule_phase =
        session_resources.steady_retained_bytes() + pending_bytes + schedule_bound.peak_bytes();
    let registry_phase = session_resources.steady_retained_bytes()
        + pending_bytes
        + angle_box_bytes
        + registry.construction_peak_bytes()
        + leaf_overhead;
    let pair_phase =
        session_resources.steady_retained_bytes() + pending_bytes + leaf_retained + face_aabb_bytes;
    let temporary_bytes = session_resources
        .revalidation_phase_peak_bytes()
        .max(schedule_phase)
        .max(registry_phase)
        .max(pair_phase);
    let publication_bytes = size_of::<OrdinaryIntervalEvidenceV2>();
    let aggregate_peak_bytes =
        temporary_bytes.max(session_resources.steady_retained_bytes() + publication_bytes);

    let limits = OrdinaryIntervalLimitsV2 {
        max_faces: face_count,
        max_hinges: hinge_count,
        max_boundary_vertex_occurrences: boundary_vertex_occurrences,
        max_excluded_shared_pairs: fixture.excluded_shared_pairs.len(),
        max_shared_feature_membership_tests: charged_shared_feature_membership_tests,
        max_collision_depth,
        max_collision_leaves,
        schedule_limits,
        max_bridge_retained_bytes: fixture.bridge.retained_bytes_upper_bound_v2(),
        max_bridge_revalidation_peak_bytes: fixture.bridge.revalidation_peak_bytes_upper_bound_v2(),
        max_schedule_retained_bytes: fixture
            .schedule
            .checked_deep_retained_bytes_v1()
            .expect("checked schedule retained bytes"),
        max_session_shell_bytes: session_resources.session_shell_bytes(),
        max_schedule_evaluation_workspace_bytes: schedule_bound.peak_bytes(),
        max_bridge_partition_search_work_per_node: session_resources
            .coverage_search_comparison_upper_bound(),
        max_interval_transform_work_per_node: INTERVAL_TRANSFORM_OPERATIONAL_WORK_CAP,
        max_interval_registry_validation_work_per_node: registry.validation_work_upper_bound(),
        max_interval_registry_sort_comparisons_per_node: registry.sort_comparison_upper_bound(),
        max_interval_registry_workspace_bytes: registry.construction_peak_bytes(),
        max_interval_registry_retained_bytes: registry.retained_registry_bytes(),
        max_ordinary_pair_node_tests: charged_ordinary_pair_node_tests,
        max_logical_work: charged_logical_work,
        max_temporary_bytes: temporary_bytes,
        max_publication_bytes: publication_bytes,
        max_aggregate_peak_bytes: aggregate_peak_bytes,
    };
    let resources = super::super::resources::preflight_resources_v2(
        &input_v2(fixture, limits),
        boundary_vertex_occurrences,
        schedule_bound,
        registry,
        session_resources,
    )
    .expect("strict fixture-derived structural preflight");
    assert_eq!(resources.charged_logical_work, charged_logical_work);
    assert_eq!(resources.charged_temporary_bytes, temporary_bytes);
    assert_eq!(resources.charged_aggregate_peak_bytes, aggregate_peak_bytes);
    limits
}

pub(super) fn input_v2<'a>(
    fixture: &'a OrdinaryFixtureV2,
    limits: OrdinaryIntervalLimitsV2,
) -> OrdinaryIntervalInputV2<'a> {
    OrdinaryIntervalInputV2 {
        geometry: &fixture.fixture.geometry,
        audit: &fixture.fixture.audit,
        pose: &fixture.pose,
        fixed_face: fixture.fixture.parent_fixed_face,
        schedule: &fixture.schedule,
        decomposition: &fixture.fixture.decomposition,
        common_pose: &fixture.common_pose,
        profile: &fixture.fixture.profile,
        dynamic_closure_bridge: &fixture.bridge,
        paper_thickness_mm: fixture.fixture.paper.thickness_mm,
        closure_tolerance: fixture.fixture.closure_tolerance,
        excluded_shared_pairs: &fixture.excluded_shared_pairs,
        limits,
    }
}

pub(super) fn bridge_revalidation_input_v2<'a>(
    fixture: &'a OrdinaryFixtureV2,
) -> CommonArticulationDynamicClosureBridgeRevalidationInputV2<'a> {
    CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
        geometry: &fixture.fixture.geometry,
        audit: &fixture.fixture.audit,
        pose: &fixture.pose,
        parent_fixed_face: fixture.fixture.parent_fixed_face,
        parent_schedule: &fixture.schedule,
        decomposition: &fixture.fixture.decomposition,
        common_pose: &fixture.common_pose,
        paper_thickness_mm: fixture.fixture.paper.thickness_mm,
        closure_tolerance: fixture.fixture.closure_tolerance,
        profile: &fixture.fixture.profile,
    }
}

fn exact_shared_pairs_v2(geometry: &MaterialHingeGraphGeometry) -> Vec<OrdinaryIntervalFacePairV2> {
    let mut result = Vec::new();
    for first in 0..geometry.face_ids().len() {
        for second in first + 1..geometry.face_ids().len() {
            let first_vertices = geometry
                .face_boundary_vertices(geometry.face_ids()[first])
                .unwrap();
            let second_vertices = geometry
                .face_boundary_vertices(geometry.face_ids()[second])
                .unwrap();
            if first_vertices
                .iter()
                .any(|vertex| second_vertices.contains(vertex))
            {
                result.push(
                    OrdinaryIntervalFacePairV2::new(
                        geometry.face_ids()[first],
                        geometry.face_ids()[second],
                    )
                    .unwrap(),
                );
            }
        }
    }
    result
}

fn nonstationary_schedule_v2(fixture: &MiuraFixtureV2) -> CanonicalCycleScheduleV1 {
    let first_block = &fixture.decomposition.blocks()[0];
    let moving = (0..3)
        .flat_map(|axis_index| {
            first_block
                .geometry()
                .hinges()
                .iter()
                .filter(move |hinge| {
                    [hinge.axis().x(), hinge.axis().y(), hinge.axis().z()][axis_index].abs() == 1.0
                        && hinge.assignment() == ori_topology::FoldAssignment::Mountain
                })
                .map(move |reference| {
                    let reference_start = [
                        reference.start().x(),
                        reference.start().y(),
                        reference.start().z(),
                    ];
                    first_block
                        .geometry()
                        .hinges()
                        .iter()
                        .filter(|hinge| {
                            let axis = [hinge.axis().x(), hinge.axis().y(), hinge.axis().z()];
                            let start = [hinge.start().x(), hinge.start().y(), hinge.start().z()];
                            axis[axis_index].abs() == 1.0
                                && axis
                                    .iter()
                                    .enumerate()
                                    .all(|(i, value)| i == axis_index || *value == 0.0)
                                && start.iter().enumerate().all(|(i, value)| {
                                    i == axis_index
                                        || value.to_bits() == reference_start[i].to_bits()
                                })
                                && hinge.assignment() == ori_topology::FoldAssignment::Mountain
                        })
                        .map(|hinge| hinge.edge())
                        .collect::<HashSet<_>>()
                })
        })
        .find(|family| family.len() == 3)
        .expect("parallel all-mountain carrier family");
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let slope = RationalCoefficientV1 {
        numerator: 1,
        denominator: 2,
    };
    let mut entries = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let moves = moving.contains(&hinge.edge());
            CycleScheduleEntryInputV1 {
                edge: hinge.edge(),
                initial_angle_degrees_bits: if moves {
                    1.0_f64.to_bits()
                } else {
                    0.0_f64.to_bits()
                },
                chebyshev_coefficients: if moves { vec![zero, slope] } else { vec![zero] },
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.parent_fixed_face,
        [-1.0, 1.0],
        entries,
        CycleScheduleLimitsV1 {
            max_hinges: fixture.geometry.hinges().len(),
            max_degree: 1,
            max_coefficient_bits: 53,
            max_work: 1 << 30,
        },
    )
    .expect("nonzero general-N schedule");
    let moving_edge = *moving.iter().next().unwrap();
    let endpoint = |parameter| {
        schedule
            .evaluate(parameter)
            .unwrap()
            .as_slice()
            .iter()
            .find(|angle| angle.edge() == moving_edge)
            .unwrap()
            .angle_degrees()
    };
    assert_eq!(endpoint(-1.0).to_bits(), 0.5_f64.to_bits());
    assert_eq!(endpoint(1.0).to_bits(), 1.5_f64.to_bits());
    schedule
}

pub(super) fn map_stop_v2(stop: OrdinaryIntervalStopV2) -> DyadicIntervalClosureStopV1 {
    match stop {
        OrdinaryIntervalStopV2::Cancelled => DyadicIntervalClosureStopV1::Cancelled,
        OrdinaryIntervalStopV2::DeadlineExceeded => DyadicIntervalClosureStopV1::DeadlineExceeded,
    }
}
