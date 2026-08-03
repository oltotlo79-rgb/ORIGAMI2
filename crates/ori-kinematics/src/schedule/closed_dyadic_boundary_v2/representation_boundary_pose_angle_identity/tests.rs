use ori_domain::ProjectId;
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{Point3, TreeHinge, TreeKinematicsLimits};

#[path = "tests/capacity.rs"]
mod capacity;
#[path = "tests/half_angle.rs"]
mod half_angle;
#[path = "tests/ordinary.rs"]
mod ordinary;
#[path = "tests/policy.rs"]
mod policy;

const FIXTURE_CLOSURE_TOLERANCE_V2: f64 = 0.0;

struct RepresentationPoseFixtureV2 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    schedule: CanonicalCycleScheduleV1,
    closed_boundary: CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
    lower_pose: ClosedMaterialHingeGraphPose,
    upper_pose: ClosedMaterialHingeGraphPose,
    schedule_limits: CycleScheduleLimitsV1,
    limits: CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
}

impl RepresentationPoseFixtureV2 {
    fn input_v2(&self) -> CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2<'_> {
        CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2 {
            geometry: &self.geometry,
            audit: &self.audit,
            schedule: &self.schedule,
            closed_boundary_evidence: &self.closed_boundary,
            lower_pose: &self.lower_pose,
            upper_pose: &self.upper_pose,
            schedule_limits: self.schedule_limits,
            limits: self.limits,
        }
    }

    fn fresh_lower_pose_v2(&self) -> ClosedMaterialHingeGraphPose {
        self.geometry
            .solve_closed(
                &self.audit,
                self.lower_pose.fixed_face(),
                self.lower_pose.hinge_angles(),
                FIXTURE_CLOSURE_TOLERANCE_V2,
            )
            .unwrap()
    }
}

fn graph_fixture_v2(
    namespace: ProjectId,
) -> (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    FaceId,
    EdgeId,
) {
    let faces = [&b"fixed"[..], &b"moving"[..]].map(|name| FaceId::derive_v5(namespace, name));
    let edge = EdgeId::derive_v5(namespace, b"hinge");
    let topology = TopologySnapshot {
        source_revision: 1,
        faces: faces
            .iter()
            .map(|id| Face {
                id: *id,
                key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
                outer: BoundaryWalk {
                    half_edges: Vec::new(),
                    signed_double_area: 1.0,
                },
                holes: Vec::new(),
                seams: Vec::new(),
                area: 0.5,
            })
            .collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: vec![FaceAdjacency {
            edge,
            first: faces[0],
            second: faces[1],
            assignment: FoldAssignment::Mountain,
        }],
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let fixed_face = audit.faces()[0];
    let moving_face = audit
        .faces()
        .iter()
        .copied()
        .find(|face| *face != fixed_face)
        .unwrap();
    let start = Point3::new(0.0, 0.0, 0.0).unwrap();
    let end = Point3::new(1.0, 0.0, 0.0).unwrap();
    let hinge = TreeHinge::new_for_test(
        edge,
        FoldAssignment::Mountain,
        fixed_face,
        moving_face,
        start,
        end,
        end,
    );
    (
        MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), vec![hinge]),
        audit,
        fixed_face,
        edge,
    )
}

fn ordinary_fixture_v2() -> RepresentationPoseFixtureV2 {
    let (geometry, audit, fixed_face, edge) =
        graph_fixture_v2(ProjectId::schema_namespace([0xa4; 16]));
    let schedule_limits = CycleScheduleLimitsV1::default();
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [90.0, 180.0],
        vec![CycleScheduleEntryInputV1 {
            edge,
            initial_angle_degrees_bits: 22.5_f64.to_bits(),
            chebyshev_coefficients: vec![rational_v2(0, 1), rational_v2(45, 2)],
        }],
        schedule_limits,
    )
    .unwrap();
    let mut meter = resources::BoundaryWorkMeterV2::new(1_000);
    let lower_angles = CanonicalHingeAngles::new(vec![
        evaluate::evaluate_ordinary_endpoint_angle_v2(
            &schedule.entries[0],
            -1.0,
            &mut meter,
            &mut || Ok(()),
        )
        .unwrap(),
    ])
    .unwrap();
    let upper_angles = CanonicalHingeAngles::new(vec![
        evaluate::evaluate_ordinary_endpoint_angle_v2(
            &schedule.entries[0],
            1.0,
            &mut meter,
            &mut || Ok(()),
        )
        .unwrap(),
    ])
    .unwrap();
    finish_fixture_v2(
        geometry,
        audit,
        schedule,
        fixed_face,
        lower_angles,
        upper_angles,
        schedule_limits,
    )
}

fn half_angle_fixture_v2() -> RepresentationPoseFixtureV2 {
    let (geometry, audit, fixed_face, edge) =
        graph_fixture_v2(ProjectId::schema_namespace([0xb5; 16]));
    let schedule_limits = CycleScheduleLimitsV1::default();
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed_face,
        vec![HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [rational_v2(-411, 1), rational_v2(127, 73)],
            numerator_power_coefficients: vec![rational_v2(411, 1), rational_v2(1, 1)],
            denominator_power_coefficients: vec![rational_v2(1_000, 1)],
        }],
        schedule_limits,
    )
    .unwrap();
    let lower_angles = schedule.try_evaluate_v1(0.0).unwrap();
    let upper_angles = schedule.try_evaluate_v1(1.0).unwrap();
    finish_fixture_v2(
        geometry,
        audit,
        schedule,
        fixed_face,
        lower_angles,
        upper_angles,
        schedule_limits,
    )
}

fn ordinary_constant_fixture_v2() -> RepresentationPoseFixtureV2 {
    let (geometry, audit, fixed_face, edge) =
        graph_fixture_v2(ProjectId::schema_namespace([0xc6; 16]));
    let schedule_limits = CycleScheduleLimitsV1 {
        max_degree: 0,
        max_coefficient_bits: 0,
        ..CycleScheduleLimitsV1::default()
    };
    let prepare_limits = CycleScheduleLimitsV1 {
        max_coefficient_bits: 1,
        ..schedule_limits
    };
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [90.0, 180.0],
        vec![CycleScheduleEntryInputV1 {
            edge,
            initial_angle_degrees_bits: 30.0_f64.to_bits(),
            chebyshev_coefficients: vec![rational_v2(0, 1)],
        }],
        prepare_limits,
    )
    .unwrap();
    let mut meter = resources::BoundaryWorkMeterV2::new(1_000);
    let angle = evaluate::evaluate_ordinary_endpoint_angle_v2(
        &schedule.entries[0],
        -1.0,
        &mut meter,
        &mut || Ok(()),
    )
    .unwrap();
    let angles = CanonicalHingeAngles::new(vec![angle]).unwrap();
    finish_fixture_v2(
        geometry,
        audit,
        schedule,
        fixed_face,
        angles.clone(),
        angles,
        schedule_limits,
    )
}

fn half_angle_constant_fixture_v2() -> RepresentationPoseFixtureV2 {
    let (geometry, audit, fixed_face, edge) =
        graph_fixture_v2(ProjectId::schema_namespace([0xd7; 16]));
    let schedule_limits = CycleScheduleLimitsV1 {
        max_degree: 0,
        max_coefficient_bits: 1,
        ..CycleScheduleLimitsV1::default()
    };
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed_face,
        vec![HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [rational_v2(0, 1), rational_v2(1, 1)],
            numerator_power_coefficients: vec![rational_v2(1, 1)],
            denominator_power_coefficients: vec![rational_v2(1, 1)],
        }],
        schedule_limits,
    )
    .unwrap();
    let angles = schedule.try_evaluate_v1(0.0).unwrap();
    finish_fixture_v2(
        geometry,
        audit,
        schedule,
        fixed_face,
        angles.clone(),
        angles,
        schedule_limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_fixture_v2(
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    schedule: CanonicalCycleScheduleV1,
    fixed_face: FaceId,
    lower_angles: CanonicalHingeAngles,
    upper_angles: CanonicalHingeAngles,
    schedule_limits: CycleScheduleLimitsV1,
) -> RepresentationPoseFixtureV2 {
    let lower_pose = geometry
        .solve_closed(
            &audit,
            fixed_face,
            &lower_angles,
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let upper_pose = geometry
        .solve_closed(
            &audit,
            fixed_face,
            &upper_angles,
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let closed_bound = schedule
        .checked_closed_dyadic_boundary_resource_bound_v2(schedule_limits)
        .unwrap();
    let closed_boundary = schedule
        .prove_closed_dyadic_boundary_evidence_v2(
            schedule_limits,
            closed_bound.logical_work_required_v2(),
            closed_bound.workspace_peak_bytes_upper_bound_v2(),
        )
        .unwrap();
    let bound = schedule
        .checked_representation_boundary_pose_angle_identity_resource_bound_v2(
            &geometry,
            &audit,
            &lower_pose,
            &upper_pose,
            schedule_limits,
        )
        .unwrap();
    let limits = exact_limits_v2(bound);
    RepresentationPoseFixtureV2 {
        geometry,
        audit,
        schedule,
        closed_boundary,
        lower_pose,
        upper_pose,
        schedule_limits,
        limits,
    }
}

const fn rational_v2(numerator: i64, denominator: u64) -> RationalCoefficientV1 {
    RationalCoefficientV1 {
        numerator,
        denominator,
    }
}

const fn exact_limits_v2(
    bound: CycleScheduleRepresentationBoundaryPoseAngleIdentityResourceBoundV2,
) -> CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
        max_hinges: bound.hinge_count_v2(),
        max_schedule_deep_retained_bytes: bound.schedule_deep_retained_bytes_v2(),
        max_representation_boundary_poses_deep_retained_bytes: bound
            .representation_boundary_poses_deep_retained_bytes_v2(),
        max_logical_work: bound.logical_work_required_v2(),
        max_workspace_bytes: bound.workspace_peak_bytes_upper_bound_v2(),
    }
}
