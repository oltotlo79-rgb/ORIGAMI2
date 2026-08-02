use std::{
    sync::{Arc, OnceLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

use ori_core::{analyze_global_flat_foldability, analyze_local_flat_foldability};
use ori_domain::{EdgeId, FaceId};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityOutcome,
    GlobalFlatFoldabilityWorkCounts, GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    GlobalFlatLayerOrderCompactPairAssignmentInputV2,
    GlobalFlatLayerOrderCompactPairAssignmentLimitsV2, GlobalFlatLayerOrderRevalidationLimitsV2,
    LayerOrderSnapshot, global_flat_layer_order_compact_pair_assignment_sha256_v2,
    issue_global_flat_layer_order_from_compact_pair_assignment_v2,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
};
use sha2::{Digest, Sha256};

use super::*;
use crate::common_articulation_extension_test_support::{
    ExtensionClearanceFixtureV1, clearance_extension_limits_v1,
    issue_extension_clearance_with_positive_v1, pose_extension_limits_v1,
    prepare_extension_miura_clearance_fixture_in_namespace_v2,
};
use crate::{
    AdmittedMultiBlockPositiveLayerInputV2, AdmittedPositiveThicknessCycleSchedulePathInputV2,
    BlockUnionCompletenessInputV1, BoundedMultiBlockExtensionLimitsV1,
    CommonArticulationBlockComposedPathExtensionInputV1,
    CommonArticulationGeneralCellTransportExtensionInputV1,
    CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
    CompleteMultiBlockPositiveLayerInputV2, CompleteMultiBlockPositiveLayerLimitsV2,
    CooperativeOperationControlV1, GeneralCellTransportErrorV1, GeneralCellTransportLimitsV1,
    MultiBlockClosureInputV1, admit_common_articulation_positive_thickness_parent_graph_v2,
    certify_admitted_general_multi_face_cell_transport_v2,
    certify_admitted_positive_thickness_cycle_schedule_path_v2,
    certify_common_articulation_general_multi_face_cell_transport_extension_v1,
    diagnose_bounded_multi_block_extension_union_completeness_v1,
    issue_bounded_multi_block_extension_closure_authority_v1,
    issue_common_articulation_block_composed_path_extension_authority_v1,
    issue_complete_multi_block_positive_layer_authority_v2,
};

const ISSUER_CONTEXT_V2: [u8; 32] = [0x71; 32];
const ARTICULATION_LAYER_FINGERPRINT_V2: [u8; 32] = [0x72; 32];

fn transport_limits_v1(
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
) -> GeneralCellTransportLimitsV1 {
    GeneralCellTransportLimitsV1 {
        max_transitions: closure.leaves().len() + 1,
        max_cells: 1_000_000,
        max_layer_records: 1_000_000,
        max_boundary_samples: 1_000_000,
    }
}

fn restrict_layer_source_v1(source: &LayerOrderSnapshot, faces: &[FaceId]) -> LayerOrderSnapshot {
    let retained_bytes = source
        .checked_restricted_deep_retained_bytes_v1(faces)
        .expect("bounded final-extension block source size");
    source
        .try_restrict_to_faces_with_retained_byte_limit_v1(faces, retained_bytes)
        .expect("fallible final-extension block source restriction")
}

struct FinalExtensionFixtureV2 {
    base: ExtensionClearanceFixtureV1,
    source: Box<LayerOrderSnapshot>,
    global_work_counts: GlobalFlatFoldabilityWorkCounts,
    block_schedules: Vec<(
        CanonicalCycleScheduleV1,
        DyadicMaterialHingeIntervalClosureCertificateV1,
    )>,
    canonical_block_keys: Vec<[u8; 16]>,
    block_sources: Vec<LayerOrderSnapshot>,
    target_angles: Vec<(EdgeId, f64)>,
    compact_source_authority: Option<GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2>,
}

impl FinalExtensionFixtureV2 {
    fn staged_v1(
        &self,
        configured_max_blocks: usize,
    ) -> CommonArticulationBlockComposedPathExtensionAuthorityV1 {
        let common_pose = self.base.pose_authority_v1(configured_max_blocks);
        let clearance = issue_extension_clearance_with_positive_v1(
            &self.base,
            &common_pose,
            configured_max_blocks,
            self.base.positive_extension_v2(configured_max_blocks),
        );
        issue_common_articulation_block_composed_path_extension_authority_v1(
            CommonArticulationBlockComposedPathExtensionInputV1 {
                geometry: &self.base.geometry,
                audit: &self.base.audit,
                pose: &self.base.pose,
                decomposition: &self.base.decomposition,
                common_pose,
                common_pose_limits: pose_extension_limits_v1(configured_max_blocks),
                schedule: &self.base.schedule,
                schedule_limits: self.base.schedule_limits,
                closure: &self.base.closure,
                paper_thickness_mm: self.base.paper_thickness_mm,
                clearance: *clearance,
                clearance_limits: clearance_extension_limits_v1(configured_max_blocks),
                blocks: self.base.canonical_edge_partition_v1(),
            },
        )
        .expect("final-extension staged prerequisite")
    }

    fn whole_parent_layer_v1(
        &self,
        configured_max_blocks: usize,
    ) -> CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1 {
        let positive = self.base.positive_extension_v2(configured_max_blocks);
        certify_common_articulation_general_multi_face_cell_transport_extension_v1(
            CommonArticulationGeneralCellTransportExtensionInputV1 {
                geometry: &self.base.geometry,
                audit: &self.base.audit,
                decomposition: &self.base.decomposition,
                configured_max_blocks,
                source: self.source.as_ref(),
                schedule: &self.base.schedule,
                closure: &self.base.closure,
                positive_continuous: &positive,
                positive_graph_limits:
                    crate::CommonArticulationPositiveThicknessGraphExtensionLimitsV1::fixed_v1(),
                paper_thickness_mm: self.base.paper_thickness_mm,
                limits: transport_limits_v1(&self.base.closure),
            },
        )
        .expect("final-extension whole-parent layer proof")
    }

    fn complete_v2(
        &self,
        configured_max_blocks: usize,
    ) -> CompleteMultiBlockPositiveLayerAuthorityV2 {
        self.try_complete_v2(configured_max_blocks)
            .expect("final-extension complete admitted V2 positive-layer authority")
    }

    fn try_complete_v2(
        &self,
        configured_max_blocks: usize,
    ) -> Result<CompleteMultiBlockPositiveLayerAuthorityV2, CompleteMultiBlockPositiveLayerErrorV2>
    {
        let limits = BoundedMultiBlockExtensionLimitsV1 {
            max_blocks: configured_max_blocks,
        };
        let closure_parent = issue_bounded_multi_block_extension_closure_authority_v1(
            self.base
                .decomposition
                .blocks()
                .iter()
                .zip(&self.block_schedules)
                .map(|(block, (schedule, closure))| MultiBlockClosureInputV1 {
                    geometry: block.geometry(),
                    audit: block.audit(),
                    schedule,
                    closure,
                })
                .collect(),
            self.base.paper_thickness_mm,
            ISSUER_CONTEXT_V2,
            limits,
        )
        .expect("final-extension bounded closure parent");
        let proofs = self
            .base
            .decomposition
            .blocks()
            .iter()
            .zip(&self.block_schedules)
            .map(|(block, (schedule, closure))| {
                let key = block
                    .geometry()
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge().canonical_bytes())
                    .min()
                    .expect("non-empty final-extension block");
                let source = &self.block_sources[self
                    .canonical_block_keys
                    .binary_search(&key)
                    .expect("canonical final-extension block source")];
                let block_parent_graph_admission = Arc::new(
                    admit_common_articulation_positive_thickness_parent_graph_v2(
                        block.geometry(),
                        crate::CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
                    )
                    .expect("exact final-extension block parent-graph admission"),
                );
                let positive = certify_admitted_positive_thickness_cycle_schedule_path_v2(
                    AdmittedPositiveThicknessCycleSchedulePathInputV2 {
                        geometry: block.geometry(),
                        audit: block.audit(),
                        fixed_face: closure.fixed_face(),
                        schedule,
                        closure,
                        paper_thickness_mm: self.base.paper_thickness_mm,
                        interval_count: 16,
                        graph_limits:
                            crate::CommonArticulationPositiveThicknessGraphExtensionLimitsV1::fixed_v1(),
                        parent_graph_admission: block_parent_graph_admission,
                    },
                )
                .expect("final-extension admitted block positive path");
                let layer = certify_admitted_general_multi_face_cell_transport_v2(
                    crate::AdmittedGeneralCellTransportInputV2 {
                        geometry: block.geometry(),
                        audit: block.audit(),
                        source,
                        schedule,
                        closure,
                        positive_continuous: &positive,
                        paper_thickness_mm: self.base.paper_thickness_mm,
                        limits: transport_limits_v1(closure),
                    },
                )
                .expect("final-extension admitted block layer proof");
                (positive, layer)
            })
            .collect::<Vec<_>>();
        let admitted_blocks = self
            .base
            .decomposition
            .blocks()
            .iter()
            .zip(proofs)
            .map(|(block, (positive, layer))| {
                let key = block
                    .geometry()
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge().canonical_bytes())
                    .min()
                    .expect("non-empty final-extension block");
                let source = &self.block_sources[self
                    .canonical_block_keys
                    .binary_search(&key)
                    .expect("canonical final-extension block source")];
                AdmittedMultiBlockPositiveLayerInputV2 {
                    geometry: block.geometry(),
                    source,
                    positive,
                    layer,
                }
            })
            .collect();
        let block_hinges = self
            .base
            .decomposition
            .blocks()
            .iter()
            .map(|block| {
                block
                    .geometry()
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let union_inputs = self
            .base
            .decomposition
            .blocks()
            .iter()
            .zip(&block_hinges)
            .map(|(block, hinges)| BlockUnionCompletenessInputV1 {
                faces: block.geometry().face_ids(),
                hinges,
            })
            .collect::<Vec<_>>();
        let report = diagnose_bounded_multi_block_extension_union_completeness_v1(
            &self.base.geometry,
            &union_inputs,
            limits,
        )
        .expect("final-extension exact live union report");
        let block_sources = self.block_source_refs_v2();
        issue_complete_multi_block_positive_layer_authority_v2(
            CompleteMultiBlockPositiveLayerInputV2 {
                geometry: &self.base.geometry,
                audit: &self.base.audit,
                decomposition: &self.base.decomposition,
                configured_max_blocks,
                report,
                closure_parent,
                blocks: admitted_blocks,
                source: self.source.as_ref(),
                block_sources: &block_sources,
                paper_thickness_mm: self.base.paper_thickness_mm,
                issuer_context: ISSUER_CONTEXT_V2,
                articulation_layer_fingerprint: ARTICULATION_LAYER_FINGERPRINT_V2,
                target_angles: &self.target_angles,
                whole_parent_schedule: &self.base.schedule,
                whole_parent_closure: &self.base.closure,
                whole_parent_positive: self.base.positive_extension_v2(configured_max_blocks),
                positive_graph_limits:
                    crate::CommonArticulationPositiveThicknessGraphExtensionLimitsV1::fixed_v1(),
                parent_graph_admission: self.base.parent_graph_admission.as_ref(),
                limits: CompleteMultiBlockPositiveLayerLimitsV2::default(),
            },
        )
    }

    fn block_source_refs_v2(&self) -> Vec<&LayerOrderSnapshot> {
        self.block_sources.iter().collect()
    }

    fn input_with_sources_v2<'a>(
        &'a self,
        configured_max_blocks: usize,
        staged: CommonArticulationBlockComposedPathExtensionAuthorityV1,
        complete: CompleteMultiBlockPositiveLayerAuthorityV2,
        whole_parent_layer: CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
        block_sources: &'a [&'a LayerOrderSnapshot],
    ) -> CommonArticulationContinuousLayerPathExtensionInputV2<'a> {
        CommonArticulationContinuousLayerPathExtensionInputV2 {
            geometry: &self.base.geometry,
            audit: &self.base.audit,
            pose: &self.base.pose,
            decomposition: &self.base.decomposition,
            staged,
            common_pose_limits: pose_extension_limits_v1(configured_max_blocks),
            schedule: &self.base.schedule,
            schedule_limits: self.base.schedule_limits,
            closure: &self.base.closure,
            paper_thickness_mm: self.base.paper_thickness_mm,
            clearance_limits: clearance_extension_limits_v1(configured_max_blocks),
            complete,
            block_sources,
            issuer_context: ISSUER_CONTEXT_V2,
            articulation_layer_fingerprint: ARTICULATION_LAYER_FINGERPRINT_V2,
            target_angles: &self.target_angles,
            source: self.source.as_ref(),
            whole_parent_layer,
            parent_graph_admission: Arc::clone(&self.base.parent_graph_admission),
        }
    }

    fn revalidation_input_v2<'a>(
        &'a self,
        configured_max_blocks: usize,
        block_sources: &'a [&'a LayerOrderSnapshot],
    ) -> CommonArticulationContinuousLayerPathExtensionRevalidationInputV2<'a> {
        CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
            geometry: &self.base.geometry,
            audit: &self.base.audit,
            pose: &self.base.pose,
            decomposition: &self.base.decomposition,
            common_pose_limits: pose_extension_limits_v1(configured_max_blocks),
            schedule: &self.base.schedule,
            schedule_limits: self.base.schedule_limits,
            closure: &self.base.closure,
            paper_thickness_mm: self.base.paper_thickness_mm,
            clearance_limits: clearance_extension_limits_v1(configured_max_blocks),
            block_sources,
            issuer_context: ISSUER_CONTEXT_V2,
            articulation_layer_fingerprint: ARTICULATION_LAYER_FINGERPRINT_V2,
            target_angles: &self.target_angles,
            source: self.source.as_ref(),
            parent_graph_admission: self.base.parent_graph_admission.as_ref(),
        }
    }
}

fn prepare_final_extension_fixture_v2(block_count: usize) -> FinalExtensionFixtureV2 {
    prepare_final_extension_fixture_in_namespace_v2(
        block_count,
        crate::miura_cactus_test_support::canonical_general_n_miura_namespace_v2(),
    )
}

fn prepare_final_extension_fixture_in_namespace_v2(
    block_count: usize,
    namespace: ori_domain::ProjectId,
) -> FinalExtensionFixtureV2 {
    let base = prepare_extension_miura_clearance_fixture_in_namespace_v2(block_count, namespace);
    let local = analyze_local_flat_foldability(&base.paper, &base.pattern);
    let (source, global_work_counts, compact_source_authority) = if block_count
        == COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2
    {
        use crate::n32_compact_pair_assignment_test_support::{
            N32_COMPACT_ASSIGNMENT_BYTES_V2, N32_COMPACT_VARIABLE_COUNT_V2,
            N32_DIRECTION_ASSIGNMENT_SHA256_HEX_V2, decode_lower_hex_v2,
            n32_compact_pair_assignment_v2,
        };

        let (variable_count, variable_registry_sha256, direction_bits_le) =
            n32_compact_pair_assignment_v2();
        assert_eq!(variable_count, N32_COMPACT_VARIABLE_COUNT_V2);
        assert_eq!(direction_bits_le.len(), N32_COMPACT_ASSIGNMENT_BYTES_V2);
        let expected_assignment_sha256: [u8; 32] =
            decode_lower_hex_v2(N32_DIRECTION_ASSIGNMENT_SHA256_HEX_V2)
                .try_into()
                .expect("N=32 assignment digest is exactly 32 bytes");
        assert_eq!(
            global_flat_layer_order_compact_pair_assignment_sha256_v2(
                variable_count,
                variable_registry_sha256,
                &direction_bits_le,
            ),
            Some(expected_assignment_sha256),
        );

        let limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
            analysis: GlobalFlatFoldabilityLimits {
                max_search_nodes: 0,
                ..GlobalFlatFoldabilityLimits::default()
            },
            ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
        };
        let authority = issue_global_flat_layer_order_from_compact_pair_assignment_v2(
            GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
                source: GlobalFlatFoldabilityInput::current_with_geometry(
                    base.namespace,
                    &base.paper,
                    &base.pattern,
                    &base.topology,
                    &local,
                ),
                variable_count,
                variable_registry_sha256,
                direction_bits_le: &direction_bits_le,
            },
            limits,
        )
        .expect("sealed N=32 compact assignment verifies against the live source without search");
        assert_eq!(authority.variable_count_v2(), variable_count);
        assert_eq!(
            authority.variable_registry_sha256_v2(),
            variable_registry_sha256
        );
        assert_eq!(
            authority.direction_assignment_sha256_v2(),
            expected_assignment_sha256
        );
        assert_eq!(authority.work_counts_v2().search_nodes, 0);
        assert_eq!(
            authority.resources_v2().compact_assignment_bytes,
            N32_COMPACT_ASSIGNMENT_BYTES_V2
        );
        assert_eq!(
            authority.layer_order_snapshot_v2().material_faces.len(),
            257
        );

        let resources = authority.resources_v2();
        let live = authority
            .revalidate_live_source_v2(
                GlobalFlatFoldabilityInput::current_with_geometry(
                    base.namespace,
                    &base.paper,
                    &base.pattern,
                    &base.topology,
                    &local,
                ),
                GlobalFlatLayerOrderRevalidationLimitsV2 {
                    analysis: limits.analysis,
                    max_source_retained_bytes: resources.layer_order_retained_bytes,
                    max_peak_bytes: limits.max_peak_bytes,
                },
            )
            .expect("sealed N=32 compact source passes independent live no-search revalidation");
        assert!(live.is_current_v2());

        let source_retained_bytes = authority
            .layer_order_snapshot_v2()
            .checked_deep_retained_bytes_v1()
            .expect("bounded N=32 compact source retained bytes");
        let source = Box::new(
            authority
                .layer_order_snapshot_v2()
                .try_clone_with_retained_byte_limit_v1(source_retained_bytes)
                .expect("fallible N=32 compact source clone"),
        );
        (source, authority.work_counts_v2(), Some(authority))
    } else {
        let max_search_nodes = base
            .geometry
            .face_ids()
            .len()
            .checked_add(block_count)
            .expect("bounded deterministic final-extension search cap");
        let global_report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                base.namespace,
                &base.paper,
                &base.pattern,
                &base.topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits {
                max_search_nodes,
                ..GlobalFlatFoldabilityLimits::default()
            },
        )
        .expect("final-extension global flat-foldability analysis");
        let global_work_counts = global_report.work_counts_v2();
        let source = match global_report.into_outcome_v2() {
            GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } => layer_order,
            other => panic!("final-extension source is unavailable: {other:?}"),
        };
        (source, global_work_counts, None)
    };
    let block_schedules = base
        .decomposition
        .blocks()
        .iter()
        .map(|block| {
            let fixed_face = block
                .geometry()
                .face_ids()
                .iter()
                .copied()
                .find(|face| base.decomposition.articulation_faces().contains(face))
                .expect("final-extension block articulation face");
            let schedule = base
                .schedule
                .restrict_to_edge_block_with_fixed_face_v1(
                    &base.geometry,
                    &base.audit,
                    block.geometry(),
                    block.audit(),
                    fixed_face,
                )
                .expect("exact final-extension schedule restriction");
            let closure = block
                .geometry()
                .prove_dyadic_schedule_closure_v1(
                    block.audit(),
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
                .expect("final-extension restricted closure");
            (schedule, closure)
        })
        .collect::<Vec<_>>();
    let mut source_records = base
        .decomposition
        .blocks()
        .iter()
        .map(|block| {
            (
                block
                    .geometry()
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge().canonical_bytes())
                    .min()
                    .expect("non-empty final-extension block"),
                restrict_layer_source_v1(source.as_ref(), block.geometry().face_ids()),
            )
        })
        .collect::<Vec<_>>();
    source_records.sort_unstable_by_key(|(edge, _)| *edge);
    let canonical_block_keys = source_records.iter().map(|(key, _)| *key).collect();
    let block_sources = source_records
        .into_iter()
        .map(|(_, source)| source)
        .collect();
    let target_angles = block_schedules
        .iter()
        .flat_map(|(schedule, _)| {
            schedule
                .evaluate(1.0)
                .expect("final-extension target angles")
                .as_slice()
                .iter()
                .map(|angle| (angle.edge(), angle.angle_degrees()))
                .collect::<Vec<_>>()
        })
        .collect();
    FinalExtensionFixtureV2 {
        base,
        source,
        global_work_counts,
        block_schedules,
        canonical_block_keys,
        block_sources,
        target_angles,
        compact_source_authority,
    }
}

fn final_extension_thirty_two_fixture_v2() -> &'static FinalExtensionFixtureV2 {
    static FIXTURE: OnceLock<FinalExtensionFixtureV2> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        prepare_final_extension_fixture_v2(
            COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2,
        )
    })
}

fn issue_final_extension_v2(
    fixture: &FinalExtensionFixtureV2,
    configured_max_blocks: usize,
) -> CommonArticulationContinuousLayerPathExtensionAuthorityV2 {
    let complete = fixture.complete_v2(configured_max_blocks);
    issue_final_extension_from_complete_v2(fixture, configured_max_blocks, complete)
}

fn issue_final_extension_from_complete_v2(
    fixture: &FinalExtensionFixtureV2,
    configured_max_blocks: usize,
    complete: CompleteMultiBlockPositiveLayerAuthorityV2,
) -> CommonArticulationContinuousLayerPathExtensionAuthorityV2 {
    let block_sources = fixture.block_source_refs_v2();
    let staged = fixture.staged_v1(configured_max_blocks);
    let whole_parent_layer = fixture.whole_parent_layer_v1(configured_max_blocks);
    issue_common_articulation_continuous_layer_path_extension_authority_v2(
        fixture.input_with_sources_v2(
            configured_max_blocks,
            staged,
            complete,
            whole_parent_layer,
            &block_sources,
        ),
    )
    .expect("final-layer extension authority")
}

fn assert_n32_complete_resource_envelope_v2(complete: &CompleteMultiBlockPositiveLayerAuthorityV2) {
    let exact = complete.exact_resource_limits_v2();
    let resources = complete.resources_v2();
    assert_eq!(
        exact.max_blocks,
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2,
    );
    assert_eq!(resources.block_count_v2(), exact.max_blocks);
    assert_eq!(exact.max_faces, resources.face_count_v2());
    assert_eq!(exact.max_hinges, resources.hinge_count_v2());
    assert_eq!(exact.max_face_pair_tests, resources.face_pair_tests_v2());
    assert_eq!(exact.max_logical_work, resources.logical_work_v2());
    assert_eq!(
        exact.max_deep_retained_bytes,
        resources.deep_retained_bytes_v2(),
    );
    complete
        .revalidate_resource_limits_v2(exact)
        .expect("N=32 complete V2 accepts its exact retained resource envelope");

    let one_short = |value: usize| {
        value
            .checked_sub(1)
            .expect("every live N=32 complete resource is positive")
    };
    for limits in [
        CompleteMultiBlockPositiveLayerLimitsV2 {
            max_blocks: one_short(exact.max_blocks),
            ..exact
        },
        CompleteMultiBlockPositiveLayerLimitsV2 {
            max_faces: one_short(exact.max_faces),
            ..exact
        },
        CompleteMultiBlockPositiveLayerLimitsV2 {
            max_hinges: one_short(exact.max_hinges),
            ..exact
        },
        CompleteMultiBlockPositiveLayerLimitsV2 {
            max_face_pair_tests: one_short(exact.max_face_pair_tests),
            ..exact
        },
        CompleteMultiBlockPositiveLayerLimitsV2 {
            max_logical_work: one_short(exact.max_logical_work),
            ..exact
        },
        CompleteMultiBlockPositiveLayerLimitsV2 {
            max_deep_retained_bytes: one_short(exact.max_deep_retained_bytes),
            ..exact
        },
    ] {
        assert_eq!(
            complete.revalidate_resource_limits_v2(limits),
            Err(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit),
        );
    }
    assert!(!complete.authorizes_continuous_motion());
    assert!(!complete.authorizes_collision_clearance());
    assert!(!complete.authorizes_layer_transport());
    assert!(!complete.authorizes_project_mutation());
    assert!(!complete.authorizes_apply());
    assert!(!complete.authorizes_viewer());
}

fn direct_final_extension_binding_v2(
    fixture: &FinalExtensionFixtureV2,
    authority: &CommonArticulationContinuousLayerPathExtensionAuthorityV2,
) -> [u8; 32] {
    let mut target_angles = fixture.target_angles.clone();
    target_angles.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MODEL_ID_V2.as_bytes());
    for value in [
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MIN_BLOCKS_V2,
        authority.configured_max_blocks,
        authority.actual_block_count,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    hash.update(fixture.base.schedule.graph_binding_fingerprint_v1());
    hash.update(fixture.base.schedule.certificate_binding_fingerprint_v2());
    hash.update(fixture.base.closure.partition_binding_fingerprint_v2());
    hash.update(fixture.base.paper_thickness_mm.to_bits().to_le_bytes());
    hash.update(authority.staged.binding_fingerprint_v1());
    hash.update(authority.complete.binding_fingerprint_v2());
    hash.update(authority.whole_parent_layer.model_id().as_bytes());
    hash.update(authority.whole_parent_layer.binding_fingerprint_v1());
    hash.update(
        authority
            .whole_parent_layer
            .paper_thickness_mm_v1()
            .to_bits()
            .to_le_bytes(),
    );
    hash.update(authority.whole_parent_layer.target_order_hash_v1());
    hash.update((authority.whole_parent_layer.transition_hashes_v1().len() as u64).to_le_bytes());
    for transition in authority.whole_parent_layer.transition_hashes_v1() {
        hash.update(transition);
    }
    hash.update((authority.whole_parent_layer.pair_order_count_v1() as u64).to_le_bytes());
    hash.update(ISSUER_CONTEXT_V2);
    hash.update(ARTICULATION_LAYER_FINGERPRINT_V2);
    let admission = fixture.base.parent_graph_admission.as_ref();
    hash.update(admission.model_id_v2().as_bytes());
    hash.update(admission.identity_namespace_v2().canonical_bytes());
    hash.update(admission.source_revision_v2().to_le_bytes());
    hash.update(admission.fold_model_fingerprint_v2());
    hash.update(admission.semantic_graph_digest_v2());
    hash.update(admission.binding_fingerprint_v2());
    let admission_limits = admission.limits_v2();
    for value in [
        admission_limits.max_faces,
        admission_limits.max_hinges,
        admission_limits.max_boundary_vertex_occurrences,
        admission_limits.max_vertices,
        admission_limits.max_edges,
        admission_limits.max_vertex_pairs,
        admission_limits.max_vertex_edge_tests,
        admission_limits.max_edge_pair_tests,
        admission_limits.max_face_pair_tests,
        admission_limits.max_point_in_polygon_edge_tests,
        admission_limits.max_exact_operations,
        admission_limits.max_logical_work,
        admission_limits.max_workspace_bytes,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    let admission_resources = admission.resources_v2();
    for value in [
        admission_resources.face_count_v2(),
        admission_resources.hinge_count_v2(),
        admission_resources.boundary_vertex_occurrences_v2(),
        admission_resources.vertex_count_v2(),
        admission_resources.edge_count_v2(),
        admission_resources.vertex_pair_tests_v2(),
        admission_resources.vertex_edge_tests_v2(),
        admission_resources.edge_pair_tests_v2(),
        admission_resources.face_pair_tests_v2(),
        admission_resources.point_in_polygon_edge_tests_v2(),
        admission_resources.exact_operations_v2(),
        admission_resources.logical_work_v2(),
        admission_resources.workspace_bytes_upper_bound_v2(),
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    hash.update((authority.blocks.len() as u64).to_le_bytes());
    for block in &authority.blocks {
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            hash.update(edge.canonical_bytes());
        }
        hash.update((block.faces.len() as u64).to_le_bytes());
        for face in &block.faces {
            hash.update(face.canonical_bytes());
        }
    }
    hash.update((target_angles.len() as u64).to_le_bytes());
    for (edge, angle) in target_angles {
        hash.update(edge.canonical_bytes());
        hash.update(angle.to_bits().to_le_bytes());
    }
    hash.finalize().into()
}

fn assert_final_extension_success_v2(
    fixture: &FinalExtensionFixtureV2,
    actual_count: usize,
    configured_max_blocks: usize,
) {
    let expected_face_count = actual_count
        .checked_mul(8)
        .and_then(|count| count.checked_add(1))
        .expect("bounded final-extension face count");
    let minimum_transitivity_constraints = expected_face_count
        .checked_mul(expected_face_count - 1)
        .and_then(|count| count.checked_mul(expected_face_count - 2))
        .map(|count| count / 6)
        .expect("bounded final-extension transitivity count");
    assert_eq!(fixture.source.material_faces.len(), expected_face_count);
    assert_eq!(fixture.global_work_counts.face_records, expected_face_count);
    assert!(
        fixture.global_work_counts.constraints >= minimum_transitivity_constraints,
        "the native source analysis must exercise the large compact family",
    );
    if actual_count == COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2 {
        use crate::n32_compact_pair_assignment_test_support::{
            N32_COMPACT_ASSIGNMENT_BYTES_V2, N32_COMPACT_VARIABLE_COUNT_V2,
            N32_DIRECTION_ASSIGNMENT_SHA256_HEX_V2, N32_PAIR_REGISTRY_SHA256_HEX_V2,
            decode_lower_hex_v2, n32_compact_pair_assignment_v2,
        };

        let compact = fixture
            .compact_source_authority
            .as_ref()
            .expect("N=32 final extension retains its production compact-source authority");
        let (variable_count, registry_sha256, direction_bits_le) = n32_compact_pair_assignment_v2();
        let expected_registry_sha256: [u8; 32] =
            decode_lower_hex_v2(N32_PAIR_REGISTRY_SHA256_HEX_V2)
                .try_into()
                .expect("N=32 registry digest is exactly 32 bytes");
        let expected_assignment_sha256: [u8; 32] =
            decode_lower_hex_v2(N32_DIRECTION_ASSIGNMENT_SHA256_HEX_V2)
                .try_into()
                .expect("N=32 assignment digest is exactly 32 bytes");
        assert_eq!(variable_count, N32_COMPACT_VARIABLE_COUNT_V2);
        assert_eq!(registry_sha256, expected_registry_sha256);
        assert_eq!(direction_bits_le.len(), N32_COMPACT_ASSIGNMENT_BYTES_V2);
        assert_eq!(compact.variable_count_v2(), variable_count);
        assert_eq!(compact.variable_registry_sha256_v2(), registry_sha256);
        assert_eq!(
            compact.direction_assignment_sha256_v2(),
            expected_assignment_sha256
        );
        assert_eq!(
            global_flat_layer_order_compact_pair_assignment_sha256_v2(
                variable_count,
                registry_sha256,
                &direction_bits_le,
            ),
            Some(expected_assignment_sha256),
        );
        assert_eq!(compact.work_counts_v2(), fixture.global_work_counts);
        assert_eq!(fixture.global_work_counts.search_nodes, 0);
        assert_eq!(
            compact.resources_v2().compact_assignment_bytes,
            N32_COMPACT_ASSIGNMENT_BYTES_V2
        );
    } else {
        assert!(fixture.compact_source_authority.is_none());
        let maximum_compact_search_nodes = expected_face_count
            .checked_add(actual_count)
            .expect("bounded final-extension compact-search regression cap");
        assert!(
            (1..=maximum_compact_search_nodes).contains(&fixture.global_work_counts.search_nodes),
            "the rejection-triggered compact search must remain within the fixture's \
             face-plus-block decision cap",
        );
    }

    let complete = fixture.complete_v2(configured_max_blocks);
    if actual_count == COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2 {
        assert_n32_complete_resource_envelope_v2(&complete);
    }
    let authority =
        issue_final_extension_from_complete_v2(fixture, configured_max_blocks, complete);
    let block_sources = fixture.block_source_refs_v2();
    assert_eq!(
        authority.model_id(),
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MODEL_ID_V2,
    );
    assert_eq!(authority.configured_max_blocks_v2(), configured_max_blocks);
    assert_eq!(authority.actual_block_count_v2(), actual_count);
    assert_eq!(authority.block_count_v2(), actual_count);
    assert_eq!(
        authority.binding_fingerprint_v2(),
        direct_final_extension_binding_v2(fixture, &authority),
    );
    authority
        .revalidate_v2(fixture.revalidation_input_v2(configured_max_blocks, &block_sources))
        .expect("final-extension exact live revalidation");
    if actual_count == COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2 {
        assert_eq!(
            fixture.base.schedule_limits.max_hinges,
            fixture.base.geometry.hinges().len()
        );
        let mut one_short = fixture.revalidation_input_v2(configured_max_blocks, &block_sources);
        one_short.schedule_limits.max_hinges -= 1;
        assert!(matches!(
            authority.revalidate_v2(one_short),
            Err(CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit)
                | Err(CommonArticulationContinuousLayerPathExtensionErrorV2::Staged(_))
        ));
    }
    assert!(!authority.authorizes_continuous_motion());
    assert!(!authority.authorizes_collision_clearance());
    assert!(!authority.authorizes_layer_transport());
    assert!(!authority.authorizes_project_mutation());
    assert!(!authority.authorizes_apply());
    assert!(!authority.authorizes_viewer());
}

#[test]
fn final_extension_issues_eleven_with_independent_oracle_v2() {
    let fixture = prepare_final_extension_fixture_v2(11);
    assert_final_extension_success_v2(&fixture, 11, 11);
}

#[test]
fn final_extension_issues_twelve_with_independent_oracle_v2() {
    let fixture = prepare_final_extension_fixture_v2(12);
    assert_final_extension_success_v2(&fixture, 12, 16);
}

#[test]
fn final_extension_issues_thirty_two_with_independent_oracle_v2() {
    assert_final_extension_success_v2(final_extension_thirty_two_fixture_v2(), 32, 32);
}

#[test]
fn final_extension_cap_replay_and_cross_cap_prerequisites_fail_closed_v2() {
    let fixture = prepare_final_extension_fixture_v2(11);
    let current = issue_final_extension_v2(&fixture, 11);
    let replay = issue_final_extension_v2(&fixture, 12);
    assert_ne!(
        current.binding_fingerprint_v2(),
        replay.binding_fingerprint_v2(),
        "configured cap is a bound u64-LE field",
    );
    let block_sources = fixture.block_source_refs_v2();
    assert_eq!(
        current
            .revalidate_v2(fixture.revalidation_input_v2(12, &block_sources))
            .expect_err("cross-cap final-extension replay"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit,
    );

    let staged_eleven = fixture.staged_v1(11);
    let complete_twelve = fixture.complete_v2(12);
    assert_eq!(
        issue_common_articulation_continuous_layer_path_extension_authority_v2(
            fixture.input_with_sources_v2(
                11,
                staged_eleven,
                complete_twelve,
                fixture.whole_parent_layer_v1(11),
                &block_sources,
            ),
        )
        .expect_err("cross-cap complete prerequisite"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit,
    );

    let staged = fixture.staged_v1(11);
    let mut corrupt_scoped_complete = fixture.complete_v2(11);
    corrupt_scoped_complete.corrupt_configured_max_for_test_v2();
    assert_eq!(
        issue_common_articulation_continuous_layer_path_extension_authority_v2(
            fixture.input_with_sources_v2(
                11,
                staged,
                corrupt_scoped_complete,
                fixture.whole_parent_layer_v1(11),
                &block_sources,
            ),
        )
        .expect_err("corrupt complete scope at extension boundary"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::ResourceLimit,
    );
}

#[test]
fn final_extension_rejects_foreign_live_inputs_and_partition_or_source_drift_v2() {
    let fixture = prepare_final_extension_fixture_v2(11);
    let foreign = prepare_final_extension_fixture_in_namespace_v2(
        11,
        ori_domain::ProjectId::schema_namespace([0xf3; 16]),
    );
    let authority = issue_final_extension_v2(&fixture, 11);
    let block_sources = fixture.block_source_refs_v2();
    let baseline = fixture.revalidation_input_v2(11, &block_sources);
    authority
        .revalidate_v2(baseline)
        .expect("baseline final-extension revalidation");
    let drifted = [
        CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
            geometry: &foreign.base.geometry,
            ..baseline
        },
        CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
            pose: &foreign.base.pose,
            ..baseline
        },
        CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
            decomposition: &foreign.base.decomposition,
            ..baseline
        },
        CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
            schedule: &foreign.base.schedule,
            ..baseline
        },
        CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
            closure: &foreign.base.closure,
            ..baseline
        },
        CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
            paper_thickness_mm: f64::from_bits(fixture.base.paper_thickness_mm.to_bits() + 1),
            ..baseline
        },
    ];
    for (drift_index, input) in drifted.into_iter().enumerate() {
        assert!(
            authority.revalidate_v2(input).is_err(),
            "foreign live-input drift {drift_index} must fail closed"
        );
    }

    let replayed_source = (*fixture.source).clone();
    assert_eq!(
        authority
            .revalidate_v2(
                CommonArticulationContinuousLayerPathExtensionRevalidationInputV2 {
                    source: &replayed_source,
                    ..baseline
                }
            )
            .expect_err("replayed whole-parent source"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::WholeParentLayerMismatch,
    );

    let mut reversed_sources = block_sources.clone();
    reversed_sources.reverse();
    assert!(
        authority
            .revalidate_v2(fixture.revalidation_input_v2(11, &reversed_sources))
            .is_err()
    );

    let staged = fixture.staged_v1(11);
    let mut corrupt_complete = fixture.complete_v2(11);
    corrupt_complete.corrupt_partition_for_test_v2();
    assert_eq!(
        issue_common_articulation_continuous_layer_path_extension_authority_v2(
            fixture.input_with_sources_v2(
                11,
                staged,
                corrupt_complete,
                fixture.whole_parent_layer_v1(11),
                &block_sources,
            ),
        )
        .expect_err("corrupt complete partition"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::BlockScheduleRestrictionMismatch,
    );
}

#[test]
fn final_extension_target_partition_is_canonical_and_exact_v2() {
    let mut fixture = prepare_final_extension_fixture_v2(11);
    let baseline = issue_final_extension_v2(&fixture, 11);
    fixture.target_angles.reverse();
    let reordered = issue_final_extension_v2(&fixture, 11);
    assert_eq!(
        baseline.binding_fingerprint_v2(),
        reordered.binding_fingerprint_v2(),
    );

    fixture.target_angles[0].1 =
        f64::from_bits(fixture.target_angles[0].1.to_bits().wrapping_add(1));
    assert_eq!(
        fixture
            .try_complete_v2(11)
            .expect_err("non-exact complete-V2 target"),
        CompleteMultiBlockPositiveLayerErrorV2::TargetAngleMismatch,
    );
}

#[test]
fn final_extension_resource_envelope_is_exact_and_one_short_fails_v2() {
    let fixture = prepare_final_extension_fixture_v2(11);
    fixture.whole_parent_layer_v1(11);
    let complete = fixture.complete_v2(11);
    let exact_complete_limits = complete.exact_resource_limits_v2();
    assert_eq!(
        exact_complete_limits.max_deep_retained_bytes,
        complete.checked_deep_retained_bytes_v2(),
    );
    let expected_block_parent_count = fixture.base.decomposition.blocks().len();
    let expected_unique_parent_count = expected_block_parent_count + 1;
    let expected_parent_reference_count = expected_block_parent_count + 2;
    assert_eq!(
        complete.resources_v2().retained_parent_count_v2(),
        expected_unique_parent_count,
    );
    assert_eq!(
        complete.resources_v2().retained_parent_alias_count_v2(),
        expected_parent_reference_count - expected_unique_parent_count,
    );
    complete
        .revalidate_resource_limits_v2(exact_complete_limits)
        .expect("exact complete-V2 retained resource envelope");
    let mut one_short_complete_limits = exact_complete_limits;
    one_short_complete_limits.max_deep_retained_bytes -= 1;
    assert_eq!(
        complete
            .revalidate_resource_limits_v2(one_short_complete_limits)
            .expect_err("one-short complete-V2 deep retained resource"),
        CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit,
    );
    let one_short = GeneralCellTransportLimitsV1 {
        max_transitions: fixture.base.closure.leaves().len(),
        ..transport_limits_v1(&fixture.base.closure)
    };
    assert_eq!(
        certify_common_articulation_general_multi_face_cell_transport_extension_v1(
            CommonArticulationGeneralCellTransportExtensionInputV1 {
                geometry: &fixture.base.geometry,
                audit: &fixture.base.audit,
                decomposition: &fixture.base.decomposition,
                configured_max_blocks: 11,
                source: fixture.source.as_ref(),
                schedule: &fixture.base.schedule,
                closure: &fixture.base.closure,
                positive_continuous: &fixture.base.positive_extension_v2(11),
                positive_graph_limits:
                    crate::CommonArticulationPositiveThicknessGraphExtensionLimitsV1::fixed_v1(),
                paper_thickness_mm: fixture.base.paper_thickness_mm,
                limits: one_short,
            },
        )
        .expect_err("one-short whole-parent transition resource"),
        GeneralCellTransportErrorV1::ResourceLimit,
    );
    assert_eq!(
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MAX_BLOCKS_V2,
        32,
    );
    assert_eq!(
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MIN_BLOCKS_V2,
        11,
    );
}

#[test]
fn final_extension_issue_and_revalidation_checkpoint_entry_mid_final_v2() {
    let fixture = prepare_final_extension_fixture_v2(11);
    let block_sources = fixture.block_source_refs_v2();
    let unbounded = CooperativeOperationControlV1::unbounded();
    let mut issue_count = 0usize;
    issue_common_articulation_continuous_layer_path_extension_authority_with_checkpoint_v2(
        fixture.input_with_sources_v2(
            11,
            fixture.staged_v1(11),
            fixture.complete_v2(11),
            fixture.whole_parent_layer_v1(11),
            &block_sources,
        ),
        &unbounded,
        &mut || {
            issue_count += 1;
            Ok(())
        },
    )
    .expect("count final-extension issue checkpoints");
    assert!(issue_count >= 6);
    for stop_at in [1, issue_count / 2, issue_count] {
        for expected in [
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled,
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                issue_common_articulation_continuous_layer_path_extension_authority_with_checkpoint_v2(
                    fixture.input_with_sources_v2(
                        11,
                        fixture.staged_v1(11),
                        fixture.complete_v2(11),
                        fixture.whole_parent_layer_v1(11),
                        &block_sources,
                    ),
                    &unbounded,
                    &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    },
                )
                .expect_err("deterministic final-extension issue stop"),
                expected,
            );
        }
    }

    let authority = issue_final_extension_v2(&fixture, 11);
    let input = fixture.revalidation_input_v2(11, &block_sources);
    let mut revalidation_count = 0usize;
    authority
        .revalidate_with_checkpoint_v2(input, &unbounded, &mut || {
            revalidation_count += 1;
            Ok(())
        })
        .expect("count final-extension revalidation checkpoints");
    assert!(revalidation_count >= 6);
    for stop_at in [1, revalidation_count / 2, revalidation_count] {
        for expected in [
            CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled,
            CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                authority
                    .revalidate_with_checkpoint_v2(input, &unbounded, &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    })
                    .expect_err("deterministic final-extension revalidation stop"),
                expected,
            );
        }
    }
}

#[test]
fn final_extension_public_control_maps_cancel_and_deadline_v2() {
    let fixture = prepare_final_extension_fixture_v2(11);
    let block_sources = fixture.block_source_refs_v2();
    let cancelled = AtomicBool::new(true);
    let active = AtomicBool::new(false);
    assert_eq!(
        issue_common_articulation_continuous_layer_path_extension_authority_with_control_v2(
            fixture.input_with_sources_v2(
                11,
                fixture.staged_v1(11),
                fixture.complete_v2(11),
                fixture.whole_parent_layer_v1(11),
                &block_sources,
            ),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .expect_err("final-extension issue cancellation"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled,
    );
    assert_eq!(
        issue_common_articulation_continuous_layer_path_extension_authority_with_control_v2(
            fixture.input_with_sources_v2(
                11,
                fixture.staged_v1(11),
                fixture.complete_v2(11),
                fixture.whole_parent_layer_v1(11),
                &block_sources,
            ),
            &CooperativeOperationControlV1::new(Some(&active), Instant::now()),
        )
        .expect_err("final-extension issue deadline"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded,
    );
    let authority = issue_final_extension_v2(&fixture, 11);
    assert_eq!(
        authority
            .revalidate_with_control_v2(
                fixture.revalidation_input_v2(11, &block_sources),
                &CooperativeOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + Duration::from_secs(1),
                ),
            )
            .expect_err("final-extension revalidation cancellation"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::Cancelled,
    );
    assert_eq!(
        authority
            .revalidate_with_control_v2(
                fixture.revalidation_input_v2(11, &block_sources),
                &CooperativeOperationControlV1::new(Some(&active), Instant::now()),
            )
            .expect_err("final-extension revalidation deadline"),
        CommonArticulationContinuousLayerPathExtensionErrorV2::DeadlineExceeded,
    );
}

#[test]
fn final_extension_domain_is_distinct_and_legacy_final_model_is_unchanged_v2() {
    assert_eq!(
        super::super::COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1,
        "common_articulation_continuous_layer_path_authority_v1",
    );
    assert_ne!(
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_EXTENSION_MODEL_ID_V2,
        super::super::COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1,
    );
}
