use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use super::*;
use crate::{
    PositiveThicknessContinuousCertificateV1,
    certify_canonical_positive_thickness_cycle_schedule_path_v1,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalEdgeBlockLimitsV1, CanonicalHingeAngles, CommonArticulationPoseExtensionAuthorityV1,
    DyadicIntervalClosureLimitsV1, HalfAngleRationalEntryInputV1, HingeAngle,
    RationalCoefficientV1, TreeKinematicsLimits,
    prove_common_articulation_pose_extension_authority_v1,
};
use ori_topology::{FaceExtractionInput, analyze_faces};
use sha2::{Digest, Sha256};

struct ExtensionClearanceFixtureV1 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    pose: ClosedMaterialHingeGraphPose,
    decomposition: CanonicalMaterialEdgeBlockDecompositionV1,
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    pairs: Vec<CommonArticulationCrossBlockFacePairV1>,
    positive: PositiveThicknessContinuousCertificateV1,
    paper_thickness_mm: f64,
}

impl ExtensionClearanceFixtureV1 {
    fn pose_authority_v1(
        &self,
        configured_max_blocks: usize,
    ) -> CommonArticulationPoseExtensionAuthorityV1 {
        prove_common_articulation_pose_extension_authority_v1(
            CommonArticulationPoseExtensionInputV1 {
                geometry: &self.geometry,
                pose: &self.pose,
                decomposition: &self.decomposition,
                paper_thickness_mm: self.paper_thickness_mm,
                limits: pose_extension_limits_v1(configured_max_blocks),
            },
        )
        .expect("extension pose authority")
    }

    fn input<'a>(
        &'a self,
        common_pose: &'a CommonArticulationPoseExtensionAuthorityV1,
        common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
        submitted_cross_block_pairs: &'a [CommonArticulationCrossBlockFacePairV1],
        whole_parent_continuous: Option<PositiveThicknessContinuousCertificateV1>,
        limits: CommonArticulationClearanceExtensionLimitsV1,
    ) -> CommonArticulationClearanceExtensionInputV1<'a> {
        CommonArticulationClearanceExtensionInputV1 {
            geometry: &self.geometry,
            audit: &self.audit,
            pose: &self.pose,
            decomposition: &self.decomposition,
            common_pose,
            common_pose_limits,
            schedule: &self.schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure: &self.closure,
            paper_thickness_mm: self.paper_thickness_mm,
            submitted_cross_block_pairs,
            whole_parent_continuous,
            limits,
        }
    }

    fn revalidation_input<'a>(
        &'a self,
        common_pose: &'a CommonArticulationPoseExtensionAuthorityV1,
        common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
        limits: CommonArticulationClearanceExtensionLimitsV1,
    ) -> CommonArticulationClearanceExtensionRevalidationInputV1<'a> {
        CommonArticulationClearanceExtensionRevalidationInputV1 {
            geometry: &self.geometry,
            audit: &self.audit,
            pose: &self.pose,
            decomposition: &self.decomposition,
            common_pose,
            common_pose_limits,
            schedule: &self.schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure: &self.closure,
            paper_thickness_mm: self.paper_thickness_mm,
            limits,
        }
    }
}

fn coefficient_v1(numerator: i64, denominator: u64) -> RationalCoefficientV1 {
    RationalCoefficientV1 {
        numerator,
        denominator,
    }
}

fn prepare_extension_schedule_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
) -> (
    CanonicalCycleScheduleV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
) {
    let entries = geometry
        .hinges()
        .iter()
        .map(|hinge| HalfAngleRationalEntryInputV1 {
            edge: hinge.edge(),
            u_domain: [coefficient_v1(0, 1), coefficient_v1(1, 1)],
            numerator_power_coefficients: vec![coefficient_v1(0, 1), coefficient_v1(0, 1)],
            denominator_power_coefficients: vec![coefficient_v1(64, 1)],
        })
        .collect();
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        geometry,
        audit,
        fixed_face,
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("canonical extension strip schedule");
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            audit,
            fixed_face,
            &schedule,
            1.0e-9,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 8,
                max_leaves: 256,
                max_work: 1_000_000,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("canonical extension strip closure");
    (schedule, closure)
}

fn independent_cross_block_pairs_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
) -> Vec<CommonArticulationCrossBlockFacePairV1> {
    let blocks = decomposition.blocks();
    let mut pairs = Vec::new();
    for first in 0..blocks.len() {
        for second in first + 1..blocks.len() {
            for first_face in blocks[first].geometry().face_ids() {
                for second_face in blocks[second].geometry().face_ids() {
                    if let Some(pair) =
                        CommonArticulationCrossBlockFacePairV1::new(*first_face, *second_face)
                    {
                        pairs.push(pair);
                    }
                }
            }
        }
    }
    pairs.sort_unstable_by(compare_pair_v1);
    pairs.dedup();
    pairs
}

fn prepare_extension_clearance_fixture_v1(block_count: usize) -> ExtensionClearanceFixtureV1 {
    assert!((10..=COMMON_ARTICULATION_CLEARANCE_EXTENSION_MAX_BLOCKS_V1).contains(&block_count));
    let namespace = ProjectId::new();
    let face_count = block_count + 1;
    let bottom = (0..=face_count)
        .map(|_| VertexId::new())
        .collect::<Vec<_>>();
    let top = (0..=face_count)
        .map(|_| VertexId::new())
        .collect::<Vec<_>>();
    let vertices = bottom
        .iter()
        .zip(&top)
        .enumerate()
        .flat_map(|(x, (&bottom, &top))| {
            let x = x as f64 * 10.0;
            [
                Vertex {
                    id: bottom,
                    position: Point2::new(x, 0.0),
                },
                Vertex {
                    id: top,
                    position: Point2::new(x, 10.0),
                },
            ]
        })
        .collect::<Vec<_>>();
    let boundary = bottom
        .iter()
        .copied()
        .chain(top.iter().rev().copied())
        .collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend((1..=block_count).map(|x| Edge {
        id: EdgeId::new(),
        start: bottom[x],
        end: top[x],
        kind: EdgeKind::Mountain,
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper_thickness_mm = 0.1;
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: paper_thickness_mm,
        ..Paper::default()
    };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("extension strip topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("extension strip geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("extension strip audit");
    assert_eq!(geometry.face_ids().len(), face_count);
    assert_eq!(geometry.hinges().len(), block_count);
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            &audit,
            CanonicalEdgeBlockLimitsV1 {
                max_blocks: block_count,
                max_faces_per_block: 2,
                max_hinges_per_block: 2,
            },
        )
        .expect("canonical extension strip decomposition");
    assert_eq!(decomposition.blocks().len(), block_count);
    let fixed_face = geometry.face_ids()[0];
    let angles = CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero extension strip angle"))
            .collect(),
    )
    .expect("canonical extension strip angles");
    let pose = geometry
        .solve_closed(&audit, fixed_face, &angles, 0.0)
        .expect("closed extension strip pose");
    let (schedule, closure) = prepare_extension_schedule_v1(&geometry, &audit, fixed_face);
    let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry,
        &audit,
        fixed_face,
        &schedule,
        &closure,
        paper_thickness_mm,
        16,
    )
    .expect("whole-parent extension positive-thickness certificate");
    let pairs = independent_cross_block_pairs_v1(&decomposition);
    assert!(!pairs.is_empty());
    ExtensionClearanceFixtureV1 {
        geometry,
        audit,
        pose,
        decomposition,
        schedule,
        closure,
        pairs,
        positive,
        paper_thickness_mm,
    }
}

fn pose_extension_limits_v1(
    configured_max_blocks: usize,
) -> CommonArticulationPoseExtensionLimitsV1 {
    CommonArticulationPoseExtensionLimitsV1::with_max_blocks_v1(configured_max_blocks)
        .expect("valid pose extension cap")
}

fn clearance_extension_limits_v1(
    configured_max_blocks: usize,
) -> CommonArticulationClearanceExtensionLimitsV1 {
    CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(configured_max_blocks)
        .expect("valid clearance extension cap")
}

fn raw_pair_candidate_count_v1(decomposition: &CanonicalMaterialEdgeBlockDecompositionV1) -> usize {
    let blocks = decomposition.blocks();
    (0..blocks.len())
        .flat_map(|first| (first + 1..blocks.len()).map(move |second| (first, second)))
        .map(|(first, second)| {
            blocks[first].geometry().face_ids().len() * blocks[second].geometry().face_ids().len()
        })
        .sum()
}

fn issue_extension_clearance_v1<'a>(
    fixture: &'a ExtensionClearanceFixtureV1,
    common_pose: &'a CommonArticulationPoseExtensionAuthorityV1,
    configured_max_blocks: usize,
) -> Box<CommonArticulationClearanceExtensionPrerequisiteV1> {
    let pose_limits = pose_extension_limits_v1(configured_max_blocks);
    let limits = clearance_extension_limits_v1(configured_max_blocks);
    match issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
        common_pose,
        pose_limits,
        &fixture.pairs,
        Some(fixture.positive.clone()),
        limits,
    ))
    .expect("extension clearance outcome")
    {
        CommonArticulationClearanceExtensionOutcomeV1::Certified(authority) => authority,
        CommonArticulationClearanceExtensionOutcomeV1::Unsupported(_) => {
            panic!("whole-parent extension certificate must close the prerequisite")
        }
    }
}

fn direct_extension_clearance_binding_v1(
    fixture: &ExtensionClearanceFixtureV1,
    authority: &CommonArticulationClearanceExtensionPrerequisiteV1,
    common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
    limits: CommonArticulationClearanceExtensionLimitsV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"common_articulation_cross_block_clearance_extension_prerequisite_v1");
    for value in [
        11_u64,
        limits.max_blocks as u64,
        fixture.decomposition.blocks().len() as u64,
    ] {
        hash.update(value.to_le_bytes());
    }
    hash.update(authority.common_pose_binding_fingerprint_v1());
    hash.update(authority.schedule_binding_fingerprint_v1());
    hash.update(authority.closure_binding_fingerprint_v1());
    hash.update(authority.paper_thickness_mm_v1().to_bits().to_be_bytes());
    for value in [
        common_pose_limits.max_blocks,
        common_pose_limits.max_faces,
        common_pose_limits.max_hinges,
        common_pose_limits.max_work,
        common_pose_limits.max_retained_bytes,
        CycleScheduleLimitsV1::default().max_hinges,
        CycleScheduleLimitsV1::default().max_degree,
        CycleScheduleLimitsV1::default().max_work,
        limits.max_blocks,
        limits.max_faces,
        limits.max_cross_block_pairs,
        limits.max_pair_candidates,
        limits.max_work,
        limits.max_storage_bytes,
    ] {
        hash.update((value as u64).to_be_bytes());
    }
    hash.update(
        CycleScheduleLimitsV1::default()
            .max_coefficient_bits
            .to_be_bytes(),
    );
    hash.update((authority.cross_block_pairs_v1().len() as u64).to_be_bytes());
    for pair in authority.cross_block_pairs_v1() {
        hash.update(pair.first().canonical_bytes());
        hash.update(pair.second().canonical_bytes());
    }
    hash.finalize().into()
}

#[test]
fn extension_domain_binds_minimum_configured_cap_and_actual_count_in_order_v1() {
    assert_eq!(COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1, 10);
    assert_eq!(COMMON_ARTICULATION_CLEARANCE_EXTENSION_MIN_BLOCKS_V1, 11);
    assert_eq!(COMMON_ARTICULATION_CLEARANCE_EXTENSION_MAX_BLOCKS_V1, 32);

    for (actual_count, current_cap, replay_cap) in [(11, 11, 12), (12, 12, 13)] {
        let fixture = prepare_extension_clearance_fixture_v1(actual_count);
        let current_pose = fixture.pose_authority_v1(current_cap);
        let replay_pose = fixture.pose_authority_v1(replay_cap);
        let current = issue_extension_clearance_v1(&fixture, &current_pose, current_cap);
        let replay = issue_extension_clearance_v1(&fixture, &replay_pose, replay_cap);

        assert_eq!(
            current.model_id(),
            COMMON_ARTICULATION_CLEARANCE_EXTENSION_PREREQUISITE_MODEL_ID_V1,
        );
        assert_eq!(current.configured_max_blocks_v1(), current_cap);
        assert_eq!(current.actual_block_count_v1(), actual_count);
        assert_eq!(replay.configured_max_blocks_v1(), replay_cap);
        assert_eq!(replay.actual_block_count_v1(), actual_count);
        assert_eq!(
            current.binding_fingerprint_v1(),
            direct_extension_clearance_binding_v1(
                &fixture,
                &current,
                pose_extension_limits_v1(current_cap),
                clearance_extension_limits_v1(current_cap),
            ),
        );
        assert_eq!(
            replay.binding_fingerprint_v1(),
            direct_extension_clearance_binding_v1(
                &fixture,
                &replay,
                pose_extension_limits_v1(replay_cap),
                clearance_extension_limits_v1(replay_cap),
            ),
        );
        assert_ne!(
            current.binding_fingerprint_v1(),
            replay.binding_fingerprint_v1(),
            "configured-cap replay must change the clearance binding",
        );
        current
            .revalidate_v1(fixture.revalidation_input(
                &current_pose,
                pose_extension_limits_v1(current_cap),
                clearance_extension_limits_v1(current_cap),
            ))
            .expect("current extension clearance revalidation");
        replay
            .revalidate_v1(fixture.revalidation_input(
                &replay_pose,
                pose_extension_limits_v1(replay_cap),
                clearance_extension_limits_v1(replay_cap),
            ))
            .expect("replay extension clearance revalidation");
        assert_eq!(
            current
                .revalidate_v1(fixture.revalidation_input(
                    &replay_pose,
                    pose_extension_limits_v1(replay_cap),
                    clearance_extension_limits_v1(replay_cap),
                ))
                .expect_err("foreign cap replay"),
            CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch,
        );
        assert!(!current.authorizes_continuous_motion());
        assert!(!current.authorizes_collision_clearance());
        assert!(!current.authorizes_project_mutation());
        assert!(!current.authorizes_apply());
        assert!(!current.authorizes_viewer());
    }
}

#[test]
fn extension_hard_thirty_two_boundary_is_inclusive_and_other_caps_fail_closed_v1() {
    for invalid in [0, 10, 33, usize::MAX] {
        assert!(
            CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(invalid).is_none()
        );
    }
    assert!(CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(11).is_some());
    assert!(CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(32).is_some());

    let thirty_two = prepare_extension_clearance_fixture_v1(32);
    let pose = thirty_two.pose_authority_v1(32);
    let authority = issue_extension_clearance_v1(&thirty_two, &pose, 32);
    assert_eq!(authority.actual_block_count_v1(), 32);
    assert_eq!(authority.configured_max_blocks_v1(), 32);
    authority
        .revalidate_v1(thirty_two.revalidation_input(
            &pose,
            pose_extension_limits_v1(32),
            clearance_extension_limits_v1(32),
        ))
        .expect("inclusive thirty-two-block clearance revalidation");

    let eleven = prepare_extension_clearance_fixture_v1(11);
    let eleven_pose = eleven.pose_authority_v1(32);
    issue_extension_clearance_v1(&eleven, &eleven_pose, 32);

    // An extension pose authority cannot be minted for ten blocks. Supplying
    // a foreign valid extension authority still reaches the explicit actual
    // cardinality gate before any issuer comparison.
    let ten = prepare_extension_clearance_fixture_v1(10);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(ten.input(
            &eleven_pose,
            pose_extension_limits_v1(32),
            &ten.pairs,
            Some(ten.positive.clone()),
            clearance_extension_limits_v1(32),
        ))
        .expect_err("ten blocks are below the extension minimum"),
        CommonArticulationClearanceErrorV1::ResourceLimit,
    );

    let valid_pose = eleven.pose_authority_v1(11);
    let baseline = clearance_extension_limits_v1(11);
    for invalid_cap in [0, 10, 33, usize::MAX] {
        let invalid = CommonArticulationClearanceExtensionLimitsV1 {
            max_blocks: invalid_cap,
            ..baseline
        };
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(eleven.input(
                &valid_pose,
                pose_extension_limits_v1(11),
                &eleven.pairs,
                Some(eleven.positive.clone()),
                invalid,
            ))
            .expect_err("invalid explicit extension cap"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }
}

#[test]
fn extension_exact_resource_envelope_passes_and_every_one_short_limit_fails_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let baseline = issue_extension_clearance_v1(&fixture, &pose, 11);
    let exact = CommonArticulationClearanceExtensionLimitsV1 {
        max_blocks: 11,
        max_faces: fixture.geometry.face_ids().len(),
        max_cross_block_pairs: fixture.pairs.len(),
        max_pair_candidates: raw_pair_candidate_count_v1(&fixture.decomposition),
        max_work: baseline.logical_work_v1(),
        max_storage_bytes: baseline.storage_bytes_upper_bound_v1(),
    };
    issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
        &pose,
        pose_extension_limits_v1(11),
        &fixture.pairs,
        Some(fixture.positive.clone()),
        exact,
    ))
    .expect("exact extension clearance resource envelope");

    let one_short = [
        CommonArticulationClearanceExtensionLimitsV1 {
            max_blocks: 10,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_faces: exact.max_faces - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_cross_block_pairs: exact.max_cross_block_pairs - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_pair_candidates: exact.max_pair_candidates - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_work: exact.max_work - 1,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_storage_bytes: exact.max_storage_bytes - 1,
            ..exact
        },
    ];
    for limits in one_short {
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
                &pose,
                pose_extension_limits_v1(11),
                &fixture.pairs,
                Some(fixture.positive.clone()),
                limits,
            ))
            .expect_err("one-short extension clearance resource"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }

    for overflow in [
        CommonArticulationClearanceExtensionLimitsV1 {
            max_faces: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_cross_block_pairs: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_pair_candidates: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_work: usize::MAX,
            ..exact
        },
        CommonArticulationClearanceExtensionLimitsV1 {
            max_storage_bytes: usize::MAX,
            ..exact
        },
    ] {
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
                &pose,
                pose_extension_limits_v1(11),
                &fixture.pairs,
                Some(fixture.positive.clone()),
                overflow,
            ))
            .expect_err("overflowing extension clearance limit"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }
    assert_eq!(
        sort_work_upper_bound_v1(usize::MAX).expect_err("checked sort-work overflow"),
        CommonArticulationClearanceErrorV1::ResourceLimit,
    );
}

#[test]
fn extension_pair_registry_pose_cap_and_whole_parent_provenance_fail_closed_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let pose_limits = pose_extension_limits_v1(11);
    let limits = clearance_extension_limits_v1(11);

    assert!(fixture.pairs.len() > 1);
    assert!(matches!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &fixture.pairs[..fixture.pairs.len() - 1],
            Some(fixture.positive.clone()),
            limits,
        )),
        Err(CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch { .. })
    ));

    let mut duplicate = fixture.pairs.clone();
    duplicate.push(fixture.pairs[0]);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &duplicate,
            Some(fixture.positive.clone()),
            limits,
        ))
        .expect_err("duplicate extension pair"),
        CommonArticulationClearanceErrorV1::DuplicateCrossBlockPair,
    );

    let mut extra = fixture.pairs.clone();
    extra.push(
        CommonArticulationCrossBlockFacePairV1::new(fixture.pairs[0].first(), FaceId::new())
            .expect("extra canonical pair"),
    );
    assert!(matches!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &extra,
            Some(fixture.positive.clone()),
            limits,
        )),
        Err(CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch { .. })
    ));

    let foreign = prepare_extension_clearance_fixture_v1(11);
    let foreign_pose = foreign.pose_authority_v1(11);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &foreign_pose,
            pose_limits,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            limits,
        ))
        .expect_err("foreign extension pose issuer"),
        CommonArticulationClearanceErrorV1::CommonPose(
            CommonArticulationPoseErrorV1::IssuerMismatch
        ),
    );
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose,
            pose_limits,
            &fixture.pairs,
            Some(foreign.positive.clone()),
            limits,
        ))
        .expect_err("foreign whole-parent extension certificate"),
        CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch,
    );

    let cap_twelve = clearance_extension_limits_v1(12);
    for (foreign_pose_limits, foreign_clearance_limits) in [
        (pose_extension_limits_v1(12), cap_twelve),
        (pose_limits, cap_twelve),
        (pose_extension_limits_v1(12), limits),
    ] {
        assert_eq!(
            issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
                &pose,
                foreign_pose_limits,
                &fixture.pairs,
                Some(fixture.positive.clone()),
                foreign_clearance_limits,
            ))
            .expect_err("inconsistent extension cap"),
            CommonArticulationClearanceErrorV1::ResourceLimit,
        );
    }
}

#[test]
fn extension_unsupported_gap_is_cap_bound_and_never_authorizes_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose_eleven = fixture.pose_authority_v1(11);
    let pose_twelve = fixture.pose_authority_v1(12);
    let outcome_eleven =
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose_eleven,
            pose_extension_limits_v1(11),
            &fixture.pairs,
            None,
            clearance_extension_limits_v1(11),
        ))
        .expect("cap-eleven unsupported outcome");
    let outcome_twelve =
        issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
            &pose_twelve,
            pose_extension_limits_v1(12),
            &fixture.pairs,
            None,
            clearance_extension_limits_v1(12),
        ))
        .expect("cap-twelve unsupported outcome");
    assert!(!outcome_eleven.is_certified());
    assert!(!outcome_twelve.is_certified());
    let gap_eleven = outcome_eleven.as_gap().expect("cap-eleven gap");
    let gap_twelve = outcome_twelve.as_gap().expect("cap-twelve gap");
    assert_eq!(
        gap_eleven.model_id(),
        COMMON_ARTICULATION_CLEARANCE_EXTENSION_GAP_MODEL_ID_V1,
    );
    assert_eq!(
        gap_eleven.reason(),
        CommonArticulationClearanceUnsupportedReasonV1::WholeParentOpenIntervalProofUnavailable,
    );
    assert_eq!(gap_eleven.configured_max_blocks_v1(), 11);
    assert_eq!(gap_twelve.configured_max_blocks_v1(), 12);
    assert_eq!(gap_eleven.actual_block_count_v1(), 11);
    assert_eq!(gap_twelve.actual_block_count_v1(), 11);
    assert_ne!(
        gap_eleven.common_pose_binding_fingerprint_v1(),
        gap_twelve.common_pose_binding_fingerprint_v1(),
        "the unsupported diagnostic must retain cap-bound pose provenance",
    );
    assert_eq!(gap_eleven.cross_block_pairs_v1(), fixture.pairs);
    assert!(!gap_eleven.endpoint_observations_are_authority_v1());
    assert!(!gap_eleven.sampled_poses_are_authority_v1());
    assert!(!gap_eleven.broad_phase_aabbs_are_authority_v1());
    assert!(!gap_eleven.per_block_certificates_are_cross_block_authority_v1());
    assert!(!gap_eleven.authorizes_continuous_motion());
    assert!(!gap_eleven.authorizes_collision_clearance());
    assert!(!gap_eleven.authorizes_project_mutation());
    assert!(!gap_eleven.authorizes_apply());
    assert!(!gap_eleven.authorizes_viewer());
}

#[test]
fn extension_revalidation_rejects_every_live_binding_and_retained_pair_drift_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let foreign = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let foreign_cap_pose = fixture.pose_authority_v1(12);
    let pose_limits = pose_extension_limits_v1(11);
    let limits = clearance_extension_limits_v1(11);
    let mut authority = issue_extension_clearance_v1(&fixture, &pose, 11);
    let baseline = fixture.revalidation_input(&pose, pose_limits, limits);
    authority
        .revalidate_v1(baseline)
        .expect("baseline extension clearance revalidation");

    let drifted = [
        (
            "geometry",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                geometry: &foreign.geometry,
                ..baseline
            },
        ),
        (
            "pose",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                pose: &foreign.pose,
                ..baseline
            },
        ),
        (
            "decomposition",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                decomposition: &foreign.decomposition,
                ..baseline
            },
        ),
        (
            "schedule",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                schedule: &foreign.schedule,
                ..baseline
            },
        ),
        (
            "closure",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                closure: &foreign.closure,
                ..baseline
            },
        ),
        (
            "thickness",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                paper_thickness_mm: f64::from_bits(fixture.paper_thickness_mm.to_bits() + 1),
                ..baseline
            },
        ),
        (
            "configured cap",
            CommonArticulationClearanceExtensionRevalidationInputV1 {
                common_pose: &foreign_cap_pose,
                common_pose_limits: pose_extension_limits_v1(12),
                limits: clearance_extension_limits_v1(12),
                ..baseline
            },
        ),
    ];
    for (label, input) in drifted {
        assert!(
            authority.revalidate_v1(input).is_err(),
            "{label} drift must fail closed",
        );
    }

    authority.cross_block_pairs.pop();
    assert!(
        authority.revalidate_v1(baseline).is_err(),
        "retained cross-block-pair drift must fail closed",
    );
}

#[test]
fn extension_issuance_and_revalidation_stop_at_entry_midpoint_and_final_v1() {
    let fixture = prepare_extension_clearance_fixture_v1(11);
    let pose = fixture.pose_authority_v1(11);
    let pose_limits = pose_extension_limits_v1(11);
    let limits = clearance_extension_limits_v1(11);

    let mut issuance_checkpoint_count = 0usize;
    issue_common_articulation_clearance_extension_prerequisite_with_checkpoint_v1(
        fixture.input(
            &pose,
            pose_limits,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            limits,
        ),
        &mut || {
            issuance_checkpoint_count += 1;
            Ok(())
        },
    )
    .expect("count extension clearance issuance checkpoints");
    assert!(issuance_checkpoint_count > 4);
    for stop_at in [1, issuance_checkpoint_count / 2, issuance_checkpoint_count] {
        for expected in [
            CommonArticulationClearanceErrorV1::Cancelled,
            CommonArticulationClearanceErrorV1::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                issue_common_articulation_clearance_extension_prerequisite_with_checkpoint_v1(
                    fixture.input(
                        &pose,
                        pose_limits,
                        &fixture.pairs,
                        Some(fixture.positive.clone()),
                        limits,
                    ),
                    &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    },
                )
                .expect_err("extension clearance issuance stop"),
                expected,
            );
        }
    }

    let authority = issue_extension_clearance_v1(&fixture, &pose, 11);
    let revalidation_input = fixture.revalidation_input(&pose, pose_limits, limits);
    let mut revalidation_checkpoint_count = 0usize;
    authority
        .revalidate_with_checkpoint_v1(revalidation_input, &mut || {
            revalidation_checkpoint_count += 1;
            Ok(())
        })
        .expect("count extension clearance revalidation checkpoints");
    assert!(revalidation_checkpoint_count > 4);
    for stop_at in [
        1,
        revalidation_checkpoint_count / 2,
        revalidation_checkpoint_count,
    ] {
        for expected in [
            CommonArticulationClearanceErrorV1::Cancelled,
            CommonArticulationClearanceErrorV1::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                authority
                    .revalidate_with_checkpoint_v1(revalidation_input, &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    })
                    .expect_err("extension clearance revalidation stop"),
                expected,
            );
        }
    }

    let cancelled = AtomicBool::new(true);
    let active = AtomicBool::new(false);
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_with_control_v1(
            fixture.input(
                &pose,
                pose_limits,
                &fixture.pairs,
                Some(fixture.positive.clone()),
                limits,
            ),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .expect_err("public extension clearance cancellation"),
        CommonArticulationClearanceErrorV1::Cancelled,
    );
    assert_eq!(
        issue_common_articulation_clearance_extension_prerequisite_with_control_v1(
            fixture.input(
                &pose,
                pose_limits,
                &fixture.pairs,
                Some(fixture.positive.clone()),
                limits,
            ),
            &CooperativeOperationControlV1::new(Some(&active), Instant::now()),
        )
        .expect_err("public extension clearance deadline"),
        CommonArticulationClearanceErrorV1::DeadlineExceeded,
    );
}
