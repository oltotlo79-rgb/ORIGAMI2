use std::collections::{BTreeMap, BTreeSet};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_topology::{
    FaceExtractionInput, LocalFlatFoldabilityReport, TopologySnapshot,
    analyze_local_flat_foldability, extract_faces_strict,
};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use super::*;

const REVISION: u64 = 41;

fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
        .expect("fixed UUID fixture")
}

fn three_panel_accordion() -> (Paper, CreasePattern, TopologySnapshot) {
    let vertices = (0..8)
        .map(|index| fixed_id::<VertexId>(0x100 + index))
        .collect::<Vec<_>>();
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(4.0, 0.0),
        Point2::new(6.0, 0.0),
        Point2::new(6.0, 2.0),
        Point2::new(4.0, 2.0),
        Point2::new(2.0, 2.0),
        Point2::new(0.0, 2.0),
    ];
    let vertex_records = vertices
        .iter()
        .copied()
        .zip(positions)
        .map(|(id, position)| Vertex { id, position })
        .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: fixed_id(0x200 + index as u64),
            start: vertices[index],
            end: vertices[(index + 1) % vertices.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: fixed_id(0x301),
        start: vertices[1],
        end: vertices[6],
        kind: EdgeKind::Mountain,
    });
    edges.push(Edge {
        id: fixed_id(0x302),
        start: vertices[2],
        end: vertices[5],
        kind: EdgeKind::Valley,
    });
    let paper = Paper {
        boundary_vertices: vertices,
        ..Paper::default()
    };
    let pattern = CreasePattern {
        vertices: vertex_records,
        edges,
    };
    let topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>(1),
        source_revision: REVISION,
        paper: &paper,
        pattern: &pattern,
    })
    .expect("three-panel accordion topology");
    (paper, pattern, topology)
}

fn canonical_n33_compact_source_v2() -> (
    ProjectId,
    Paper,
    CreasePattern,
    TopologySnapshot,
    LocalFlatFoldabilityReport,
) {
    canonical_n33_compact_source_with_namespace_v2(canonical_n33_namespace_v2())
}

fn canonical_n33_namespace_v2() -> ProjectId {
    ProjectId::schema_namespace([
        0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x4e, 0x5f, 0x56, 0x32, 0, 0, 2,
    ])
}

fn canonical_n33_compact_source_with_namespace_v2(
    namespace: ProjectId,
) -> (
    ProjectId,
    Paper,
    CreasePattern,
    TopologySnapshot,
    LocalFlatFoldabilityReport,
) {
    let cells = (0_i8..33)
        .flat_map(|index| {
            let x = index.checked_mul(2).expect("N33 fixture x fits i8");
            let y = if index % 2 == 0 { 0_i8 } else { -2_i8 };
            (x..=x + 2).flat_map(move |x| (y..=y + 2).map(move |y| (x, y)))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (pattern, paper) = compact_miura_pattern_v2(&cells, namespace);
    let topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .expect("genuine fixed-namespace Miura topology");
    let local = analyze_local_flat_foldability(&paper, &pattern);
    (namespace, paper, pattern, topology, local)
}

fn compact_miura_pattern_v2(cells: &[(i8, i8)], namespace: ProjectId) -> (CreasePattern, Paper) {
    let mut points = BTreeSet::new();
    let mut incidence = BTreeMap::<((i8, i8), (i8, i8)), (usize, (i8, i8), (i8, i8))>::new();
    for &(x, y) in cells {
        let corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
        points.extend(corners);
        for index in 0..4 {
            let start = corners[index];
            let end = corners[(index + 1) % 4];
            let key = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            incidence
                .entry(key)
                .and_modify(|entry| entry.0 += 1)
                .or_insert((1, start, end));
        }
    }
    let vertices = points
        .iter()
        .map(|&(x, y)| Vertex {
            id: VertexId::derive_v5(namespace, &[0xc1, (x + 4) as u8, (y + 4) as u8]),
            position: Point2::new(f64::from(x) * 20.0, f64::from(y) * 20.0),
        })
        .collect::<Vec<_>>();
    let vertex = |point: (i8, i8)| {
        vertices[points
            .iter()
            .position(|candidate| *candidate == point)
            .expect("N33 cell corner")]
        .id
    };
    let edges = incidence
        .iter()
        .map(|(&(first, second), &(count, start, end))| Edge {
            id: EdgeId::derive_v5(
                namespace,
                &[
                    0xc2,
                    (first.0 + 4) as u8,
                    (first.1 + 4) as u8,
                    (second.0 + 4) as u8,
                    (second.1 + 4) as u8,
                ],
            ),
            start: vertex(start),
            end: vertex(end),
            kind: if count == 1 {
                EdgeKind::Boundary
            } else if first.1 == second.1 {
                EdgeKind::Mountain
            } else if first.1.rem_euclid(2) == 0 {
                EdgeKind::Valley
            } else {
                EdgeKind::Mountain
            },
        })
        .collect::<Vec<_>>();
    let directed = incidence
        .values()
        .filter(|(count, _, _)| *count == 1)
        .map(|(_, start, end)| (*start, *end))
        .collect::<Vec<_>>();
    let mut boundary = vec![directed[0].0];
    while boundary.len() < directed.len() {
        let cursor = *boundary.last().expect("N33 boundary start");
        boundary.push(
            directed
                .iter()
                .find(|(start, _)| *start == cursor)
                .expect("next N33 boundary edge")
                .1,
        );
    }
    let boundary_vertices = boundary.into_iter().map(vertex).collect();
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices,
            thickness_mm: 0.1,
            ..Paper::default()
        },
    )
}

#[path = "../../../../test-support/n33_compact_pair_assignment_v2.rs"]
mod n33_compact_pair_assignment_fixture_v2;
use n33_compact_pair_assignment_fixture_v2::{
    N33_COMPACT_ASSIGNMENT_BYTES_V2, N33_COMPACT_VARIABLE_COUNT_V2,
    n33_compact_pair_assignment_sha256_v2, n33_compact_pair_assignment_v2,
};

#[test]
fn n33_compact_assignment_receipt_is_pinned_v2() {
    let (variable_count, registry_digest, direction_bits) = n33_compact_pair_assignment_v2();
    assert_eq!(
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &direction_bits,
        )
        .expect("checked N=33 assignment digest"),
        n33_compact_pair_assignment_sha256_v2()
    );
}

#[test]
fn genuine_n33_compact_assignment_issues_without_search_v2() {
    let (variable_count, registry_digest, direction_bits) = n33_compact_pair_assignment_v2();
    assert_eq!(variable_count, N33_COMPACT_VARIABLE_COUNT_V2);
    assert_eq!(direction_bits.len(), N33_COMPACT_ASSIGNMENT_BYTES_V2);
    assert!(!facewise::compact_assignment_has_nonzero_tail_v2(
        &direction_bits,
        variable_count,
    ));
    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        analysis,
        ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
    };

    let (namespace, paper, pattern, topology, local) = canonical_n33_compact_source_v2();
    let first = issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
            source: GlobalFlatFoldabilityInput::current_with_geometry(
                namespace, &paper, &pattern, &topology, &local,
            ),
            variable_count,
            variable_registry_sha256: registry_digest,
            direction_bits_le: &direction_bits,
        },
        limits,
    )
    .expect("fixed-namespace N33 compact assignment issues without search");
    assert_eq!(first.variable_count_v2(), variable_count);
    assert_eq!(first.variable_registry_sha256_v2(), registry_digest);
    let expected_assignment_digest = n33_compact_pair_assignment_sha256_v2();
    assert_eq!(
        first.direction_assignment_sha256_v2(),
        expected_assignment_digest,
        "the sealed N=33 authority binds the checked assignment asset"
    );
    assert_eq!(
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &direction_bits,
        )
        .expect("checked N=33 assignment digest"),
        expected_assignment_digest,
        "the production domain-separated digest reproduces the pinned receipt"
    );
    assert_eq!(
        first.resources_v2().compact_assignment_bytes,
        N33_COMPACT_ASSIGNMENT_BYTES_V2
    );
    assert_eq!(first.work_counts_v2().search_nodes, 0);
    assert_eq!(first.layer_order_snapshot_v2().material_faces.len(), 265);
    assert_eq!(
        first.layer_order_snapshot_v2().face_pair_orders.len(),
        variable_count
    );
    assert_eq!(
        first
            .layer_order_snapshot_v2()
            .proof_summary
            .expect("N33 facewise summary")
            .search_nodes,
        0,
    );
}

#[test]
fn compact_pair_assignment_reconstructs_without_search_and_is_exactly_bounded() {
    let (paper, pattern, topology) = three_panel_accordion();
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let source = || {
        GlobalFlatFoldabilityInput::current_with_geometry(
            fixed_id::<ProjectId>(1),
            &paper,
            &pattern,
            &topology,
            &local,
        )
    };
    let report = analyze_global_flat_foldability(source(), GlobalFlatFoldabilityLimits::default())
        .expect("baseline compact-assignment source");
    let snapshot = report.layer_order().expect("possible accordion source");
    let (variable_count, registry_digest, direction_bits) =
        facewise::compact_assignment_from_snapshot_for_test_v2(snapshot);
    assert_eq!(variable_count, 3);
    assert_eq!(direction_bits.len(), 1);

    let compact_input = || GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
        source: source(),
        variable_count,
        variable_registry_sha256: registry_digest,
        direction_bits_le: &direction_bits,
    };
    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let baseline_limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        analysis,
        ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
    };
    let authority = issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        compact_input(),
        baseline_limits,
    )
    .expect("canonical complete assignment reconstructs without search");
    assert_eq!(authority.work_counts_v2().search_nodes, 0);
    assert_eq!(authority.variable_count_v2(), variable_count);
    assert_eq!(authority.variable_registry_sha256_v2(), registry_digest);
    assert_eq!(
        authority.direction_assignment_sha256_v2(),
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &direction_bits,
        )
        .expect("well-formed compact assignment digest")
    );
    let mut wrong_domain = Sha256::new();
    wrong_domain.update(GLOBAL_FLAT_LAYER_ORDER_PAIR_REGISTRY_DOMAIN_V2);
    wrong_domain.update(
        u64::try_from(variable_count)
            .expect("fixture variable count fits u64")
            .to_le_bytes(),
    );
    wrong_domain.update(registry_digest);
    wrong_domain.update(
        u64::try_from(direction_bits.len())
            .expect("fixture assignment length fits u64")
            .to_le_bytes(),
    );
    wrong_domain.update(&direction_bits);
    assert_ne!(
        authority.direction_assignment_sha256_v2(),
        <[u8; 32]>::from(wrong_domain.finalize()),
        "the assignment receipt must not reuse the registry hash domain"
    );
    let mut foreign_registry = registry_digest;
    foreign_registry[0] ^= 1;
    assert_ne!(
        authority.direction_assignment_sha256_v2(),
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            foreign_registry,
            &direction_bits,
        )
        .expect("foreign registry still has a well-formed digest")
    );
    let mut different_bits = direction_bits.clone();
    different_bits[0] ^= 1;
    assert_ne!(
        authority.direction_assignment_sha256_v2(),
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &different_bits,
        )
        .expect("changed direction remains structurally well formed")
    );
    assert_eq!(
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &[],
        ),
        None
    );
    let mut nonzero_tail = direction_bits.clone();
    *nonzero_tail.last_mut().expect("three-pair byte") |= 0x80;
    assert_eq!(
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &nonzero_tail,
        ),
        None
    );
    assert_eq!(authority.exact_limits_v2(), baseline_limits);
    assert_eq!(
        authority
            .layer_order_snapshot_v2()
            .proof_summary
            .expect("facewise summary")
            .search_nodes,
        0
    );
    assert_eq!(
        authority.layer_order_snapshot_v2().face_pair_orders,
        snapshot.face_pair_orders
    );
    let resources = authority.resources_v2();
    assert_eq!(resources.compact_assignment_bytes, direction_bits.len());
    assert_eq!(
        resources.layer_order_retained_bytes,
        authority
            .layer_order_snapshot_v2()
            .checked_deep_retained_bytes_v1()
            .expect("issued retained bytes")
    );
    assert!(resources.observed_peak_bytes >= resources.borrowed_live_bytes);
    assert!(resources.observed_peak_bytes >= resources.layer_order_retained_bytes);

    let exact_limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        max_compact_assignment_bytes: resources.compact_assignment_bytes,
        max_layer_order_retained_bytes: resources.layer_order_retained_bytes,
        max_peak_bytes: resources.observed_peak_bytes,
        ..baseline_limits
    };
    issue_global_flat_layer_order_from_compact_pair_assignment_v2(compact_input(), exact_limits)
        .expect("all exact compact/result/peak equalities are admitted");
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
                max_compact_assignment_bytes: resources.compact_assignment_bytes - 1,
                ..baseline_limits
            },
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CompactPairAssignmentBytes,
                limit,
                observed,
            }
        }) if limit + 1 == observed && observed == resources.compact_assignment_bytes
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
                max_layer_order_retained_bytes: resources.layer_order_retained_bytes - 1,
                ..baseline_limits
            },
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::LayerOrderResultBytes,
                limit,
                observed,
            }
        }) if limit + 1 == observed && observed == resources.layer_order_retained_bytes
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
                max_peak_bytes: resources.observed_peak_bytes - 1,
                ..baseline_limits
            },
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
                limit,
                observed,
            }
        }) if limit + 1 == resources.observed_peak_bytes && observed > limit
    ));

    let internal_peak = resources
        .observed_facewise_peak_bytes
        .checked_sub(resources.borrowed_live_bytes)
        .expect("borrowed compact/canonical bytes are part of facewise peak");
    let certificate_limit = internal_peak.max(authority.work_counts_v2().certificate_bytes);
    let mut certificate_exact = exact_limits;
    certificate_exact.analysis.max_certificate_bytes = certificate_limit;
    issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        compact_input(),
        certificate_exact,
    )
    .expect("exact certificate/workspace equality is admitted");
    certificate_exact.analysis.max_certificate_bytes -= 1;
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            certificate_exact,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        }) if limit + 1 == certificate_limit && observed > limit
    ));

    let mut pair_one_short = baseline_limits;
    pair_one_short.analysis.max_overlap_face_pairs = variable_count - 1;
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            compact_input(),
            pair_one_short,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::OverlapFacePairs,
                limit,
                observed,
            }
        }) if limit + 1 == variable_count && observed == variable_count
    ));

    let retained = resources.layer_order_retained_bytes;
    authority
        .revalidate_live_source_v2(
            source(),
            GlobalFlatLayerOrderRevalidationLimitsV2 {
                analysis,
                max_source_retained_bytes: retained,
                max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
            },
        )
        .expect("consumer live revalidation rebuilds more than provenance");
}

#[test]
fn compact_pair_assignment_is_canonical_and_rejects_drift_tamper_and_stops() {
    struct StopObserver {
        remaining: usize,
        stop: GlobalFlatFoldabilityCheckpoint,
    }
    impl GlobalFlatFoldabilityObserver for StopObserver {
        fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
            if self.remaining == 0 {
                self.stop
            } else {
                self.remaining -= 1;
                GlobalFlatFoldabilityCheckpoint::Continue
            }
        }
    }

    let (paper, pattern, topology) = three_panel_accordion();
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let source = || {
        GlobalFlatFoldabilityInput::current_with_geometry(
            fixed_id::<ProjectId>(1),
            &paper,
            &pattern,
            &topology,
            &local,
        )
    };
    let report = analyze_global_flat_foldability(source(), GlobalFlatFoldabilityLimits::default())
        .expect("baseline canonical assignment");
    let snapshot = report.layer_order().expect("possible accordion source");
    let (variable_count, registry_digest, direction_bits) =
        facewise::compact_assignment_from_snapshot_for_test_v2(snapshot);
    let mut reordered_snapshot = snapshot.clone();
    reordered_snapshot.face_pair_orders.reverse();
    assert_eq!(
        facewise::compact_assignment_from_snapshot_for_test_v2(&reordered_snapshot),
        (variable_count, registry_digest, direction_bits.clone()),
        "registry and packed bits are independent of source record order"
    );

    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        analysis,
        ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
    };
    let compact = |source, digest, bits: &[u8]| {
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source,
                variable_count,
                variable_registry_sha256: digest,
                direction_bits_le: bits,
            },
            limits,
        )
    };

    let mut bad_digest = registry_digest;
    bad_digest[0] ^= 1;
    assert!(matches!(
        compact(source(), bad_digest, &direction_bits),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::RegistryMismatch)
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: source(),
                variable_count: variable_count + 1,
                variable_registry_sha256: registry_digest,
                direction_bits_le: &direction_bits,
            },
            limits,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::RegistryMismatch)
    ));
    let mut nonzero_tail = direction_bits.clone();
    *nonzero_tail.last_mut().unwrap() |= 0x80;
    assert!(matches!(
        compact(source(), registry_digest, &nonzero_tail),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
            GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::NonZeroTailBits
        ))
    ));
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: source(),
                variable_count,
                variable_registry_sha256: registry_digest,
                direction_bits_le: &[],
            },
            limits,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
            GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::ByteLength {
                expected: 1,
                actual: 0,
            }
        ))
    ));
    let mut rejected_direction = false;
    for index in 0..variable_count {
        let mut tampered = direction_bits.clone();
        tampered[index / 8] ^= 1_u8 << (index % 8);
        if matches!(
            compact(source(), registry_digest, &tampered),
            Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::AssignmentRejected)
        ) {
            rejected_direction = true;
            break;
        }
    }
    assert!(
        rejected_direction,
        "at least one trusted hinge direction must reject bit tampering"
    );

    let mut spare_capacity_bits = Vec::with_capacity(direction_bits.len() + 257);
    spare_capacity_bits.extend_from_slice(&direction_bits);
    let spare_authority = compact(source(), registry_digest, &spare_capacity_bits)
        .expect("borrowed compact bytes are length-defined across allocators");
    assert_eq!(
        spare_authority.resources_v2().compact_assignment_bytes,
        direction_bits.len()
    );

    let paper_clone = paper.clone();
    let pattern_clone = pattern.clone();
    let topology_clone = topology.clone();
    let local_clone = local.clone();
    let equal_instance = GlobalFlatFoldabilityInput::current_with_geometry(
        fixed_id::<ProjectId>(1),
        &paper_clone,
        &pattern_clone,
        &topology_clone,
        &local_clone,
    );
    let equal_authority = compact(equal_instance, registry_digest, &direction_bits)
        .expect("equal geometry in a separate allocation has the same registry bytes");
    assert_eq!(
        equal_authority.variable_registry_sha256_v2(),
        registry_digest
    );

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let reordered_topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>(1),
        source_revision: REVISION,
        paper: &paper,
        pattern: &reordered_pattern,
    })
    .expect("reordered semantic-equal topology");
    let reordered_local = analyze_local_flat_foldability(&paper, &reordered_pattern);
    let reordered_source = GlobalFlatFoldabilityInput::current_with_geometry(
        fixed_id::<ProjectId>(1),
        &paper,
        &reordered_pattern,
        &reordered_topology,
        &reordered_local,
    );
    let reordered_authority = compact(reordered_source, registry_digest, &direction_bits)
        .expect("canonical registry ignores live record storage order");
    assert_eq!(
        reordered_authority.variable_registry_sha256_v2(),
        registry_digest
    );
    assert_eq!(
        reordered_authority
            .layer_order_snapshot_v2()
            .face_pair_orders,
        spare_authority.layer_order_snapshot_v2().face_pair_orders
    );

    let foreign_namespace = fixed_id::<ProjectId>(2);
    let foreign_topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: foreign_namespace,
        source_revision: REVISION,
        paper: &paper,
        pattern: &pattern,
    })
    .expect("foreign-namespace topology");
    let foreign_source = GlobalFlatFoldabilityInput::current_with_geometry(
        foreign_namespace,
        &paper,
        &pattern,
        &foreign_topology,
        &local,
    );
    assert!(matches!(
        compact(foreign_source, registry_digest, &direction_bits),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::RegistryMismatch)
    ));

    let mut drifted_pattern = pattern.clone();
    let coordinate = &mut drifted_pattern.vertices[0].position.x;
    *coordinate = f64::from_bits(coordinate.to_bits() + 1);
    let drifted_source = GlobalFlatFoldabilityInput::current_with_geometry(
        fixed_id::<ProjectId>(1),
        &paper,
        &drifted_pattern,
        &topology,
        &local,
    );
    let drifted_authority = compact(drifted_source, registry_digest, &direction_bits)
        .expect("a still-valid assignment may be newly issued for changed geometry");
    assert_ne!(
        drifted_authority.provenance_v2().source_fingerprint,
        spare_authority.provenance_v2().source_fingerprint
    );
    assert!(
        spare_authority
            .revalidate_live_source_v2(
                drifted_source,
                GlobalFlatLayerOrderRevalidationLimitsV2 {
                    analysis,
                    max_source_retained_bytes: spare_authority
                        .resources_v2()
                        .layer_order_retained_bytes,
                    max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
                },
            )
            .is_err(),
        "one-ULP source drift invalidates the previously issued authority"
    );

    let mut invalid_limits = limits;
    invalid_limits.max_peak_bytes = usize::MAX;
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: source(),
                variable_count,
                variable_registry_sha256: registry_digest,
                direction_bits_le: &direction_bits,
            },
            invalid_limits,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::InvalidLimits)
    ));

    let mut pre_hash_stop = StopObserver {
        remaining: 0,
        stop: GlobalFlatFoldabilityCheckpoint::Cancelled,
    };
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_with_observer_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: source(),
                variable_count,
                variable_registry_sha256: registry_digest,
                direction_bits_le: &direction_bits,
            },
            GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
                max_peak_bytes: direction_bits.len() - 1,
                ..limits
            },
            &mut pre_hash_stop,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
            reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
                limit,
                observed,
            }
        }) if limit + 1 == observed && observed == direction_bits.len()
    ));

    let observer_input = || GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
        source: source(),
        variable_count,
        variable_registry_sha256: registry_digest,
        direction_bits_le: &direction_bits,
    };
    let mut cancelled = StopObserver {
        remaining: 10,
        stop: GlobalFlatFoldabilityCheckpoint::Cancelled,
    };
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_with_observer_v2(
            observer_input(),
            limits,
            &mut cancelled,
        ),
        Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    let mut deadline = StopObserver {
        remaining: 10,
        stop: GlobalFlatFoldabilityCheckpoint::DeadlineReached,
    };
    assert!(matches!(
        issue_global_flat_layer_order_from_compact_pair_assignment_with_observer_v2(
            observer_input(),
            limits,
            &mut deadline,
        ),
        Err(
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
            }
        )
    ));
}
