use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, HalfAngleRationalEntryInputV1, Point3, RationalCoefficientV1,
};

mod adaptive_and_control;
mod exact_parallel_cut;
mod stationary_and_half_angle;

fn face(id: FaceId) -> Face {
    Face {
        id,
        key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
        outer: BoundaryWalk {
            half_edges: Vec::new(),
            signed_double_area: 1.0,
        },
        holes: Vec::new(),
        seams: Vec::new(),
        area: 0.5,
    }
}

fn topology(faces: &[FaceId], hinges: &[(EdgeId, FaceId, FaceId)]) -> TopologySnapshot {
    TopologySnapshot {
        source_revision: 1,
        faces: faces.iter().copied().map(face).collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: hinges
            .iter()
            .map(|(edge, first, second)| FaceAdjacency {
                edge: *edge,
                first: *first,
                second: *second,
                assignment: FoldAssignment::Mountain,
            })
            .collect(),
        material_components: Vec::new(),
    }
}

struct Fixture {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    ordinary: CanonicalCycleScheduleV1,
    exact: CanonicalCycleScheduleV1,
    schedule_limits: CycleScheduleLimitsV1,
}

fn fixture() -> Fixture {
    let namespace = ProjectId::schema_namespace([
        0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
        0x40,
    ]);
    let faces = [b"workspace-a", b"workspace-b", b"workspace-c"]
        .map(|name| FaceId::derive_v5(namespace, name));
    let edges = [b"workspace-ab", b"workspace-bc", b"workspace-ca"]
        .map(|name| EdgeId::derive_v5(namespace, name));
    let topology = topology(
        &faces,
        &[
            (edges[0], faces[0], faces[1]),
            (edges[1], faces[1], faces[2]),
            (edges[2], faces[2], faces[0]),
        ],
    );
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        audit.faces().to_vec(),
        [
            (edges[2], faces[2], faces[0]),
            (edges[0], faces[0], faces[1]),
            (edges[1], faces[1], faces[2]),
        ]
        .into_iter()
        .map(|(edge, left, right)| {
            TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                left,
                right,
                origin,
                axis,
                axis,
            )
        })
        .collect(),
    );
    let fixed_face = audit.faces()[0];
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: 3,
        max_degree: 0,
        max_coefficient_bits: 8,
        max_work: 1_024,
    };
    let mut canonical_edges = edges.to_vec();
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let one = RationalCoefficientV1 {
        numerator: 1,
        denominator: 1,
    };
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        canonical_edges
            .iter()
            .map(|edge| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: 120.0_f64.to_bits(),
                chebyshev_coefficients: vec![zero],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    let exact = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed_face,
        canonical_edges
            .iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [zero, one],
                numerator_power_coefficients: vec![zero],
                denominator_power_coefficients: vec![one],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    Fixture {
        geometry,
        audit,
        fixed_face,
        ordinary,
        exact,
        schedule_limits,
    }
}

fn nonstationary_exact_tree_fixture() -> Fixture {
    let namespace = ProjectId::schema_namespace([
        0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
        0x50,
    ]);
    let faces = [b"exact-tree-a", b"exact-tree-b"].map(|name| FaceId::derive_v5(namespace, name));
    let edge = EdgeId::derive_v5(namespace, b"exact-tree-edge");
    let topology = topology(&faces, &[(edge, faces[0], faces[1])]);
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        audit.faces().to_vec(),
        vec![TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            faces[0],
            faces[1],
            origin,
            axis,
            axis,
        )],
    );
    let fixed_face = audit.faces()[0];
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: 1,
        max_degree: 1,
        max_coefficient_bits: 64,
        max_work: 4_096,
    };
    let rational = |numerator, denominator| RationalCoefficientV1 {
        numerator,
        denominator,
    };
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        vec![CycleScheduleEntryInputV1 {
            edge,
            initial_angle_degrees_bits: 90.0_f64.to_bits(),
            chebyshev_coefficients: vec![rational(0, 1), rational(1, 1)],
        }],
        schedule_limits,
    )
    .unwrap();
    let exact = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed_face,
        vec![HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [rational(0, 1), rational(1, 1)],
            numerator_power_coefficients: vec![rational(1, 1), rational(1, 1)],
            denominator_power_coefficients: vec![rational(1, 1)],
        }],
        schedule_limits,
    )
    .unwrap();
    Fixture {
        geometry,
        audit,
        fixed_face,
        ordinary,
        exact,
        schedule_limits,
    }
}

fn adaptive_correlated_cycle_fixture() -> Fixture {
    let mut fixture = fixture();
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: fixture.geometry.hinges().len(),
        max_degree: 1,
        max_coefficient_bits: 53,
        max_work: 4_096,
    };
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let mut edges = fixture
        .geometry
        .hinges()
        .iter()
        .map(TreeHinge::edge)
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        [0.0, 1.0],
        edges
            .iter()
            .enumerate()
            .map(|(index, edge)| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: 120.0_f64.to_bits(),
                chebyshev_coefficients: vec![
                    zero,
                    RationalCoefficientV1 {
                        numerator: if index < 2 { 1 } else { -2 },
                        denominator: 1,
                    },
                ],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    fixture.ordinary = ordinary;
    fixture.schedule_limits = schedule_limits;
    fixture
}

fn generous_limits(
    schedule_limits: CycleScheduleLimitsV1,
) -> DyadicIntervalClosureWorkspaceLimitsV2 {
    let ceiling = usize::MAX - 1;
    DyadicIntervalClosureWorkspaceLimitsV2 {
        max_depth: 0,
        max_leaves: 1,
        max_work: 1_000_000,
        schedule_limits,
        max_theorem_recognizer_work: ceiling,
        max_theorem_recognizer_workspace_bytes: ceiling,
        max_carrier_index_workspace_bytes: ceiling,
        max_schedule_evaluation_workspace_bytes: ceiling,
        max_big_rational_payload_bytes: ceiling,
        max_exact_rational_object_bytes: ceiling,
        max_interval_closure_workspace_bytes: ceiling,
        max_partition_workspace_bytes: ceiling,
        max_retained_material_bytes: ceiling,
        max_publication_workspace_bytes: ceiling,
        max_peak_workspace_bytes: ceiling,
    }
}

fn exact_limits(
    mut limits: DyadicIntervalClosureWorkspaceLimitsV2,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
) -> DyadicIntervalClosureWorkspaceLimitsV2 {
    limits.max_theorem_recognizer_work = resources.charged_theorem_recognizer_work;
    limits.max_theorem_recognizer_workspace_bytes =
        resources.charged_theorem_recognizer_upper_bound_bytes;
    limits.max_carrier_index_workspace_bytes =
        resources.charged_carrier_index_workspace_upper_bound_bytes;
    limits.max_schedule_evaluation_workspace_bytes =
        resources.charged_schedule_evaluation_workspace_upper_bound_bytes;
    limits.max_big_rational_payload_bytes =
        resources.charged_big_rational_payload_upper_bound_bytes;
    limits.max_exact_rational_object_bytes =
        resources.charged_exact_rational_object_upper_bound_bytes;
    limits.max_interval_closure_workspace_bytes =
        resources.charged_interval_closure_workspace_upper_bound_bytes;
    limits.max_partition_workspace_bytes = resources.charged_partition_workspace_upper_bound_bytes;
    limits.max_retained_material_bytes = resources.charged_retained_material_upper_bound_bytes;
    limits.max_publication_workspace_bytes =
        resources.charged_publication_workspace_upper_bound_bytes;
    limits.max_peak_workspace_bytes = resources.charged_peak_workspace_upper_bound_bytes;
    limits
}

fn issue(
    fixture: &Fixture,
    schedule: &CanonicalCycleScheduleV1,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> Result<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2, DyadicIntervalClosureControlErrorV1>
{
    issue_at_tolerance(fixture, schedule, limits, 1.0e-8)
}

fn issue_at_tolerance(
    fixture: &Fixture,
    schedule: &CanonicalCycleScheduleV1,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
    tolerance: f64,
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
            || Ok(()),
        )
}
