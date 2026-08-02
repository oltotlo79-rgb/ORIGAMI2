use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{Point3, TreeHinge, TreeKinematicsLimits};

fn fixture() -> (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    FaceId,
    Vec<EdgeId>,
) {
    let ns = ProjectId::new();
    let faces = [b"a", b"b", b"c"].map(|name| FaceId::derive_v5(ns, name));
    let edges = [b"ab", b"bc", b"ca"].map(|name| EdgeId::derive_v5(ns, name));
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
        hinge_adjacency: (0..3)
            .map(|i| FaceAdjacency {
                edge: edges[i],
                first: faces[i],
                second: faces[(i + 1) % 3],
                assignment: FoldAssignment::Mountain,
            })
            .collect(),
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let start = Point3::new(0.0, 0.0, 0.0).unwrap();
    let end = Point3::new(1.0, 0.0, 0.0).unwrap();
    let hinges = (0..3)
        .map(|i| {
            TreeHinge::new_for_test(
                edges[i],
                FoldAssignment::Mountain,
                faces[i],
                faces[(i + 1) % 3],
                start,
                end,
                end,
            )
        })
        .collect();
    (
        MaterialHingeGraphGeometry::new_for_test(faces.to_vec(), hinges),
        audit,
        faces[0],
        edges.to_vec(),
    )
}

fn single_hinge_block(
    source: &MaterialHingeGraphGeometry,
    edge: EdgeId,
) -> (MaterialHingeGraphGeometry, MaterialHingeGraphAudit, FaceId) {
    let hinge = source
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == edge)
        .unwrap();
    let faces = [hinge.left_face(), hinge.right_face()];
    let topology = TopologySnapshot {
        source_revision: 2,
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
            first: hinge.left_face(),
            second: hinge.right_face(),
            assignment: hinge.assignment(),
        }],
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let fixed = audit.faces()[0];
    (
        MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), vec![hinge.clone()]),
        audit,
        fixed,
    )
}

fn single_hinge_foreign_edge_audit(
    geometry: &MaterialHingeGraphGeometry,
    foreign_edge: EdgeId,
) -> MaterialHingeGraphAudit {
    let hinge = &geometry.hinges()[0];
    let topology = TopologySnapshot {
        source_revision: 3,
        faces: geometry
            .face_ids()
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
            edge: foreign_edge,
            first: hinge.left_face(),
            second: hinge.right_face(),
            assignment: hinge.assignment(),
        }],
        material_components: Vec::new(),
    };
    MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap()
}

fn entries(edges: &[EdgeId]) -> Vec<CycleScheduleEntryInputV1> {
    let mut entries = edges
        .iter()
        .map(|edge| CycleScheduleEntryInputV1 {
            edge: *edge,
            initial_angle_degrees_bits: 90.0_f64.to_bits(),
            chebyshev_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 10,
                    denominator: 1,
                },
            ],
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    entries
}

fn fingerprint_test_edge(name: &[u8]) -> EdgeId {
    let namespace = ProjectId::schema_namespace([
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f,
    ]);
    EdgeId::derive_v5(namespace, name)
}

const fn rational(numerator: i64, denominator: u64) -> RationalCoefficientV1 {
    RationalCoefficientV1 {
        numerator,
        denominator,
    }
}

fn prepared_half_angle_fingerprint_entry(
    edge: EdgeId,
    u_domain: [RationalCoefficientV1; 2],
    numerator: Vec<RationalCoefficientV1>,
    denominator: Vec<RationalCoefficientV1>,
) -> PreparedHalfAngleRationalEntryV1 {
    PreparedHalfAngleRationalEntryV1::prepare(
        HalfAngleRationalEntryInputV1 {
            edge,
            u_domain,
            numerator_power_coefficients: numerator,
            denominator_power_coefficients: denominator,
        },
        CycleScheduleLimitsV1::default(),
    )
    .expect("the fingerprint fixture must be an admitted half-angle profile")
}

fn legacy_unframed_schedule_preimage_for_regression(
    entries: &[Entry],
    half_angle_entries: &[PreparedHalfAngleRationalEntryV1],
) -> Vec<u8> {
    let mut preimage = Vec::new();
    for entry in entries {
        preimage.extend(entry.edge.canonical_bytes());
        preimage.extend(entry.initial.to_bits().to_be_bytes());
        for coefficient in &entry.coefficients {
            preimage.extend(coefficient.to_bits().to_be_bytes());
        }
    }
    for entry in half_angle_entries {
        preimage.extend(entry.edge.canonical_bytes());
        for value in entry
            .u_domain
            .iter()
            .chain(&entry.numerator_power_coefficients)
            .chain(&entry.denominator_power_coefficients)
        {
            let (numerator_sign, numerator) = value.numer().to_bytes_be();
            let (_, denominator) = value.denom().to_bytes_be();
            preimage.extend([match numerator_sign {
                num_bigint::Sign::Minus => 0,
                num_bigint::Sign::NoSign => 1,
                num_bigint::Sign::Plus => 2,
            }]);
            preimage.extend(
                u64::try_from(numerator.len())
                    .expect("test numerator length must fit u64")
                    .to_be_bytes(),
            );
            preimage.extend(numerator);
            preimage.extend(
                u64::try_from(denominator.len())
                    .expect("test denominator length must fit u64")
                    .to_be_bytes(),
            );
            preimage.extend(denominator);
        }
    }
    preimage
}

fn fingerprint_hex(fingerprint: [u8; 32]) -> String {
    fingerprint
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn exact_common_linear_schedule_for_test(edge_count: usize) -> CanonicalCycleScheduleV1 {
    let mut edges = (0..edge_count)
        .map(|index| fingerprint_test_edge(format!("common-linear-{index}").as_bytes()))
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let entries = edges
        .into_iter()
        .map(|edge| Entry {
            edge,
            initial: 90.0,
            coefficients: vec![0.0, 10.0],
            derivative_bound: 20.0,
        })
        .collect::<Vec<_>>();
    CanonicalCycleScheduleV1 {
        binding_fingerprint: [0x6d; 32],
        schedule_fingerprint_v2: schedule_fingerprint_v2([0.0, 1.0], &entries, &[]),
        fixed_face: FaceId::derive_v5(
            ProjectId::schema_namespace([0x42; 16]),
            b"common-linear-fixed",
        ),
        domain: [0.0, 1.0],
        entries,
        half_angle_entries: Vec::new(),
    }
}

fn refresh_exact_common_linear_schedule_fingerprint_v2(schedule: &mut CanonicalCycleScheduleV1) {
    schedule.schedule_fingerprint_v2 = schedule_fingerprint_v2(
        schedule.domain,
        &schedule.entries,
        &schedule.half_angle_entries,
    );
}

fn unbounded_exact_common_linear_limits_v1() -> ExactCommonLinearCycleProfileLimitsV1 {
    ExactCommonLinearCycleProfileLimitsV1 {
        max_edges: EXACT_COMMON_LINEAR_MAX_EDGES_V1,
        max_work: usize::MAX,
        max_retained_bytes: usize::MAX,
        max_peak_bytes: usize::MAX,
    }
}

#[test]
fn exact_common_linear_profile_accepts_complete_two_and_three_edge_carriers() {
    for edge_count in [2, 3] {
        let schedule = exact_common_linear_schedule_for_test(edge_count);
        let canonical = schedule
            .entries
            .iter()
            .map(|entry| entry.edge)
            .collect::<Vec<_>>();
        let first = schedule
            .prove_exact_common_linear_profile_v1(
                &canonical,
                ExactCommonLinearCycleProfileLimitsV1::default(),
            )
            .expect("a complete bit-identical linear carrier must be proved");
        let mut reversed = canonical.clone();
        reversed.reverse();
        let reordered = schedule
            .prove_exact_common_linear_profile_v1(
                &reversed,
                ExactCommonLinearCycleProfileLimitsV1::default(),
            )
            .expect("caller edge order is canonicalized internally");

        assert_eq!(first, reordered);
        assert_eq!(first.edge_ids(), canonical);
        assert!(!first.authorizes_closure());
        assert!(!first.authorizes_collision_clearance());
        assert!(!first.authorizes_project_mutation());
        first
            .revalidate_issuer_schedule_v1(
                &schedule,
                ExactCommonLinearCycleProfileLimitsV1::default(),
            )
            .expect("the exact issuer must revalidate its opaque proof");
    }
}

#[test]
fn exact_common_linear_profile_rejects_ordinary_negative_matrix() {
    let schedule = exact_common_linear_schedule_for_test(3);
    let edges = schedule
        .entries
        .iter()
        .map(|entry| entry.edge)
        .collect::<Vec<_>>();

    assert_eq!(
        schedule.prove_exact_common_linear_profile_v1(
            &edges[..1],
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
    );
    let fourth = fingerprint_test_edge(b"common-linear-fourth");
    assert_eq!(
        schedule.prove_exact_common_linear_profile_v1(
            &[edges[0], edges[1], edges[2], fourth],
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
    );
    assert_eq!(
        schedule.prove_exact_common_linear_profile_v1(
            &[edges[0], edges[0], edges[1]],
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
    );
    assert_eq!(
        schedule.prove_exact_common_linear_profile_v1(
            &edges[..2],
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::CarrierSetMismatch)
    );

    let two_edge_schedule = exact_common_linear_schedule_for_test(2);
    let mut extra = two_edge_schedule
        .entries
        .iter()
        .map(|entry| entry.edge)
        .collect::<Vec<_>>();
    extra.push(fourth);
    assert_eq!(
        two_edge_schedule.prove_exact_common_linear_profile_v1(
            &extra,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::CarrierSetMismatch)
    );

    let mut noncanonical_schedule = schedule.clone();
    noncanonical_schedule.entries.swap(0, 1);
    refresh_exact_common_linear_schedule_fingerprint_v2(&mut noncanonical_schedule);
    assert_eq!(
        noncanonical_schedule.prove_exact_common_linear_profile_v1(
            &edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::CarrierSetMismatch)
    );

    let mut half_angle = schedule.clone();
    half_angle.entries.clear();
    half_angle
        .half_angle_entries
        .push(prepared_half_angle_fingerprint_entry(
            edges[0],
            [rational(0, 1), rational(1, 1)],
            vec![rational(0, 1), rational(1, 1)],
            vec![rational(1, 1)],
        ));
    assert_eq!(
        half_angle.prove_exact_common_linear_profile_v1(
            &edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::UnsupportedRepresentation)
    );

    for coefficients in [vec![10.0], vec![0.0, 10.0, 0.0], vec![1.0, 0.0]] {
        let mut invalid_degree = schedule.clone();
        for entry in &mut invalid_degree.entries {
            entry.coefficients = coefficients.clone();
        }
        refresh_exact_common_linear_schedule_fingerprint_v2(&mut invalid_degree);
        assert_eq!(
            invalid_degree.prove_exact_common_linear_profile_v1(
                &edges,
                ExactCommonLinearCycleProfileLimitsV1::default(),
            ),
            Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
        );
    }

    for invalid in [f64::NAN, f64::INFINITY, -0.0] {
        for endpoint in 0..2 {
            let mut invalid_domain = schedule.clone();
            invalid_domain.domain[endpoint] = invalid;
            assert_eq!(
                invalid_domain.prove_exact_common_linear_profile_v1(
                    &edges,
                    ExactCommonLinearCycleProfileLimitsV1::default(),
                ),
                Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
            );
        }

        let mut invalid_initial = schedule.clone();
        invalid_initial.entries[0].initial = invalid;
        assert_eq!(
            invalid_initial.prove_exact_common_linear_profile_v1(
                &edges,
                ExactCommonLinearCycleProfileLimitsV1::default(),
            ),
            Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
        );

        for coefficient in 0..2 {
            let mut invalid_coefficient = schedule.clone();
            invalid_coefficient.entries[0].coefficients[coefficient] = invalid;
            assert_eq!(
                invalid_coefficient.prove_exact_common_linear_profile_v1(
                    &edges,
                    ExactCommonLinearCycleProfileLimitsV1::default(),
                ),
                Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
            );
        }
    }

    let mut reversed_domain = schedule.clone();
    reversed_domain.domain = [1.0, 0.0];
    assert_eq!(
        reversed_domain.prove_exact_common_linear_profile_v1(
            &edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
    );

    for field in 0..3 {
        let mut different_profile = schedule.clone();
        match field {
            0 => {
                different_profile.entries[1].initial =
                    f64::from_bits(different_profile.entries[1].initial.to_bits() + 1);
            }
            1 | 2 => {
                different_profile.entries[1].coefficients[field - 1] = f64::from_bits(
                    different_profile.entries[1].coefficients[field - 1].to_bits() + 1,
                );
            }
            _ => unreachable!(),
        }
        refresh_exact_common_linear_schedule_fingerprint_v2(&mut different_profile);
        assert_eq!(
            different_profile.prove_exact_common_linear_profile_v1(
                &edges,
                ExactCommonLinearCycleProfileLimitsV1::default(),
            ),
            Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
        );
    }
}

#[test]
fn exact_common_linear_profile_does_not_accept_equal_endpoint_or_bound_surrogates() {
    let schedule = exact_common_linear_schedule_for_test(2);
    let edges = schedule
        .entries
        .iter()
        .map(|entry| entry.edge)
        .collect::<Vec<_>>();
    let mut interior_different = schedule.clone();
    for entry in &mut interior_different.entries {
        entry.initial = 89.0;
        entry.coefficients = vec![0.0, 10.0, 1.0];
        // This observation cache is deliberately kept bit-identical: it
        // must never substitute for exact coefficient identity.
        entry.derivative_bound = 20.0;
    }
    refresh_exact_common_linear_schedule_fingerprint_v2(&mut interior_different);

    assert_eq!(schedule.evaluate(0.0), interior_different.evaluate(0.0));
    assert_eq!(schedule.evaluate(1.0), interior_different.evaluate(1.0));
    assert!(
        schedule
            .entries
            .iter()
            .zip(&interior_different.entries)
            .all(|(first, second)| first.derivative_bound.to_bits()
                == second.derivative_bound.to_bits())
    );
    assert_eq!(
        interior_different.prove_exact_common_linear_profile_v1(
            &edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::InvalidInput)
    );
}

#[test]
fn exact_common_linear_profile_revalidation_binds_issuer_fingerprint_and_every_profile_bit() {
    let schedule = exact_common_linear_schedule_for_test(3);
    let edges = schedule
        .entries
        .iter()
        .map(|entry| entry.edge)
        .collect::<Vec<_>>();
    let proof = schedule
        .prove_exact_common_linear_profile_v1(
            &edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        )
        .unwrap();

    let mut foreign = schedule.clone();
    foreign.binding_fingerprint[0] ^= 1;
    assert_eq!(
        proof.revalidate_issuer_schedule_v1(
            &foreign,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch)
    );

    let mut forged_fingerprint = schedule.clone();
    forged_fingerprint.schedule_fingerprint_v2[0] ^= 1;
    assert_eq!(
        forged_fingerprint.prove_exact_common_linear_profile_v1(
            &edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch)
    );

    let mut one_ulp = schedule.clone();
    for entry in &mut one_ulp.entries {
        entry.initial = f64::from_bits(entry.initial.to_bits() + 1);
    }
    refresh_exact_common_linear_schedule_fingerprint_v2(&mut one_ulp);
    assert_eq!(
        proof.revalidate_issuer_schedule_v1(
            &one_ulp,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch)
    );

    let mut forged_proof = proof.clone();
    forged_proof.proof_fingerprint_v1[31] ^= 1;
    assert_eq!(
        forged_proof.revalidate_issuer_schedule_v1(
            &schedule,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        ),
        Err(ExactCommonLinearCycleProfileErrorV1::IssuerMismatch)
    );
}

#[test]
fn exact_common_linear_profile_limits_are_exact_at_equality_and_one_short() {
    let schedule = exact_common_linear_schedule_for_test(3);
    let edges = schedule
        .entries
        .iter()
        .map(|entry| entry.edge)
        .collect::<Vec<_>>();
    let mut audit_meter =
        ExactCommonLinearCycleProfileMeterV1::new(unbounded_exact_common_linear_limits_v1());
    schedule
        .prove_exact_common_linear_profile_v1_with_meter(&edges, &mut audit_meter)
        .unwrap();
    assert_eq!(
        audit_meter.retained_bytes,
        exact_common_linear_retained_bytes_v1(edges.len()).unwrap()
    );
    assert_eq!(
        audit_meter.peak_bytes,
        edges.len() * EXACT_COMMON_LINEAR_EDGE_BYTES_V1
            + 2 * EXACT_COMMON_LINEAR_FINGERPRINT_BYTES_V1
            + EXACT_COMMON_LINEAR_SHA256_SCRATCH_BYTES_V1
    );

    let exact = ExactCommonLinearCycleProfileLimitsV1 {
        max_edges: edges.len(),
        max_work: audit_meter.work,
        max_retained_bytes: audit_meter.retained_bytes,
        max_peak_bytes: audit_meter.peak_bytes,
    };
    schedule
        .prove_exact_common_linear_profile_v1(&edges, exact)
        .expect("every exact limit must admit equality");

    for one_short in [
        ExactCommonLinearCycleProfileLimitsV1 {
            max_edges: exact.max_edges - 1,
            ..exact
        },
        ExactCommonLinearCycleProfileLimitsV1 {
            max_work: exact.max_work - 1,
            ..exact
        },
        ExactCommonLinearCycleProfileLimitsV1 {
            max_retained_bytes: exact.max_retained_bytes - 1,
            ..exact
        },
        ExactCommonLinearCycleProfileLimitsV1 {
            max_peak_bytes: exact.max_peak_bytes - 1,
            ..exact
        },
    ] {
        assert_eq!(
            schedule.prove_exact_common_linear_profile_v1(&edges, one_short),
            Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)
        );
    }
}

#[test]
fn exact_common_linear_profile_meter_fails_closed_on_checked_overflow() {
    let limits = unbounded_exact_common_linear_limits_v1();

    let mut work = ExactCommonLinearCycleProfileMeterV1::new(limits);
    work.work = usize::MAX;
    assert_eq!(
        work.charge_work(1),
        Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)
    );

    let mut retained = ExactCommonLinearCycleProfileMeterV1::new(limits);
    retained.retained_bytes = usize::MAX;
    assert_eq!(
        retained.retain(1),
        Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)
    );

    let mut temporary = ExactCommonLinearCycleProfileMeterV1::new(limits);
    temporary.temporary_bytes = usize::MAX;
    assert_eq!(
        temporary.begin_temporary(1),
        Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)
    );

    assert_eq!(
        exact_common_linear_retained_bytes_v1(usize::MAX),
        Err(ExactCommonLinearCycleProfileErrorV1::ResourceLimit)
    );
}

#[test]
fn schedule_fingerprint_v2_separates_half_angle_p_q_flatten_collision() {
    let edge = fingerprint_test_edge(b"p-q-flatten-collision");
    let first = prepared_half_angle_fingerprint_entry(
        edge,
        [rational(0, 1), rational(1, 1)],
        vec![rational(1, 1)],
        vec![rational(2, 1), rational(3, 1)],
    );
    let second = prepared_half_angle_fingerprint_entry(
        edge,
        [rational(0, 1), rational(1, 1)],
        vec![rational(1, 1), rational(2, 1)],
        vec![rational(3, 1)],
    );
    assert_eq!(
        legacy_unframed_schedule_preimage_for_regression(&[], core::slice::from_ref(&first),),
        legacy_unframed_schedule_preimage_for_regression(&[], core::slice::from_ref(&second),),
        "the regression fixtures must reproduce the V1 P/Q boundary collision"
    );
    assert_ne!(
        schedule_fingerprint_v2([0.0, 1.0], &[], &[first]),
        schedule_fingerprint_v2([0.0, 1.0], &[], &[second]),
        "independent P/Q counts must frame the V2 preimage"
    );
}

#[test]
fn schedule_fingerprint_v2_binds_outer_ordinary_domain() {
    let entry = Entry {
        edge: fingerprint_test_edge(b"ordinary-domain"),
        initial: 90.0,
        coefficients: vec![0.0, 10.0],
        derivative_bound: 20.0,
    };
    assert_ne!(
        schedule_fingerprint_v2([0.0, 1.0], core::slice::from_ref(&entry), &[]),
        schedule_fingerprint_v2([0.0, 2.0], core::slice::from_ref(&entry), &[]),
        "the same coefficient profile over a different physical domain is different motion"
    );
}

#[test]
fn schedule_fingerprint_v2_frames_entries_and_coefficient_counts() {
    let first_edge = fingerprint_test_edge(b"entry-framing-first");
    let second_edge = fingerprint_test_edge(b"entry-framing-second");
    let second_edge_bytes = second_edge.canonical_bytes();
    let absorbed_edge_high = f64::from_bits(u64::from_be_bytes(
        second_edge_bytes[..8].try_into().unwrap(),
    ));
    let absorbed_edge_low = f64::from_bits(u64::from_be_bytes(
        second_edge_bytes[8..].try_into().unwrap(),
    ));
    let flattened = vec![Entry {
        edge: first_edge,
        initial: 11.0,
        coefficients: vec![12.0, absorbed_edge_high, absorbed_edge_low, 22.0, 23.0],
        derivative_bound: 0.0,
    }];
    let framed = vec![
        Entry {
            edge: first_edge,
            initial: 11.0,
            coefficients: vec![12.0],
            derivative_bound: 0.0,
        },
        Entry {
            edge: second_edge,
            initial: 22.0,
            coefficients: vec![23.0],
            derivative_bound: 0.0,
        },
    ];
    assert_eq!(
        legacy_unframed_schedule_preimage_for_regression(&flattened, &[]),
        legacy_unframed_schedule_preimage_for_regression(&framed, &[]),
        "the regression fixtures must reproduce the V1 entry-boundary collision"
    );
    assert_ne!(
        schedule_fingerprint_v2([0.0, 1.0], &flattened, &[]),
        schedule_fingerprint_v2([0.0, 1.0], &framed, &[]),
        "entry and coefficient counts must frame the V2 preimage"
    );
}

#[test]
fn schedule_fingerprint_v2_separates_representation_kinds() {
    let edge = fingerprint_test_edge(b"kind-separation");
    let ordinary = Entry {
        edge,
        initial: 90.0,
        coefficients: vec![0.0, 1.0],
        derivative_bound: 2.0,
    };
    let half_angle = prepared_half_angle_fingerprint_entry(
        edge,
        [rational(0, 1), rational(1, 1)],
        vec![rational(0, 1), rational(1, 1)],
        vec![rational(1, 1)],
    );
    assert_ne!(
        schedule_fingerprint_v2([0.0, 1.0], &[ordinary], &[]),
        schedule_fingerprint_v2([0.0, 1.0], &[], &[half_angle]),
        "ordinary and half-angle representations need independent kind domains"
    );
}

#[test]
fn schedule_fingerprint_v2_length_frames_and_binds_both_model_ids() {
    let ordinary = Entry {
        edge: fingerprint_test_edge(b"model-id-binding"),
        initial: 0.0,
        coefficients: vec![1.0],
        derivative_bound: 1.0,
    };
    let frozen = schedule_fingerprint_v2([0.0, 1.0], core::slice::from_ref(&ordinary), &[]);
    assert_eq!(
        frozen,
        schedule_fingerprint_v2_with_model_ids(
            [0.0, 1.0],
            core::slice::from_ref(&ordinary),
            &[],
            CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes(),
            DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
        )
    );
    assert_ne!(
        frozen,
        schedule_fingerprint_v2_with_model_ids(
            [0.0, 1.0],
            core::slice::from_ref(&ordinary),
            &[],
            b"canonical_cycle_schedule_deterministic_transcendental_v3",
            DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
        ),
        "changing the canonical-schedule model must invalidate existing authorities"
    );
    assert_ne!(
        frozen,
        schedule_fingerprint_v2_with_model_ids(
            [0.0, 1.0],
            core::slice::from_ref(&ordinary),
            &[],
            CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes(),
            b"deterministic_transcendental_v2",
        ),
        "changing the transcendental evaluator must invalidate existing authorities"
    );
    assert_ne!(
        schedule_fingerprint_v2_with_model_ids(
            [0.0, 1.0],
            core::slice::from_ref(&ordinary),
            &[],
            b"ab",
            b"c",
        ),
        schedule_fingerprint_v2_with_model_ids(
            [0.0, 1.0],
            core::slice::from_ref(&ordinary),
            &[],
            b"a",
            b"bc",
        ),
        "length prefixes must prevent adjacent model identifiers from flattening"
    );
}

#[test]
fn schedule_fingerprint_v2_has_cross_runtime_golden_vectors() {
    let ordinary = Entry {
        edge: fingerprint_test_edge(b"golden-ordinary"),
        initial: 90.0,
        coefficients: vec![-0.0, 0.5, -2.25],
        derivative_bound: 0.0,
    };
    assert_eq!(
        fingerprint_hex(schedule_fingerprint_v2([-2.5, 4.0], &[ordinary], &[])),
        "627120fbfcdc42522a7e53628ad461e9b2dbbfd7c45e5f318484a3ccf79be224"
    );

    let half_angle = prepared_half_angle_fingerprint_entry(
        fingerprint_test_edge(b"golden-half"),
        [rational(0, 1), rational(2, 5)],
        vec![rational(1, 2), rational(-1, 7)],
        vec![rational(2, 3), rational(1, 5)],
    );
    assert_eq!(
        fingerprint_hex(schedule_fingerprint_v2([0.0, 1.0], &[], &[half_angle])),
        "26cb0fe665c66f67b0ab4074c521af934eab2b6dcf422c67b41f4168d22ef446"
    );
}

#[test]
fn streaming_big_rational_framing_matches_the_frozen_byte_encoding() {
    let values = [
        BigRational::from_integer(0.into()),
        BigRational::new((-257).into(), 65_537.into()),
        BigRational::new(
            (BigInt::from(1_u8) << 96) + BigInt::from(0x0102_0304_u32),
            (BigInt::from(1_u8) << 65) + BigInt::from(3_u8),
        ),
    ];
    for value in values {
        let mut streamed = Sha256::new();
        update_canonical_big_rational_v2(&mut streamed, &value);

        let mut reference = Sha256::new();
        let (sign, mut numerator) = value.numer().to_bytes_be();
        if numerator.is_empty() {
            numerator.push(0);
        }
        let (_, denominator) = value.denom().to_bytes_be();
        reference.update([match sign {
            num_bigint::Sign::Minus => 0,
            num_bigint::Sign::NoSign => 1,
            num_bigint::Sign::Plus => 2,
        }]);
        reference.update((numerator.len() as u64).to_be_bytes());
        reference.update(numerator);
        reference.update((denominator.len() as u64).to_be_bytes());
        reference.update(denominator);

        assert_eq!(streamed.finalize(), reference.finalize());
    }
}

#[test]
fn schedule_fingerprint_v2_is_deterministic_across_reorder_and_restriction() {
    let (geometry, audit, fixed_face, edges) = fixture();
    let schedule_entries = entries(&edges);
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        schedule_entries.clone(),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();

    let mut reordered_hinges = geometry.hinges().to_vec();
    reordered_hinges.reverse();
    let reordered_geometry =
        MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), reordered_hinges);
    let reordered = CanonicalCycleScheduleV1::prepare(
        &reordered_geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        schedule_entries,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        schedule.certificate_binding_fingerprint_v2(),
        reordered.certificate_binding_fingerprint_v2(),
        "material hinge storage order must not change canonical schedule authority"
    );

    let block_hinges = geometry.hinges()[..2].to_vec();
    let first_block =
        MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), block_hinges.clone());
    let mut reversed_block_hinges = block_hinges;
    reversed_block_hinges.reverse();
    let reversed_block =
        MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), reversed_block_hinges);
    let first_restriction = schedule
        .restrict_to_edge_block_v1(&geometry, &audit, &first_block, &audit)
        .unwrap();
    let reversed_restriction = schedule
        .restrict_to_edge_block_v1(&geometry, &audit, &reversed_block, &audit)
        .unwrap();
    assert_eq!(
        first_restriction.certificate_binding_fingerprint_v2(),
        reversed_restriction.certificate_binding_fingerprint_v2(),
        "restricting the same canonical carrier must be independent of block storage order"
    );
    assert_ne!(
        schedule.certificate_binding_fingerprint_v2(),
        first_restriction.certificate_binding_fingerprint_v2(),
        "restricting the carrier must bind the reduced entry count"
    );
}

#[test]
fn three_block_restriction_rebases_leaf_fixed_faces_exactly() {
    let (geometry, audit, original_fixed_face, edges) = fixture();
    let schedule_entries = entries(&edges);
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        original_fixed_face,
        [0.0, 1.0],
        schedule_entries.clone(),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();

    for (index, hinge) in geometry.hinges().iter().enumerate() {
        let edge = hinge.edge();
        let block_geometry = MaterialHingeGraphGeometry::new_for_test(
            vec![hinge.left_face(), hinge.right_face()],
            vec![hinge.clone()],
        );
        let block_fixed_face = block_geometry.face_ids()[index % 2];
        let restricted = schedule
            .restrict_to_edge_block_with_fixed_face_v1(
                &geometry,
                &audit,
                &block_geometry,
                &audit,
                block_fixed_face,
            )
            .unwrap();
        let independently_prepared = CanonicalCycleScheduleV1::prepare(
            &block_geometry,
            &audit,
            block_fixed_face,
            [0.0, 1.0],
            schedule_entries
                .iter()
                .filter(|entry| entry.edge == edge)
                .cloned()
                .collect(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(restricted, independently_prepared);
    }

    let first_hinge = geometry.hinges()[0].clone();
    let first_block = MaterialHingeGraphGeometry::new_for_test(
        vec![first_hinge.left_face(), first_hinge.right_face()],
        vec![first_hinge],
    );
    assert_eq!(
        schedule
            .restrict_to_edge_block_v1(&geometry, &audit, &first_block, &audit)
            .unwrap(),
        schedule
            .restrict_to_edge_block_with_fixed_face_v1(
                &geometry,
                &audit,
                &first_block,
                &audit,
                original_fixed_face,
            )
            .unwrap()
    );
    assert_eq!(
        schedule.restrict_to_edge_block_with_fixed_face_v1(
            &geometry,
            &audit,
            &first_block,
            &audit,
            FaceId::new(),
        ),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    let source_hinge = &geometry.hinges()[0];
    let foreign_hinge = TreeHinge::new_for_test(
        source_hinge.edge(),
        FoldAssignment::Valley,
        source_hinge.left_face(),
        source_hinge.right_face(),
        source_hinge.start(),
        source_hinge.end(),
        source_hinge.axis(),
    );
    let foreign_block = MaterialHingeGraphGeometry::new_for_test(
        first_block.face_ids().to_vec(),
        vec![foreign_hinge],
    );
    assert_eq!(
        schedule.restrict_to_edge_block_with_fixed_face_v1(
            &geometry,
            &audit,
            &foreign_block,
            &audit,
            original_fixed_face,
        ),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    let empty_block =
        MaterialHingeGraphGeometry::new_for_test(first_block.face_ids().to_vec(), Vec::new());
    assert_eq!(
        schedule.restrict_to_edge_block_with_fixed_face_v1(
            &geometry,
            &audit,
            &empty_block,
            &audit,
            original_fixed_face,
        ),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    let duplicate_block = MaterialHingeGraphGeometry::new_for_test(
        first_block.face_ids().to_vec(),
        vec![geometry.hinges()[0].clone(), geometry.hinges()[0].clone()],
    );
    assert_eq!(
        schedule.restrict_to_edge_block_with_fixed_face_v1(
            &geometry,
            &audit,
            &duplicate_block,
            &audit,
            original_fixed_face,
        ),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
}

#[test]
fn kawasaki_degree_four_generator_is_deterministic_and_resource_bounded() {
    let ns = ProjectId::new();
    let faces = [b"a", b"b", b"c", b"d"].map(|name| FaceId::derive_v5(ns, name));
    let edges = [b"ab", b"bc", b"cd", b"da"].map(|name| EdgeId::derive_v5(ns, name));
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
        hinge_adjacency: (0..4)
            .map(|index| FaceAdjacency {
                edge: edges[index],
                first: faces[index],
                second: faces[(index + 1) % 4],
                assignment: if index == 3 {
                    FoldAssignment::Mountain
                } else {
                    FoldAssignment::Valley
                },
            })
            .collect(),
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let start = Point3::new(0.0, 0.0, 0.0).unwrap();
    let ends = [
        Point3::new(1.0, 0.0, 0.0).unwrap(),
        Point3::new(-0.5, 0.0, 0.866_025_403_784_438_6).unwrap(),
        Point3::new(-0.5, 0.0, -0.866_025_403_784_438_6).unwrap(),
        Point3::new(0.5, 0.0, -0.866_025_403_784_438_6).unwrap(),
    ];
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        faces.to_vec(),
        (0..4)
            .map(|index| {
                TreeHinge::new_for_test(
                    edges[index],
                    if index == 3 {
                        FoldAssignment::Mountain
                    } else {
                        FoldAssignment::Valley
                    },
                    faces[index],
                    faces[(index + 1) % 4],
                    start,
                    ends[index],
                    ends[index],
                )
            })
            .collect(),
    );
    let first = generate_kawasaki_120_120_60_60_path_candidate_v1(
        &geometry,
        &audit,
        audit.faces()[0],
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let second = generate_kawasaki_120_120_60_60_path_candidate_v1(
        &geometry,
        &audit,
        audit.faces()[0],
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        first.schedule().certificate_binding_fingerprint_v2(),
        second.schedule().certificate_binding_fingerprint_v2(),
    );
    assert!(
        first
            .schedule()
            .kawasaki_120_120_60_60_half_angle_pairs_v1()
            .is_some()
    );
    let exact_work = (1..=CycleScheduleLimitsV1::default().max_work)
        .find(|max_work| {
            generate_kawasaki_120_120_60_60_path_candidate_v1(
                &geometry,
                &audit,
                audit.faces()[0],
                CycleScheduleLimitsV1 {
                    max_work: *max_work,
                    ..CycleScheduleLimitsV1::default()
                },
            )
            .is_ok()
        })
        .expect("the bounded Kawasaki generator must have a finite exact work threshold");
    assert!(
        generate_kawasaki_120_120_60_60_path_candidate_v1(
            &geometry,
            &audit,
            audit.faces()[0],
            CycleScheduleLimitsV1 {
                max_work: exact_work,
                ..CycleScheduleLimitsV1::default()
            },
        )
        .is_ok()
    );
    assert_eq!(
        generate_kawasaki_120_120_60_60_path_candidate_v1(
            &geometry,
            &audit,
            audit.faces()[0],
            CycleScheduleLimitsV1 {
                max_work: exact_work - 1,
                ..CycleScheduleLimitsV1::default()
            },
        ),
        Err(MultiHingePathCandidateErrorV1::ResourceLimit)
    );
    let automatic = generate_bounded_degree_four_kawasaki_path_candidate_v1(
        &geometry,
        &audit,
        audit.faces()[0],
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(automatic.moving_hinges().len(), 4);
    assert!(
        [0.0, 0.5, 1.0]
            .into_iter()
            .all(|u| automatic.schedule().evaluate(u).is_some())
    );
    let mut reversed_hinges = geometry.hinges().to_vec();
    reversed_hinges.reverse();
    let reversed = MaterialHingeGraphGeometry::new_for_test(faces.to_vec(), reversed_hinges);
    let reordered = generate_bounded_degree_four_kawasaki_path_candidate_v1(
        &reversed,
        &audit,
        audit.faces()[0],
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        automatic.schedule().certificate_binding_fingerprint_v2(),
        reordered.schedule().certificate_binding_fingerprint_v2(),
    );
    let axes = [
        Point3::new(1.0, 0.0, 0.0).unwrap(),
        Point3::new(-3.0 / 5.0, 0.0, 4.0 / 5.0).unwrap(),
        Point3::new(-7.0 / 25.0, 0.0, -24.0 / 25.0).unwrap(),
        Point3::new(3.0 / 5.0, 0.0, -4.0 / 5.0).unwrap(),
    ];
    let rational_geometry = MaterialHingeGraphGeometry::new_for_test(
        audit.faces().to_vec(),
        (0..4)
            .map(|index| {
                TreeHinge::new_for_test(
                    edges[index],
                    if index == 3 {
                        FoldAssignment::Mountain
                    } else {
                        FoldAssignment::Valley
                    },
                    faces[index],
                    faces[(index + 1) % 4],
                    start,
                    axes[index],
                    axes[index],
                )
            })
            .collect(),
    );
    let candidate = generate_bounded_degree_four_kawasaki_path_candidate_v1(
        &rational_geometry,
        &audit,
        audit.faces()[0],
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        candidate
            .schedule()
            .bounded_symmetric_kawasaki_profile_v1()
            .map(|(_, _, numerator, denominator)| (numerator, denominator)),
        Some((3, 5))
    );
    let closure = rational_geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            audit.faces()[0],
            candidate.schedule(),
            1.0e-9,
            crate::DyadicIntervalClosureLimitsV1 {
                max_depth: 16,
                max_leaves: 65_536,
                max_work: 1_048_576,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("the bounded 3/5 exact profile has analytic closure authority");
    assert_eq!(closure.leaves().len(), 1);
    let mut rotated_axes = axes;
    rotated_axes.rotate_left(1);
    let cyclic_start_geometry = MaterialHingeGraphGeometry::new_for_test(
        faces.to_vec(),
        (0..4)
            .map(|index| {
                TreeHinge::new_for_test(
                    edges[index],
                    if index == 2 {
                        FoldAssignment::Mountain
                    } else {
                        FoldAssignment::Valley
                    },
                    faces[index],
                    faces[(index + 1) % 4],
                    start,
                    rotated_axes[index],
                    rotated_axes[index],
                )
            })
            .collect(),
    );
    let cyclic_start = generate_bounded_degree_four_kawasaki_path_candidate_v1(
        &cyclic_start_geometry,
        &audit,
        faces[0],
        CycleScheduleLimitsV1::default(),
    )
    .expect("the exact profile is invariant to its angular cycle start");
    assert_eq!(
        cyclic_start
            .schedule()
            .bounded_symmetric_kawasaki_profile_v1()
            .map(|(_, _, numerator, denominator)| (numerator, denominator)),
        Some((3, 5))
    );
    for (numerator, denominator, complement) in [(5.0, 13.0, 12.0), (7.0, 25.0, 24.0)] {
        let ratio = numerator / denominator;
        let sine = complement / denominator;
        let axes = [
            Point3::new(1.0, 0.0, 0.0).unwrap(),
            Point3::new(-ratio, 0.0, sine).unwrap(),
            Point3::new(2.0 * ratio * ratio - 1.0, 0.0, -2.0 * ratio * sine).unwrap(),
            Point3::new(ratio, 0.0, -sine).unwrap(),
        ];
        let exact_geometry = MaterialHingeGraphGeometry::new_for_test(
            faces.to_vec(),
            (0..4)
                .map(|index| {
                    TreeHinge::new_for_test(
                        edges[index],
                        if index == 3 {
                            FoldAssignment::Mountain
                        } else {
                            FoldAssignment::Valley
                        },
                        faces[index],
                        faces[(index + 1) % 4],
                        start,
                        axes[index],
                        axes[index],
                    )
                })
                .collect(),
        );
        let exact = generate_bounded_degree_four_kawasaki_path_candidate_v1(
            &exact_geometry,
            &audit,
            faces[0],
            CycleScheduleLimitsV1::default(),
        )
        .expect("the bounded Pythagorean Kawasaki family must be admitted");
        assert_eq!(
            exact
                .schedule()
                .bounded_symmetric_kawasaki_profile_v1()
                .map(|(_, _, numerator, denominator)| (numerator, denominator)),
            Some((numerator as i64, denominator as u64))
        );
        assert!(
            [0.0, 0.5, 1.0]
                .into_iter()
                .all(|parameter| exact.schedule().evaluate(parameter).is_some()),
            "the generated exact family remains defined over its full bounded domain"
        );
        let mut previous_endpoint = f64::INFINITY;
        for endpoint_denominator in [1, 2, 4, 8, 16] {
            let bounded =
                generate_bounded_degree_four_kawasaki_path_candidate_at_dyadic_endpoint_v1(
                    &exact_geometry,
                    &audit,
                    faces[0],
                    endpoint_denominator,
                    CycleScheduleLimitsV1::default(),
                )
                .expect("each bounded dyadic endpoint remains an exact candidate");
            let endpoint = bounded.schedule().evaluate(1.0).unwrap();
            let maximum = endpoint
                .as_slice()
                .iter()
                .map(|angle| angle.angle_degrees())
                .fold(0.0_f64, f64::max);
            assert!(maximum < previous_endpoint || endpoint_denominator == 1);
            previous_endpoint = maximum;
            assert!(
                [0.0, 0.5, 1.0]
                    .into_iter()
                    .all(|parameter| bounded.schedule().evaluate(parameter).is_some()),
                "bounded endpoint candidate remains defined over its full domain"
            );
        }
    }
    assert_eq!(
        generate_bounded_degree_four_kawasaki_path_candidate_v1(
            &geometry,
            &audit,
            faces[0],
            CycleScheduleLimitsV1 {
                max_hinges: 3,
                ..CycleScheduleLimitsV1::default()
            },
        ),
        Err(MultiHingePathCandidateErrorV1::InvalidBinding)
    );
    let assignment_tamper = MaterialHingeGraphGeometry::new_for_test(
        faces.to_vec(),
        geometry
            .hinges()
            .iter()
            .map(|hinge| {
                TreeHinge::new_for_test(
                    hinge.edge(),
                    FoldAssignment::Mountain,
                    hinge.left_face(),
                    hinge.right_face(),
                    hinge.start(),
                    hinge.end(),
                    hinge.axis(),
                )
            })
            .collect(),
    );
    assert_eq!(
        generate_bounded_degree_four_kawasaki_path_candidate_v1(
            &assignment_tamper,
            &audit,
            faces[0],
            CycleScheduleLimitsV1::default(),
        ),
        Err(MultiHingePathCandidateErrorV1::CandidateRejected)
    );
    let non_kawasaki = MaterialHingeGraphGeometry::new_for_test(
        faces.to_vec(),
        geometry
            .hinges()
            .iter()
            .map(|hinge| {
                let end = if hinge.edge() == edges[2] {
                    Point3::new(0.0, 0.0, -1.0).unwrap()
                } else {
                    hinge.end()
                };
                TreeHinge::new_for_test(
                    hinge.edge(),
                    hinge.assignment(),
                    hinge.left_face(),
                    hinge.right_face(),
                    hinge.start(),
                    end,
                    end,
                )
            })
            .collect(),
    );
    assert_eq!(
        generate_bounded_degree_four_kawasaki_path_candidate_v1(
            &non_kawasaki,
            &audit,
            faces[0],
            CycleScheduleLimitsV1::default(),
        ),
        Err(MultiHingePathCandidateErrorV1::CandidateRejected)
    );
    assert_eq!(
        generate_kawasaki_120_120_60_60_path_candidate_v1(
            &geometry,
            &audit,
            faces[0],
            CycleScheduleLimitsV1 {
                max_hinges: 3,
                ..CycleScheduleLimitsV1::default()
            },
        ),
        Err(MultiHingePathCandidateErrorV1::InvalidBinding),
    );
}

fn bounded_kawasaki_ratio_fixture(
    numerator: i64,
    denominator: u64,
) -> (MaterialHingeGraphGeometry, MaterialHingeGraphAudit, FaceId) {
    let namespace = ProjectId::new();
    let faces = [b"a", b"b", b"c", b"d"].map(|name| FaceId::derive_v5(namespace, name));
    let edges = [b"ab", b"bc", b"cd", b"da"].map(|name| EdgeId::derive_v5(namespace, name));
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
        hinge_adjacency: (0..4)
            .map(|index| FaceAdjacency {
                edge: edges[index],
                first: faces[index],
                second: faces[(index + 1) % 4],
                assignment: if index == 3 {
                    FoldAssignment::Mountain
                } else {
                    FoldAssignment::Valley
                },
            })
            .collect(),
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let ratio = numerator as f64 / denominator as f64;
    let complement = (1.0 - ratio * ratio).sqrt();
    let axes = [
        Point3::new(1.0, 0.0, 0.0).unwrap(),
        Point3::new(-ratio, 0.0, complement).unwrap(),
        Point3::new(2.0 * ratio * ratio - 1.0, 0.0, -2.0 * ratio * complement).unwrap(),
        Point3::new(ratio, 0.0, -complement).unwrap(),
    ];
    let start = Point3::new(0.0, 0.0, 0.0).unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        faces.to_vec(),
        (0..4)
            .map(|index| {
                TreeHinge::new_for_test(
                    edges[index],
                    if index == 3 {
                        FoldAssignment::Mountain
                    } else {
                        FoldAssignment::Valley
                    },
                    faces[index],
                    faces[(index + 1) % 4],
                    start,
                    axes[index],
                    axes[index],
                )
            })
            .collect(),
    );
    (geometry, audit, faces[0])
}

#[test]
fn bounded_kawasaki_ratio_boundaries_preserve_candidate_coefficient_limits() {
    for (numerator, denominator) in [(37, 64), (33, 65), (2_945, 4_993), (4_095, 8_192)] {
        assert_eq!(
            bounded_kawasaki_sector_ratio_v1(numerator as f64 / denominator as f64),
            Some((numerator, denominator))
        );
    }
    assert_eq!(bounded_kawasaki_sector_ratio_v1(4_097.0 / 8_193.0), None);

    for (numerator, denominator) in [(37, 64), (33, 65), (2_945, 4_993)] {
        let (geometry, audit, fixed_face) = bounded_kawasaki_ratio_fixture(numerator, denominator);
        let candidate = generate_bounded_degree_four_kawasaki_path_candidate_v1(
            &geometry,
            &audit,
            fixed_face,
            CycleScheduleLimitsV1::default(),
        )
        .expect("an in-bound rational ratio must produce a schedule candidate");
        assert_eq!(
            candidate
                .schedule()
                .bounded_symmetric_kawasaki_profile_v1()
                .map(|(_, _, actual_numerator, actual_denominator)| {
                    (actual_numerator, actual_denominator)
                }),
            Some((numerator, denominator))
        );
        assert!(!candidate.authorizes_closure());
        assert!(!candidate.authorizes_collision_clearance());
    }

    let (largest_geometry, largest_audit, largest_fixed_face) =
        bounded_kawasaki_ratio_fixture(4_095, 8_192);
    let largest = generate_bounded_degree_four_kawasaki_path_candidate_at_dyadic_endpoint_v1(
        &largest_geometry,
        &largest_audit,
        largest_fixed_face,
        16,
        CycleScheduleLimitsV1::default(),
    )
    .expect("the upper ratio bound remains within the coefficient-bit budget");
    assert_eq!(
        largest
            .schedule()
            .bounded_symmetric_kawasaki_profile_v1()
            .map(|(_, _, numerator, denominator)| (numerator, denominator)),
        Some((4_095, 8_192))
    );
    assert!(!largest.authorizes_closure());
    assert!(!largest.authorizes_collision_clearance());

    let (over_geometry, over_audit, over_fixed_face) = bounded_kawasaki_ratio_fixture(4_097, 8_193);
    assert_eq!(
        generate_bounded_degree_four_kawasaki_path_candidate_v1(
            &over_geometry,
            &over_audit,
            over_fixed_face,
            CycleScheduleLimitsV1::default(),
        ),
        Err(MultiHingePathCandidateErrorV1::CandidateRejected)
    );
}

#[test]
fn canonical_schedule_evaluates_and_bounds_derivative() {
    let (geometry, audit, fixed, edges) = fixture();
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed,
        [0.0, 1.0],
        entries(&edges),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert!(schedule.matches_binding(&geometry, &audit, fixed));
    let forged_fixed = audit
        .faces()
        .iter()
        .copied()
        .find(|face| *face != fixed)
        .unwrap();
    assert!(!schedule.matches_binding(&geometry, &audit, forged_fixed));
    assert_eq!(
        schedule.evaluate(0.5).unwrap().as_slice()[0].angle_degrees(),
        90.0
    );
    assert_eq!(schedule.derivative_bound(edges[0]), Some(20.0));
    assert!(schedule.evaluate(-0.1).is_none());
}

#[test]
fn canonical_schedule_derivative_bound_uses_exact_polynomial_constancy() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let prepare = |domain: [f64; 2], coefficients: Vec<RationalCoefficientV1>| {
        let inputs = edges
            .iter()
            .map(|edge| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: 90.0_f64.to_bits(),
                chebyshev_coefficients: coefficients.clone(),
            })
            .collect();
        CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            domain,
            inputs,
            CycleScheduleLimitsV1::default(),
        )
        .expect("prepare bounded polynomial schedule")
    };
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let one = RationalCoefficientV1 {
        numerator: 1,
        denominator: 1,
    };

    for schedule in [
        prepare([0.0, 1.0], Vec::new()),
        prepare([0.0, 1.0], vec![zero]),
        prepare([0.0, 1.0], vec![one]),
        prepare([0.0, 1.0], vec![one, zero]),
    ] {
        assert!(edges.iter().all(|edge| {
            schedule
                .derivative_bound(*edge)
                .is_some_and(|bound| bound.to_bits() == 0.0_f64.to_bits())
        }));
    }

    let nonconstant = prepare([0.0, 1.0], vec![zero, one]);
    assert!(
        edges
            .iter()
            .all(|edge| nonconstant.derivative_bound(*edge) == Some(2.0))
    );

    let underflowed = prepare([-f64::MAX, f64::MAX], vec![zero, one]);
    assert!(edges.iter().all(|edge| {
        underflowed
            .derivative_bound(*edge)
            .is_some_and(|bound| bound.is_infinite() && bound.is_sign_positive())
    }));

    let infinite = prepare([0.0, f64::from_bits(1)], vec![zero, one]);
    assert!(edges.iter().all(|edge| {
        infinite
            .derivative_bound(*edge)
            .is_some_and(|bound| bound.is_infinite() && bound.is_sign_positive())
    }));
}

#[test]
fn linear_multi_hinge_candidate_is_bounded_deterministic_and_not_authority() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let angles = |value| {
        CanonicalHingeAngles::new(
            edges
                .iter()
                .map(|edge| HingeAngle::new(*edge, value).unwrap())
                .collect(),
        )
        .unwrap()
    };
    let initial = angles(20.0);
    let requested = angles(40.0);
    let candidate = generate_linear_multi_hinge_path_candidate_v1(
        &geometry,
        &audit,
        fixed,
        &initial,
        &requested,
        MultiHingePathCandidateLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(candidate.moving_hinges(), edges);
    assert_eq!(
        candidate.schedule().collective_profile_edges_v1(),
        Some(edges.clone())
    );
    assert!(!candidate.authorizes_closure());
    assert!(!candidate.authorizes_collision_clearance());
    for (parameter, expected) in [(0.0, 20.0), (1.0, 40.0)] {
        assert!(
            candidate
                .schedule()
                .evaluate(parameter)
                .unwrap()
                .as_slice()
                .iter()
                .all(|angle| angle.angle_degrees() == expected)
        );
    }
    assert_eq!(
        generate_linear_multi_hinge_path_candidate_v1(
            &geometry,
            &audit,
            fixed,
            &initial,
            &initial,
            MultiHingePathCandidateLimitsV1::default(),
        ),
        Err(MultiHingePathCandidateErrorV1::NoMotion)
    );
    assert_eq!(
        generate_linear_multi_hinge_path_candidate_v1(
            &geometry,
            &audit,
            fixed,
            &initial,
            &requested,
            MultiHingePathCandidateLimitsV1 {
                max_work: edges.len() * 2 - 1,
                ..MultiHingePathCandidateLimitsV1::default()
            },
        ),
        Err(MultiHingePathCandidateErrorV1::ResourceLimit)
    );
    let exact_limits = MultiHingePathCandidateLimitsV1 {
        max_hinges: edges.len(),
        max_candidates: 1,
        max_work: edges.len() * 2,
    };
    assert!(
        generate_linear_multi_hinge_path_candidate_v1(
            &geometry,
            &audit,
            fixed,
            &initial,
            &requested,
            exact_limits,
        )
        .is_ok()
    );
    for one_short in [
        MultiHingePathCandidateLimitsV1 {
            max_hinges: exact_limits.max_hinges - 1,
            ..exact_limits
        },
        MultiHingePathCandidateLimitsV1 {
            max_candidates: 0,
            ..exact_limits
        },
        MultiHingePathCandidateLimitsV1 {
            max_work: exact_limits.max_work - 1,
            ..exact_limits
        },
    ] {
        assert_eq!(
            generate_linear_multi_hinge_path_candidate_v1(
                &geometry, &audit, fixed, &initial, &requested, one_short,
            ),
            Err(MultiHingePathCandidateErrorV1::ResourceLimit)
        );
    }
}

#[test]
fn nonclosing_linear_candidate_never_produces_closure_authority() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let angles = |value| {
        CanonicalHingeAngles::new(
            edges
                .iter()
                .map(|edge| HingeAngle::new(*edge, value).unwrap())
                .collect(),
        )
        .unwrap()
    };
    let candidate = generate_linear_multi_hinge_path_candidate_v1(
        &geometry,
        &audit,
        fixed,
        &angles(20.0),
        &angles(40.0),
        MultiHingePathCandidateLimitsV1::default(),
    )
    .unwrap();
    let result = geometry.prove_dyadic_schedule_closure_v1(
        &audit,
        fixed,
        candidate.schedule(),
        1.0e-9,
        crate::DyadicIntervalClosureLimitsV1 {
            max_depth: 0,
            max_leaves: 1,
            max_work: 1_000_000,
            schedule_limits: CycleScheduleLimitsV1 {
                max_degree: 1,
                max_work: 100_000,
                ..CycleScheduleLimitsV1::default()
            },
        },
    );
    assert!(
        matches!(
            result,
            Err(crate::DyadicIntervalClosureErrorV1::ResourceLimit)
        ),
        "{result:?}"
    );
}

#[test]
fn schedule_binding_rejects_assignment_and_axis_aba() {
    let (geometry, audit, fixed, edges) = fixture();
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed,
        [0.0, 1.0],
        entries(&edges),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let rebuild = |change_assignment: bool, change_axis: bool| {
        let hinges = geometry
            .hinges()
            .iter()
            .enumerate()
            .map(|(index, hinge)| {
                TreeHinge::new_for_test(
                    hinge.edge(),
                    if change_assignment && index == 0 {
                        match hinge.assignment() {
                            FoldAssignment::Mountain => FoldAssignment::Valley,
                            FoldAssignment::Valley => FoldAssignment::Mountain,
                        }
                    } else {
                        hinge.assignment()
                    },
                    hinge.left_face(),
                    hinge.right_face(),
                    hinge.start(),
                    hinge.end(),
                    if change_axis && index == 0 {
                        Point3::new(0.0, 1.0, 0.0).unwrap()
                    } else {
                        hinge.axis()
                    },
                )
            })
            .collect();
        MaterialHingeGraphGeometry::new_for_test(geometry.face_ids().to_vec(), hinges)
    };
    assert!(!schedule.matches_binding(&rebuild(true, false), &audit, fixed));
    assert!(!schedule.matches_binding(&rebuild(false, true), &audit, fixed));
}

#[test]
fn malformed_order_coefficients_and_limits_fail_closed() {
    let (geometry, audit, fixed, edges) = fixture();
    let limits = CycleScheduleLimitsV1::default();
    let mut reversed = entries(&edges);
    reversed.reverse();
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], reversed, limits),
        Err(CycleSchedulePrepareErrorV1::NonCanonical)
    );
    let mut bad = entries(&edges);
    bad[0].chebyshev_coefficients[0].denominator = 0;
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], bad, limits),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    let mut excessive = entries(&edges);
    excessive[0].chebyshev_coefficients.resize(
        limits.max_degree + 2,
        RationalCoefficientV1 {
            numerator: 0,
            denominator: 1,
        },
    );
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], excessive, limits),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
    let mut wide = entries(&edges);
    wide[0].chebyshev_coefficients[0].numerator = 1_i64 << 54;
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], wide, limits),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    let mut out_of_range = entries(&edges);
    out_of_range[0].chebyshev_coefficients[1].numerator = 91;
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            out_of_range,
            limits,
        ),
        Err(CycleSchedulePrepareErrorV1::AngleRange)
    );
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            entries(&edges),
            CycleScheduleLimitsV1 {
                max_work: 1,
                ..limits
            },
        ),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
}

#[test]
fn canonical_schedule_limits_admit_equality_and_reject_each_one_short_dimension() {
    let (geometry, audit, fixed, edges) = fixture();
    let exact = CycleScheduleLimitsV1 {
        max_hinges: edges.len(),
        max_degree: 1,
        max_coefficient_bits: 4,
        max_work: edges.len() * 2,
    };
    assert!(
        CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            entries(&edges),
            exact,
        )
        .is_ok()
    );
    for (one_short, expected) in [
        (
            CycleScheduleLimitsV1 {
                max_hinges: exact.max_hinges - 1,
                ..exact
            },
            CycleSchedulePrepareErrorV1::InvalidInput,
        ),
        (
            CycleScheduleLimitsV1 {
                max_degree: exact.max_degree - 1,
                ..exact
            },
            CycleSchedulePrepareErrorV1::ResourceLimit,
        ),
        (
            CycleScheduleLimitsV1 {
                max_coefficient_bits: exact.max_coefficient_bits - 1,
                ..exact
            },
            CycleSchedulePrepareErrorV1::InvalidInput,
        ),
        (
            CycleScheduleLimitsV1 {
                max_work: exact.max_work - 1,
                ..exact
            },
            CycleSchedulePrepareErrorV1::ResourceLimit,
        ),
    ] {
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                entries(&edges),
                one_short,
            ),
            Err(expected)
        );
    }
}

#[test]
fn half_angle_schedule_and_dyadic_limits_are_exact_at_equality_and_one_short() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let inputs = || {
        edges
            .iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [rational(0, 1), rational(1, 1)],
                numerator_power_coefficients: vec![rational(1, 1)],
                denominator_power_coefficients: vec![rational(1, 1)],
            })
            .collect::<Vec<_>>()
    };
    let exact_prepare = CycleScheduleLimitsV1 {
        max_hinges: edges.len(),
        max_degree: 0,
        max_coefficient_bits: 1,
        max_work: 1,
    };
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        inputs(),
        exact_prepare,
    )
    .expect("per-entry half-angle work equality must be admitted");
    assert_eq!(
        CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs(),
            CycleScheduleLimitsV1 {
                max_work: exact_prepare.max_work - 1,
                ..exact_prepare
            },
        ),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
    assert_eq!(
        CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs(),
            CycleScheduleLimitsV1 {
                max_hinges: exact_prepare.max_hinges - 1,
                ..exact_prepare
            },
        ),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    assert_eq!(
        CanonicalCycleScheduleV1::prepare_half_angle_rational(
            &geometry,
            &audit,
            fixed,
            inputs(),
            CycleScheduleLimitsV1 {
                max_coefficient_bits: 0,
                ..exact_prepare
            },
        ),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );

    let unbounded_evaluation = CycleScheduleLimitsV1 {
        max_hinges: edges.len(),
        max_degree: 0,
        max_coefficient_bits: 1,
        max_work: usize::MAX,
    };
    let dyadic_work = schedule
        .evaluate_angle_box_dyadic(0, 0, unbounded_evaluation)
        .expect("unbounded audit evaluation")
        .iter()
        .map(|(_, angle)| angle.work())
        .max()
        .expect("nonempty angle boxes");
    let exact_dyadic = CycleScheduleLimitsV1 {
        max_work: dyadic_work,
        ..unbounded_evaluation
    };
    assert!(
        schedule
            .evaluate_angle_box_dyadic(0, 0, exact_dyadic)
            .is_ok()
    );
    assert_eq!(
        schedule.evaluate_angle_box_dyadic(
            0,
            0,
            CycleScheduleLimitsV1 {
                max_work: exact_dyadic.max_work - 1,
                ..exact_dyadic
            },
        ),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
    assert_eq!(
        schedule.evaluate_angle_box_dyadic(
            0,
            0,
            CycleScheduleLimitsV1 {
                max_hinges: exact_dyadic.max_hinges - 1,
                ..exact_dyadic
            },
        ),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    let endpoint_work = schedule
        .evaluate_endpoint_angle_box(false, unbounded_evaluation)
        .expect("unbounded endpoint audit evaluation")
        .iter()
        .map(|(_, angle)| angle.work())
        .max()
        .expect("nonempty endpoint boxes");
    let exact_endpoint = CycleScheduleLimitsV1 {
        max_work: endpoint_work,
        ..unbounded_evaluation
    };
    assert!(
        schedule
            .evaluate_endpoint_angle_box(false, exact_endpoint)
            .is_ok()
    );
    assert_eq!(
        schedule.evaluate_endpoint_angle_box(
            false,
            CycleScheduleLimitsV1 {
                max_work: exact_endpoint.max_work - 1,
                ..exact_endpoint
            },
        ),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
}

#[test]
fn half_angle_restriction_preflights_all_work_before_output_allocation() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        edges
            .iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [rational(0, 1), rational(1, 1)],
                numerator_power_coefficients: vec![rational(1, 1)],
                denominator_power_coefficients: vec![rational(1, 1)],
            })
            .collect(),
        CycleScheduleLimitsV1 {
            max_hinges: edges.len(),
            max_degree: 0,
            max_coefficient_bits: 1,
            max_work: 1,
        },
    )
    .unwrap();
    let ceiling = usize::MAX - 1;
    let generous = CycleScheduleRestrictionWorkspaceLimitsV2 {
        max_work: ceiling,
        max_restricted_schedule_retained_bytes: ceiling,
        max_restriction_peak_bytes: ceiling,
    };
    let issued = schedule
        .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
            &geometry,
            &audit,
            &geometry,
            &audit,
            fixed,
            generous,
            || Ok(()),
        )
        .unwrap();
    let exact = CycleScheduleRestrictionWorkspaceLimitsV2 {
        max_work: issued.resources.charged_work,
        max_restricted_schedule_retained_bytes: issued
            .resources
            .charged_restricted_schedule_retained_upper_bound_bytes,
        max_restriction_peak_bytes: issued.resources.charged_restriction_peak_upper_bound_bytes,
    };
    schedule
        .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
            &geometry,
            &audit,
            &geometry,
            &audit,
            fixed,
            exact,
            || Ok(()),
        )
        .expect("all exact restriction ceilings must be admitted");

    for one_short in [
        CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_work: exact.max_work - 1,
            ..exact
        },
        CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_restricted_schedule_retained_bytes: exact.max_restricted_schedule_retained_bytes
                - 1,
            ..exact
        },
        CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_restriction_peak_bytes: exact.max_restriction_peak_bytes - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            schedule.restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                &geometry,
                &audit,
                &geometry,
                &audit,
                fixed,
                one_short,
                || Ok(()),
            ),
            Err(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)
        ));
    }
}

#[test]
fn ordinary_and_half_angle_proper_block_restrictions_have_tight_resources() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: edges.len(),
        max_degree: 1,
        max_coefficient_bits: 53,
        max_work: 4_096,
    };
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed,
        [0.0, 1.0],
        entries(&edges),
        schedule_limits,
    )
    .unwrap();
    let half_angle = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        edges
            .iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [rational(0, 1), rational(1, 1)],
                numerator_power_coefficients: vec![rational(1_000_003, 1), rational(1, 1)],
                denominator_power_coefficients: vec![rational(1_000_033, 1)],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    let (block_geometry, block_audit, block_fixed) = single_hinge_block(&geometry, edges[0]);
    let ceiling = usize::MAX - 1;
    let generous = CycleScheduleRestrictionWorkspaceLimitsV2 {
        max_work: ceiling,
        max_restricted_schedule_retained_bytes: ceiling,
        max_restriction_peak_bytes: ceiling,
    };

    for schedule in [&ordinary, &half_angle] {
        let first = schedule
            .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                &geometry,
                &audit,
                &block_geometry,
                &block_audit,
                block_fixed,
                generous,
                || Ok(()),
            )
            .unwrap();
        assert_eq!(
            first.schedule.entries.len() + first.schedule.half_angle_entries.len(),
            1
        );
        let resources = first.resources;
        let deep_retained = first.schedule.checked_deep_retained_bytes_v1().unwrap();
        assert!(
            resources.charged_restricted_schedule_retained_upper_bound_bytes >= deep_retained,
            "the charge must cover every physical outer/nested capacity and BigInt payload"
        );
        assert_eq!(
            resources.charged_restriction_peak_upper_bound_bytes,
            resources
                .charged_restricted_schedule_retained_upper_bound_bytes
                .checked_add(std::mem::size_of::<Sha256>())
                .unwrap(),
            "the completed schedule and streaming hash are simultaneously live"
        );
        if !first.schedule.half_angle_entries.is_empty() {
            let outer_only = std::mem::size_of::<CanonicalCycleScheduleV1>()
                + std::mem::size_of::<PreparedHalfAngleRationalEntryV1>()
                    * first.schedule.half_angle_entries.capacity();
            assert!(
                resources.charged_restricted_schedule_retained_upper_bound_bytes > outer_only,
                "exact-rational vectors and BigInt payload must be included"
            );
        }
        let exact = CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_work: resources.charged_work,
            max_restricted_schedule_retained_bytes: resources
                .charged_restricted_schedule_retained_upper_bound_bytes,
            max_restriction_peak_bytes: resources.charged_restriction_peak_upper_bound_bytes,
        };
        let exact_issue = schedule
            .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                &geometry,
                &audit,
                &block_geometry,
                &block_audit,
                block_fixed,
                exact,
                || Ok(()),
            )
            .unwrap();
        assert_eq!(exact_issue.resources, resources);

        for one_short in [
            CycleScheduleRestrictionWorkspaceLimitsV2 {
                max_work: exact.max_work - 1,
                ..exact
            },
            CycleScheduleRestrictionWorkspaceLimitsV2 {
                max_restricted_schedule_retained_bytes: exact
                    .max_restricted_schedule_retained_bytes
                    - 1,
                ..exact
            },
            CycleScheduleRestrictionWorkspaceLimitsV2 {
                max_restriction_peak_bytes: exact.max_restriction_peak_bytes - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                schedule.restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                    &geometry,
                    &audit,
                    &block_geometry,
                    &block_audit,
                    block_fixed,
                    one_short,
                    || Ok(()),
                ),
                Err(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)
            ));
        }
    }
}

#[test]
fn restriction_policy_stop_and_foreign_input_classification_is_exact() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: edges.len(),
        max_degree: 1,
        max_coefficient_bits: 53,
        max_work: 4_096,
    };
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed,
        [0.0, 1.0],
        entries(&edges),
        schedule_limits,
    )
    .unwrap();
    let half_angle = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        edges
            .iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [rational(0, 1), rational(1, 1)],
                numerator_power_coefficients: vec![rational(1, 1), rational(1, 1)],
                denominator_power_coefficients: vec![rational(2, 1)],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    let (block_geometry, block_audit, block_fixed) = single_hinge_block(&geometry, edges[0]);
    let ceiling = usize::MAX - 1;
    let generous = CycleScheduleRestrictionWorkspaceLimitsV2 {
        max_work: ceiling,
        max_restricted_schedule_retained_bytes: ceiling,
        max_restriction_peak_bytes: ceiling,
    };

    for schedule in [&ordinary, &half_angle] {
        let mut successful_polls = 0usize;
        schedule
            .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                &geometry,
                &audit,
                &block_geometry,
                &block_audit,
                block_fixed,
                generous,
                || {
                    successful_polls += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert!(successful_polls > 1);
        for stop in [
            CycleScheduleRestrictionStopV1::Cancelled,
            CycleScheduleRestrictionStopV1::DeadlineExceeded,
        ] {
            for stop_at in 1..=successful_polls {
                let mut polls = 0usize;
                let error = schedule
                    .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                        &geometry,
                        &audit,
                        &block_geometry,
                        &block_audit,
                        block_fixed,
                        generous,
                        || {
                            polls += 1;
                            if polls == stop_at { Err(stop) } else { Ok(()) }
                        },
                    )
                    .unwrap_err();
                assert_eq!(
                    error,
                    match stop {
                        CycleScheduleRestrictionStopV1::Cancelled => {
                            CycleScheduleRestrictionWorkspaceErrorV2::Cancelled
                        }
                        CycleScheduleRestrictionStopV1::DeadlineExceeded => {
                            CycleScheduleRestrictionWorkspaceErrorV2::DeadlineExceeded
                        }
                    }
                );
            }
        }
    }

    for invalid_policy in [
        CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_work: usize::MAX,
            ..generous
        },
        CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_restricted_schedule_retained_bytes: usize::MAX,
            ..generous
        },
        CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_restriction_peak_bytes: usize::MAX,
            ..generous
        },
    ] {
        assert!(matches!(
            ordinary.restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                &geometry,
                &audit,
                &block_geometry,
                &block_audit,
                block_fixed,
                invalid_policy,
                || Ok(()),
            ),
            Err(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)
        ));
    }

    let (foreign_geometry, foreign_audit, _, foreign_edges) = fixture();
    let (foreign_block_geometry, foreign_block_audit, foreign_block_fixed) =
        single_hinge_block(&foreign_geometry, foreign_edges[0]);
    let wrong_block_audit = single_hinge_foreign_edge_audit(
        &block_geometry,
        EdgeId::derive_v5(
            ProjectId::schema_namespace([0xd7; 16]),
            b"foreign-block-audit-edge",
        ),
    );
    let absent_fixed = geometry
        .face_ids()
        .iter()
        .copied()
        .find(|face| !block_geometry.face_ids().contains(face))
        .unwrap();
    for (source_geometry, source_audit, candidate_geometry, candidate_audit, candidate_fixed) in [
        (
            &foreign_geometry,
            &foreign_audit,
            &block_geometry,
            &block_audit,
            block_fixed,
        ),
        (
            &geometry,
            &foreign_audit,
            &block_geometry,
            &block_audit,
            block_fixed,
        ),
        (
            &geometry,
            &audit,
            &foreign_block_geometry,
            &foreign_block_audit,
            foreign_block_fixed,
        ),
        (
            &geometry,
            &audit,
            &block_geometry,
            &wrong_block_audit,
            block_fixed,
        ),
        (
            &geometry,
            &audit,
            &block_geometry,
            &block_audit,
            absent_fixed,
        ),
    ] {
        assert!(matches!(
            ordinary.restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                source_geometry,
                source_audit,
                candidate_geometry,
                candidate_audit,
                candidate_fixed,
                generous,
                || Ok(()),
            ),
            Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput)
        ));
    }
}

#[test]
fn fallible_schedule_buffers_reject_capacity_overflow_as_resource_limit() {
    assert!(matches!(
        try_schedule_vec_with_capacity_v1::<u8>(usize::MAX),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    ));
    assert!(matches!(
        try_multi_hinge_vec_with_capacity_v1::<u8>(usize::MAX),
        Err(MultiHingePathCandidateErrorV1::ResourceLimit)
    ));
}

#[test]
fn bernstein_binomial_overflow_fails_closed() {
    assert_eq!(checked_binomial_v1(5, 2), Some(10));
    assert_eq!(checked_binomial_v1(2, 3), None);
    assert_eq!(checked_binomial_v1(256, 128), None);
}

#[test]
fn exact_bernstein_certificate_proves_only_strict_single_sign_denominators() {
    let positive = prepare_pole_free_bernstein_certificate_v1(
        &[
            RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            },
            RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            },
        ],
        4,
        8,
        16,
    )
    .unwrap();
    assert!(positive.positive);
    assert_eq!(positive.degree, 1);
    assert!(
        positive
            .coefficients
            .iter()
            .all(|value| value.is_positive())
    );
    let denominator = prepare_pole_free_bernstein_certificate_v1(
        &[RationalCoefficientV1 {
            numerator: 2,
            denominator: 1,
        }],
        4,
        8,
        16,
    )
    .unwrap();
    let quotient = evaluate_pole_free_rational_interval_v1(&positive, &denominator, 16).unwrap();
    assert!(quotient.lower() <= 0.5);
    assert!(quotient.upper() >= 1.0);
    assert_eq!(
        evaluate_pole_free_rational_interval_v1(&positive, &denominator, 1),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
    let near_zero = prepare_pole_free_bernstein_certificate_v1(
        &[RationalCoefficientV1 {
            numerator: 1,
            denominator: 1_u64 << 50,
        }],
        4,
        53,
        16,
    )
    .unwrap();
    let large = evaluate_pole_free_rational_interval_v1(&positive, &near_zero, 16).unwrap();
    assert!(large.upper().is_finite());
    assert!(large.lower() > 0.0);
    for invalid in [
        vec![
            RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            },
            RationalCoefficientV1 {
                numerator: -2,
                denominator: 1,
            },
        ],
        vec![RationalCoefficientV1 {
            numerator: 1,
            denominator: 0,
        }],
    ] {
        assert!(prepare_pole_free_bernstein_certificate_v1(&invalid, 4, 8, 16).is_err());
    }
    assert!(
        prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator: 256,
                denominator: 1,
            }],
            4,
            8,
            16,
        )
        .is_err()
    );
    assert!(
        prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }; 5],
            3,
            8,
            16,
        )
        .is_err()
    );
}

#[test]
fn half_angle_domain_separates_tangent_poles_and_encloses_known_angles() {
    assert_eq!(
        deterministic_half_angle_tangent_v1(-90.0).map(f64::to_bits),
        Some((-1.0_f64).to_bits())
    );
    assert_eq!(
        deterministic_half_angle_tangent_v1(-0.0).map(f64::to_bits),
        Some(0.0_f64.to_bits())
    );
    assert_eq!(
        deterministic_half_angle_tangent_v1(90.0).map(f64::to_bits),
        Some(1.0_f64.to_bits())
    );
    let below_right_angle = f64::from_bits(90.0_f64.to_bits() - 1);
    let above_right_angle = f64::from_bits(90.0_f64.to_bits() + 1);
    assert!(deterministic_half_angle_tangent_v1(below_right_angle).unwrap() < 1.0);
    assert!(deterministic_half_angle_tangent_v1(above_right_angle).unwrap() > 1.0);

    let domain = HalfAngleDomainV1::prepare([-90.0, 90.0]).unwrap();
    assert_eq!(domain.angle_degrees(), [-90.0, 90.0]);
    let tangent = domain.half_angle_tangent();
    assert!(tangent.lower() <= -1.0);
    assert!(tangent.upper() >= 1.0);
    for invalid in [[-180.0, 0.0], [0.0, 180.0], [1.0, 1.0], [f64::NAN, 1.0]] {
        assert_eq!(
            HalfAngleDomainV1::prepare(invalid),
            Err(CycleSchedulePrepareErrorV1::InvalidInput)
        );
    }
    let near_poles = HalfAngleDomainV1::prepare([-179.0, 179.0]).unwrap();
    assert!(near_poles.half_angle_tangent().lower() < -100.0);
    assert!(near_poles.half_angle_tangent().upper() > 100.0);
}

#[test]
fn half_angle_point_evaluation_uses_frozen_transcendental_bits() {
    assert_eq!(
        CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2,
        "canonical_cycle_schedule_deterministic_transcendental_v2"
    );
    assert_eq!(
        deterministic_half_angle_ratio_degrees_v1(1.0, 1.0).map(f64::to_bits),
        Some(90.0_f64.to_bits())
    );
    assert_eq!(
        deterministic_half_angle_ratio_degrees_v1(-1.0, 1.0).map(f64::to_bits),
        Some((-90.0_f64).to_bits())
    );

    let one_below = f64::from_bits(1.0_f64.to_bits() - 1);
    let one_above = f64::from_bits(1.0_f64.to_bits() + 1);
    assert_eq!(
        deterministic_half_angle_ratio_degrees_v1(one_below, 1.0).map(f64::to_bits),
        Some(90.0_f64.to_bits())
    );
    assert_eq!(
        deterministic_half_angle_ratio_degrees_v1(one_above, 1.0).map(f64::to_bits),
        Some(90.0_f64.to_bits() + 1)
    );

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            deterministic_half_angle_ratio_degrees_v1(invalid, 1.0),
            None
        );
        assert_eq!(
            deterministic_half_angle_ratio_degrees_v1(1.0, invalid),
            None
        );
    }
}

#[test]
fn pole_free_atan2_encloses_all_strict_quadrants_and_half_angles() {
    let certificate = |numerator| {
        prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator,
                denominator: 1,
            }],
            1,
            8,
            4,
        )
        .unwrap()
    };
    let positive = certificate(1);
    let negative = certificate(-1);
    for (y, x, expected) in [
        (&positive, &positive, core::f64::consts::FRAC_PI_4),
        (&positive, &negative, 3.0 * core::f64::consts::FRAC_PI_4),
        (&negative, &negative, -3.0 * core::f64::consts::FRAC_PI_4),
        (&negative, &positive, -core::f64::consts::FRAC_PI_4),
    ] {
        let angle = evaluate_pole_free_atan2_interval_v1(y, x, 262_144).unwrap();
        assert!(angle.lower() <= expected && expected <= angle.upper());
    }
    let half =
        evaluate_half_angle_rational_degrees_interval_v1(&positive, &positive, 262_144).unwrap();
    assert!(half.lower() <= 90.0 && half.upper() >= 90.0);
    assert_eq!(
        evaluate_pole_free_atan2_interval_v1(&positive, &positive, 1),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
}

#[test]
fn exact_bernstein_derivative_and_same_degree_sub_are_bounded() {
    let range = ExactBernsteinRangeV1 {
        coefficients: [1_i64, 3, 6]
            .map(|value| BigRational::from_integer(value.into()))
            .to_vec(),
    };
    let derivative = range.derivative(16, 8).unwrap();
    assert_eq!(
        derivative.coefficients,
        [4_i64, 6]
            .map(|value| BigRational::from_integer(value.into()))
            .to_vec()
    );
    let difference = range
        .sub_same_degree(
            &ExactBernsteinRangeV1 {
                coefficients: [1_i64, 1, 1]
                    .map(|value| BigRational::from_integer(value.into()))
                    .to_vec(),
            },
            16,
            8,
        )
        .unwrap();
    assert_eq!(
        difference.coefficients,
        [0_i64, 2, 5]
            .map(|value| BigRational::from_integer(value.into()))
            .to_vec()
    );
    assert_eq!(
        range.derivative(2, 8),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
    assert_eq!(
        range.sub_same_degree(&derivative, 16, 8),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
    let linear = ExactBernsteinRangeV1 {
        coefficients: [1_i64, 2]
            .map(|value| BigRational::from_integer(value.into()))
            .to_vec(),
    };
    let square = linear.product(&linear, 16, 8).unwrap();
    assert_eq!(
        square.coefficients,
        [1_i64, 2, 4]
            .map(|value| BigRational::from_integer(value.into()))
            .to_vec()
    );
    let elevated = linear.elevate(2, 16, 8).unwrap();
    assert_eq!(
        elevated.coefficients,
        [
            BigRational::from_integer(1.into()),
            BigRational::new(3.into(), 2.into()),
            BigRational::from_integer(2.into()),
        ]
    );
    assert_eq!(
        elevated.sub(&linear, 16, 16).unwrap().coefficients,
        vec![BigRational::zero(); 3]
    );
    assert_eq!(
        linear.product(&linear, 16, 1),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
    let p = prepare_pole_free_bernstein_certificate_v1(
        &[
            RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            },
            RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            },
        ],
        2,
        32,
        16,
    )
    .unwrap();
    let q = prepare_pole_free_bernstein_certificate_v1(
        &[RationalCoefficientV1 {
            numerator: 1,
            denominator: 1,
        }],
        2,
        32,
        16,
    )
    .unwrap();
    let derivative = evaluate_half_angle_rational_derivative_interval_v1(&p, &q, 64, 64).unwrap();
    assert!(derivative.lower() <= 0.4);
    assert!(derivative.upper() >= 1.0);
    assert_eq!(
        evaluate_pole_free_rational_dyadic_v1(&p, &q, 0.5, 64, 16).unwrap(),
        BigRational::new(3.into(), 2.into())
    );
    for invalid in [f64::NAN, -0.1, 1.1, f64::MIN_POSITIVE / 2.0] {
        assert!(evaluate_pole_free_rational_dyadic_v1(&p, &q, invalid, 64, 16).is_err());
    }
    assert_eq!(
        evaluate_pole_free_rational_dyadic_v1(&p, &q, 0.5, 64, 0),
        Err(CycleSchedulePrepareErrorV1::ResourceLimit)
    );
}

#[test]
fn half_angle_schedule_maps_unit_parameters_to_non_unit_domain_endpoints_v1() {
    let (geometry, audit, fixed_face, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed_face,
        edges
            .into_iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge,
                u_domain: [rational(-1, 1), rational(1, 1)],
                numerator_power_coefficients: vec![rational(1, 1), rational(1, 1)],
                denominator_power_coefficients: vec![rational(64, 1)],
            })
            .collect(),
        CycleScheduleLimitsV1::default(),
    )
    .expect("non-unit-domain half-angle schedule");

    let lower = schedule.evaluate(0.0).expect("normalized lower endpoint");
    let upper = schedule.evaluate(1.0).expect("normalized upper endpoint");
    let expected_lower =
        deterministic_half_angle_ratio_degrees_v1(0.0, 64.0).expect("finite lower endpoint");
    let expected_upper =
        deterministic_half_angle_ratio_degrees_v1(2.0, 64.0).expect("finite upper endpoint");
    assert_eq!(lower.as_slice().len(), geometry.hinges().len());
    assert_eq!(upper.as_slice().len(), geometry.hinges().len());
    assert!(
        lower
            .as_slice()
            .iter()
            .all(|angle| { angle.angle_degrees().to_bits() == expected_lower.to_bits() })
    );
    assert!(
        upper
            .as_slice()
            .iter()
            .all(|angle| { angle.angle_degrees().to_bits() == expected_upper.to_bits() })
    );
}

#[test]
fn half_angle_entry_uses_exact_u_domain_and_horner_evaluation() {
    let entry = PreparedHalfAngleRationalEntryV1::prepare(
        HalfAngleRationalEntryInputV1 {
            edge: EdgeId::derive_v5(ProjectId::new(), b"half-angle-entry"),
            u_domain: [
                RationalCoefficientV1 {
                    numerator: -1,
                    denominator: 4,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 2,
                },
            ],
            numerator_power_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 2,
                    denominator: 1,
                },
            ],
            denominator_power_coefficients: vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
        },
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        entry
            .evaluate_exact(
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 4,
                },
                128,
                16,
            )
            .unwrap(),
        BigRational::new(3.into(), 2.into())
    );
    assert!(
        entry
            .evaluate_exact(
                RationalCoefficientV1 {
                    numerator: 3,
                    denominator: 4,
                },
                128,
                16,
            )
            .is_err()
    );
}

#[test]
fn half_angle_entry_canonicalizes_sign_and_proves_exact_proportional_constants() {
    let edge = EdgeId::derive_v5(ProjectId::new(), b"projective-sign");
    let input = |numerator, denominator| HalfAngleRationalEntryInputV1 {
        edge,
        u_domain: [
            RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            },
            RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            },
        ],
        numerator_power_coefficients: numerator,
        denominator_power_coefficients: denominator,
    };
    let coefficient = |numerator| RationalCoefficientV1 {
        numerator,
        denominator: 1,
    };
    let positive_zero = PreparedHalfAngleRationalEntryV1::prepare(
        input(vec![coefficient(0)], vec![coefficient(1)]),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let negative_zero = PreparedHalfAngleRationalEntryV1::prepare(
        input(vec![coefficient(0)], vec![coefficient(-1)]),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(negative_zero, positive_zero);
    assert!(negative_zero.is_exact_constant_profile_v1());
    assert_eq!(
        negative_zero.derivative_bound_degrees_bits,
        0.0_f64.to_bits()
    );
    assert_eq!(negative_zero.evaluate_degrees(0.5), Some(0.0));

    let proportional = PreparedHalfAngleRationalEntryV1::prepare(
        input(
            vec![coefficient(1), coefficient(1)],
            vec![coefficient(2), coefficient(2)],
        ),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert!(proportional.is_exact_constant_profile_v1());
    assert_eq!(
        proportional.derivative_bound_degrees_bits,
        0.0_f64.to_bits()
    );
    for numerator in [0, 1] {
        assert_eq!(
            proportional
                .evaluate_exact(
                    RationalCoefficientV1 {
                        numerator,
                        denominator: 1,
                    },
                    64,
                    16,
                )
                .unwrap(),
            BigRational::new(BigInt::from(1_u8), BigInt::from(2_u8))
        );
    }
    let constant_180 = PreparedHalfAngleRationalEntryV1::prepare(
        input(vec![coefficient(1)], vec![coefficient(0)]),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    assert!(constant_180.is_exact_constant_profile_v1());
    assert_eq!(
        constant_180.derivative_bound_degrees_bits,
        0.0_f64.to_bits()
    );
    assert_eq!(constant_180.evaluate_degrees(0.5), Some(180.0));

    assert_eq!(
        PreparedHalfAngleRationalEntryV1::prepare(
            input(vec![coefficient(1)], vec![coefficient(-1)]),
            CycleScheduleLimitsV1::default(),
        ),
        Err(CycleSchedulePrepareErrorV1::AngleRange)
    );
}

#[test]
fn projective_half_angle_allows_closed_q_zero_endpoint_but_not_crossing_or_origin() {
    let edge = EdgeId::derive_v5(ProjectId::new(), b"projective-endpoint");
    let input = |numerator, denominator| HalfAngleRationalEntryInputV1 {
        edge,
        u_domain: [
            RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            },
            RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            },
        ],
        numerator_power_coefficients: numerator,
        denominator_power_coefficients: denominator,
    };
    let entry = PreparedHalfAngleRationalEntryV1::prepare(
        input(
            vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
            vec![
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
        ),
        CycleScheduleLimitsV1::default(),
    )
    .expect("q=u is projectively regular on the closed domain");
    let endpoint = entry
        .endpoint_angle_enclosure(false, 128, CycleScheduleLimitsV1::default().max_work)
        .unwrap();
    assert!(endpoint.lower() <= 180.0 && endpoint.upper() >= 180.0);
    assert!(
        PreparedHalfAngleRationalEntryV1::prepare(
            input(
                vec![RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1
                }],
                vec![RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1
                }],
            ),
            CycleScheduleLimitsV1::default(),
        )
        .is_ok(),
        "constant 180-degree projective entry is regular"
    );

    for invalid in [
        input(
            vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
            vec![
                RationalCoefficientV1 {
                    numerator: -1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 2,
                    denominator: 1,
                },
            ],
        ),
        input(
            vec![RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            }],
            vec![
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
        ),
        input(
            vec![RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            }],
            vec![
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: -1,
                    denominator: 1,
                },
            ],
        ),
    ] {
        assert!(
            PreparedHalfAngleRationalEntryV1::prepare(invalid, CycleScheduleLimitsV1::default(),)
                .is_err()
        );
    }
}

#[test]
fn dyadic_angle_boxes_cover_in_canonical_shared_endpoint_order() {
    let (geometry, audit, fixed, edges) = fixture();
    let mut inputs = edges
        .into_iter()
        .map(|edge| HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
            denominator_power_coefficients: vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        inputs,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let evaluated = schedule.evaluate(0.5).expect("certified point evaluation");
    assert!(evaluated.as_slice().iter().all(|angle| {
        (angle.angle_degrees() - 90.0).abs() <= 1.0e-12
            && schedule
                .derivative_bound(angle.edge())
                .is_some_and(|bound| bound.to_bits() == 0.0_f64.to_bits())
    }));
    let left = schedule
        .evaluate_angle_box_dyadic(1, 0, CycleScheduleLimitsV1::default())
        .unwrap();
    let right = schedule
        .evaluate_angle_box_dyadic(1, 1, CycleScheduleLimitsV1::default())
        .unwrap();
    assert_eq!(left, right);
    for upper in [false, true] {
        let endpoint = schedule
            .evaluate_endpoint_angle_box(upper, CycleScheduleLimitsV1::default())
            .unwrap();
        assert!(
            endpoint
                .iter()
                .all(|(_, angle)| { angle.lower() <= 90.0 && angle.upper() >= 90.0 })
        );
    }
    assert_eq!(
        schedule.evaluate_angle_box_dyadic(1, 2, CycleScheduleLimitsV1::default()),
        Err(CycleSchedulePrepareErrorV1::InvalidInput)
    );
}

#[test]
fn dyadic_angle_boxes_admit_a_certified_flat_endpoint() {
    let (geometry, audit, fixed, edges) = fixture();
    let mut inputs = edges
        .into_iter()
        .map(|edge| HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
            denominator_power_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 5,
                    denominator: 1,
                },
            ],
        })
        .collect::<Vec<_>>();
    inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        inputs,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();

    let root = schedule
        .evaluate_angle_box_dyadic(
            8,
            0,
            CycleScheduleLimitsV1 {
                max_work: 1_048_576,
                ..CycleScheduleLimitsV1::default()
            },
        )
        .unwrap();
    assert!(
        root.iter()
            .all(|(_, angle)| angle.lower() <= 180.0 && angle.upper() >= 180.0)
    );
}

#[test]
fn collective_profile_rejects_nonidentical_moving_schedules() {
    let (geometry, audit, fixed, edges) = fixture();
    let mut inputs = edges
        .into_iter()
        .enumerate()
        .map(|(index, edge)| HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: index as i64 + 1,
                    denominator: 1,
                },
            ],
            denominator_power_coefficients: vec![RationalCoefficientV1 {
                numerator: 5,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        inputs,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();

    assert!(schedule.collective_half_angle_profile_edges_v1().is_none());
}

#[test]
fn collective_profile_normalizes_trailing_zero_inactive_numerators() {
    let (geometry, audit, fixed, mut edges) = fixture();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let active = edges[0];
    let inputs = edges
        .iter()
        .map(|edge| {
            let active = *edge == active;
            HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                numerator_power_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: i64::from(active),
                        denominator: 1,
                    },
                ],
                denominator_power_coefficients: if active {
                    vec![RationalCoefficientV1 {
                        numerator: 64,
                        denominator: 1,
                    }]
                } else {
                    vec![
                        RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ]
                },
            }
        })
        .collect();
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        inputs,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();

    assert!(!schedule.is_exact_constant_profile_v1(active));
    assert!(
        edges
            .iter()
            .copied()
            .filter(|edge| *edge != active)
            .all(|edge| {
                schedule.is_exact_constant_profile_v1(edge)
                    && schedule
                        .derivative_bound(edge)
                        .is_some_and(|bound| bound.to_bits() == 0.0_f64.to_bits())
            })
    );
    assert_eq!(
        schedule.collective_half_angle_profile_edges_v1(),
        Some(vec![active])
    );
}

#[test]
fn half_angle_schedule_admission_binds_both_endpoints_bit_exactly() {
    let (geometry, audit, fixed, edges) = fixture();
    let mut inputs = edges
        .into_iter()
        .map(|edge| HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            numerator_power_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            denominator_power_coefficients: vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    inputs.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed,
        inputs,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let initial = schedule.evaluate(0.0).unwrap();
    let requested = schedule.evaluate(1.0).unwrap();
    let admitted =
        admit_canonical_multi_hinge_path_candidate_v1(schedule.clone(), &initial, &requested)
            .unwrap();
    assert_eq!(admitted.moving_hinges().len(), geometry.hinges().len());

    let mut forged = requested.as_slice().to_vec();
    forged[0] = HingeAngle::new(
        forged[0].edge(),
        f64::from_bits(forged[0].angle_degrees().to_bits() - 1),
    )
    .unwrap();
    let forged = CanonicalHingeAngles::new(forged).unwrap();
    assert_eq!(
        admit_canonical_multi_hinge_path_candidate_v1(schedule, &initial, &forged),
        Err(MultiHingePathCandidateErrorV1::InvalidBinding)
    );
}
