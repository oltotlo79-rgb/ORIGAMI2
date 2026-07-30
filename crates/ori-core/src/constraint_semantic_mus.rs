use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    VertexId,
};
use ori_numeric::{
    DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1, deterministic_transcendental_model_supported_v1,
};

use crate::{
    BoundedDirectMusObserverV1, BoundedDirectMusV1, ConstraintSolvePreviewV1,
    GeometricConstraintSetV1, MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1,
    constraint_exactification::MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1,
    constraint_exactification::MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_EDGES_V1,
    constraint_exactification::MAX_PAIR_CONSTRAINT_ALGEBRAIC_CANDIDATES_V1,
    constraint_exactification::MAX_PAIR_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1,
    constraint_exactification::MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1,
    constraint_exactification::MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1,
    constraint_exactification::MAX_UNIT_TERMINAL_TWO_HOP_PARALLEL_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1,
    constraint_exactification::MAX_UNIT_TWO_HOP_PARALLEL_RESIDUAL_ONLY_OVERLAY_VERTICES_V1,
    constraint_exactification::construct_length_constraint_residual_exact_assignment_v1,
    constraint_exactification::construct_pair_constraint_algebraic_exact_assignment_v1,
    constraint_exactification::construct_pair_constraint_exact_assignment_v1,
    constraint_exactification::construct_single_constraint_exact_assignment_v1,
    constraint_exactification::construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1,
    constraint_exactification::construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1,
    constraint_exactification::construct_unit_two_hop_parallel_residual_exact_deletion_assignment_v1,
    constraint_exactification::construct_zero_length_closure_residual_exact_assignment_v1,
    constraint_exactification::zero_length_closure_constructive_candidate_bound_v1,
    constraint_solver::certify_binary64_residual_only_constraint_overlay_v1,
    exactify_axis_aligned_constraint_preview_v1, find_bounded_direct_mus_with_observer_v1,
};

pub const GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1: &str =
    "geometric_constraint_deterministic_binary64_semantic_mus_v4";
pub const MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1: usize =
    MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1;
pub const MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1: usize = 20_000_000;
pub(crate) const MAX_ANCHORED_MIRROR_RESIDUAL_ONLY_OVERLAY_VERTICES_V1: usize = 256;

const GENERAL_GRAPH_PREFLIGHT_LOGICAL_WORK_CEILING_V1: usize = 80_000;
const ZERO_CLOSURE_WORK_PER_PATTERN_EDGE_V1: usize = 4;
const ZERO_CLOSURE_LINEAR_WORK_PER_CONSTRAINT_V1: usize = 24;
const ZERO_CLOSURE_QUADRATIC_WORK_PER_CONSTRAINT_PAIR_V1: usize = 24;

/// Caller-tightenable limits for the positive deletion-witness phase.
///
/// The direct-conflict subset oracle retains its independent 16-constraint
/// and 65,535-call ceilings. V1 witness work reserves a checked conservative
/// upper bound before each setup, exact-certificate, or axis-exactification
/// phase. Those reservations include canonical sorting, map construction,
/// direct preflight closures, residual scans, cloning, and projection work,
/// and are separate from direct-oracle calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedSemanticMusLimitsV1 {
    pub max_deletion_witness_checks: usize,
    pub max_deletion_witness_work: usize,
}

impl Default for BoundedSemanticMusLimitsV1 {
    fn default() -> Self {
        Self {
            max_deletion_witness_checks: MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1,
            max_deletion_witness_work: MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1,
        }
    }
}

impl BoundedSemanticMusLimitsV1 {
    fn effective(self) -> Self {
        Self {
            max_deletion_witness_checks: self
                .max_deletion_witness_checks
                .min(MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_CHECKS_V1),
            max_deletion_witness_work: self
                .max_deletion_witness_work
                .min(MAX_BOUNDED_SEMANTIC_MUS_DELETION_WITNESS_WORK_V1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedSemanticMusProgressV1 {
    pub direct_oracle_calls: usize,
    pub deletion_witness_checks: usize,
    pub certified_deletion_witnesses: usize,
    pub deletion_witness_work: usize,
}

impl BoundedSemanticMusProgressV1 {
    const fn new() -> Self {
        Self {
            direct_oracle_calls: 0,
            deletion_witness_checks: 0,
            certified_deletion_witnesses: 0,
            deletion_witness_work: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedSemanticMusObserverControlV1 {
    Continue,
    Cancelled,
    DeadlineReached,
}

/// Cooperative stop hook spanning both the direct subset oracle and every
/// deletion-witness boundary.
pub trait BoundedSemanticMusObserverV1 {
    fn checkpoint(
        &mut self,
        progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBoundedSemanticMusObserverV1;

impl BoundedSemanticMusObserverV1 for NoopBoundedSemanticMusObserverV1 {
    fn checkpoint(
        &mut self,
        _progress: BoundedSemanticMusProgressV1,
    ) -> BoundedSemanticMusObserverControlV1 {
        BoundedSemanticMusObserverControlV1::Continue
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedSemanticMusUnknownReasonV1 {
    DirectOracleIncomplete,
    DeletionWitnessLimitExceeded,
    DeletionWitnessWorkLimitExceeded,
    DeletionWitnessUnavailable,
    Cancelled,
    DeadlineReached,
}

/// Opaque positive certificate for one deterministic-binary64 semantic MUS.
///
/// The direct theorem proves the complete core unsatisfiable. For every
/// returned ID, V1 independently certified an explicit complete assignment
/// for the core with that one record deleted. Immediate-deletion
/// satisfiability is sufficient for deletion minimality because removing
/// further constraints preserves each witnessed assignment.
///
/// The assignments are discarded after checking to keep the result bounded;
/// this opaque type can only be created by the native certification path. It
/// is not project/revision authority. Cross-runtime re-certification is
/// advertised only on targets covered by the frozen transcendental model; no
/// serialized assignment witness is carried or claimed portable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentRuntimeSemanticMusV1 {
    constraint_ids: Vec<ConstraintId>,
    direct_oracle_calls: usize,
    deletion_witness_checks: usize,
    deletion_witness_work: usize,
    current_assignment_witness_count: usize,
    axis_exactification_witness_count: usize,
    single_constraint_constructive_witness_count: usize,
    pair_constraint_constructive_witness_count: usize,
    pair_constraint_algebraic_witness_count: usize,
    length_constraint_constructive_witness_count: usize,
    zero_length_closure_constructive_witness_count: usize,
    anchored_mirror_residual_only_witness_count: usize,
    unit_parallel_fixed_angle_residual_only_witness_count: usize,
    unit_terminal_two_hop_parallel_angle_residual_only_witness_count: usize,
    unit_two_hop_parallel_residual_only_witness_count: usize,
}

impl CurrentRuntimeSemanticMusV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_SEMANTIC_MUS_MODEL_ID_V1
    }

    #[must_use]
    pub const fn transcendental_model_id(&self) -> &'static str {
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    }

    #[must_use]
    pub fn constraint_ids(&self) -> &[ConstraintId] {
        &self.constraint_ids
    }

    #[must_use]
    pub const fn direct_oracle_calls(&self) -> usize {
        self.direct_oracle_calls
    }

    #[must_use]
    pub const fn deletion_witness_checks(&self) -> usize {
        self.deletion_witness_checks
    }

    #[must_use]
    pub const fn deletion_witness_work(&self) -> usize {
        self.deletion_witness_work
    }

    #[must_use]
    pub const fn current_assignment_witness_count(&self) -> usize {
        self.current_assignment_witness_count
    }

    #[must_use]
    pub const fn axis_exactification_witness_count(&self) -> usize {
        self.axis_exactification_witness_count
    }

    #[must_use]
    pub const fn single_constraint_constructive_witness_count(&self) -> usize {
        self.single_constraint_constructive_witness_count
    }

    #[must_use]
    pub const fn pair_constraint_constructive_witness_count(&self) -> usize {
        self.pair_constraint_constructive_witness_count
    }

    #[must_use]
    pub const fn pair_constraint_algebraic_witness_count(&self) -> usize {
        self.pair_constraint_algebraic_witness_count
    }

    #[must_use]
    pub const fn length_constraint_constructive_witness_count(&self) -> usize {
        self.length_constraint_constructive_witness_count
    }

    #[must_use]
    pub const fn zero_length_closure_constructive_witness_count(&self) -> usize {
        self.zero_length_closure_constructive_witness_count
    }

    #[must_use]
    pub const fn anchored_mirror_residual_only_witness_count(&self) -> usize {
        self.anchored_mirror_residual_only_witness_count
    }

    #[must_use]
    pub const fn unit_parallel_fixed_angle_residual_only_witness_count(&self) -> usize {
        self.unit_parallel_fixed_angle_residual_only_witness_count
    }

    #[must_use]
    pub const fn unit_terminal_two_hop_parallel_angle_residual_only_witness_count(&self) -> usize {
        self.unit_terminal_two_hop_parallel_angle_residual_only_witness_count
    }

    #[must_use]
    pub const fn unit_two_hop_parallel_residual_only_witness_count(&self) -> usize {
        self.unit_two_hop_parallel_residual_only_witness_count
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn replayable_across_runtimes(&self) -> bool {
        deterministic_transcendental_model_supported_v1()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedCurrentRuntimeSemanticMusV1 {
    Certified(CurrentRuntimeSemanticMusV1),
    Unknown {
        reason: BoundedSemanticMusUnknownReasonV1,
        direct_core_constraint_ids: Vec<ConstraintId>,
        direct_oracle_calls: usize,
        deletion_witness_checks: usize,
        certified_deletion_witnesses: usize,
        deletion_witness_work: usize,
    },
}

impl BoundedCurrentRuntimeSemanticMusV1 {
    /// Returns the canonical direct-theorem core already found by the single
    /// bounded oracle run. This remains available when later semantic witness
    /// checks fail, but an `Unknown` outcome never promotes it to a semantic
    /// MUS. The slice is empty when the direct oracle itself was incomplete.
    #[must_use]
    pub fn direct_core_constraint_ids(&self) -> &[ConstraintId] {
        match self {
            Self::Certified(certificate) => certificate.constraint_ids(),
            Self::Unknown {
                direct_core_constraint_ids,
                ..
            } => direct_core_constraint_ids,
        }
    }
}

#[must_use]
pub fn certify_bounded_current_runtime_semantic_mus_v1(
    set: &GeometricConstraintSetV1<'_>,
) -> BoundedCurrentRuntimeSemanticMusV1 {
    certify_bounded_current_runtime_semantic_mus_with_observer_v1(
        set,
        BoundedSemanticMusLimitsV1::default(),
        &mut NoopBoundedSemanticMusObserverV1,
    )
}

#[must_use]
pub fn certify_bounded_current_runtime_semantic_mus_with_observer_v1(
    set: &GeometricConstraintSetV1<'_>,
    limits: BoundedSemanticMusLimitsV1,
    observer: &mut impl BoundedSemanticMusObserverV1,
) -> BoundedCurrentRuntimeSemanticMusV1 {
    let limits = limits.effective();
    let mut progress = BoundedSemanticMusProgressV1::new();
    let (direct, direct_stop) = {
        let mut adapter = DirectObserverAdapter {
            observer,
            stop: None,
        };
        let direct = find_bounded_direct_mus_with_observer_v1(set, &mut adapter);
        (direct, adapter.stop)
    };
    let (constraint_ids, direct_oracle_calls) = match direct {
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } => (constraint_ids, oracle_calls),
        BoundedDirectMusV1::Unknown { oracle_calls } => {
            progress.direct_oracle_calls = oracle_calls;
            return unknown(
                direct_stop
                    .map(stop_reason)
                    .unwrap_or(BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete),
                progress,
                &[],
            );
        }
    };
    progress.direct_oracle_calls = direct_oracle_calls;
    if let Some(stop) = direct_stop {
        return unknown(stop_reason(stop), progress, &constraint_ids);
    }
    if let Some(reason) = checkpoint(observer, progress) {
        return unknown(reason, progress, &constraint_ids);
    }
    if constraint_ids.len() > limits.max_deletion_witness_checks {
        return unknown(
            BoundedSemanticMusUnknownReasonV1::DeletionWitnessLimitExceeded,
            progress,
            &constraint_ids,
        );
    }

    let Some(setup_work) = witness_setup_work(constraint_ids.len()) else {
        return unknown(
            BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            progress,
            &constraint_ids,
        );
    };
    if !charge_witness_work(&mut progress, setup_work, limits.max_deletion_witness_work) {
        return unknown(
            BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
            progress,
            &constraint_ids,
        );
    }
    let mut core = Vec::with_capacity(constraint_ids.len());
    for id in &constraint_ids {
        let Some(record) = set.residual_role_preserving_record(*id) else {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
                progress,
                &constraint_ids,
            );
        };
        core.push(record);
    }
    if let Some(reason) = checkpoint(observer, progress) {
        return unknown(reason, progress, &constraint_ids);
    }

    let mut current_assignment_witness_count = 0;
    let mut axis_exactification_witness_count = 0;
    let mut single_constraint_constructive_witness_count = 0;
    let mut pair_constraint_constructive_witness_count = 0;
    let mut pair_constraint_algebraic_witness_count = 0;
    let mut length_constraint_constructive_witness_count = 0;
    let mut zero_length_closure_constructive_witness_count = 0;
    let mut anchored_mirror_residual_only_witness_count = 0;
    let mut unit_parallel_fixed_angle_residual_only_witness_count = 0;
    let mut unit_terminal_two_hop_parallel_angle_residual_only_witness_count = 0;
    let mut unit_two_hop_parallel_residual_only_witness_count = 0;
    let anchored_mirror_shape = is_anchored_mirror_core_shape(&core);
    let unit_parallel_fixed_angle_shape = is_unit_parallel_fixed_angle_core_shape(&core);
    let unit_terminal_two_hop_parallel_angle_shape =
        is_unit_terminal_two_hop_parallel_angle_core_shape(&core);
    let unit_two_hop_parallel_shape = is_unit_two_hop_parallel_core_shape(&core);
    for removed in &constraint_ids {
        progress.deletion_witness_checks += 1;
        if let Some(reason) = checkpoint(observer, progress) {
            return unknown(reason, progress, &constraint_ids);
        }

        let deletion_constraint_count = core.len().saturating_sub(1);
        if unit_parallel_fixed_angle_shape {
            let Some(parallel_angle_work) =
                unit_parallel_fixed_angle_residual_only_phase_work(set, deletion_constraint_count)
            else {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            };
            if !charge_witness_work(
                &mut progress,
                parallel_angle_work,
                limits.max_deletion_witness_work,
            ) {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            }
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            let document = deletion_document(&core, *removed);
            let parallel_angle_is_exact =
                construct_unit_parallel_fixed_angle_residual_exact_deletion_assignment_v1(
                    set.source_pattern(),
                    &core,
                    *removed,
                    &document,
                )
                .is_some();
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            if parallel_angle_is_exact {
                unit_parallel_fixed_angle_residual_only_witness_count += 1;
                progress.certified_deletion_witnesses += 1;
                continue;
            }
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
                progress,
                &constraint_ids,
            );
        }

        if unit_two_hop_parallel_shape {
            let Some(parallel_work) =
                unit_two_hop_parallel_residual_only_phase_work(set, deletion_constraint_count)
            else {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            };
            if !charge_witness_work(
                &mut progress,
                parallel_work,
                limits.max_deletion_witness_work,
            ) {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            }
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            let document = deletion_document(&core, *removed);
            let parallel_is_exact =
                construct_unit_two_hop_parallel_residual_exact_deletion_assignment_v1(
                    set.source_pattern(),
                    &core,
                    *removed,
                    &document,
                )
                .is_some();
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            if parallel_is_exact {
                unit_two_hop_parallel_residual_only_witness_count += 1;
                progress.certified_deletion_witnesses += 1;
                continue;
            }
        }

        if unit_terminal_two_hop_parallel_angle_shape {
            let Some(parallel_angle_work) =
                unit_terminal_two_hop_parallel_angle_residual_only_phase_work(
                    set,
                    deletion_constraint_count,
                )
            else {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            };
            if !charge_witness_work(
                &mut progress,
                parallel_angle_work,
                limits.max_deletion_witness_work,
            ) {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            }
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            let document = deletion_document(&core, *removed);
            let parallel_angle_is_exact =
                construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
                    set.source_pattern(),
                    &core,
                    *removed,
                    &document,
                )
                .is_some();
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            if parallel_angle_is_exact {
                unit_terminal_two_hop_parallel_angle_residual_only_witness_count += 1;
                progress.certified_deletion_witnesses += 1;
                continue;
            }
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
                progress,
                &constraint_ids,
            );
        }

        if anchored_mirror_shape {
            let Some(mirror_work) =
                anchored_mirror_residual_only_phase_work(set, deletion_constraint_count)
            else {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            };
            if !charge_witness_work(&mut progress, mirror_work, limits.max_deletion_witness_work) {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            }
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            let document = deletion_document(&core, *removed);
            let mirror_is_exact = construct_anchored_mirror_residual_only_deletion_witness(
                set.source_pattern(),
                &core,
                *removed,
                &document,
            );
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            if mirror_is_exact {
                anchored_mirror_residual_only_witness_count += 1;
                progress.certified_deletion_witnesses += 1;
                continue;
            }
        }

        let Some(current_work) = current_certificate_phase_work(set, deletion_constraint_count)
        else {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        };
        if !charge_witness_work(
            &mut progress,
            current_work,
            limits.max_deletion_witness_work,
        ) {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        }
        let document = deletion_document(&core, *removed);
        let current_is_exact = certify_binary64_exact_geometric_constraint_satisfaction_v1(
            set.source_pattern(),
            &document,
        )
        .ok()
        .flatten()
        .is_some();
        if let Some(reason) = checkpoint(observer, progress) {
            return unknown(reason, progress, &constraint_ids);
        }
        if current_is_exact {
            current_assignment_witness_count += 1;
            progress.certified_deletion_witnesses += 1;
            continue;
        }

        let Some(axis_work) = axis_exactification_phase_work(set, document.constraints.len())
        else {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        };
        if !charge_witness_work(&mut progress, axis_work, limits.max_deletion_witness_work) {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        }
        let axis_is_exact = exactify_axis_aligned_constraint_preview_v1(
            set.source_pattern(),
            &document,
            &empty_preview(),
        )
        .is_some();
        if let Some(reason) = checkpoint(observer, progress) {
            return unknown(reason, progress, &constraint_ids);
        }
        if axis_is_exact {
            axis_exactification_witness_count += 1;
            progress.certified_deletion_witnesses += 1;
            continue;
        }
        if document.constraints.len() == 1 {
            let Some(single_constraint_work) =
                single_constraint_constructive_phase_work(set, document.constraints.len())
            else {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            };
            if !charge_witness_work(
                &mut progress,
                single_constraint_work,
                limits.max_deletion_witness_work,
            ) {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            }
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            let single_constraint_is_exact =
                construct_single_constraint_exact_assignment_v1(set.source_pattern(), &document)
                    .is_some();
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            if !single_constraint_is_exact {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
                    progress,
                    &constraint_ids,
                );
            }
            single_constraint_constructive_witness_count += 1;
            progress.certified_deletion_witnesses += 1;
            continue;
        }
        if document.constraints.len() == 2 {
            let Some(pair_constraint_work) =
                pair_constraint_constructive_phase_work(set, document.constraints.len())
            else {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            };
            if !charge_witness_work(
                &mut progress,
                pair_constraint_work,
                limits.max_deletion_witness_work,
            ) {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            }
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            let pair_constraint_is_exact =
                construct_pair_constraint_exact_assignment_v1(set.source_pattern(), &document)
                    .is_some();
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            if pair_constraint_is_exact {
                pair_constraint_constructive_witness_count += 1;
                progress.certified_deletion_witnesses += 1;
                continue;
            }

            let Some(pair_algebraic_work) =
                pair_constraint_algebraic_phase_work(set, document.constraints.len())
            else {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            };
            if !charge_witness_work(
                &mut progress,
                pair_algebraic_work,
                limits.max_deletion_witness_work,
            ) {
                return unknown(
                    BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                    progress,
                    &constraint_ids,
                );
            }
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            let pair_algebraic_is_exact = construct_pair_constraint_algebraic_exact_assignment_v1(
                set.source_pattern(),
                &document,
            )
            .is_some();
            if let Some(reason) = checkpoint(observer, progress) {
                return unknown(reason, progress, &constraint_ids);
            }
            if pair_algebraic_is_exact {
                pair_constraint_algebraic_witness_count += 1;
                progress.certified_deletion_witnesses += 1;
                continue;
            }
        }

        let Some(length_constraint_work) =
            length_constraint_constructive_phase_work(set, document.constraints.len())
        else {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        };
        if !charge_witness_work(
            &mut progress,
            length_constraint_work,
            limits.max_deletion_witness_work,
        ) {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        }
        if let Some(reason) = checkpoint(observer, progress) {
            return unknown(reason, progress, &constraint_ids);
        }
        let length_constraint_is_exact = construct_length_constraint_residual_exact_assignment_v1(
            set.source_pattern(),
            &document,
        )
        .is_some();
        if let Some(reason) = checkpoint(observer, progress) {
            return unknown(reason, progress, &constraint_ids);
        }
        if length_constraint_is_exact {
            length_constraint_constructive_witness_count += 1;
            progress.certified_deletion_witnesses += 1;
            continue;
        }

        let Some(zero_length_closure_work) =
            zero_length_closure_constructive_phase_work(set, &document)
        else {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        };
        if !charge_witness_work(
            &mut progress,
            zero_length_closure_work,
            limits.max_deletion_witness_work,
        ) {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessWorkLimitExceeded,
                progress,
                &constraint_ids,
            );
        }
        if let Some(reason) = checkpoint(observer, progress) {
            return unknown(reason, progress, &constraint_ids);
        }
        let zero_length_closure_is_exact =
            construct_zero_length_closure_residual_exact_assignment_v1(
                set.source_pattern(),
                &document,
            )
            .is_some();
        if let Some(reason) = checkpoint(observer, progress) {
            return unknown(reason, progress, &constraint_ids);
        }
        if !zero_length_closure_is_exact {
            return unknown(
                BoundedSemanticMusUnknownReasonV1::DeletionWitnessUnavailable,
                progress,
                &constraint_ids,
            );
        }
        zero_length_closure_constructive_witness_count += 1;
        progress.certified_deletion_witnesses += 1;
    }

    if let Some(reason) = checkpoint(observer, progress) {
        return unknown(reason, progress, &constraint_ids);
    }
    debug_assert_eq!(progress.certified_deletion_witnesses, constraint_ids.len());
    debug_assert_eq!(
        current_assignment_witness_count
            + axis_exactification_witness_count
            + single_constraint_constructive_witness_count
            + pair_constraint_constructive_witness_count
            + pair_constraint_algebraic_witness_count
            + length_constraint_constructive_witness_count
            + zero_length_closure_constructive_witness_count
            + anchored_mirror_residual_only_witness_count
            + unit_parallel_fixed_angle_residual_only_witness_count
            + unit_terminal_two_hop_parallel_angle_residual_only_witness_count
            + unit_two_hop_parallel_residual_only_witness_count,
        constraint_ids.len(),
    );
    BoundedCurrentRuntimeSemanticMusV1::Certified(CurrentRuntimeSemanticMusV1 {
        constraint_ids,
        direct_oracle_calls,
        deletion_witness_checks: progress.deletion_witness_checks,
        deletion_witness_work: progress.deletion_witness_work,
        current_assignment_witness_count,
        axis_exactification_witness_count,
        single_constraint_constructive_witness_count,
        pair_constraint_constructive_witness_count,
        pair_constraint_algebraic_witness_count,
        length_constraint_constructive_witness_count,
        zero_length_closure_constructive_witness_count,
        anchored_mirror_residual_only_witness_count,
        unit_parallel_fixed_angle_residual_only_witness_count,
        unit_terminal_two_hop_parallel_angle_residual_only_witness_count,
        unit_two_hop_parallel_residual_only_witness_count,
    })
}

fn is_unit_parallel_fixed_angle_core_shape(core: &[GeometricConstraintRecordV1]) -> bool {
    if core.len() != 3 {
        return false;
    }
    let mut fixed_unit_count = 0;
    let mut fixed_angle_count = 0;
    let mut parallel_count = 0;
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::FixedLength { length_mm, .. }
                if length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed_unit_count += 1;
            }
            GeometricConstraintKindV1::FixedAngle { angle_degrees, .. }
                if [45.0_f64, 135.0_f64]
                    .into_iter()
                    .any(|angle| angle_degrees.to_bits() == angle.to_bits()) =>
            {
                fixed_angle_count += 1;
            }
            GeometricConstraintKindV1::Parallel { .. } => parallel_count += 1,
            _ => return false,
        }
    }
    (fixed_unit_count, fixed_angle_count, parallel_count) == (1, 1, 1)
}

fn is_unit_terminal_two_hop_parallel_angle_core_shape(
    core: &[GeometricConstraintRecordV1],
) -> bool {
    if core.len() != 5 {
        return false;
    }
    let mut fixed_unit_count = 0;
    let mut fixed_right_angle_count = 0;
    let mut parallel_count = 0;
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::FixedLength { length_mm, .. }
                if length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed_unit_count += 1;
            }
            GeometricConstraintKindV1::FixedAngle { angle_degrees, .. }
                if angle_degrees.to_bits() == 90.0_f64.to_bits() =>
            {
                fixed_right_angle_count += 1;
            }
            GeometricConstraintKindV1::Parallel { .. } => parallel_count += 1,
            _ => return false,
        }
    }
    (fixed_unit_count, fixed_right_angle_count, parallel_count) == (2, 1, 2)
}

fn is_unit_two_hop_parallel_core_shape(core: &[GeometricConstraintRecordV1]) -> bool {
    if core.len() != 5 {
        return false;
    }
    let mut fixed_unit_count = 0;
    let mut horizontal_count = 0;
    let mut vertical_count = 0;
    let mut parallel_count = 0;
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::FixedLength { length_mm, .. }
                if length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed_unit_count += 1;
            }
            GeometricConstraintKindV1::Horizontal { .. } => horizontal_count += 1,
            GeometricConstraintKindV1::Vertical { .. } => vertical_count += 1,
            GeometricConstraintKindV1::Parallel { .. } => parallel_count += 1,
            _ => return false,
        }
    }
    (
        fixed_unit_count,
        horizontal_count,
        vertical_count,
        parallel_count,
    ) == (1, 1, 1, 2)
}

#[derive(Debug, Clone, Copy)]
struct AnchoredMirrorCoreV1 {
    mirror_id: ConstraintId,
    fixed_length_id: ConstraintId,
    horizontal_id: ConstraintId,
    vertical_id: ConstraintId,
    raw_source: VertexId,
    raw_target: VertexId,
    axis_end: VertexId,
    separation_length: f64,
}

fn is_anchored_mirror_core_shape(core: &[GeometricConstraintRecordV1]) -> bool {
    if core.len() != 4 {
        return false;
    }
    let mut mirror_count = 0;
    let mut fixed_length_count = 0;
    let mut horizontal_count = 0;
    let mut vertical_count = 0;
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::MirrorSymmetry { .. } => mirror_count += 1,
            GeometricConstraintKindV1::FixedLength { .. } => fixed_length_count += 1,
            GeometricConstraintKindV1::Horizontal { .. } => horizontal_count += 1,
            GeometricConstraintKindV1::Vertical { .. } => vertical_count += 1,
            _ => return false,
        }
    }
    (
        mirror_count,
        fixed_length_count,
        horizontal_count,
        vertical_count,
    ) == (1, 1, 1, 1)
}

fn anchored_mirror_core(
    pattern: &CreasePattern,
    core: &[GeometricConstraintRecordV1],
) -> Option<AnchoredMirrorCoreV1> {
    if !is_anchored_mirror_core_shape(core) {
        return None;
    }
    let mut mirror = None;
    let mut fixed_length = None;
    let mut horizontal = None;
    let mut vertical = None;
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                axis_edge,
            } => mirror = Some((record.id, *first_vertex, *second_vertex, *axis_edge)),
            GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
                fixed_length = Some((record.id, *edge, *length_mm));
            }
            GeometricConstraintKindV1::Horizontal { edge } => {
                horizontal = Some((record.id, *edge));
            }
            GeometricConstraintKindV1::Vertical { edge } => {
                vertical = Some((record.id, *edge));
            }
            _ => return None,
        }
    }
    let (mirror_id, raw_source, raw_target, axis_edge_id) = mirror?;
    let (fixed_length_id, separation_edge_id, separation_length) = fixed_length?;
    let (horizontal_id, horizontal_edge_id) = horizontal?;
    let (vertical_id, vertical_edge_id) = vertical?;
    if horizontal_edge_id != vertical_edge_id
        || !separation_length.is_finite()
        || separation_length <= 0.0
    {
        return None;
    }
    let axis_edge = find_pattern_edge(pattern, axis_edge_id)?;
    let connector_edge = find_pattern_edge(pattern, horizontal_edge_id)?;
    let separation_edge = find_pattern_edge(pattern, separation_edge_id)?;
    if !edge_has_endpoints(connector_edge, axis_edge.start, raw_source)
        || !edge_has_endpoints(separation_edge, raw_source, raw_target)
    {
        return None;
    }
    Some(AnchoredMirrorCoreV1 {
        mirror_id,
        fixed_length_id,
        horizontal_id,
        vertical_id,
        raw_source,
        raw_target,
        axis_end: axis_edge.end,
        separation_length,
    })
}

fn construct_anchored_mirror_residual_only_deletion_witness(
    pattern: &CreasePattern,
    core: &[GeometricConstraintRecordV1],
    removed: ConstraintId,
    document: &GeometricConstraintDocumentV1,
) -> bool {
    if pattern.vertices.len() > MAX_ANCHORED_MIRROR_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 {
        return false;
    }
    let Some(context) = anchored_mirror_core(pattern, core) else {
        return false;
    };
    let zero = Point2::new(0.0, 0.0);
    let mut axis_end = zero;
    let mut source = zero;
    let mut target = zero;
    if removed == context.mirror_id {
        target = Point2::new(context.separation_length, 0.0);
    } else if removed == context.fixed_length_id {
        axis_end = Point2::new(1.0, 0.0);
    } else if removed == context.horizontal_id {
        let half = context.separation_length * 0.5;
        if half == 0.0 || !half.is_finite() {
            return false;
        }
        axis_end = Point2::new(1.0, 0.0);
        source = Point2::new(0.0, half);
        target = Point2::new(0.0, -half);
    } else if removed == context.vertical_id {
        let half = context.separation_length * 0.5;
        if half == 0.0 || !half.is_finite() {
            return false;
        }
        axis_end = Point2::new(0.0, 1.0);
        source = Point2::new(half, 0.0);
        target = Point2::new(-half, 0.0);
    } else {
        return false;
    }

    let mut overlay = Vec::new();
    if overlay.try_reserve_exact(pattern.vertices.len()).is_err() {
        return false;
    }
    for vertex in &pattern.vertices {
        let point = if vertex.id == context.axis_end {
            axis_end
        } else if vertex.id == context.raw_source {
            source
        } else if vertex.id == context.raw_target {
            target
        } else {
            zero
        };
        overlay.push((vertex.id, point));
    }
    certify_binary64_residual_only_constraint_overlay_v1(pattern, document, &overlay)
        .ok()
        .flatten()
        .is_some()
}

fn find_pattern_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn edge_has_endpoints(edge: &Edge, first: VertexId, second: VertexId) -> bool {
    (edge.start == first && edge.end == second) || (edge.start == second && edge.end == first)
}

fn deletion_document(
    core: &[GeometricConstraintRecordV1],
    removed: ConstraintId,
) -> GeometricConstraintDocumentV1 {
    GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: core
            .iter()
            .filter(|record| record.id != removed)
            .cloned()
            .collect(),
    }
}

fn empty_preview() -> ConstraintSolvePreviewV1 {
    ConstraintSolvePreviewV1 {
        positions: Vec::new(),
        iterations: 0,
        maximum_residual: 0.0,
        rank: 0,
        degrees_of_freedom: 0,
        equation_count: 0,
        condition_estimate: 1.0,
    }
}

/// Logical witness-work units deliberately over-count collection visits,
/// comparisons, scalar operations, and map operations. A complete upper bound
/// is reserved before a phase begins, so a one-short budget cannot start it.
///
/// Direct joins and ratio paths are bounded from the deletion-subset size
/// (at most fifteen) by a cubic term with canonical-order factors. The two
/// general graph routines retain their combined 80,000-unit ceiling. Bounded
/// zero closure uses its edge/linear/quadratic admission formula with an
/// ordered-map factor. Pattern registries and projection sorts use explicit
/// ceil-log2 comparison bounds. All arithmetic is checked.
fn witness_setup_work(core_constraint_count: usize) -> Option<usize> {
    let dimension = core_constraint_count.checked_add(1)?;
    checked_mul(32, checked_mul(dimension, dimension)?)
}

fn current_certificate_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    let vertices = set.source_pattern().vertices.len();
    let edges = set.source_pattern().edges.len();
    checked_sum([
        deletion_document_build_work(constraint_count)?,
        prepare_and_preflight_work(vertices, edges, constraint_count)?,
        residual_certificate_work(vertices, edges, constraint_count)?,
    ])
}

fn axis_exactification_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    let vertices = set.source_pattern().vertices.len();
    let edges = set.source_pattern().edges.len();
    checked_sum([
        prepare_and_preflight_work(vertices, edges, constraint_count)?,
        axis_projection_work(vertices, edges, constraint_count)?,
        prepare_and_preflight_work(vertices, edges, constraint_count)?,
        residual_certificate_work(vertices, edges, constraint_count)?,
    ])
}

fn single_constraint_constructive_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    let vertices = set.source_pattern().vertices.len();
    let edges = set.source_pattern().edges.len();
    let candidate_work = checked_sum([
        checked_mul(checked_sum([vertices, edges, constraint_count, 1])?, 64)?,
        checked_mul(sort_work(vertices)?, 16)?,
        prepare_and_preflight_work(vertices, edges, constraint_count)?,
        residual_certificate_work(vertices, edges, constraint_count)?,
    ])?;
    checked_sum([
        prepare_and_preflight_work(vertices, edges, constraint_count)?,
        checked_mul(
            MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1,
            candidate_work,
        )?,
    ])
}

fn pair_constraint_constructive_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    pair_constraint_constructive_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        constraint_count,
    )
}

fn pair_constraint_constructive_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    let template_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            96,
        )?,
        checked_mul(sort_work(vertex_count)?, 32)?,
        checked_mul(sort_work(edge_count)?, 32)?,
    ])?;
    let candidate_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            64,
        )?,
        checked_mul(sort_work(vertex_count)?, 16)?,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])?;
    checked_sum([
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        template_work,
        checked_mul(
            MAX_PAIR_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1,
            candidate_work,
        )?,
    ])
}

fn pair_constraint_algebraic_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    pair_constraint_algebraic_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        constraint_count,
    )
}

fn pair_constraint_algebraic_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    let template_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            96,
        )?,
        checked_mul(sort_work(edge_count)?, 16)?,
    ])?;
    let candidate_work = checked_sum([
        checked_mul(vertex_count.checked_add(1)?, 96)?,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])?;
    checked_sum([
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        template_work,
        checked_mul(MAX_PAIR_CONSTRAINT_ALGEBRAIC_CANDIDATES_V1, candidate_work)?,
    ])
}

fn anchored_mirror_residual_only_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    anchored_mirror_residual_only_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        constraint_count,
    )
}

fn anchored_mirror_residual_only_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    let classification_and_overlay_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            128,
        )?,
        checked_mul(vertex_count.checked_add(1)?, 64)?,
    ])?;
    checked_sum([
        deletion_document_build_work(constraint_count)?,
        classification_and_overlay_work,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])
}

fn unit_parallel_fixed_angle_residual_only_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    unit_parallel_fixed_angle_residual_only_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        constraint_count,
    )
}

fn unit_parallel_fixed_angle_residual_only_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    if vertex_count > MAX_UNIT_PARALLEL_FIXED_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 {
        return None;
    }
    let classification_and_overlay_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            128,
        )?,
        checked_mul(vertex_count.checked_add(1)?, 64)?,
        checked_mul(sort_work(constraint_count)?, 16)?,
    ])?;
    checked_sum([
        deletion_document_build_work(constraint_count)?,
        classification_and_overlay_work,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])
}

fn unit_terminal_two_hop_parallel_angle_residual_only_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    unit_terminal_two_hop_parallel_angle_residual_only_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        constraint_count,
    )
}

fn unit_terminal_two_hop_parallel_angle_residual_only_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    if vertex_count > MAX_UNIT_TERMINAL_TWO_HOP_PARALLEL_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 {
        return None;
    }
    let classification_and_overlay_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            128,
        )?,
        checked_mul(vertex_count.checked_add(1)?, 64)?,
        checked_mul(sort_work(constraint_count)?, 16)?,
    ])?;
    checked_sum([
        deletion_document_build_work(constraint_count)?,
        classification_and_overlay_work,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])
}

fn unit_two_hop_parallel_residual_only_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    unit_two_hop_parallel_residual_only_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        constraint_count,
    )
}

fn unit_two_hop_parallel_residual_only_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    if vertex_count > MAX_UNIT_TWO_HOP_PARALLEL_RESIDUAL_ONLY_OVERLAY_VERTICES_V1 {
        return None;
    }
    let classification_and_overlay_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            128,
        )?,
        checked_mul(vertex_count.checked_add(1)?, 64)?,
        checked_mul(sort_work(constraint_count)?, 16)?,
    ])?;
    checked_sum([
        deletion_document_build_work(constraint_count)?,
        classification_and_overlay_work,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])
}

fn length_constraint_constructive_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    constraint_count: usize,
) -> Option<usize> {
    length_constraint_constructive_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        constraint_count,
    )
}

fn length_constraint_constructive_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    let referenced_edge_bound = checked_mul(constraint_count, 2)?;
    let registry_work = checked_sum([
        checked_mul(
            checked_sum([
                vertex_count,
                edge_count,
                constraint_count,
                referenced_edge_bound,
                1,
            ])?,
            128,
        )?,
        checked_mul(sort_work(edge_count)?, 32)?,
        checked_mul(sort_work(constraint_count)?, 32)?,
        checked_mul(sort_work(referenced_edge_bound)?, 32)?,
    ])?;
    let propagation_work = checked_mul(
        MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_EDGES_V1,
        checked_mul(
            checked_sum([constraint_count, referenced_edge_bound, 1])?,
            128,
        )?,
    )?;
    let overlay_work = checked_mul(
        checked_sum([vertex_count, MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_EDGES_V1, 1])?,
        128,
    )?;
    checked_sum([
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        registry_work,
        checked_mul(
            MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1,
            constraint_count.checked_add(1)?,
        )?,
        propagation_work,
        overlay_work,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])
}

fn zero_length_closure_constructive_phase_work(
    set: &GeometricConstraintSetV1<'_>,
    document: &GeometricConstraintDocumentV1,
) -> Option<usize> {
    zero_length_closure_constructive_work(
        set.source_pattern().vertices.len(),
        set.source_pattern().edges.len(),
        document.constraints.len(),
        zero_length_closure_constructive_candidate_bound_v1(document),
    )
}

fn zero_length_closure_constructive_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
    candidate_count: usize,
) -> Option<usize> {
    let classification_work = checked_mul(constraint_count.checked_add(1)?, 128)?;
    let registry_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            128,
        )?,
        checked_mul(sort_work(vertex_count)?, 32)?,
        checked_mul(sort_work(edge_count)?, 32)?,
        checked_mul(sort_work(constraint_count)?, 32)?,
    ])?;
    let candidate_work = checked_sum([
        checked_mul(
            checked_sum([vertex_count, edge_count, constraint_count, 1])?,
            256,
        )?,
        checked_mul(sort_work(vertex_count)?, 32)?,
        checked_mul(vertex_count.checked_add(1)?, 128)?,
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        residual_certificate_work(vertex_count, edge_count, constraint_count)?,
    ])?;
    checked_sum([
        prepare_and_preflight_work(vertex_count, edge_count, constraint_count)?,
        classification_work,
        registry_work,
        checked_mul(candidate_count, candidate_work)?,
    ])
}

fn deletion_document_build_work(constraint_count: usize) -> Option<usize> {
    checked_mul(constraint_count.checked_add(1)?, 8)
}

fn prepare_and_preflight_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    checked_sum([
        prepare_work(vertex_count, edge_count, constraint_count)?,
        preflight_work(edge_count, constraint_count)?,
    ])
}

fn prepare_work(vertex_count: usize, edge_count: usize, constraint_count: usize) -> Option<usize> {
    let linear = checked_sum([vertex_count, edge_count, constraint_count, 1])?;
    let ordered = checked_sum([
        sort_work(vertex_count)?,
        sort_work(edge_count)?,
        sort_work(constraint_count)?,
    ])?;
    let lookup_factor = checked_sum([
        ceil_log2_plus_one(vertex_count)?,
        ceil_log2_plus_one(edge_count)?,
        1,
    ])?;
    checked_sum([
        checked_mul(linear, 32)?,
        checked_mul(ordered, 16)?,
        checked_mul(
            checked_mul(constraint_count.checked_add(1)?, lookup_factor)?,
            64,
        )?,
    ])
}

fn preflight_work(edge_count: usize, constraint_count: usize) -> Option<usize> {
    let constraint_dimension = constraint_count.checked_add(1)?;
    let constraint_log = ceil_log2_plus_one(constraint_dimension)?;
    let cubic = checked_mul(
        checked_mul(constraint_dimension, constraint_dimension)?,
        constraint_dimension,
    )?;
    let direct_and_ratio_graphs = checked_mul(checked_mul(cubic, constraint_log)?, 160)?;
    let general_graphs = checked_mul(
        GENERAL_GRAPH_PREFLIGHT_LOGICAL_WORK_CEILING_V1,
        constraint_log,
    )?;
    let zero_closure = checked_sum([
        checked_mul(edge_count, ZERO_CLOSURE_WORK_PER_PATTERN_EDGE_V1)?,
        checked_mul(constraint_count, ZERO_CLOSURE_LINEAR_WORK_PER_CONSTRAINT_V1)?,
        checked_mul(
            checked_mul(constraint_count, constraint_count)?,
            ZERO_CLOSURE_QUADRATIC_WORK_PER_CONSTRAINT_PAIR_V1,
        )?,
    ])?;
    let zero_order_factor = ceil_log2_plus_one(checked_sum([edge_count, constraint_count, 1])?)?;
    let pattern_index = checked_mul(
        checked_mul(edge_count.checked_add(1)?, constraint_count.checked_add(1)?)?,
        checked_mul(
            checked_sum([ceil_log2_plus_one(edge_count)?, constraint_log])?,
            64,
        )?,
    )?;
    checked_sum([
        direct_and_ratio_graphs,
        general_graphs,
        checked_mul(zero_closure, zero_order_factor)?,
        pattern_index,
    ])
}

fn residual_certificate_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    checked_sum([
        checked_mul(checked_sum([vertex_count, edge_count, 1])?, 64)?,
        checked_mul(constraint_count.checked_add(1)?, 256)?,
    ])
}

fn axis_projection_work(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    let vertex_log = ceil_log2_plus_one(vertex_count)?;
    let edge_log = ceil_log2_plus_one(edge_count)?;
    let linear = checked_sum([vertex_count, edge_count, constraint_count, 1])?;
    let ordered_maps = checked_sum([
        sort_work(vertex_count)?,
        checked_mul(vertex_count, vertex_log)?,
        checked_mul(edge_count, checked_sum([vertex_log, edge_log])?)?,
        checked_mul(constraint_count.checked_add(1)?, edge_log)?,
    ])?;
    checked_sum([checked_mul(linear, 32)?, checked_mul(ordered_maps, 32)?])
}

fn sort_work(item_count: usize) -> Option<usize> {
    checked_mul(item_count, ceil_log2_plus_one(item_count)?)
}

fn ceil_log2_plus_one(value: usize) -> Option<usize> {
    if value <= 1 {
        return Some(1);
    }
    let ceil_log2 = usize::BITS.checked_sub((value - 1).leading_zeros())?;
    usize::try_from(ceil_log2).ok()?.checked_add(1)
}

fn checked_mul(left: usize, right: usize) -> Option<usize> {
    left.checked_mul(right)
}

fn checked_sum<const N: usize>(values: [usize; N]) -> Option<usize> {
    values.into_iter().try_fold(0_usize, usize::checked_add)
}

#[cfg(test)]
pub(crate) fn witness_phase_work_for_test(
    vertex_count: usize,
    edge_count: usize,
    core_constraint_count: usize,
    deletion_constraint_count: usize,
) -> Option<(usize, usize, usize, usize, usize, usize, usize)> {
    Some((
        witness_setup_work(core_constraint_count)?,
        checked_sum([
            deletion_document_build_work(deletion_constraint_count)?,
            prepare_and_preflight_work(vertex_count, edge_count, deletion_constraint_count)?,
            residual_certificate_work(vertex_count, edge_count, deletion_constraint_count)?,
        ])?,
        checked_sum([
            prepare_and_preflight_work(vertex_count, edge_count, deletion_constraint_count)?,
            axis_projection_work(vertex_count, edge_count, deletion_constraint_count)?,
            prepare_and_preflight_work(vertex_count, edge_count, deletion_constraint_count)?,
            residual_certificate_work(vertex_count, edge_count, deletion_constraint_count)?,
        ])?,
        checked_sum([
            prepare_and_preflight_work(vertex_count, edge_count, deletion_constraint_count)?,
            checked_mul(
                MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1,
                checked_sum([
                    checked_mul(
                        checked_sum([vertex_count, edge_count, deletion_constraint_count, 1])?,
                        64,
                    )?,
                    checked_mul(sort_work(vertex_count)?, 16)?,
                    prepare_and_preflight_work(
                        vertex_count,
                        edge_count,
                        deletion_constraint_count,
                    )?,
                    residual_certificate_work(vertex_count, edge_count, deletion_constraint_count)?,
                ])?,
            )?,
        ])?,
        pair_constraint_constructive_work(vertex_count, edge_count, deletion_constraint_count)?,
        pair_constraint_algebraic_work(vertex_count, edge_count, deletion_constraint_count)?,
        length_constraint_constructive_work(vertex_count, edge_count, deletion_constraint_count)?,
    ))
}

#[cfg(test)]
pub(crate) fn zero_length_closure_phase_work_for_test(
    vertex_count: usize,
    edge_count: usize,
    document: &GeometricConstraintDocumentV1,
) -> Option<usize> {
    zero_length_closure_constructive_work(
        vertex_count,
        edge_count,
        document.constraints.len(),
        zero_length_closure_constructive_candidate_bound_v1(document),
    )
}

#[cfg(test)]
pub(crate) fn anchored_mirror_residual_only_phase_work_for_test(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    anchored_mirror_residual_only_work(vertex_count, edge_count, constraint_count)
}

#[cfg(test)]
pub(crate) fn unit_parallel_fixed_angle_residual_only_phase_work_for_test(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    unit_parallel_fixed_angle_residual_only_work(vertex_count, edge_count, constraint_count)
}

#[cfg(test)]
pub(crate) fn unit_two_hop_parallel_residual_only_phase_work_for_test(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    unit_two_hop_parallel_residual_only_work(vertex_count, edge_count, constraint_count)
}

#[cfg(test)]
pub(crate) fn unit_terminal_two_hop_parallel_angle_residual_only_phase_work_for_test(
    vertex_count: usize,
    edge_count: usize,
    constraint_count: usize,
) -> Option<usize> {
    unit_terminal_two_hop_parallel_angle_residual_only_work(
        vertex_count,
        edge_count,
        constraint_count,
    )
}

fn charge_witness_work(
    progress: &mut BoundedSemanticMusProgressV1,
    amount: usize,
    maximum: usize,
) -> bool {
    let Some(next) = progress.deletion_witness_work.checked_add(amount) else {
        return false;
    };
    if next > maximum {
        false
    } else {
        progress.deletion_witness_work = next;
        true
    }
}

fn checkpoint(
    observer: &mut impl BoundedSemanticMusObserverV1,
    progress: BoundedSemanticMusProgressV1,
) -> Option<BoundedSemanticMusUnknownReasonV1> {
    match observer.checkpoint(progress) {
        BoundedSemanticMusObserverControlV1::Continue => None,
        BoundedSemanticMusObserverControlV1::Cancelled => {
            Some(BoundedSemanticMusUnknownReasonV1::Cancelled)
        }
        BoundedSemanticMusObserverControlV1::DeadlineReached => {
            Some(BoundedSemanticMusUnknownReasonV1::DeadlineReached)
        }
    }
}

fn stop_reason(stop: BoundedSemanticMusObserverControlV1) -> BoundedSemanticMusUnknownReasonV1 {
    match stop {
        BoundedSemanticMusObserverControlV1::Continue => {
            BoundedSemanticMusUnknownReasonV1::DirectOracleIncomplete
        }
        BoundedSemanticMusObserverControlV1::Cancelled => {
            BoundedSemanticMusUnknownReasonV1::Cancelled
        }
        BoundedSemanticMusObserverControlV1::DeadlineReached => {
            BoundedSemanticMusUnknownReasonV1::DeadlineReached
        }
    }
}

fn unknown(
    reason: BoundedSemanticMusUnknownReasonV1,
    progress: BoundedSemanticMusProgressV1,
    direct_core_constraint_ids: &[ConstraintId],
) -> BoundedCurrentRuntimeSemanticMusV1 {
    BoundedCurrentRuntimeSemanticMusV1::Unknown {
        reason,
        direct_core_constraint_ids: direct_core_constraint_ids.to_vec(),
        direct_oracle_calls: progress.direct_oracle_calls,
        deletion_witness_checks: progress.deletion_witness_checks,
        certified_deletion_witnesses: progress.certified_deletion_witnesses,
        deletion_witness_work: progress.deletion_witness_work,
    }
}

struct DirectObserverAdapter<'a, Observer> {
    observer: &'a mut Observer,
    stop: Option<BoundedSemanticMusObserverControlV1>,
}

impl<Observer: BoundedSemanticMusObserverV1> BoundedDirectMusObserverV1
    for DirectObserverAdapter<'_, Observer>
{
    fn should_cancel(&mut self, completed_oracle_calls: usize) -> bool {
        let progress = BoundedSemanticMusProgressV1 {
            direct_oracle_calls: completed_oracle_calls,
            ..BoundedSemanticMusProgressV1::new()
        };
        match self.observer.checkpoint(progress) {
            BoundedSemanticMusObserverControlV1::Continue => false,
            stop => {
                self.stop = Some(stop);
                true
            }
        }
    }
}
