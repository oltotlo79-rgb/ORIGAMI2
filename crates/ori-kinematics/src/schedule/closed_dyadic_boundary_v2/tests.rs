use ori_domain::ProjectId;

use super::*;

#[path = "tests/binding_tests.rs"]
mod binding_tests;
#[path = "tests/half_angle_tests.rs"]
mod half_angle_tests;
#[path = "tests/ordinary_tests.rs"]
mod ordinary_tests;
#[path = "tests/policy_tests.rs"]
mod policy_tests;

pub(super) fn test_edge_v2(name: &[u8]) -> EdgeId {
    EdgeId::derive_v5(ProjectId::schema_namespace([0x7a; 16]), name)
}

pub(super) const fn rational_v2(numerator: i64, denominator: u64) -> RationalCoefficientV1 {
    RationalCoefficientV1 {
        numerator,
        denominator,
    }
}

pub(super) fn ordinary_schedule_v2(
    domain: [f64; 2],
    mut entries: Vec<Entry>,
) -> CanonicalCycleScheduleV1 {
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    ordinary_schedule_in_order_v2(domain, entries)
}

pub(super) fn ordinary_schedule_in_order_v2(
    domain: [f64; 2],
    entries: Vec<Entry>,
) -> CanonicalCycleScheduleV1 {
    CanonicalCycleScheduleV1 {
        binding_fingerprint: [0x35; 32],
        schedule_fingerprint_v2: schedule_fingerprint_v2(domain, &entries, &[]),
        fixed_face: FaceId::derive_v5(ProjectId::schema_namespace([0x7a; 16]), b"fixed"),
        domain,
        entries,
        half_angle_entries: Vec::new(),
    }
}

pub(super) fn half_angle_schedule_v2(
    inputs: Vec<HalfAngleRationalEntryInputV1>,
) -> CanonicalCycleScheduleV1 {
    let limits = CycleScheduleLimitsV1::default();
    let mut prepared = inputs
        .into_iter()
        .map(|input| PreparedHalfAngleRationalEntryV1::prepare(input, limits).unwrap())
        .collect::<Vec<_>>();
    prepared.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
    CanonicalCycleScheduleV1 {
        binding_fingerprint: [0x35; 32],
        schedule_fingerprint_v2: schedule_fingerprint_v2([0.0, 1.0], &[], &prepared),
        fixed_face: FaceId::derive_v5(ProjectId::schema_namespace([0x7a; 16]), b"fixed"),
        domain: [0.0, 1.0],
        entries: Vec::new(),
        half_angle_entries: prepared,
    }
}

pub(super) fn ordinary_entry_v2(name: &[u8], initial: f64, coefficients: Vec<f64>) -> Entry {
    Entry {
        edge: test_edge_v2(name),
        initial,
        coefficients,
        derivative_bound: 180.0,
    }
}

pub(super) fn prove_exact_v2(
    schedule: &CanonicalCycleScheduleV1,
    limits: CycleScheduleLimitsV1,
) -> CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {
    let bound = schedule
        .checked_closed_dyadic_boundary_resource_bound_v2(limits)
        .unwrap();
    schedule
        .prove_closed_dyadic_boundary_evidence_v2(
            limits,
            bound.logical_work_required_v2(),
            bound.workspace_peak_bytes_upper_bound_v2(),
        )
        .unwrap()
}

pub(super) fn fingerprint_hex_v2(fingerprint: [u8; 32]) -> String {
    fingerprint
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
